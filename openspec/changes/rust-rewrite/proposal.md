## Why

The semtree CLI is Python — fine for prototyping but the 700ms cold-start per invocation (200ms Python startup + 456ms model load + 139ms JSON .vec parsing) makes interactive use sluggish and benchmarking slow. The embedding model load dominates but cannot be amortized across invocations without a persistent process. A Rust rewrite eliminates interpreter overhead, enables daemon mode for sub-5ms warm routing, and produces a single static binary with no runtime dependencies.

## What Changes

- **BREAKING**: Replace `src/semtree/` Python package with a Rust binary crate at `cli/`
- New Rust binary implementing: `build`, `embed`, `query`, `route`, `bench` subcommands
- New daemon mode (`semtree serve`) keeping the embedding model resident for warm routing
- New binary `.vec` format (flat f32 arrays with header) replacing JSON sidecars, with mmap I/O
- ONNX Runtime integration via `ort` crate for embedding inference (same BAAI/bge-small-en-v1.5 model)
- Python package remains for `bench/` analysis code (Pareto, hypervolume) — Rust handles data collection, Python handles statistical analysis
- `.sem/*.md` record format unchanged — Rust reads/writes the same YAML frontmatter + Markdown body
- Walker, hasher, summarizer logic ported from Python to Rust
- `srt-navigate` and `srt-build` skills updated to use the Rust binary

## Capabilities

### New Capabilities
- `rust-cli`: Core CLI binary with subcommands (build, embed, query, route, bench)
- `daemon-mode`: Persistent server keeping embedding model loaded for warm routing via Unix socket
- `binary-vec`: Compact binary .vec format with mmap reads replacing JSON sidecars
- `rust-embedder`: ONNX Runtime embedding inference and cosine ranking in Rust
- `rust-walker`: Filesystem traversal with git-aware filtering in Rust
- `rust-hasher`: SHA-256 content hashing with directory hash aggregation
- `rust-records`: .sem/ record I/O (YAML frontmatter + Markdown body) in Rust
- `rust-summarizer`: LLM summarization via claude CLI subprocess (same approach as Python)

### Modified Capabilities
<!-- No existing spec-level behavior changes — the Rust CLI produces identical outputs -->

## Impact

- **Build system**: New Cargo workspace at `cli/` alongside existing Python `src/`
- **Dependencies**: ort (ONNX Runtime), serde/serde_yaml, sha2, clap, tokio (for daemon)
- **Distribution**: Single binary installable via `cargo install` or `brew`
- **Python bench/**: Retained for analysis — imports TSV results, not semtree library
- **Skills**: srt-navigate and srt-build updated to reference Rust binary path
- **CI**: Need Rust toolchain + ONNX Runtime in CI
- **.vec files**: Binary format is not backward-compatible with JSON — migration needed (one-time `semtree embed --force`)
