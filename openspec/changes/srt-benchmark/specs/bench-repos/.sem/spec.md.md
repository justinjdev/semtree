---
path: openspec/changes/srt-benchmark/specs/bench-repos/spec.md
type: file
content_hash: 9ff44102dd562589e536489416c4541c46d0121fa74e30b3a1c29acb62163c70
---

This specification defines a repository manager for the SRT benchmark system that handles cloning and caching of benchmark repositories at pinned commits to ensure reproducible performance testing. The manager supports three size tiers (small <200 files, medium 200-1000 files, large >1000 files) and reads repository definitions from a `bench/repos.yaml` configuration file containing git URLs, commit SHAs, and metadata for each benchmark repo. Key features include local caching in `bench/.repos/` to avoid repeated network fetches, automatic re-cloning when cached repos don't match the pinned commit, and a cleanup command to reclaim disk space. The system is designed to provide deterministic benchmark environments by ensuring all runs use identical repository states defined by specific commit SHAs rather than moving branch heads.
