---
path: openspec/changes/srt-benchmark/specs/bench-harness/spec.md
type: file
content_hash: 64ee1cbb00faf470bae079e472adcda1ca81f4e14bc5ab6a18412c3d23c2cb38
---

This specification defines requirements for a semtree benchmark harness that provides a `semtree bench <phase>` CLI command for performance testing. The system supports running individual benchmark phases (build, quality, routing, incremental) or all phases sequentially, with an optional `--repo` flag to select the target repository. The harness executes phases by calling semtree library functions directly rather than using subprocesses, collects wall-clock timing automatically, and logs all metrics to a standardized TSV file with columns for timestamp, phase, repo, metric, value, and notes. Each phase returns structured metric data as tuples, and the system includes proper error handling for invalid phase names and unknown repositories.
