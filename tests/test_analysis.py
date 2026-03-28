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
