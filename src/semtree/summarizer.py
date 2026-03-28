"""LLM summarization for SRT nodes.

Default provider: Claude Code CLI via `claude -p`.
"""

import subprocess
import sys
import time
from typing import Protocol, runtime_checkable


OVERSIZED_PLACEHOLDER = "summary unavailable: oversized file"

FILE_PROMPT = """\
Summarize this source file. Be concise but capture the file's purpose, \
key exports, and important implementation details. \
Write 2-5 sentences of plain prose.

File: {path}

```
{content}
```"""

DIR_PROMPT = """\
Summarize this directory based on its children. Write a concise prose overview \
(2-4 sentences), then a ## Children section listing every immediate child \
with a brief description (one line each, as a bullet list using **bold** \
for the child path).

Directory: {path}

Children:
{children_block}"""


@runtime_checkable
class Summarizer(Protocol):
    def summarize(self, prompt: str) -> str: ...


class ClaudeCLISummarizer:
    """Summarizer that calls `claude -p` via subprocess."""

    def __init__(self, model: str = "claude-sonnet-4-20250514", max_retries: int = 3):
        self.model = model
        self.max_retries = max_retries

    def summarize(self, prompt: str) -> str:
        for attempt in range(self.max_retries):
            try:
                result = subprocess.run(
                    ["claude", "-p", "--model", self.model],
                    input=prompt,
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                if result.returncode == 0 and result.stdout.strip():
                    return result.stdout.strip()

                if attempt < self.max_retries - 1:
                    time.sleep(2 ** attempt)
                    continue

                raise RuntimeError(
                    f"claude CLI failed (exit {result.returncode}): {result.stderr.strip()}"
                )
            except FileNotFoundError:
                raise RuntimeError(
                    "claude CLI not found on PATH. Install Claude Code: https://claude.ai/code"
                )
            except subprocess.TimeoutExpired:
                if attempt < self.max_retries - 1:
                    time.sleep(2 ** attempt)
                    continue
                raise RuntimeError("claude CLI timed out after 120s")

        raise RuntimeError("summarization failed after all retries")


def is_oversized(file_bytes_len: int, max_tokens: int) -> bool:
    """Check if a file exceeds the token limit (estimated as bytes / 4)."""
    return file_bytes_len / 4 > max_tokens


def build_file_prompt(path: str, content: str) -> str:
    """Build the summarization prompt for a file leaf."""
    return FILE_PROMPT.format(path=path, content=content)


def build_dir_prompt(path: str, children: list[tuple[str, str]]) -> str:
    """Build the summarization prompt for a directory node.

    children: list of (child_repo_relative_path, child_summary) pairs.
    """
    blocks = []
    for child_path, child_summary in children:
        blocks.append(f"- **{child_path}**: {child_summary}")
    children_block = "\n".join(blocks)
    return DIR_PROMPT.format(path=path or "(repository root)", children_block=children_block)
