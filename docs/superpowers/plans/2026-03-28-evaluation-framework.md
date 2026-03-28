# SRT Evaluation Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a benchmark harness that collects (cost, latency, quality) measurements across control grids, then analyzes them via Pareto frontiers, hypervolume, and frontier geometry diagnostics per v9's evaluation framework.

**Architecture:** A `bench/` package contains phase runners that collect raw data into `results.tsv`, plus an analysis module that consumes the data and computes v9's evaluation objects. The harness calls semtree library functions directly (no subprocess). A grep/glob baseline provides comparison.

**Tech Stack:** Python 3.11+, pyyaml (query sets, repo config), numpy (analysis math), semtree internals (builder, walker, hasher, records, embedder)

**Spec:** `docs/superpowers/specs/2026-03-28-evaluation-framework-design.md`

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `bench/__init__.py` | Create | Package init |
| `bench/harness.py` | Create | TSV I/O, timing, phase dispatch |
| `bench/repos.py` | Create | Repo clone, pin, cache |
| `bench/repos.yaml` | Create | Pinned repo config |
| `bench/build_phase.py` | Create | Full + incremental build measurement |
| `bench/quality.py` | Create | Structural correctness checks |
| `bench/routing.py` | Create | Simulated SRT descent with control grid |
| `bench/baseline.py` | Create | Grep/glob baseline system |
| `bench/incremental.py` | Create | Incremental rebuild measurement |
| `bench/analysis.py` | Create | Pareto, hypervolume, frontier diagnostics |
| `bench/queries/fellowship.yaml` | Create | Labeled query set with graded relevance |
| `src/semtree/cli.py` | Modify | Add `semtree bench` subcommand |
| `tests/test_harness.py` | Create | TSV I/O and timing tests |
| `tests/test_analysis.py` | Create | Pareto, hypervolume, frontier tests |
| `tests/test_routing_bench.py` | Create | Routing phase tests with mocked LLM |
| `tests/test_quality_bench.py` | Create | Quality phase tests with fixture records |

---

### Task 1: Bench package and TSV I/O

**Files:**
- Create: `bench/__init__.py`
- Create: `bench/harness.py`
- Create: `tests/test_harness.py`

- [ ] **Step 1: Write failing tests for TSV I/O**

Create `tests/test_harness.py`:

```python
"""Tests for bench.harness module."""

from pathlib import Path

from bench.harness import append_results, read_results, MetricRecord


class TestMetricRecord:
    def test_fields(self) -> None:
        r = MetricRecord(
            timestamp="2026-03-28T12:00:00",
            phase="build",
            repo="fellowship",
            system="srt",
            query_id="",
            control_json="",
            metric="build_time_s",
            value=180.5,
        )
        assert r.phase == "build"
        assert r.value == 180.5


class TestAppendResults:
    def test_creates_file_with_header(self, tmp_path: Path) -> None:
        tsv = tmp_path / "results.tsv"
        records = [
            MetricRecord("2026-03-28T12:00:00", "build", "fellowship", "srt", "", "", "build_time_s", 180.5),
        ]
        append_results(tsv, records)

        lines = tsv.read_text().strip().split("\n")
        assert len(lines) == 2  # header + 1 data row
        assert lines[0].startswith("timestamp\t")
        assert "180.5" in lines[1]

    def test_appends_without_repeating_header(self, tmp_path: Path) -> None:
        tsv = tmp_path / "results.tsv"
        r1 = [MetricRecord("2026-03-28T12:00:00", "build", "f", "srt", "", "", "m1", 1.0)]
        r2 = [MetricRecord("2026-03-28T12:01:00", "build", "f", "srt", "", "", "m2", 2.0)]
        append_results(tsv, r1)
        append_results(tsv, r2)

        lines = tsv.read_text().strip().split("\n")
        assert len(lines) == 3  # header + 2 data rows
        assert lines[0].startswith("timestamp\t")

    def test_routing_row_includes_query_and_control(self, tmp_path: Path) -> None:
        tsv = tmp_path / "results.tsv"
        records = [
            MetricRecord("2026-03-28T12:00:00", "routing", "fellowship", "srt", "q03", '{"beam":3}', "ndcg@10", 0.85),
        ]
        append_results(tsv, records)

        lines = tsv.read_text().strip().split("\n")
        assert "q03" in lines[1]
        assert '{"beam":3}' in lines[1]


class TestReadResults:
    def test_round_trip(self, tmp_path: Path) -> None:
        tsv = tmp_path / "results.tsv"
        records = [
            MetricRecord("2026-03-28T12:00:00", "build", "f", "srt", "", "", "m1", 1.5),
            MetricRecord("2026-03-28T12:00:00", "routing", "f", "srt", "q01", "{}", "ndcg@10", 0.9),
        ]
        append_results(tsv, records)
        loaded = read_results(tsv)
        assert len(loaded) == 2
        assert loaded[0].metric == "m1"
        assert loaded[0].value == 1.5
        assert loaded[1].query_id == "q01"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_harness.py -v 2>&1 | tail -10`
Expected: ImportError

- [ ] **Step 3: Implement harness module**

Create `bench/__init__.py`:

```python
```

Create `bench/harness.py`:

```python
"""Benchmark harness: TSV I/O, timing, phase dispatch."""

from dataclasses import dataclass, fields
from pathlib import Path


HEADER_FIELDS = ("timestamp", "phase", "repo", "system", "query_id", "control_json", "metric", "value")


@dataclass
class MetricRecord:
    timestamp: str
    phase: str
    repo: str
    system: str
    query_id: str
    control_json: str
    metric: str
    value: float


def append_results(tsv_path: Path, records: list[MetricRecord]) -> None:
    """Append metric records to a TSV file, creating with header if needed."""
    write_header = not tsv_path.exists()
    with tsv_path.open("a", encoding="utf-8") as f:
        if write_header:
            f.write("\t".join(HEADER_FIELDS) + "\n")
        for r in records:
            row = "\t".join(str(getattr(r, f)) for f in HEADER_FIELDS)
            f.write(row + "\n")


def read_results(tsv_path: Path) -> list[MetricRecord]:
    """Read all metric records from a TSV file."""
    records = []
    lines = tsv_path.read_text(encoding="utf-8").strip().split("\n")
    if len(lines) < 2:
        return records
    for line in lines[1:]:  # skip header
        parts = line.split("\t")
        if len(parts) != len(HEADER_FIELDS):
            continue
        records.append(MetricRecord(
            timestamp=parts[0],
            phase=parts[1],
            repo=parts[2],
            system=parts[3],
            query_id=parts[4],
            control_json=parts[5],
            metric=parts[6],
            value=float(parts[7]),
        ))
    return records
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_harness.py -v 2>&1 | tail -15`
Expected: All 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add bench/__init__.py bench/harness.py tests/test_harness.py
git commit -m "feat: add bench harness with TSV I/O"
```

---

### Task 2: Repo manager

**Files:**
- Create: `bench/repos.py`
- Create: `bench/repos.yaml`

- [ ] **Step 1: Create repo config**

Create `bench/repos.yaml`:

```yaml
repos:
  - name: fellowship
    url: https://github.com/justinjdev/fellowship.git
    commit: HEAD
    tier: small
    description: "Go CLI + SvelteKit dashboard + Claude Code plugin (~160 files)"
