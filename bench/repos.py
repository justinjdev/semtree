"""Benchmark repo cloning, pinning, and caching."""

import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

import yaml


DEFAULT_CACHE_DIR = Path(__file__).parent / ".repos"


@dataclass
class RepoConfig:
    name: str
    url: str
    commit: str
    tier: str
    description: str


def load_repo_configs(config_path: Path | None = None) -> list[RepoConfig]:
    """Load repo configs from YAML file."""
    if config_path is None:
        config_path = Path(__file__).parent / "repos.yaml"
    data = yaml.safe_load(config_path.read_text(encoding="utf-8"))
    return [
        RepoConfig(
            name=r["name"],
            url=r["url"],
            commit=r["commit"],
            tier=r["tier"],
            description=r["description"],
        )
        for r in data["repos"]
    ]


def get_repo(name: str, cache_dir: Path = DEFAULT_CACHE_DIR, config_path: Path | None = None) -> Path:
    """Get path to a cached benchmark repo, cloning if needed."""
    configs = load_repo_configs(config_path)
    config = next((c for c in configs if c.name == name), None)
    if config is None:
        available = [c.name for c in configs]
        raise ValueError(f"Unknown repo '{name}'. Available: {available}")

    repo_path = cache_dir / name
    if repo_path.exists():
        # Verify pinned commit
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_path, capture_output=True, text=True,
        )
        if result.returncode == 0 and result.stdout.strip().startswith(config.commit[:7]):
            return repo_path
        # Wrong commit — re-clone
        shutil.rmtree(repo_path)

    cache_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["git", "clone", config.url, str(repo_path)],
        check=True, capture_output=True,
    )
    if config.commit != "HEAD":
        subprocess.run(
            ["git", "checkout", config.commit],
            cwd=repo_path, check=True, capture_output=True,
        )
    return repo_path


def clean_cache(cache_dir: Path = DEFAULT_CACHE_DIR) -> None:
    """Remove all cached benchmark repos."""
    if cache_dir.exists():
        shutil.rmtree(cache_dir)
