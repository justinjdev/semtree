"""Tests for bench.routing module."""

from pathlib import Path
from unittest.mock import patch

import pytest
import yaml

from bench.routing import (
    load_queries,
    simulate_descent,
    ndcg_at_k,
    precision,
    recall,
    mrr,
    compute_rho_l,
    log_dilution_penalty,
    ratio_dilution_penalty,
    LevelTelemetry,
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
            # Select all children up to beam_width, return (path, score) pairs
            return [(c[0], 0.5) for c in children[:beam_width]] if children else []

        result = simulate_descent(
            repo_path=tmp_path,
            question="How does auth work?",
            select_fn=mock_select,
            beam_width=3,
            max_depth=3,
            token_budget=50000,
        )

        assert len(result.files_reached) > 0
        assert result.llm_calls >= 1
        assert len(result.level_telemetry) >= 1
        assert result.level_telemetry[0].n_candidates > 0


class TestRhoL:
    def test_all_relevant(self) -> None:
        selected = ["src/auth.py", "src/db.py"]
        relevant = {"src/auth.py", "src/db.py"}
        assert compute_rho_l(selected, relevant) == 0.0

    def test_none_relevant(self) -> None:
        selected = ["src/auth.py", "src/db.py"]
        relevant = {"docs/readme.md"}
        assert compute_rho_l(selected, relevant) == 1.0

    def test_mixed(self) -> None:
        selected = ["src", "docs"]
        relevant = {"src/auth.py"}
        # "src" is an ancestor of relevant, "docs" is not
        assert compute_rho_l(selected, relevant) == pytest.approx(0.5)

    def test_empty_selected(self) -> None:
        assert compute_rho_l([], {"src/auth.py"}) == 0.0


class TestPrecisionRecallMRR:
    def test_precision_all_relevant(self) -> None:
        assert precision(["a", "b"], {"a", "b", "c"}) == pytest.approx(1.0)

    def test_precision_none_relevant(self) -> None:
        assert precision(["x", "y"], {"a", "b"}) == pytest.approx(0.0)

    def test_precision_empty_retrieved(self) -> None:
        assert precision([], {"a"}) == pytest.approx(0.0)

    def test_recall_all_retrieved(self) -> None:
        assert recall(["a", "b"], {"a", "b"}) == pytest.approx(1.0)

    def test_recall_partial(self) -> None:
        assert recall(["a", "x"], {"a", "b"}) == pytest.approx(0.5)

    def test_recall_empty_relevant(self) -> None:
        assert recall(["a"], set()) == pytest.approx(0.0)

    def test_mrr_first_position(self) -> None:
        assert mrr(["a", "b", "c"], {"a"}) == pytest.approx(1.0)

    def test_mrr_second_position(self) -> None:
        assert mrr(["x", "a", "b"], {"a"}) == pytest.approx(0.5)

    def test_mrr_no_overlap(self) -> None:
        assert mrr(["x", "y"], {"a"}) == pytest.approx(0.0)


class TestDilutionPenalties:
    def test_log_dilution(self) -> None:
        telemetry = [
            LevelTelemetry(depth=0, n_candidates=5, n_selected=3, selected_paths=["a", "b", "c"]),
            LevelTelemetry(depth=1, n_candidates=10, n_selected=5, selected_paths=["d", "e", "f", "g", "h"]),
        ]
        import math
        expected = math.log(1 + 3) + math.log(1 + 5)
        assert log_dilution_penalty(telemetry) == pytest.approx(expected)

    def test_ratio_dilution(self) -> None:
        telemetry = [
            LevelTelemetry(depth=0, n_candidates=5, n_selected=3, selected_paths=["a", "b", "c"], rho_l=0.33),
            LevelTelemetry(depth=1, n_candidates=10, n_selected=5, selected_paths=[], rho_l=0.8),
        ]
        expected = 0.33 + 0.8
        assert ratio_dilution_penalty(telemetry) == pytest.approx(expected)

    def test_empty_telemetry(self) -> None:
        assert log_dilution_penalty([]) == 0.0
        assert ratio_dilution_penalty([]) == 0.0