```

Note: Replace `HEAD` with the actual pinned SHA after first clone.

- [ ] **Step 2: Implement repos module**

Create `bench/repos.py`:

```python
"""Benchmark repo cloning, pinning, and caching."""

import shutil
import subprocess
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
```

Add the missing import at top:

```python
from dataclasses import dataclass
```

- [ ] **Step 3: Commit**

```bash
git add bench/repos.py bench/repos.yaml
git commit -m "feat: add benchmark repo manager with clone and cache"
```

---

### Task 3: Analysis module — Pareto and hypervolume

**Files:**
- Create: `bench/analysis.py`
- Create: `tests/test_analysis.py`

- [ ] **Step 1: Write failing tests for Pareto pruning**

Create `tests/test_analysis.py`:

```python
"""Tests for bench.analysis module."""

import numpy as np
import pytest

from bench.analysis import pareto_prune, normalize_to_utility, hypervolume


class TestParetoPrune:
    def test_single_point(self) -> None:
        points = np.array([[1.0, 2.0, 0.8]])  # cost, latency, quality
        frontier = pareto_prune(points)
        assert len(frontier) == 1

    def test_dominated_point_removed(self) -> None:
        # Point B dominates Point A (lower cost, lower latency, higher quality)
        points = np.array([
            [5.0, 3.0, 0.5],  # A: dominated
            [2.0, 1.0, 0.8],  # B: dominates A
        ])
        frontier = pareto_prune(points)
        assert len(frontier) == 1
        assert frontier[0][2] == pytest.approx(0.8)

    def test_non_dominated_points_kept(self) -> None:
        # A is cheaper, B is higher quality — neither dominates
        points = np.array([
            [1.0, 2.0, 0.5],  # A: cheap but low quality
            [5.0, 1.0, 0.9],  # B: expensive but high quality
        ])
        frontier = pareto_prune(points)
        assert len(frontier) == 2

    def test_empty_input(self) -> None:
        points = np.empty((0, 3))
        frontier = pareto_prune(points)
        assert len(frontier) == 0

    def test_three_objectives_mixed(self) -> None:
        points = np.array([
            [1.0, 1.0, 0.9],  # best on all — dominates everything
            [2.0, 2.0, 0.8],  # dominated by first
            [3.0, 0.5, 0.7],  # low latency, but dominated by first
        ])
        frontier = pareto_prune(points)
        assert len(frontier) == 1


class TestNormalizeToUtility:
    def test_utility_transform(self) -> None:
        frontier = np.array([
            [0.0, 0.0, 1.0],  # min cost, min latency, max quality → (1,1,1)
            [1.0, 1.0, 0.0],  # max cost, max latency, min quality → (0,0,0)
        ])
        global_ranges = {"cost": (0.0, 1.0), "latency": (0.0, 1.0), "quality": (0.0, 1.0)}
        utility = normalize_to_utility(frontier, global_ranges)
        assert utility[0] == pytest.approx([1.0, 1.0, 1.0])
        assert utility[1] == pytest.approx([0.0, 0.0, 0.0])

    def test_mid_range_values(self) -> None:
        frontier = np.array([[0.5, 0.5, 0.5]])
        global_ranges = {"cost": (0.0, 1.0), "latency": (0.0, 1.0), "quality": (0.0, 1.0)}
        utility = normalize_to_utility(frontier, global_ranges)
        assert utility[0] == pytest.approx([0.5, 0.5, 0.5])


class TestHypervolume:
    def test_single_point(self) -> None:
        # Point at (0.5, 0.5, 0.5) in utility space → volume = 0.125
        utility_frontier = np.array([[0.5, 0.5, 0.5]])
        hv = hypervolume(utility_frontier)
        assert hv == pytest.approx(0.125)

    def test_perfect_point(self) -> None:
        # Point at (1,1,1) → volume = 1.0
        utility_frontier = np.array([[1.0, 1.0, 1.0]])
        hv = hypervolume(utility_frontier)
        assert hv == pytest.approx(1.0)

    def test_two_points_union(self) -> None:
        # Two non-overlapping boxes
        utility_frontier = np.array([
            [1.0, 0.5, 0.5],  # box volume = 0.25
            [0.5, 1.0, 0.5],  # box volume = 0.25, overlap = 0.5*0.5*0.5 = 0.125
        ])
        hv = hypervolume(utility_frontier)
        # Union = 0.25 + 0.25 - 0.125 = 0.375
        assert hv == pytest.approx(0.375)

    def test_empty_frontier(self) -> None:
        utility_frontier = np.empty((0, 3))
        hv = hypervolume(utility_frontier)
        assert hv == 0.0
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_analysis.py -v 2>&1 | tail -10`
Expected: ImportError

- [ ] **Step 3: Implement Pareto, normalization, and hypervolume**

Create `bench/analysis.py`:

```python
"""Evaluation analysis: Pareto frontiers, hypervolume, frontier diagnostics."""

import numpy as np


def pareto_prune(points: np.ndarray) -> np.ndarray:
    """Extract non-dominated points from (cost, latency, quality) triples.

    A point x dominates y if: x.cost <= y.cost, x.latency <= y.latency,
    x.quality >= y.quality, with at least one strict inequality.
    Returns the Pareto frontier as an ndarray.
    """
    if len(points) == 0:
        return points

    # Convert to "all maximize" form for easier comparison
    # Negate cost and latency so higher = better for all objectives
    converted = points.copy()
    converted[:, 0] = -converted[:, 0]  # negate cost
    converted[:, 1] = -converted[:, 1]  # negate latency

    mask = np.ones(len(converted), dtype=bool)
    for i in range(len(converted)):
        if not mask[i]:
            continue
        for j in range(len(converted)):
            if i == j or not mask[j]:
                continue
            # Does j dominate i? (j >= i on all, j > i on at least one)
            if np.all(converted[j] >= converted[i]) and np.any(converted[j] > converted[i]):
                mask[i] = False
                break

    return points[mask]


def normalize_to_utility(
    frontier: np.ndarray,
    global_ranges: dict[str, tuple[float, float]],
) -> np.ndarray:
    """Map (cost, latency, quality) to utility coordinates [0,1].

    u_c = 1 - (c - c_min)/(c_max - c_min)
    u_l = 1 - (l - l_min)/(l_max - l_min)
    u_a = (a - a_min)/(a_max - a_min)
    """
    if len(frontier) == 0:
        return frontier

    result = np.zeros_like(frontier)
    c_min, c_max = global_ranges["cost"]
    l_min, l_max = global_ranges["latency"]
    a_min, a_max = global_ranges["quality"]

    c_range = c_max - c_min if c_max > c_min else 1.0
    l_range = l_max - l_min if l_max > l_min else 1.0
    a_range = a_max - a_min if a_max > a_min else 1.0

    result[:, 0] = 1.0 - (frontier[:, 0] - c_min) / c_range
    result[:, 1] = 1.0 - (frontier[:, 1] - l_min) / l_range
    result[:, 2] = (frontier[:, 2] - a_min) / a_range

    return result


