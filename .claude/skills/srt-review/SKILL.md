---
name: srt-review
description: Use when reviewing code changes (PRs, commits, uncommitted work) in a repository that has .sem/ directories. Runs semtree review to generate a structured manifest with severity triage, semantic context, and cross-cutting warnings, then conducts a phased review using that context.
---

# SRT-Assisted Code Review

Review code changes using the Semantic Resolution Tree for architectural context. This skill runs `semtree review` to generate a review manifest, then conducts a three-phase review informed by severity triage, semantic context, and cross-cutting awareness.

## Prerequisites

- Repository has `.sem/` directories (built with `semtree build`)
- `semtree` binary is on PATH
- If either is missing, inform the user and fall back to a normal review without SRT context

## Phase 0: Generate Review Manifest

Run `semtree review` to get the manifest:

```bash
# Uncommitted changes:
semtree review .

# PR / branch comparison:
semtree review "main..HEAD" .

# Specific commits:
semtree review "abc123..def456" .
```

Parse the output. It has three sections:

1. **Triage table** — each changed file with severity (HIGH/MEDIUM/LOW) based on embedding fan-out (how many files in the codebase are conceptually related)
2. **Per-file context** — what each file does, what module it belongs to, and related files to read
3. **Cross-cutting warnings** — files that should have changed but didn't (high-confidence from `__dir__.md` collaborator docs, plus embedding-similarity suggestions)

If `semtree review` fails, skip to Phase 2 with standard diff-only context.

## Phase 1: Context Loading

Before reading any diff, load context guided by the manifest:

**For HIGH severity files:**
- Read the full `.sem/` summary (not just the first line from the manifest)
- Read 2-3 of the related files listed in the manifest
- Read the parent directory's `__dir__.md` for module-level understanding

**For MEDIUM severity files:**
- Read the `.sem/` summary
- Read 1 related file if the change touches APIs or interfaces

**For LOW severity files:**
- The manifest's first-line summary is sufficient context — skip extra reads

This is the critical step that differentiates SRT review from standard review. You now understand the **architectural role** of each changed file before reading the diff.

## Phase 2: Phased Diff Review

Review the diff in three passes, ordered by severity:

### Pass 1: Architectural Integrity (HIGH severity files)

For each HIGH severity file:
- **Purpose alignment**: Does the change match the file's documented purpose from its `.sem/` summary? Changes that drift from a file's role are red flags.
- **Interface consistency**: Do related files (from the manifest) still work with the changed interfaces? If `builder.rs` changed its public API, does `lib.rs` still re-export correctly?
- **Cross-cutting completeness**: Check the manifest's high-confidence warnings. If it says "execute.rs collaborates with builder.rs" and only builder.rs changed, flag this explicitly.

### Pass 2: Correctness and Quality (MEDIUM + LOW files)

Standard review concerns, now informed by context:
- Correctness — logic errors, off-by-ones, null handling
- Error handling — are errors propagated, logged, or silently swallowed?
- Testing — are the changes tested? Do existing tests still cover the behavior?
- Performance — anything obviously expensive in a hot path?

### Pass 3: Completeness Check

Using the manifest's cross-cutting warnings:
- **High-confidence gaps**: For each warning, assess whether the unchanged collaborator actually needs updating. Not all warnings are actionable — the reviewer decides.
- **Embedding suggestions**: Scan the "Consider Also" list. These are lower confidence but may surface files the author forgot.
- **Test coverage**: Are there tests for the HIGH severity files? The related files list often surfaces the test files.

## Output Format

Structure your review as:

```markdown
## Review Summary

**Scope:** N files changed, M high-severity
**Overall risk:** HIGH / MEDIUM / LOW

## Architectural Concerns
[From Pass 1 — purpose drift, interface breaks, cross-cutting gaps]

## Issues

### Critical (Must Fix)
- file:line — what's wrong, why it matters

### Important (Should Fix)
- file:line — what's wrong, why it matters

### Minor
- file:line — suggestion

## Cross-Cutting Gaps
[From the manifest's warnings — which ones are real concerns vs. false positives]

## Files to Test
[Based on fan-out analysis — which files have the widest blast radius]

## Verdict
Ready to merge? [Yes / With fixes / No]
```

## Integration with Other Review Skills

This skill provides the **context-loading phase** that other review skills lack. It pairs well with:

- **pr-review-toolkit:code-reviewer** — run SRT review first for context, then dispatch the code reviewer with the manifest as additional context
- **superpowers:requesting-code-review** — include the manifest output in the `{DESCRIPTION}` field so the code reviewer subagent has architectural context
- **pr-review-toolkit:silent-failure-hunter** — use the HIGH severity files from the manifest to prioritize which error handling to inspect

## Example

```
1. semtree review "main..HEAD" .
   → Triage:
     crates/cache/src/multiplexer.rs  HIGH  fan_out=18
     crates/cache/src/http.rs         MEDIUM  fan_out=7
     crates/cache/src/config.rs       LOW  fan_out=2
   → Cross-cutting: signature_authentication.rs collaborates with multiplexer.rs

2. Phase 1 — Load context:
   - Read multiplexer.rs summary: "Coordinates local and remote cache backends"
   - Read related: async_cache.rs (0.91), http.rs (0.88), lib.rs (0.85)
   - Read cache/__dir__.md for module overview

3. Phase 2 — Review diff:
   Pass 1: multiplexer.rs changed retry logic. The related async_cache.rs
   also has retry logic — are they consistent? http.rs uses the same
   retry path — verified it still works.

   Pass 2: http.rs added a new header. config.rs updated the schema.
   Standard review — looks correct.

   Pass 3: signature_authentication.rs was flagged as collaborator.
   It validates signatures on cached artifacts — the retry change
   could cause signature re-validation. Worth checking.

4. Output:
   ## Architectural Concerns
   - multiplexer.rs retry logic changed but signature_authentication.rs
     wasn't updated. If retries re-fetch, signatures get re-validated
     on each attempt — potential performance issue.

   ## Issues
   ### Important
   - multiplexer.rs:142 — retry count not configurable (hardcoded 3)

   ## Cross-Cutting Gaps
   - signature_authentication.rs: REAL concern — verify retry + signature interaction

   ## Verdict: With fixes
```

## When NOT to Use

- Single-file doc/config changes
- Repository has no `.sem/` directories
- `semtree` binary not available (fall back to standard review)
