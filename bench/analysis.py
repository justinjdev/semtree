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