def hypervolume(utility_frontier: np.ndarray) -> float:
    """Compute dominated hypervolume in utility space, reference point (0,0,0).

    Uses inclusion-exclusion for small frontiers (sufficient for our use case).
    """
    if len(utility_frontier) == 0:
        return 0.0

    n = len(utility_frontier)
    if n == 1:
        return float(np.prod(utility_frontier[0]))

    # Inclusion-exclusion over all subsets
    total = 0.0
    for k in range(1, n + 1):
        from itertools import combinations
        for subset in combinations(range(n), k):
            # Intersection box: min of each coordinate across subset
            box = np.min(utility_frontier[list(subset)], axis=0)
            vol = float(np.prod(np.maximum(box, 0.0)))
            if k % 2 == 1:
                total += vol
            else:
                total -= vol

    return total
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_analysis.py -v 2>&1 | tail -15`
Expected: All 9 tests PASS

- [ ] **Step 5: Commit**

```bash
git add bench/analysis.py tests/test_analysis.py
git commit -m "feat: add Pareto pruning, normalization, and hypervolume"
```

---

### Task 4: Analysis module — frontier slices and diagnostics

**Files:**
- Modify: `bench/analysis.py`
- Modify: `tests/test_analysis.py`

- [ ] **Step 1: Write failing tests for slices and diagnostics**

Append to `tests/test_analysis.py`:

```python
from bench.analysis import (
    budget_slice,
    latency_slice,
    initial_ascent,
    knee_location,
    flattening_rate,
    workload_hypervolume,
    category_hypervolume,
)


class TestBudgetSlice:
    def test_filters_by_latency_band(self) -> None:
        # Points: (cost, latency, quality)
        points = np.array([
            [1.0, 0.5, 0.3],
            [2.0, 0.6, 0.5],
            [3.0, 5.0, 0.9],  # outside latency band
            [4.0, 0.4, 0.7],
        ])
        # latency band [0.3, 0.7] → includes points 0, 1, 3
        curve = budget_slice(points, latency_band=(0.3, 0.7))
        assert len(curve) == 3
        # Curve is (cost, quality) pairs sorted by cost
        assert curve[0][0] < curve[1][0]

    def test_empty_band(self) -> None:
        points = np.array([[1.0, 5.0, 0.5]])
        curve = budget_slice(points, latency_band=(0.0, 0.1))
        assert len(curve) == 0


class TestLatencySlice:
    def test_filters_by_cost_band(self) -> None:
        points = np.array([
            [1.0, 0.5, 0.3],
            [1.5, 0.6, 0.5],
            [10.0, 0.1, 0.9],  # outside cost band
        ])
        curve = latency_slice(points, cost_band=(0.5, 2.0))
        assert len(curve) == 2


class TestInitialAscent:
    def test_steep_ascent(self) -> None:
        # (resource, quality) pairs — quality jumps quickly
        curve = [(0.1, 0.0), (0.2, 0.5), (0.5, 0.8), (1.0, 0.9)]
        slope = initial_ascent(curve)
        assert slope > 0

    def test_flat_start(self) -> None:
        curve = [(0.1, 0.0), (0.5, 0.01), (1.0, 0.8)]
        slope = initial_ascent(curve)
        assert slope < 1.0


class TestKneeLocation:
    def test_finds_knee(self) -> None:
        # Quality rises fast then flattens
        curve = [(0.1, 0.0), (0.2, 0.5), (0.3, 0.75), (0.5, 0.8), (1.0, 0.82), (2.0, 0.83)]
        knee = knee_location(curve, tau=0.1)
        assert 0.3 <= knee <= 1.0

    def test_no_knee_always_steep(self) -> None:
        curve = [(0.1, 0.0), (0.2, 0.5), (0.3, 1.0)]
        knee = knee_location(curve, tau=0.01)
        # No knee found → returns last resource value
        assert knee == 0.3


class TestFlatteningRate:
    def test_positive_for_saturating_curve(self) -> None:
        curve = [(0.1, 0.0), (0.2, 0.5), (0.5, 0.8), (1.0, 0.85), (2.0, 0.86)]
        rate = flattening_rate(curve)
        assert rate > 0  # positive means it is flattening


class TestWorkloadHypervolume:
    def test_mean_and_ci(self) -> None:
        per_query_hvs = [0.3, 0.4, 0.5, 0.6, 0.35, 0.45, 0.55, 0.5, 0.4, 0.42]
        mean, ci_low, ci_high = workload_hypervolume(per_query_hvs)
        assert ci_low <= mean <= ci_high
        assert 0.3 < mean < 0.6


class TestCategoryHypervolume:
    def test_per_category(self) -> None:
        per_query_hvs = [0.3, 0.5, 0.7, 0.4, 0.6, 0.8]
        categories = ["focused", "focused", "module", "module", "cross-cutting", "cross-cutting"]
        result = category_hypervolume(per_query_hvs, categories)
        assert "focused" in result
        assert "module" in result
        assert "cross-cutting" in result
        assert result["focused"] == pytest.approx(0.4)
        assert result["module"] == pytest.approx(0.55)
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_analysis.py::TestBudgetSlice -v 2>&1 | tail -10`
Expected: ImportError

- [ ] **Step 3: Implement slices and diagnostics**

Append to `bench/analysis.py`:

```python
def budget_slice(
    points: np.ndarray,
    latency_band: tuple[float, float],
) -> list[tuple[float, float]]:
    """Extract quality-vs-cost curve at a fixed latency band.

    Returns [(cost, quality)] sorted by cost, upper envelope.
    """
    if len(points) == 0:
        return []

    lo, hi = latency_band
    mask = (points[:, 1] >= lo) & (points[:, 1] <= hi)
    filtered = points[mask]
    if len(filtered) == 0:
        return []

    # Sort by cost, take upper envelope of quality
    order = np.argsort(filtered[:, 0])
    filtered = filtered[order]
    curve = [(float(row[0]), float(row[2])) for row in filtered]

    # Upper envelope: keep only points where quality is non-decreasing
    envelope = [curve[0]]
    for cost, quality in curve[1:]:
        if quality >= envelope[-1][1]:
            envelope.append((cost, quality))
        else:
            envelope.append((cost, envelope[-1][1]))

    return envelope


