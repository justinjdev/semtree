## 1. Project Setup

- [x] 1.1 Create `cli/` directory with `cargo init --name semtree`
- [x] 1.2 Add dependencies to Cargo.toml: clap, serde, serde_json, serde_yaml, sha2, ort, tokio, memmap2, walkdir
- [x] 1.3 Set up clap subcommand skeleton (build, embed, query, route, serve, bench, vec) with flag definitions matching rust-cli spec
- [x] 1.4 Verify `cargo build` produces a working binary with `--help` output

## 2. Records I/O (rust-records)

- [x] 2.1 Implement record_path_for_file, record_path_for_dir, record_path_for_dir_sibling
- [x] 2.2 Implement write_record (YAML frontmatter + Markdown body, create .sem/ dirs)
- [x] 2.3 Implement read_record (parse frontmatter, extract summary)
- [x] 2.4 Write tests: round-trip, missing file, malformed YAML, path generation

## 3. Hasher (rust-hasher)

- [x] 3.1 Implement hash_file (SHA-256 of raw bytes)
- [x] 3.2 Implement hash_directory (SHA-256 of sorted child_path:child_hash pairs)
- [x] 3.3 Write tests: determinism, order-independence, hash propagation

## 4. Walker (rust-walker)

- [x] 4.1 Implement git-aware walk using `git ls-files` subprocess
- [x] 4.2 Implement filesystem fallback walk (for non-git repos)
- [x] 4.3 Implement filters: binary, dotfiles, dot-directories, symlinks
- [x] 4.4 Implement --exclude glob pattern support
- [x] 4.5 Implement post-order DFS sort (children before parents)
- [x] 4.6 Write tests: git filtering, post-order, exclude patterns, binary detection

## 5. Binary .vec Format (binary-vec)

- [x] 5.1 Define VecHeader struct and binary layout (SVEC magic, version, dims, hash_len)
- [x] 5.2 Implement write_vec (header + hash + model + f32 array)
- [x] 5.3 Implement read_vec with mmap (memory-map file, cast f32 slice from pointer)
- [x] 5.4 Implement is_vec_fresh (compare content_hash and model)
- [x] 5.5 Implement `semtree vec inspect` subcommand
- [x] 5.6 Write tests: round-trip, freshness check, mmap read, inspect output

## 6. Embedder (rust-embedder)

- [x] 6.1 Implement embedding via Python/fastembed subprocess (native ort deferred)
- [x] 6.2 Implement cosine_rank (native Rust, no subprocess)
- [x] 6.3 Implement embed_directory (walk records, check freshness, batch embed, write .vec)
- [x] 6.4 Implement query_directory (load child .vec files, embed query, cosine rank)
- [x] 6.5 Implement route_directory (BFS beam search, rank children at each level)
- [x] 6.6 Write tests for embedder (cosine ranking, etc.)
- [x] 6.7 Native ort embedding (replace Python subprocess — future optimization)

## 7. Embed Command

- [x] 7.1 Implement find_sem_records (rglob .sem/*.md)
- [x] 7.2 Implement embed_directory wired to CLI
- [x] 7.3 Wire `semtree embed` subcommand with --model, --force flags

## 8. Query Command

- [x] 8.1 Wire `semtree query` subcommand with --top-k, --threshold, --model flags

## 9. Route Command

- [x] 9.1 Wire `semtree route` subcommand with --beam-width, --max-depth, --model flags
- [x] 9.2 Add timing output to stderr
- [x] 9.3 Smoke test: route against fellowship (252ms, correct results)

## 10. Summarizer (rust-summarizer)

- [x] 10.1 Implement claude_summarize (subprocess to `claude -p`, capture stdout)
- [x] 10.2 Implement retry with exponential backoff
- [x] 10.3 Implement is_oversized (file_bytes / 4 > max_tokens)
- [x] 10.4 Implement build_file_prompt and build_dir_prompt
- [x] 10.5 Write tests: prompt format, oversized detection

## 11. Build Pipeline

- [x] 11.1 Implement build function: walk → hash → check freshness → summarize → write records
- [x] 11.2 Implement directory sibling record writing (parent-level .sem/<dirname>.md)
- [x] 11.3 Integrate optional embed step after summarization (--no-embed to skip)
- [x] 11.4 Implement progress output (node counts, skip/summarize/error stats)
- [x] 11.5 Wire `semtree build` subcommand with all flags

## 12. Daemon Mode (daemon-mode)

- [x] 12.1 Implement Unix socket server with tokio
- [x] 12.2 Implement PID file management
- [x] 12.3 Implement newline-delimited JSON protocol (route, query methods)
- [x] 12.4 Implement model preloading at daemon startup (requires native ort)
- [x] 12.5 Implement client-side daemon detection in query/route commands
- [x] 12.6 Implement graceful shutdown on SIGTERM/SIGINT
- [x] 12.7 Implement stale socket detection
- [x] 12.8 Wire `semtree serve` subcommand with --socket flag
- [x] 12.9 Write tests: serialization, expand_tilde, helper functions

## 13. Bench Data Collection

- [x] 13.1 Implement TSV append (same format as Python harness)
- [x] 13.2 Implement quality phase (structural checks on .sem/ records)
- [ ] 13.3 Implement routing phase (control grid sweep using route_directory)
- [x] 13.4 Wire `semtree bench` subcommand with --repo-path, --results, --phase flags
- [x] 13.5 Write tests: TSV format, quality checks

## 14. Integration and Migration

- [x] 14.1 Update srt-navigate skill to reference Rust binary
- [x] 14.2 Update srt-build skill to reference Rust binary
- [ ] 14.3 Update Python bench/ to shell out to Rust binary instead of importing Python semtree
- [ ] 14.4 Run full benchmark suite with Rust binary and compare results to Python baseline
- [ ] 14.5 Add `semtree embed --force` migration note for JSON → binary .vec conversion
- [x] 14.6 Update CLAUDE.md with Rust build instructions
