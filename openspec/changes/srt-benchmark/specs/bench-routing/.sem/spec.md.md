---
path: openspec/changes/srt-benchmark/specs/bench-routing/spec.md
type: file
content_hash: c6815d6f816e20eba7b1396f73b26e4f6a0f6bdceffa64cf53a8f5868482ddb4
---

This specification defines requirements for benchmarking SRT routing performance through simulated agent descent. The routing phase loads query sets from YAML files containing questions and expected target files, then simulates the SRT protocol by reading `__dir__.md` summaries, making LLM routing decisions at each directory level, and recursively descending into selected children until reaching leaf files. Key metrics include `recall@k` (fraction of expected files found in top-k results, defaulting to k=5) and `files_opened` (total `.sem/` records read, measuring navigation cost). The benchmark emphasizes reproducibility by requiring `temperature=0` for all LLM routing calls and provides graceful handling of missing or malformed query entries.
