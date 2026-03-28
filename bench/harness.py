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
