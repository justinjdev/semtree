"""Build configuration."""

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class BuildConfig:
    target_path: Path
    model: str = "claude-sonnet-4-20250514"
    max_tokens: int = 100_000
    force: bool = False