def latency_slice(
    points: np.ndarray,
    cost_band: tuple[float, float],
) -> list[tuple[float, float]]:
    """Extract quality-vs-latency curve at a fixed cost band.

    Returns [(latency, quality)] sorted by latency.
    """
    if len(points) == 0:
        return []

    lo, hi = cost_band
    mask = (points[:, 0] >= lo) & (points[:, 0] <= hi)
    filtered = points[mask]
    if len(filtered) == 0:
        return []

    order = np.argsort(filtered[:, 1])
    filtered = filtered[order]
    curve = [(float(row[1]), float(row[2])) for row in filtered]

    envelope = [curve[0]]
    for resource, quality in curve[1:]:
        if quality >= envelope[-1][1]:
            envelope.append((resource, quality))
        else:
            envelope.append((resource, envelope[-1][1]))

    return envelope


def initial_ascent(curve: list[tuple[float, float]]) -> float:
    """Slope near minimum resource value. curve is [(resource, quality)]."""
    if len(curve) < 2:
        return 0.0
    r0, q0 = curve[0]
    r1, q1 = curve[1]
    dr = r1 - r0
    if dr == 0:
        return 0.0
    return (q1 - q0) / dr


def knee_location(curve: list[tuple[float, float]], tau: float) -> float:
    """Smallest resource where marginal gain drops below tau."""
    if len(curve) < 2:
        return curve[0][0] if curve else 0.0

    for i in range(1, len(curve)):
        r_prev, q_prev = curve[i - 1]
        r_curr, q_curr = curve[i]
        dr = r_curr - r_prev
        if dr == 0:
            continue
        slope = (q_curr - q_prev) / dr
        if slope < tau:
            return r_prev

    return curve[-1][0]


def flattening_rate(curve: list[tuple[float, float]]) -> float:
    """Post-knee decay of marginal gain via second differences."""
    if len(curve) < 3:
        return 0.0

    slopes = []
    for i in range(1, len(curve)):
        dr = curve[i][0] - curve[i - 1][0]
        if dr == 0:
            continue
        slopes.append((curve[i][1] - curve[i - 1][1]) / dr)

    if len(slopes) < 2:
        return 0.0

    second_diffs = [slopes[i] - slopes[i - 1] for i in range(1, len(slopes))]
    return -float(np.mean(second_diffs))


def workload_hypervolume(
    per_query_hvs: list[float],
    n_bootstrap: int = 1000,
) -> tuple[float, float, float]:
    """Mean hypervolume with bootstrap 95% confidence interval."""
    arr = np.array(per_query_hvs)
    mean = float(np.mean(arr))

    rng = np.random.default_rng(42)
    boot_means = []
    for _ in range(n_bootstrap):
        sample = rng.choice(arr, size=len(arr), replace=True)
        boot_means.append(float(np.mean(sample)))

    ci_low = float(np.percentile(boot_means, 2.5))
    ci_high = float(np.percentile(boot_means, 97.5))
    return mean, ci_low, ci_high


def category_hypervolume(
    per_query_hvs: list[float],
    categories: list[str],
) -> dict[str, float]:
    """Mean hypervolume per query category."""
    result: dict[str, list[float]] = {}
    for hv, cat in zip(per_query_hvs, categories):
        result.setdefault(cat, []).append(hv)
    return {cat: float(np.mean(vals)) for cat, vals in result.items()}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_analysis.py -v 2>&1 | tail -20`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add bench/analysis.py tests/test_analysis.py
git commit -m "feat: add frontier slices, diagnostics, and workload hypervolume"
```

---

### Task 5: Quality phase

**Files:**
- Create: `bench/quality.py`
- Create: `tests/test_quality_bench.py`

- [ ] **Step 1: Write failing tests**

Create `tests/test_quality_bench.py`:

```python
"""Tests for bench.quality module."""

from pathlib import Path

from bench.quality import run_quality_phase
from semtree.records import write_record


def _make_valid_tree(tmp_path: Path) -> None:
    """Create a minimal valid .sem/ tree."""
    sem = tmp_path / ".sem"
    sem.mkdir()
    write_record(sem / "foo.py.md", "foo.py", "file", "hash_foo", "Does foo things.")
    write_record(sem / "bar.py.md", "bar.py", "file", "hash_bar", "Does bar things.")
    write_record(
        sem / "__dir__.md", ".", "directory", "hash_dir",
        "Root.\n\n## Children\n\n- **foo.py**: Does foo things.\n- **bar.py**: Does bar things.",
    )
    # Create source files
    (tmp_path / "foo.py").write_text("# foo")
    (tmp_path / "bar.py").write_text("# bar")


class TestQualityPhase:
    def test_valid_tree_passes(self, tmp_path: Path) -> None:
        _make_valid_tree(tmp_path)
        records = run_quality_phase(tmp_path)
        metrics = {r.metric: r.value for r in records}
        assert metrics["children_coverage"] == 1.0
        assert metrics["frontmatter_errors"] == 0
        assert metrics["orphan_records"] == 0

    def test_missing_child_in_routing_table(self, tmp_path: Path) -> None:
        _make_valid_tree(tmp_path)
        # Add a file but don't mention it in __dir__.md children
        sem = tmp_path / ".sem"
        write_record(sem / "baz.py.md", "baz.py", "file", "hash_baz", "Does baz.")
        (tmp_path / "baz.py").write_text("# baz")

        records = run_quality_phase(tmp_path)
        metrics = {r.metric: r.value for r in records}
        assert metrics["children_coverage"] < 1.0

    def test_orphan_record_detected(self, tmp_path: Path) -> None:
        _make_valid_tree(tmp_path)
        # Create orphan: .sem record but no source file
        sem = tmp_path / ".sem"
        write_record(sem / "ghost.py.md", "ghost.py", "file", "hash_ghost", "Gone.")

        records = run_quality_phase(tmp_path)
        metrics = {r.metric: r.value for r in records}
        assert metrics["orphan_records"] >= 1
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_quality_bench.py -v 2>&1 | tail -10`
Expected: ImportError

- [ ] **Step 3: Implement quality phase**

Create `bench/quality.py`:

