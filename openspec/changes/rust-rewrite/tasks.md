## 1. Project Setup

- [ ] 1.1 Create `cli/` directory with `cargo init --name semtree`
- [ ] 1.2 Add dependencies to Cargo.toml: clap, serde, serde_json, serde_yaml, sha2, ort, tokio, memmap2, walkdir
- [ ] 1.3 Set up clap subcommand skeleton (build, embed, query, route, serve, bench, vec) with flag definitions matching rust-cli spec
- [ ] 1.4 Verify `cargo build` produces a working binary with `--help` output

## 2. Records I/O (rust-records)

- [ ] 2.1 Implement record_path_for_file, record_path_for_dir, record_path_for_dir_sibling
- [ ] 2.2 Implement write_record (YAML frontmatter + Markdown body, create .sem/ dirs)
- [ ] 2.3 Implement read_record (parse frontmatter, extract summary)
- [ ] 2.4 Write tests: round-trip, missing file, malformed YAML, path generation

## 3. Hasher (rust-hasher)

- [ ] 3.1 Implement hash_file (SHA-256 of raw bytes)
- [ ] 3.2 Implement hash_directory (SHA-256 of sorted child_path:child_hash pairs)
- [ ] 3.3 Write tests: determinism, order-independence, hash propagation

## 4. Walker (rust-walker)

- [ ] 4.1 Implement git-aware walk using `git ls-files` subprocess
- [ ] 4.2 Implement filesystem fallback walk (for non-git repos)
- [ ] 4.3 Implement filters: binary, dotfiles, dot-directories, symlinks
- [ ] 4.4 Implement --exclude glob pattern support
- [ ] 4.5 Implement post-order DFS sort (children before parents)
- [ ] 4.6 Write tests: git filtering, post-order, exclude patterns, binary detection

## 5. Binary .vec Format (binary-vec)

- [ ] 5.1 Define VecHeader struct and binary layout (SVEC magic, version, dims, hash_len)
- [ ] 5.2 Implement write_vec (header + hash + model + f32 array)
- [ ] 5.3 Implement read_vec with mmap (memory-map file, cast f32 slice from pointer)
- [ ] 5.4 Implement is_vec_fresh (compare content_hash and model)
- [ ] 5.5 Implement `semtree vec inspect` subcommand
- [ ] 5.6 Write tests: round-trip, freshness check, mmap read, inspect output

## 6. Embedder (rust-embedder)

- [ ] 6.1 Implement ONNX model download and caching to ~/.cache/semtree/models/
- [ ] 6.2 Implement model loading via ort crate (Session from .onnx file)
- [ ] 6.3 Implement tokenization (bge-small-en-v1.5 uses BERT tokenizer — use tokenizers crate or simple whitespace + truncation)
- [ ] 6.4 Implement embed_texts (batch document embedding with "passage: " prefix)
- [ ] 6.5 Implement embed_query (single query embedding with "query: " prefix)
- [ ] 6.6 Implement cosine_rank (cosine similarity ranking over f32 slices)
- [ ] 6.7 Write tests: vector dimensions, cosine ranking correctness, batch embedding

## 7. Embed Command

- [ ] 7.1 Implement find_sem_records (rglob .sem/*.md)
- [ ] 7.2 Implement embed_directory (walk records, check freshness, batch embed, write .vec)
- [ ] 7.3 Wire `semtree embed` subcommand with --model, --force flags
- [ ] 7.4 Write tests: creates .vec files, skips fresh, force re-embeds

## 8. Query Command

- [ ] 8.1 Implement query_directory (load child .vec files, embed query, cosine rank, return results)
- [ ] 8.2 Wire `semtree query` subcommand with --top-k, --threshold, --model flags
- [ ] 8.3 Write tests: ranked output, empty results, top-k limiting

## 9. Route Command

- [ ] 9.1 Implement route_directory (BFS beam search, rank children at each level, collect file candidates)
- [ ] 9.2 Wire `semtree route` subcommand with --beam-width, --max-depth, --model flags
- [ ] 9.3 Add timing output to stderr
- [ ] 9.4 Write tests: multi-level descent, depth limit, beam width

## 10. Summarizer (rust-summarizer)

- [ ] 10.1 Implement claude_summarize (subprocess to `claude -p`, capture stdout)
- [ ] 10.2 Implement retry with exponential backoff (1s, 2s, 4s, max 3 retries)
- [ ] 10.3 Implement is_oversized (file_bytes / 4 > max_tokens)
- [ ] 10.4 Implement build_file_prompt and build_dir_prompt
- [ ] 10.5 Write tests: prompt format, oversized detection, retry logic (with mock)

## 11. Build Pipeline

- [ ] 11.1 Implement build function: walk → hash → check freshness → summarize → write records
- [ ] 11.2 Implement directory sibling record writing (parent-level .sem/<dirname>.md)
- [ ] 11.3 Integrate optional embed step after summarization (--no-embed to skip)
- [ ] 11.4 Implement progress output (node counts, skip/summarize/error stats)
- [ ] 11.5 Wire `semtree build` subcommand with all flags
- [ ] 11.6 Write tests: full pipeline with mock summarizer, incremental rebuild, sibling records

## 12. Daemon Mode (daemon-mode)

- [ ] 12.1 Implement Unix socket server with tokio (listen on ~/.cache/semtree/semtree.sock)
- [ ] 12.2 Implement PID file management (write on start, remove on shutdown)
- [ ] 12.3 Implement newline-delimited JSON protocol (route, query methods)
- [ ] 12.4 Implement model preloading at daemon startup
- [ ] 12.5 Implement client-side daemon detection in query/route commands (check socket, delegate or fall back)
- [ ] 12.6 Implement graceful shutdown on SIGTERM/SIGINT
- [ ] 12.7 Implement stale socket detection (connection refused → remove socket → cold path)
- [ ] 12.8 Wire `semtree serve` subcommand with --socket flag
- [ ] 12.9 Write tests: start/stop lifecycle, warm route latency, stale socket recovery

## 13. Bench Data Collection

- [ ] 13.1 Implement TSV append (same format as Python harness)
- [ ] 13.2 Implement quality phase (structural checks on .sem/ records)
- [ ] 13.3 Implement routing phase (control grid sweep using route_directory)
- [ ] 13.4 Wire `semtree bench` subcommand with --repo-path, --results, --phase flags
- [ ] 13.5 Write tests: TSV format, quality checks

## 14. Integration and Migration

- [ ] 14.1 Update srt-navigate skill to reference Rust binary
- [ ] 14.2 Update srt-build skill to reference Rust binary
- [ ] 14.3 Update Python bench/ to shell out to Rust binary instead of importing Python semtree
- [ ] 14.4 Run full benchmark suite with Rust binary and compare results to Python baseline
- [ ] 14.5 Add `semtree embed --force` migration note for JSON → binary .vec conversion
- [ ] 14.6 Update CLAUDE.md with Rust build instructions
