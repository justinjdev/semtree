"""CLI entry point for semtree."""

import argparse
import shutil
import sys
from pathlib import Path

from semtree.config import BuildConfig
from semtree.builder import build


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="semtree",
        description="Semantic Resolution Tree indexer",
    )
    sub = parser.add_subparsers(dest="command")

    build_parser = sub.add_parser("build", help="Build or update the SRT for a repository")
    build_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Repository root path (default: current directory)",
    )
    build_parser.add_argument(
        "--model",
        default="claude-sonnet-4-20250514",
        help="LLM model for summarization (default: claude-sonnet-4-20250514)",
    )
    build_parser.add_argument(
        "--max-tokens",
        type=int,
        default=100_000,
        help="Max estimated tokens before marking file as oversized (default: 100000)",
    )
    build_parser.add_argument(
        "--force",
        action="store_true",
        help="Rebuild all records, ignoring hash freshness checks",
    )
    build_parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        help="Glob pattern to exclude (can be repeated, e.g. --exclude 'static/_app/*')",
    )

    build_parser.add_argument(
        "--no-embed",
        action="store_true",
        help="Skip embedding computation after build",
    )
    build_parser.add_argument(
        "--embed-model",
        default="BAAI/bge-small-en-v1.5",
        help="Embedding model name (default: BAAI/bge-small-en-v1.5)",
    )

    embed_parser = sub.add_parser("embed", help="Compute embeddings for existing .sem/ records")
    embed_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Repository root path (default: current directory)",
    )
    embed_parser.add_argument(
        "--model",
        default="BAAI/bge-small-en-v1.5",
        help="Embedding model name (default: BAAI/bge-small-en-v1.5)",
    )
    embed_parser.add_argument(
        "--force",
        action="store_true",
        help="Re-embed all records, ignoring freshness checks",
    )

    query_parser = sub.add_parser("query", help="Rank directory children by similarity to a query")
    query_parser.add_argument(
        "query",
        help="Natural language query",
    )
    query_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Directory whose children to rank (default: current directory)",
    )
    query_parser.add_argument(
        "--model",
        default="BAAI/bge-small-en-v1.5",
        help="Embedding model name (default: BAAI/bge-small-en-v1.5)",
    )
    query_parser.add_argument(
        "--top-k",
        type=int,
        default=None,
        help="Return only top K results",
    )
    query_parser.add_argument(
        "--threshold",
        type=float,
        default=None,
        help="Minimum cosine similarity score",
    )

    route_parser = sub.add_parser("route", help="Full top-down descent ranking children at each level")
    route_parser.add_argument(
        "query",
        help="Natural language query",
    )
    route_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Root directory to start descent from (default: current directory)",
    )
    route_parser.add_argument(
        "--model",
        default="BAAI/bge-small-en-v1.5",
        help="Embedding model name (default: BAAI/bge-small-en-v1.5)",
    )
    route_parser.add_argument(
        "--beam-width",
        type=int,
        default=3,
        help="Number of children to select at each level (default: 3)",
    )
    route_parser.add_argument(
        "--max-depth",
        type=int,
        default=10,
        help="Maximum descent depth (default: 10)",
    )

    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        sys.exit(1)

    if args.command == "build":
        target = Path(args.path).resolve()
        if not target.is_dir():
            print(f"error: {args.path} is not a directory", file=sys.stderr)
            sys.exit(1)

        if not shutil.which("claude"):
            print(
                "error: claude CLI not found on PATH. "
                "Install Claude Code: https://claude.ai/code",
                file=sys.stderr,
            )
            sys.exit(1)

        config = BuildConfig(
            target_path=target,
            model=args.model,
            max_tokens=args.max_tokens,
            force=args.force,
            exclude=tuple(args.exclude),
            embed=not args.no_embed,
            embed_model=args.embed_model,
        )
        build(config)

    elif args.command == "embed":
        target = Path(args.path).resolve()
        if not target.is_dir():
            print(f"error: {args.path} is not a directory", file=sys.stderr)
            sys.exit(1)

        from semtree.embedder import embed_directory

        stats = embed_directory(target, model=args.model, force=args.force)
        print(
            f"Done: {stats['embedded']} embedded, "
            f"{stats['skipped']} skipped, "
            f"{stats['errored']} errored",
            file=sys.stderr,
        )

    elif args.command == "query":
        target = Path(args.path).resolve()
        if not target.is_dir():
            print(f"error: {args.path} is not a directory", file=sys.stderr)
            sys.exit(1)

        from semtree.embedder import query_directory

        results = query_directory(
            target,
            query=args.query,
            model=args.model,
            top_k=args.top_k,
            threshold=args.threshold,
        )

        if not results:
            print("No results (missing .vec files? Run: semtree embed)", file=sys.stderr)
            sys.exit(1)

        for score, path, first_line in results:
            print(f"{score:.4f}\t{path}\t{first_line}")

    elif args.command == "route":
        target = Path(args.path).resolve()
        if not target.is_dir():
            print(f"error: {args.path} is not a directory", file=sys.stderr)
            sys.exit(1)

        from semtree.embedder import route_directory
        import time

        t0 = time.monotonic()
        levels = route_directory(
            target,
            query=args.query,
            model=args.model,
            beam_width=args.beam_width,
            max_depth=args.max_depth,
        )
        elapsed = time.monotonic() - t0

        if not levels:
            print("No results (missing .vec files? Run: semtree embed)", file=sys.stderr)
            sys.exit(1)

        candidates = []
        for level in levels:
            dir_label = level["dir"]
            n = level["all_children"]
            print(f"\n{dir_label}/ ({n} children):")
            for path, score, first_line in level["selected"]:
                marker = "/" if any(
                    l["dir"] == path or l["dir"].startswith(path + "/")
                    for l in levels
                ) else ""
                print(f"  {score:.4f}  {path}{marker}\t{first_line[:80]}")
                if not marker:
                    candidates.append(path)

        if candidates:
            print(f"\nCandidates: {', '.join(candidates)}")
        print(f"Time: {elapsed:.2f}s", file=sys.stderr)