```python
"""Benchmark quality phase: structural correctness checks on .sem/ records."""

import re
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from semtree.records import SEM_DIR, read_record


def run_quality_phase(repo_path: Path, repo_name: str = "local") -> list[MetricRecord]:
    """Run structural quality checks on all .sem/ records."""
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    records: list[MetricRecord] = []

    all_sem_records = list(repo_path.rglob(f"{SEM_DIR}/*.md"))
    frontmatter_errors = 0
    orphan_count = 0
    coverage_scores = []

    for md_path in all_sem_records:
        data = read_record(md_path)
        if data is None:
            frontmatter_errors += 1
            continue

        # Frontmatter validity
        for field in ("path", "type", "content_hash"):
            if field not in data:
                frontmatter_errors += 1
                break
        if data.get("type") not in ("file", "directory"):
            frontmatter_errors += 1

        # Orphan check: does the source exist?
        rel_path = data.get("path", "")
        if data.get("type") == "file":
            source = repo_path / rel_path
            if not source.exists():
                orphan_count += 1
        elif data.get("type") == "directory":
            dir_path = repo_path / rel_path if rel_path and rel_path != "." else repo_path
            if not dir_path.is_dir():
                orphan_count += 1

        # Children coverage (for directory records)
        if data.get("type") == "directory":
            summary = data.get("summary", "")
            mentioned = set(re.findall(r"\*\*([^*]+)\*\*", summary))
            dir_path = repo_path / rel_path if rel_path and rel_path != "." else repo_path
            sem_dir = dir_path / SEM_DIR
            if sem_dir.is_dir():
                child_records = [
                    p.stem.replace(".md", "") if p.name != "__dir__.md" else None
                    for p in sem_dir.glob("*.md")
                ]
                child_names = {c for c in child_records if c is not None}
                if child_names:
                    found = sum(1 for c in child_names if c in mentioned)
                    coverage_scores.append(found / len(child_names))

    avg_coverage = sum(coverage_scores) / len(coverage_scores) if coverage_scores else 1.0

    records.append(MetricRecord(now, "quality", repo_name, "srt", "", "", "children_coverage", avg_coverage))
    records.append(MetricRecord(now, "quality", repo_name, "srt", "", "", "frontmatter_errors", frontmatter_errors))
    records.append(MetricRecord(now, "quality", repo_name, "srt", "", "", "orphan_records", orphan_count))

    return records
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_quality_bench.py -v 2>&1 | tail -15`
Expected: All 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add bench/quality.py tests/test_quality_bench.py
git commit -m "feat: add benchmark quality phase with structural checks"
```

---

### Task 6: Build phase

**Files:**
- Create: `bench/build_phase.py`

- [ ] **Step 1: Implement build phase**

Create `bench/build_phase.py`:

```python
"""Benchmark build phase: measure full and incremental build cost."""

import shutil
import time
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from semtree.config import BuildConfig
from semtree.records import SEM_DIR


def run_build_phase(repo_path: Path, repo_name: str = "local") -> list[MetricRecord]:
    """Run full build, then incremental no-op build. Returns metric records."""
    from semtree.builder import build

    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    records: list[MetricRecord] = []

    # Clean existing .sem/ dirs for fresh build
    for sem_dir in list(repo_path.rglob(SEM_DIR)):
        if sem_dir.is_dir():
            shutil.rmtree(sem_dir)

    # Full build
    config = BuildConfig(target_path=repo_path, force=True, embed=False)
    t0 = time.monotonic()
    build(config)
    build_time = time.monotonic() - t0

    # Count nodes by counting .sem/*.md files
    node_count = len(list(repo_path.rglob(f"{SEM_DIR}/*.md")))

    records.append(MetricRecord(now, "build", repo_name, "srt", "", "", "build_time_s", round(build_time, 2)))
    records.append(MetricRecord(now, "build", repo_name, "srt", "", "", "node_count", node_count))

    # Incremental no-op build
    config_incr = BuildConfig(target_path=repo_path, force=False, embed=False)
    t0 = time.monotonic()
    build(config_incr)
    incr_time = time.monotonic() - t0

    records.append(MetricRecord(now, "build", repo_name, "srt", "", "", "incr_build_time_s", round(incr_time, 2)))

    return records
```

- [ ] **Step 2: Commit**

```bash
git add bench/build_phase.py
git commit -m "feat: add benchmark build phase"
```

---

### Task 7: Incremental phase

**Files:**
- Create: `bench/incremental.py`

- [ ] **Step 1: Implement incremental phase**

Create `bench/incremental.py`:

```python
"""Benchmark incremental phase: modify files, rebuild, verify correctness."""

import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from semtree.config import BuildConfig
from semtree.hasher import hash_file
from semtree.records import SEM_DIR, read_record


# Files to modify for incremental test (repo-specific)
FELLOWSHIP_MODIFY_FILES = [
    "cli/internal/state/state.go",
    "plugin/skills/quest.md",
]

MARKER = "\n// benchmark-incremental-marker\n"


def run_incremental_phase(repo_path: Path, repo_name: str = "local") -> list[MetricRecord]:
    """Modify files, rebuild incrementally, verify only changed subtree updated."""
    from semtree.builder import build

    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    records: list[MetricRecord] = []

    # Snapshot hashes before modification
    pre_hashes = {}
    for md_path in repo_path.rglob(f"{SEM_DIR}/*.md"):
        data = read_record(md_path)
        if data:
            pre_hashes[data["path"]] = data["content_hash"]

    # Modify files
    modify_files = FELLOWSHIP_MODIFY_FILES if repo_name == "fellowship" else []
    for rel_path in modify_files:
        fpath = repo_path / rel_path
        if fpath.exists():
            fpath.write_text(fpath.read_text() + MARKER)

    # Incremental rebuild
    config = BuildConfig(target_path=repo_path, force=False, embed=False)
    t0 = time.monotonic()
    build(config)
    rebuild_time = time.monotonic() - t0

    # Count re-summarized nodes
    post_hashes = {}
    for md_path in repo_path.rglob(f"{SEM_DIR}/*.md"):
        data = read_record(md_path)
        if data:
            post_hashes[data["path"]] = data["content_hash"]

    changed = sum(1 for p in post_hashes if pre_hashes.get(p) != post_hashes[p])

    records.append(MetricRecord(now, "incremental", repo_name, "srt", "", "", "incr_rebuild_time_s", round(rebuild_time, 2)))
    records.append(MetricRecord(now, "incremental", repo_name, "srt", "", "", "nodes_resummarized", changed))

    # Revert modifications
    subprocess.run(["git", "checkout", "."], cwd=repo_path, capture_output=True)

    return records
