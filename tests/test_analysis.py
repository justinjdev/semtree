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
            [1.0, 0.5, 0.9],  # best on all — dominates everything
            [2.0, 2.0, 0.8],  # dominated by first
            [3.0, 1.0, 0.7],  # dominated by first
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
