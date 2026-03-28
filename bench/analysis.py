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