```

- [ ] **Step 2: Commit**

```bash
git add bench/incremental.py
git commit -m "feat: add benchmark incremental phase"
```

---

### Task 8: Query set for fellowship

**Files:**
- Create: `bench/queries/fellowship.yaml`

- [ ] **Step 1: Create labeled query set**

Create `bench/queries/fellowship.yaml`:

```yaml
queries:
  # Focused queries (target specific file/function)
  - id: q01
    question: "How does the quest state machine track phase transitions?"
    category: focused
    relevant:
      - path: cli/internal/state/state.go
        relevance: 3
      - path: cli/internal/hooks/submit.go
        relevance: 2

  - id: q02
    question: "How does the gate guard decide whether to block a tool call?"
    category: focused
    relevant:
      - path: cli/internal/hooks/guard.go
        relevance: 3
      - path: cli/internal/state/state.go
        relevance: 2

  - id: q03
    question: "How does the lembas prerequisite detection work?"
    category: focused
    relevant:
      - path: cli/internal/hooks/prereq.go
        relevance: 3

  - id: q04
    question: "What is the SQLite schema for quest state?"
    category: focused
    relevant:
      - path: cli/internal/db/migrate.go
        relevance: 3
      - path: cli/internal/state/state.go
        relevance: 2

  - id: q05
    question: "How does the file tracking hook record which files were touched?"
    category: focused
    relevant:
      - path: cli/internal/hooks/files.go
        relevance: 3
      - path: cli/internal/tome/tome.go
        relevance: 2

  # Module-level queries (subsystem understanding)
  - id: q06
    question: "How does the hook system enforce quest workflow discipline?"
    category: module
    relevant:
      - path: cli/internal/hooks/guard.go
        relevance: 3
      - path: cli/internal/hooks/submit.go
        relevance: 3
      - path: cli/internal/hooks/prereq.go
        relevance: 2
      - path: cli/internal/hooks/files.go
        relevance: 2
      - path: cli/internal/hooks/completion.go
        relevance: 2

  - id: q07
    question: "How does the dashboard backend serve real-time quest updates?"
    category: module
    relevant:
      - path: cli/internal/dashboard/server.go
        relevance: 3
      - path: cli/internal/dashboard/hub.go
        relevance: 3

  - id: q08
    question: "How does the health monitoring system classify quest states?"
    category: module
    relevant:
      - path: cli/internal/eagles/eagles.go
        relevance: 3
      - path: cli/internal/herald/herald.go
        relevance: 2

  - id: q09
    question: "What are all the agent types and their behavioral protocols?"
    category: module
    relevant:
      - path: plugin/agents/quest-runner.md
        relevance: 3
      - path: plugin/agents/balrog.md
        relevance: 3
      - path: plugin/agents/palantir.md
        relevance: 3
      - path: plugin/agents/scout.md
        relevance: 3

  - id: q10
    question: "How does the errand system track quest work items?"
    category: module
    relevant:
      - path: cli/internal/errand/errand.go
        relevance: 3

  # Cross-cutting queries (multi-module evidence)
  - id: q11
    question: "What happens end-to-end when a quest submits a gate?"
    category: cross-cutting
    relevant:
      - path: cli/internal/hooks/submit.go
        relevance: 3
      - path: cli/internal/state/state.go
        relevance: 3
      - path: cli/internal/tome/tome.go
        relevance: 2
      - path: cli/internal/herald/herald.go
        relevance: 2
      - path: cli/internal/hooks/enrich.go
        relevance: 2

  - id: q12
    question: "How do the CLI, dashboard, and plugin coordinate gate approval?"
    category: cross-cutting
    relevant:
      - path: cli/cmd/fellowship/main.go
        relevance: 3
      - path: cli/internal/dashboard/server.go
        relevance: 3
      - path: plugin/hooks/hooks.json
        relevance: 2
      - path: cli/internal/state/state.go
        relevance: 2

  - id: q13
    question: "How does quest failure get recorded and surfaced to the user?"
    category: cross-cutting
    relevant:
      - path: cli/internal/autopsy/autopsy.go
        relevance: 3
      - path: cli/internal/herald/herald.go
        relevance: 2
      - path: cli/internal/eagles/eagles.go
        relevance: 2

  - id: q14
    question: "How does the bulletin board enable cross-quest knowledge sharing?"
    category: cross-cutting
    relevant:
      - path: cli/internal/bulletin/bulletin.go
        relevance: 3

  - id: q15
    question: "How does fellowship install and configure itself as a Claude Code plugin?"
    category: cross-cutting
    relevant:
      - path: cli/internal/install/install.go
        relevance: 3
      - path: plugin/hooks/hooks.json
        relevance: 2
      - path: plugin/CLAUDE.md
        relevance: 2
```

- [ ] **Step 2: Commit**

```bash
mkdir -p bench/queries
git add bench/queries/fellowship.yaml
git commit -m "feat: add fellowship query set with graded relevance labels"
```

---

### Task 9: Routing phase with control grid

**Files:**
- Create: `bench/routing.py`
- Create: `tests/test_routing_bench.py`

- [ ] **Step 1: Write failing tests**

Create `tests/test_routing_bench.py`:

```python
"""Tests for bench.routing module."""

from pathlib import Path
from unittest.mock import patch

import pytest
import yaml

from bench.routing import (
    load_queries,
    simulate_descent,
    ndcg_at_k,
    Query,
)
from semtree.records import write_record


def _make_sem_tree(tmp_path: Path) -> None:
    """Create a multi-level .sem/ tree for routing tests."""
    root_sem = tmp_path / ".sem"
    root_sem.mkdir()
    write_record(
        root_sem / "__dir__.md", ".", "directory", "h_root",
        "Root.\n\n## Children\n\n- **src**: Source code.\n- **docs**: Documentation.",
    )
    write_record(root_sem / "src.md", "src", "directory", "h_src", "Source code.")
    write_record(root_sem / "docs.md", "docs", "directory", "h_docs", "Documentation.")

    src_sem = tmp_path / "src" / ".sem"
    src_sem.mkdir(parents=True)
    write_record(
        src_sem / "__dir__.md", "src", "directory", "h_src2",
        "Source.\n\n## Children\n\n- **auth.py**: Authentication.\n- **db.py**: Database layer.",
    )
    write_record(src_sem / "auth.py.md", "src/auth.py", "file", "h_auth", "Authentication module.")
    write_record(src_sem / "db.py.md", "src/db.py", "file", "h_db", "Database layer.")

    # Create actual dirs for traversal
    (tmp_path / "src").mkdir(exist_ok=True)
    (tmp_path / "docs").mkdir(exist_ok=True)


class TestLoadQueries:
    def test_loads_yaml(self, tmp_path: Path) -> None:
        qfile = tmp_path / "queries.yaml"
        qfile.write_text(yaml.dump({"queries": [
            {"id": "q01", "question": "How does auth work?", "category": "focused",
             "relevant": [{"path": "src/auth.py", "relevance": 3}]},
        ]}))
        queries = load_queries(qfile)
        assert len(queries) == 1
        assert queries[0].id == "q01"
        assert queries[0].relevant[0]["path"] == "src/auth.py"


class TestNDCGAtK:
    def test_perfect_ranking(self) -> None:
        retrieved = ["a", "b", "c"]
        relevant = {"a": 3, "b": 2, "c": 1}
        score = ndcg_at_k(retrieved, relevant, k=3)
        assert score == pytest.approx(1.0)

    def test_no_relevant_retrieved(self) -> None:
        retrieved = ["x", "y", "z"]
        relevant = {"a": 3}
        score = ndcg_at_k(retrieved, relevant, k=3)
        assert score == 0.0

    def test_partial_match(self) -> None:
        retrieved = ["x", "a", "y"]  # a is at rank 2
        relevant = {"a": 3}
        score = ndcg_at_k(retrieved, relevant, k=3)
        assert 0.0 < score < 1.0


class TestSimulateDescent:
    def test_reaches_files_with_mock_llm(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)

        def mock_select(question, children, beam_width):
            # Always select first child
            return [children[0][0]] if children else []

        result = simulate_descent(
            repo_path=tmp_path,
            question="How does auth work?",
            select_fn=mock_select,
            beam_width=1,
            max_depth=3,
            token_budget=50000,
        )

        assert len(result.files_reached) > 0
        assert result.llm_calls >= 1
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_routing_bench.py -v 2>&1 | tail -10`
Expected: ImportError

- [ ] **Step 3: Implement routing module**

Create `bench/routing.py`:

```python
"""Benchmark routing phase: simulated SRT descent with control grid."""

import json
import math
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

