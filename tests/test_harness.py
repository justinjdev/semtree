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
