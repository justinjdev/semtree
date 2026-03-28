"""Build pipeline: walk → hash → check freshness → summarize → write records."""

import sys
from pathlib import Path

from semtree.config import BuildConfig
from semtree.hasher import hash_directory, hash_file
from semtree.records import (
    read_record,
    record_path_for_dir,
    record_path_for_dir_sibling,
    record_path_for_file,
    write_record,
)
from semtree.summarizer import (
    OVERSIZED_PLACEHOLDER,
    ClaudeCLISummarizer,
    build_dir_prompt,
    build_file_prompt,
    is_oversized,
)
from semtree.walker import walk


def build(config: BuildConfig) -> None:
    """Run the full SRT build pipeline."""
    summarizer = ClaudeCLISummarizer(model=config.model)
    nodes = walk(config.target_path, exclude=config.exclude)

    # Track hashes and summaries for directory aggregation
    node_hashes: dict[str, str] = {}   # repo_relative_path -> content_hash
    node_summaries: dict[str, str] = {}  # repo_relative_path -> summary text

    stats = {"summarized": 0, "skipped": 0, "errored": 0}
    total = len(nodes)

    for i, node in enumerate(nodes, 1):
        rel = node.repo_relative_path
        label = rel or "(root)"

        if node.is_directory:
            # Compute directory hash from children
            child_pairs = [
                (c, node_hashes[c])
                for c in node.children
                if c in node_hashes
            ]
            content_hash = hash_directory(child_pairs)
            node_hashes[rel] = content_hash

            rec_path = record_path_for_dir(config.target_path, rel)
            existing = read_record(rec_path)

            if not config.force and existing and existing.get("content_hash") == content_hash:
                node_summaries[rel] = existing.get("summary", "")
                # Ensure sibling record exists at parent level
                sibling_path = record_path_for_dir_sibling(config.target_path, rel)
                if sibling_path != rec_path:
                    sibling_existing = read_record(sibling_path)
                    if not sibling_existing or sibling_existing.get("content_hash") != content_hash:
                        write_record(sibling_path, rel, "directory", content_hash, existing.get("summary", ""))
                stats["skipped"] += 1
                print(f"[{i}/{total}] skip {label} (up-to-date)", file=sys.stderr)
                continue

            # Build directory prompt from child summaries
            child_summary_pairs = [
                (c, node_summaries.get(c, OVERSIZED_PLACEHOLDER))
                for c in node.children
            ]
            prompt = build_dir_prompt(rel, child_summary_pairs)

            try:
                summary = summarizer.summarize(prompt)
            except RuntimeError as e:
                print(f"[{i}/{total}] ERROR {label}: {e}", file=sys.stderr)
                stats["errored"] += 1
                node_summaries[rel] = ""
                continue

            write_record(rec_path, rel or ".", "directory", content_hash, summary)
            # Also write sibling record at parent level for embedding/routing
            sibling_path = record_path_for_dir_sibling(config.target_path, rel)
            if sibling_path != rec_path:  # skip for root (they're the same)
                write_record(sibling_path, rel, "directory", content_hash, summary)
            node_summaries[rel] = summary
            stats["summarized"] += 1
            print(f"[{i}/{total}] summarized {label}", file=sys.stderr)

        else:
            # File node
            content_hash = hash_file(node.absolute_path)
            node_hashes[rel] = content_hash

            rec_path = record_path_for_file(config.target_path, rel)
            existing = read_record(rec_path)

            if not config.force and existing and existing.get("content_hash") == content_hash:
                node_summaries[rel] = existing.get("summary", "")
                stats["skipped"] += 1
                print(f"[{i}/{total}] skip {label} (up-to-date)", file=sys.stderr)
                continue

            # Check oversized
            file_size = node.absolute_path.stat().st_size
            if is_oversized(file_size, config.max_tokens):
                write_record(rec_path, rel, "file", content_hash, OVERSIZED_PLACEHOLDER)
                node_summaries[rel] = OVERSIZED_PLACEHOLDER
                stats["summarized"] += 1
                print(f"[{i}/{total}] oversized {label}", file=sys.stderr)
                continue

            file_content = node.absolute_path.read_text(encoding="utf-8", errors="replace")
            prompt = build_file_prompt(rel, file_content)

            try:
                summary = summarizer.summarize(prompt)
            except RuntimeError as e:
                print(f"[{i}/{total}] ERROR {label}: {e}", file=sys.stderr)
                stats["errored"] += 1
                node_summaries[rel] = ""
                continue

            write_record(rec_path, rel, "file", content_hash, summary)
            node_summaries[rel] = summary
            stats["summarized"] += 1
            print(f"[{i}/{total}] summarized {label}", file=sys.stderr)

    print(
        f"\nDone: {stats['summarized']} summarized, "
        f"{stats['skipped']} skipped, "
        f"{stats['errored']} errored",
        file=sys.stderr,
    )

    if config.embed:
        from semtree.embedder import embed_directory

        print("\nComputing embeddings...", file=sys.stderr)
        embed_stats = embed_directory(
            config.target_path,
            model=config.embed_model,
            force=config.force,
        )
        print(
            f"Embeddings: {embed_stats['embedded']} embedded, "
            f"{embed_stats['skipped']} skipped, "
            f"{embed_stats['errored']} errored",
            file=sys.stderr,
        )
