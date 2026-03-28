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
        )
        build(config)
