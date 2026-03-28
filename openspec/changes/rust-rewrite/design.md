## Context

semtree is a Python CLI that builds Semantic Resolution Trees (.sem/ summary records) and provides embedding-assisted routing for code navigation. Profiling shows 700ms cold-start per invocation: 200ms Python startup, 456ms ONNX model load, 139ms JSON .vec parsing. The actual computation (embed + cosine rank) is 7ms. The Python implementation was a successful prototype — it validated the SRT concept and produced benchmark results showing SRT outperforms Shire, ripgrep, and grep on structural code navigation queries. Now we want production-grade performance.

Current Python codebase:
- `src/semtree/` — walker, hasher, summarizer, records, embedder, config, builder, cli
- `bench/` — harness, analysis, routing, baseline, shire_adapter, queries
- `.claude/skills/` — srt-navigate, srt-build

## Goals / Non-Goals

**Goals:**
- Single static binary with no runtime dependencies
- Sub-5ms warm route latency via daemon mode
- <300ms cold-start route latency
- Binary .vec format with mmap for near-zero I/O cost
- Feature parity with Python CLI: build, embed, query, route, bench
- Same .sem/*.md record format (YAML frontmatter + Markdown body) — interoperable with existing trees

**Non-Goals:**
- Rewriting the bench analysis module in Rust (Python numpy/scipy is fine for offline stats)
- Changing the .sem/ record format or the SRT data structure
- Building a GUI or web interface
- Supporting embedding models other than ONNX-format models (candle/native inference is future work)
- Windows support in v1

## Decisions

### 1. Cargo workspace layout

```
cli/
  Cargo.toml          # workspace root
  src/
    main.rs           # CLI entry point (clap)
    walker.rs         # filesystem traversal
    hasher.rs         # SHA-256 hashing
    records.rs        # .sem/ record I/O
    summarizer.rs     # claude CLI subprocess
    embedder.rs       # ONNX Runtime embedding + cosine
    vec_store.rs      # binary .vec format + mmap
    builder.rs        # build pipeline orchestration
    server.rs         # daemon mode (Unix socket)
    bench.rs          # benchmark data collection
```

**Why not a library crate + binary crate?** YAGNI. Single binary crate. Extract a lib later if needed.

**Why `cli/` not project root?** The Python bench analysis code stays at project root. Rust lives alongside, not replacing the full repo structure.

### 2. ONNX Runtime via `ort` crate

Use the `ort` crate (official ONNX Runtime Rust bindings) for embedding inference. Same BAAI/bge-small-en-v1.5 model fastembed uses, but loaded directly as an ONNX model file.

**Why not candle?** ort is battle-tested, fastembed's ONNX models work directly, and we know the exact performance characteristics. candle would require model format conversion and is less mature for production inference.

**Model file location:** `~/.cache/semtree/models/bge-small-en-v1.5.onnx` — downloaded once, shared across repos.

### 3. Binary .vec format

```
Header (16 bytes):
  magic:    [u8; 4]   = b"SVEC"
  version:  u16       = 1
  dims:     u16       = 384 (for bge-small-en-v1.5)
  hash_len: u32       = 64 (hex chars of content_hash)
  reserved: u32       = 0

Body:
  content_hash: [u8; hash_len]   = SHA-256 hex string
  model_name:   null-terminated  = "BAAI/bge-small-en-v1.5\0"
  vector:       [f32; dims]      = raw IEEE 754 floats
```

Total size for 384-dim: 16 + 64 + ~25 + 1536 = ~1641 bytes (vs ~3KB for JSON). But the real win is mmap — read the f32 array directly from the memory-mapped file with zero parsing.

**Why not a single index file?** Per-node files match the existing colocated design and allow incremental updates. A single index would require rewriting on every node change.

### 4. Daemon mode via Unix socket

`semtree serve` starts a background process listening on `~/.cache/semtree/semtree.sock`. Commands (`route`, `query`) detect the socket and send requests over it. If the daemon isn't running, they fall back to cold-start inline execution.

Protocol: newline-delimited JSON over Unix socket (simple, debuggable).

**Why not HTTP?** Unnecessary complexity for local-only communication. Unix socket is faster and doesn't need port management.

**Why not a long-running MCP server?** MCP is for tool integration with AI agents. The daemon serves the CLI itself, not external clients.

### 5. Summarizer: still subprocess to claude CLI

The summarizer calls `claude -p` as a subprocess, same as Python. LLM summarization is API-bound — Rust adds no speed benefit. Keeping this as subprocess means we don't need an Anthropic SDK dependency.

### 6. Migration path

1. Rust binary coexists with Python package during development
2. `semtree embed --force` regenerates .vec files in binary format (reads existing .sem/*.md, writes new .vec)
3. Python `bench/` analysis code reads results.tsv (unchanged format)
4. Skills updated to reference Rust binary
5. Python package deprecated after Rust achieves feature parity

## Risks / Trade-offs

- **ONNX model distribution** — Users need the .onnx model file. Mitigate: auto-download on first `embed`/`query`/`route`, cache in `~/.cache/semtree/`.
- **ONNX Runtime linking** — ort can be tricky to link statically. Mitigate: use `ort` with `download-binaries` feature for dev, `system` feature for distribution.
- **Binary .vec not human-readable** — JSON was inspectable. Mitigate: `semtree vec inspect <path>` command for debugging.
- **Two build systems** — Cargo for Rust, setuptools for Python bench. Mitigate: Python bench is stable and rarely changes; this is temporary.
- **Cross-platform ONNX** — ONNX Runtime has platform-specific binaries. Mitigate: CI builds for macOS arm64 + x86_64 + Linux x86_64. Windows deferred.

## Open Questions

- Should the daemon auto-start on first `route`/`query` call, or require explicit `semtree serve`?
- Should we ship the ONNX model embedded in the binary (larger binary, simpler UX) or download on first use?
- Do we want `semtree bench` in Rust, or keep it as a Python-only tool that reads TSV?
