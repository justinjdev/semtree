# SRT for Code Review

## The Problem

AI code review today suffers from **context poverty**. An agent reviewing a PR sees:

1. The diff (what changed)
2. Maybe the full files (immediate context)
3. Maybe some grep results (ad-hoc context)

But a good human reviewer has **structural understanding** -- they know that `auth.rs` is the security boundary, that `config.rs` feeds into everything, that changes to the `engine` crate ripple through `task-executor` and `run-summary`. This understanding takes weeks to build and is exactly what SRT encodes.

## Approaches

### 1. Change Impact Radius

`semtree impact` finds related files via embedding similarity. But for review, the question isn't "what's similar?" but **"what could break?"** The SRT hierarchy encodes containment and context:

- A change to `crates/turborepo-engine/src/builder.rs` -- the directory summary tells you this is "the core execution orchestrator." The parent (`crates/`) routing table shows what depends on it. You instantly know this is a high-blast-radius change.
- A change to `crates/turborepo-unescape/src/lib.rs` -- the summary tells you it's a string utility. Low blast radius.

**The SRT gives you severity triage for free.** No dependency analysis needed -- the summaries already encode architectural importance.

### 2. Semantic Diff Context

Current PR review tools show syntactic context (surrounding lines). SRT provides **semantic context**: "This file is the cache multiplexer that coordinates local and remote caches. The function being changed handles artifact upload retry logic."

For every file in the diff, the `.sem/` record already contains this. An agent reads the summary *before* reading the diff, giving it the "why does this file exist" framing that makes review comments actually useful.

### 3. Orphan Detection on Diffs (BottleSum for PRs)

After a PR lands, some `.sem/` summaries become stale -- the code changed but the summary didn't. You could:

1. For each changed file, re-embed the new content
2. Check cosine similarity against the existing `.sem/` summary embedding
3. If it drops below threshold, the summary is stale -- flag it
4. Walk up the tree: if a file summary is stale, its parent directory summary might be too

This is **summary drift detection** -- a PR review check that says "this change made 3 summaries inaccurate, they should be regenerated." It's a CI check, not a review comment.

### 4. Cross-Cutting Change Detection

The hardest review task: detecting when a change *should* have touched files it didn't. "You changed the auth middleware but didn't update the corresponding test" or "you added a new config field but didn't update the schema validation."

SRT's cross-cutting concerns sections (in directory summaries) explicitly list file collaborations. An agent could:

1. Read the directory summary for each changed file
2. Check the cross-cutting concerns: "auth.rs and permissions.rs collaborate on session validation"
3. If `auth.rs` changed but `permissions.rs` didn't -- flag for review

### 5. Review Routing (Who Should Review What)

For large PRs touching many files, SRT could partition the review:

- Group changed files by their directory summaries (which module do they belong to?)
- Route each group to the reviewer who owns that module
- Provide the module summary as context for the reviewer

## Synthesis

SRT turns structural knowledge into a queryable API. Today that API serves code navigation. For review, the same API serves impact analysis, context enrichment, staleness detection, and completeness checking.

## Most Immediately Buildable

**Semantic diff context (#2) + summary drift CI check (#3)** require no new infrastructure -- just reading existing `.sem/` records alongside the diff.

A `semtree review <commit-range>` command could:

1. Parse `git diff` to get changed file paths
2. For each changed file, read its `.sem/` summary (semantic context)
3. Re-embed the changed file content, compare against stored embedding (drift detection)
4. Walk up to parent directories, check cross-cutting concerns for missed collaborators
5. Output: severity triage, semantic context per file, stale summaries, potentially missed files
