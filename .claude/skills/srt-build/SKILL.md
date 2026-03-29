---
name: srt-build
description: Build the Semantic Resolution Tree for a repository using parallel subagents. Use when you need to generate .sem/ summary records for a codebase. Much faster than the CLI for initial builds.
---

# SRT Build Skill

Build the Semantic Resolution Tree (`.sem/` summary records) for a repository using parallel subagents. Agents read files, generate summaries, and write `.sem/` records directly.

## Usage

```
/srt-build [path]
```

If no path is provided, use the current working directory.

## Protocol

### Step 1: Walk the tree and identify stale nodes

Run this via Bash to get the stale file list and directory structure:

```bash
semtree build TARGET_PATH --no-embed 2>&1 | head -1
# Or for a dry-run to see what's stale, run build with defaults.
# The CLI handles walking, hashing, and staleness detection internally.
```

### Step 2: Summarize stale files via parallel agents that WRITE records directly

Split stale files into batches of ~15. For each batch, launch a subagent that:
1. Reads each file
2. Generates a summary
3. **Writes the `.sem/` record directly** using Bash + Python

Agent prompt template:

```
For each file listed below, read it, write a 2-5 sentence summary, then write the .sem/ record.

Use this Bash command to write each record (fill in the values):

semtree build REPO_ROOT --no-embed
# Or for manual agent-driven writes, agents write .sem/ records directly via file I/O.

Files to process:
- path: <rel_path>, hash: <hash>
- path: <rel_path>, hash: <hash>
...
```

Launch up to 5-8 agents in parallel.

### Step 3: Build directory summaries bottom-up via agents

After all file records are written, directories need LLM-generated summaries. Process bottom-up (deepest directories first).

Group directories by depth level. For each level, launch parallel agents that:
1. Read existing child `.sem/` records to get child summaries
2. Generate a prose overview + `## Children` routing table via LLM (the agent IS the LLM)
3. Write the directory `.sem/` record directly

Agent prompt template for directories:

```
Summarize each directory listed below. For each one:

1. Read its children's .sem/ records to understand what each child does
2. Write a concise prose overview (2-4 sentences)
3. Write a ## Children section listing EVERY immediate child with a one-line description
4. Write the .sem/ record

Read child summaries by reading .sem/<child>.md files directly.
Write directory records by writing .sem/__dir__.md and sibling .sem/<dirname>.md files directly.
The `semtree build` command handles this automatically for CLI-driven builds.

Directories to process (with their children and hashes):
- dir: <path>, hash: <hash>, children: [child1, child2, ...]
```

### Step 3.5: Compute embeddings for routing

After all records are written, run `semtree embed` to create `.vec` sidecar files for embedding-assisted routing:

```bash
semtree embed REPO_ROOT
```

This enables `semtree route` and `semtree query` to rank children by cosine similarity.

### Step 4: Report results

Print summary of files/dirs summarized vs skipped.

## Key Rules

- **Agents write records directly** — no extract-parse-write pipeline. Each agent owns its records end-to-end.
- **Directory summaries are LLM-generated prose** — not mechanical concatenation. The agent reads child summaries and writes a natural-language overview + routing table.
- **Every child MUST appear in the `## Children` routing table** by name with a description.
- **Bottom-up ordering for directories** — deepest first, so child summaries exist before parents need them.
- **Directory sibling records** — every directory (except root) must have a sibling record at the parent level (`<parent>/.sem/<dirname>.md`) in addition to its own `<dir>/.sem/__dir__.md`. This enables embedding-based routing to rank directories alongside files.
- **File summaries are independent** — parallelize aggressively across batches.
- **Use the semtree library** for hashing, record paths, and record I/O. Don't reimplement.