import yaml

from bench.harness import MetricRecord
from semtree.records import SEM_DIR, DIR_RECORD, read_record


@dataclass
class Query:
    id: str
    question: str
    category: str
    relevant: list[dict]  # [{"path": str, "relevance": int}]


@dataclass
class DescentResult:
    files_reached: list[str]
    llm_calls: int
    tokens_loaded: int
    elapsed_s: float


def load_queries(query_file: Path) -> list[Query]:
    """Load query set from YAML file."""
    data = yaml.safe_load(query_file.read_text(encoding="utf-8"))
    return [
        Query(
            id=q["id"],
            question=q["question"],
            category=q["category"],
            relevant=q.get("relevant", []),
        )
        for q in data["queries"]
    ]


def ndcg_at_k(retrieved: list[str], relevant: dict[str, int], k: int = 10) -> float:
    """Compute NDCG@k with graded relevance."""
    if not relevant:
        return 0.0

    # DCG of retrieved
    dcg = 0.0
    for i, path in enumerate(retrieved[:k]):
        rel = relevant.get(path, 0)
        dcg += (2 ** rel - 1) / math.log2(i + 2)

    # Ideal DCG: sort relevant by relevance desc
    ideal_rels = sorted(relevant.values(), reverse=True)[:k]
    idcg = 0.0
    for i, rel in enumerate(ideal_rels):
        idcg += (2 ** rel - 1) / math.log2(i + 2)

    if idcg == 0:
        return 0.0
    return dcg / idcg


SelectFn = Callable[[str, list[tuple[str, str]], int], list[str]]


def simulate_descent(
    repo_path: Path,
    question: str,
    select_fn: SelectFn,
    beam_width: int = 3,
    max_depth: int = 10,
    token_budget: int = 50000,
) -> DescentResult:
    """Simulate SRT routing protocol descent.

    select_fn(question, [(child_path, child_summary)], beam_width) -> [selected_paths]
    """
    files_reached = []
    llm_calls = 0
    tokens_loaded = 0
    t0 = time.monotonic()

    # Start at root
    queue = [("", 0)]  # (dir_relative_path, depth)

    while queue and tokens_loaded < token_budget:
        rel_path, depth = queue.pop(0)
        if depth > max_depth:
            continue

        # Read directory record
        if rel_path == "":
            dir_record_path = repo_path / SEM_DIR / DIR_RECORD
        else:
            dir_record_path = repo_path / rel_path / SEM_DIR / DIR_RECORD

        data = read_record(dir_record_path)
        if data is None:
            continue

        summary = data.get("summary", "")
        tokens_loaded += len(summary) // 4  # rough token estimate

        # Extract children from summary
        children = _extract_children(repo_path, rel_path)
        if not children:
            continue

        # LLM selects children
        selected = select_fn(question, children, beam_width)
        llm_calls += 1

        for child_path in selected:
            child_full = repo_path / child_path
            if child_full.is_dir():
                queue.append((child_path, depth + 1))
            else:
                files_reached.append(child_path)
                # Load file summary tokens
                sem_dir = child_full.parent / SEM_DIR
                file_record = sem_dir / f"{child_full.name}.md"
                file_data = read_record(file_record)
                if file_data:
                    tokens_loaded += len(file_data.get("summary", "")) // 4

    elapsed = time.monotonic() - t0
    return DescentResult(
        files_reached=files_reached,
        llm_calls=llm_calls,
        tokens_loaded=tokens_loaded,
        elapsed_s=elapsed,
    )


def _extract_children(repo_path: Path, dir_rel_path: str) -> list[tuple[str, str]]:
    """Extract (child_path, child_summary) pairs from .sem/ records."""
    if dir_rel_path == "":
        sem_dir = repo_path / SEM_DIR
    else:
        sem_dir = repo_path / dir_rel_path / SEM_DIR

    if not sem_dir.is_dir():
        return []

    children = []
    for md_path in sorted(sem_dir.glob("*.md")):
        if md_path.name == DIR_RECORD:
            continue
        data = read_record(md_path)
        if data:
            children.append((data["path"], data.get("summary", "")))
    return children


# Control grid for SRT
SRT_CONTROL_GRID = [
    {"beam_width": bw, "max_depth": md, "token_budget": tb}
    for bw in [1, 2, 3, 5]
    for md in [1, 2, 3, 100]  # 100 = unlimited
    for tb in [1000, 2000, 5000, 10000, 20000, 50000]
]

# Per-call cost estimate (Claude Haiku for routing)
COST_PER_LLM_CALL = 0.001  # $0.001 per call estimate


def run_routing_phase(
    repo_path: Path,
    query_file: Path,
    select_fn: SelectFn,
    repo_name: str = "local",
) -> list[MetricRecord]:
    """Run routing phase: sweep control grid, collect metrics per query per setting."""
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    queries = load_queries(query_file)
    records: list[MetricRecord] = []

    for query in queries:
        relevant_map = {r["path"]: r["relevance"] for r in query.relevant}

        for control in SRT_CONTROL_GRID:
            control_json = json.dumps(control, sort_keys=True)

            result = simulate_descent(
                repo_path=repo_path,
                question=query.question,
                select_fn=select_fn,
                beam_width=control["beam_width"],
                max_depth=control["max_depth"],
                token_budget=control["token_budget"],
            )

            ndcg = ndcg_at_k(result.files_reached, relevant_map, k=10)
            cost = result.llm_calls * COST_PER_LLM_CALL

            for metric, value in [
                ("ndcg@10", ndcg),
                ("cost_usd", cost),
                ("latency_s", result.elapsed_s),
                ("tokens_loaded", result.tokens_loaded),
                ("llm_calls", result.llm_calls),
            ]:
                records.append(MetricRecord(
                    now, "routing", repo_name, "srt",
                    query.id, control_json, metric, value,
                ))

    return records
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_routing_bench.py -v 2>&1 | tail -15`
Expected: All 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add bench/routing.py tests/test_routing_bench.py
git commit -m "feat: add routing phase with control grid sweep and NDCG"
```

---

### Task 10: Grep/glob baseline

**Files:**
- Create: `bench/baseline.py`

- [ ] **Step 1: Implement baseline**

Create `bench/baseline.py`:

