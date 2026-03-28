## 1. Project Scaffolding

- [x] 1.1 Create `pyproject.toml` with package metadata, `semtree` entry point, and dependencies (pyyaml)
- [x] 1.2 Create `src/semtree/__init__.py` and module files: `cli.py`, `walker.py`, `hasher.py`, `summarizer.py`, `records.py`, `config.py`
- [x] 1.3 Verify `pip install -e .` works and `semtree` command is available on PATH

## 2. Filesystem Traversal (tree-construction)

- [x] 2.1 Implement `walker.py`: post-order DFS using `os.walk(topdown=False)` with lexicographic sorting within each directory
- [x] 2.2 Implement ignore rules: skip dotfiles, dot-directories, symlinks, and binary files (null byte check in first 8192 bytes)
- [x] 2.3 Yield `(repo_relative_path, is_directory, children)` tuples for each node in post-order
- [x] 2.4 Write tests for traversal ordering, ignore rules, and repo-relative path computation

## 3. Content Hashing (content-hashing)

- [x] 3.1 Implement `hasher.py`: `hash_file(path) -> str` computing SHA-256 hex digest of raw file bytes
- [x] 3.2 Implement `hash_directory(children: list[tuple[str, str]]) -> str` computing SHA-256 of sorted `path:hash` pairs joined by newlines
- [x] 3.3 Write tests for file hashing, directory hashing, and upward propagation

## 4. Record Storage (record-storage)

- [x] 4.1 Implement `records.py`: `write_record(dir_path, filename, path, type, content_hash, summary)` writing YAML frontmatter + Markdown body to `.sem/` directory
- [x] 4.2 Implement `read_record(record_path) -> dict` parsing YAML frontmatter to extract `content_hash` and other fields
- [x] 4.3 Handle `__dir__.md` for directory records and `<filename>.md` for file records
- [x] 4.4 Write tests for record writing, reading, and round-trip consistency

## 5. Summarization (summarization)

- [x] 5.1 Implement `summarizer.py`: `Summarizer` protocol with `summarize(prompt: str, content: str) -> str`
- [x] 5.2 Implement `ClaudeCLISummarizer`: default provider that calls `claude -p` via subprocess with the prompt and content on stdin
- [x] 5.3 Implement file summarization prompt: send repo-relative path + file contents, return summary
- [x] 5.4 Implement directory summarization prompt: send child (path, summary) pairs, return summary with `## Children` routing table
- [x] 5.5 Implement oversized file detection: `len(file_bytes) / 4 > max_tokens` → placeholder summary
- [x] 5.6 Implement retry logic (3 attempts with backoff) and graceful skip on persistent failure
- [x] 5.7 Write tests for prompt construction, oversized detection, and error handling (mock the subprocess)

## 6. Incremental Rebuild (incremental-rebuild)

- [x] 6.1 Before summarizing a node, check for existing `.sem/` record and compare stored `content_hash` against freshly computed hash
- [x] 6.2 Skip LLM call and preserve existing record when hashes match
- [x] 6.3 Treat missing records as stale (always summarize)
- [x] 6.4 Implement `--force` flag to bypass all freshness checks
- [x] 6.5 Write tests for skip-on-match, re-summarize-on-mismatch, and force rebuild

## 7. CLI and Build Orchestration (cli)

- [x] 7.1 Implement `cli.py`: argparse with `build` subcommand, `--model`, `--max-tokens`, `--force` flags
- [x] 7.2 Implement `config.py`: gather CLI args into a config object (model, max_tokens, force, target_path)
- [x] 7.3 Wire the build pipeline: walk → hash → check freshness → summarize → write records, in post-order
- [x] 7.4 Implement progress output to stderr: nodes processed, skipped, errored
- [x] 7.5 Handle error cases: target path doesn't exist, `claude` CLI not found
- [x] 7.6 End-to-end integration test: run `semtree build` on a small fixture repo and verify `.sem/` output
