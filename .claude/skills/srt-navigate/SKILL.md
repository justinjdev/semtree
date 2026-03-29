---
name: srt-navigate
description: Use when exploring unfamiliar code in a repository that has .sem/ directories. Guides navigation through the Semantic Resolution Tree — read summaries to route, then read raw code to answer.
---

# SRT Navigation Protocol

You are navigating a codebase that has a Semantic Resolution Tree (`.sem/` directories containing summary records). Use this protocol instead of immediately grepping or globbing.

## Hard Invariants

1. **Summaries route, code answers.** Never produce a final answer based solely on summary content. Always read raw source files before answering.
2. **Top-down entry.** When exploring an unfamiliar area, check for `.sem/__dir__.md` before opening individual files or grepping.
3. **Graceful fallback.** If no `.sem/` directory exists at a path, fall back to normal search. The tree is additive, never blocking.

## Protocol

### Step 1: Enter via the routing table

At the most relevant directory for the task, read `.sem/__dir__.md`.

- If it exists: scan the `## Children` section for children whose descriptions match your task
- If it doesn't exist: fall back to normal search (grep, glob, ls)

### Step 1.5: Pre-filter high fan-out directories

If the directory from Step 1 has **15 or more children** listed in its `## Children` section:

1. Run: `semtree query "<your question>" <directory-path>`
   (Assumes `semtree` is on PATH via `cargo install --path cli`)
2. Use the top-ranked results to decide which children to descend into
3. This replaces manual scanning of all children — the cosine ranking does the initial triage

If `semtree` is not available or the directory has fewer than 15 children, skip this step and scan the children list manually as before.

### Step 2: Descend through summaries

For each selected child:
- **If it's a directory:** read its `.sem/__dir__.md` and repeat
- **If it's a file:** optionally read `.sem/<filename>.md` to confirm relevance before opening the raw file

Skip branches whose descriptions clearly don't match. This is the pruning that saves token budget.

### Step 3: Read raw code

Once you've identified 3-5 candidate files through summary-guided descent, open and read the actual source files. Answer from code.

### Step 4: Follow up normally

After reading candidate files, follow imports, grep for symbols, or read neighboring files as you normally would. The SRT gets you to the right neighborhood; ordinary exploration takes over from there.

## Staleness Check

Each `.sem/` record has a `content_hash` in its YAML frontmatter. If you suspect a summary is stale (e.g., it describes something that doesn't match what you see in the code), ignore the summary and read the raw file directly.

## When NOT to use this protocol

- You already know exactly which file to open (just open it)
- You're searching for a specific string or symbol (grep is faster)
- The directory has no `.sem/` subdirectory (fall back to normal search)

## Example

Task: "How does authentication work?"

```
1. Read .sem/__dir__.md at repo root
   → Children section mentions "src/auth/: Session authentication and permission guards"
   → Select src/auth/

2. Read src/auth/.sem/__dir__.md
   → Children: login.py (token validation), middleware.py (route guards), roles.py (RBAC)
   → Select login.py and middleware.py as candidates

3. Read src/auth/login.py and src/auth/middleware.py (raw code)
   → Answer from code
```