```python
"""Grep/glob baseline: simulates agent search without SRT summaries."""

import json
import re
import subprocess
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from bench.routing import Query, load_queries, ndcg_at_k


@dataclass
class BaselineResult:
    files_found: list[str]
    tokens_loaded: int
    elapsed_s: float


def _extract_keywords(question: str) -> list[str]:
    """Extract search keywords from a question (simple heuristic, no LLM)."""
    stop_words = {"how", "does", "the", "a", "an", "is", "what", "where", "which", "when", "do", "are", "in", "of", "to", "for", "and", "or", "with"}
    words = re.findall(r"[a-zA-Z_]+", question.lower())
    return [w for w in words if w not in stop_words and len(w) > 2]


def grep_search(
    repo_path: Path,
    question: str,
    max_files: int = 5,
    strategy: str = "grep_only",
) -> BaselineResult:
    """Search repo using grep/glob, return found files."""
    t0 = time.monotonic()
    keywords = _extract_keywords(question)
    found_files: dict[str, int] = {}  # path -> match count

    for keyword in keywords:
        result = subprocess.run(
            ["grep", "-rl", "--include=*.go", "--include=*.py", "--include=*.md",
             "--include=*.ts", "--include=*.js", keyword, "."],
            cwd=repo_path, capture_output=True, text=True,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().split("\n"):
                path = line.lstrip("./")
                if path and not path.startswith(".sem/"):
                    found_files[path] = found_files.get(path, 0) + 1

    # Rank by match count, take top max_files
    ranked = sorted(found_files.items(), key=lambda x: x[1], reverse=True)
    top_files = [path for path, _ in ranked[:max_files]]

    # Estimate tokens loaded (read file sizes)
    tokens = 0
    for f in top_files:
        fpath = repo_path / f
        if fpath.exists():
            tokens += fpath.stat().st_size // 4

    elapsed = time.monotonic() - t0
    return BaselineResult(files_found=top_files, tokens_loaded=tokens, elapsed_s=elapsed)


# Control grid for baseline
BASELINE_CONTROL_GRID = [
    {"max_files": mf, "strategy": strat, "token_budget": tb}
    for mf in [3, 5, 10, 20]
    for strat in ["grep_only", "glob_then_grep"]
    for tb in [1000, 2000, 5000, 10000, 20000, 50000]
]


def run_baseline_phase(
    repo_path: Path,
    query_file: Path,
    repo_name: str = "local",
) -> list[MetricRecord]:
    """Run baseline phase: sweep control grid, collect metrics."""
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    queries = load_queries(query_file)
    records: list[MetricRecord] = []

    for query in queries:
        relevant_map = {r["path"]: r["relevance"] for r in query.relevant}

        for control in BASELINE_CONTROL_GRID:
            control_json = json.dumps(control, sort_keys=True)

            result = grep_search(
                repo_path=repo_path,
                question=query.question,
                max_files=control["max_files"],
                strategy=control["strategy"],
            )

            # Truncate by token budget
            files_within_budget = []
            token_sum = 0
            for f in result.files_found:
                fsize = (repo_path / f).stat().st_size // 4 if (repo_path / f).exists() else 0
                if token_sum + fsize <= control["token_budget"]:
                    files_within_budget.append(f)
                    token_sum += fsize

            ndcg = ndcg_at_k(files_within_budget, relevant_map, k=10)

            for metric, value in [
                ("ndcg@10", ndcg),
                ("cost_usd", 0.0),  # grep is free
                ("latency_s", result.elapsed_s),
                ("tokens_loaded", token_sum),
                ("llm_calls", 0),
            ]:
                records.append(MetricRecord(
                    now, "routing", repo_name, "baseline",
                    query.id, control_json, metric, value,
                ))

    return records
```

- [ ] **Step 2: Commit**

```bash
git add bench/baseline.py
git commit -m "feat: add grep/glob baseline for benchmark comparison"
```

---

### Task 11: CLI bench subcommand

**Files:**
- Modify: `src/semtree/cli.py`

- [ ] **Step 1: Add bench subcommand**

Read `src/semtree/cli.py` first. Then add the bench subparser after the query_parser block (before `args = parser.parse_args()`):

```python
    bench_parser = sub.add_parser("bench", help="Run benchmark evaluation phases")
    bench_parser.add_argument(
        "phase",
        nargs="?",
        default="all",
        choices=["build", "quality", "routing", "incremental", "analysis", "all"],
        help="Phase to run (default: all)",
    )
    bench_parser.add_argument(
        "--repo",
        default="fellowship",
        help="Benchmark repo name (default: fellowship)",
    )
    bench_parser.add_argument(
        "--clean",
        action="store_true",
        help="Remove cached benchmark repos",
    )
    bench_parser.add_argument(
        "--results",
        default="results.tsv",
        help="Path to results TSV file (default: results.tsv)",
    )
```

Add the handler after the query handler:

```python
    elif args.command == "bench":
        if args.clean:
            from bench.repos import clean_cache
            clean_cache()
            print("Cleaned benchmark repo cache.", file=sys.stderr)
            return

        from bench.repos import get_repo
        from bench.harness import append_results

        try:
            repo_path = get_repo(args.repo)
        except ValueError as e:
            print(f"error: {e}", file=sys.stderr)
            sys.exit(1)

        results_path = Path(args.results)
        phases = ["build", "quality", "routing", "incremental"] if args.phase == "all" else [args.phase]

        for phase in phases:
            print(f"\nRunning {phase} phase...", file=sys.stderr)

            if phase == "build":
                from bench.build_phase import run_build_phase
                records = run_build_phase(repo_path, repo_name=args.repo)
            elif phase == "quality":
                from bench.quality import run_quality_phase
                records = run_quality_phase(repo_path, repo_name=args.repo)
            elif phase == "routing":
                query_file = Path(__file__).parent / "../../bench/queries" / f"{args.repo}.yaml"
                if not query_file.exists():
                    print(f"error: no query set for repo '{args.repo}'", file=sys.stderr)
                    sys.exit(1)
                # For now, use a placeholder select_fn — real LLM integration is separate
                print("  (routing phase requires LLM select_fn — skipping for now)", file=sys.stderr)
                records = []
            elif phase == "incremental":
                from bench.incremental import run_incremental_phase
                records = run_incremental_phase(repo_path, repo_name=args.repo)
            elif phase == "analysis":
                from bench.analysis import pareto_prune, normalize_to_utility, hypervolume
                from bench.harness import read_results
                print("  Running analysis on existing results...", file=sys.stderr)
                # Analysis reads from results.tsv — implementation deferred to when data exists
                records = []
            else:
                records = []

            if records:
                append_results(results_path, records)
                print(f"  Wrote {len(records)} metrics to {results_path}", file=sys.stderr)

        print("\nBenchmark complete.", file=sys.stderr)
```

- [ ] **Step 2: Run all tests**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/ -v 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/semtree/cli.py
git commit -m "feat: add semtree bench CLI subcommand"
```

---

### Task 12: Update openspec benchmark design

**Files:**
- Modify: `openspec/changes/srt-benchmark/design.md`

- [ ] **Step 1: Update design doc**

Read `openspec/changes/srt-benchmark/design.md`, then add a new section at the end titled "## v9 Evaluation Framework Update" documenting:
- The 5th analysis phase
- Control grids for SRT and baseline
- Updated results format with system, query_id, control_json columns
- Query set format with graded relevance and categories
- Analysis functions (Pareto, hypervolume, frontier diagnostics)
- Grep/glob baseline

Update the Non-Goals section to remove "Full paper evaluation framework" since we're now implementing it.

- [ ] **Step 2: Commit**

```bash
git add openspec/changes/srt-benchmark/design.md
git commit -m "docs: update benchmark design with v9 evaluation framework"
```
