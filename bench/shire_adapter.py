"""Shire MCP adapter for benchmark comparison."""

import json
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from bench.routing import Query, load_queries, ndcg_at_k


class ShireClient:
    """Manages a shire MCP server subprocess."""

    def __init__(self, repo_path: Path):
        self.proc = subprocess.Popen(
            ["shire", "serve", "--root", str(repo_path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self._id = 0
        self._initialize()

    def _next_id(self) -> int:
        self._id += 1
        return self._id

    def _send(self, method: str, params: dict | None = None, is_notification: bool = False) -> dict | None:
        msg: dict = {"jsonrpc": "2.0", "method": method}
        if params:
            msg["params"] = params
        if not is_notification:
            msg["id"] = self._next_id()
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        if is_notification:
            return None
        line = self.proc.stdout.readline()
        return json.loads(line) if line else None

    def _initialize(self) -> None:
        self._send("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "bench", "version": "0.1"},
        })
        self._send("notifications/initialized", is_notification=True)

    def call_tool(self, name: str, arguments: dict) -> list[dict]:
        """Call an MCP tool and return parsed results."""
        resp = self._send("tools/call", {"name": name, "arguments": arguments})
        if resp is None:
            return []
        content = resp.get("result", {}).get("content", [{}])
        text = content[0].get("text", "[]") if content else "[]"
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return []

    def search_symbols(self, query: str, limit: int = 20) -> list[dict]:
        return self.call_tool("search_symbols", {"query": query, "limit": limit})

    def search_files(self, query: str, limit: int = 20) -> list[dict]:
        return self.call_tool("search_files", {"query": query, "limit": limit})

    def close(self) -> None:
        self.proc.terminate()
        self.proc.wait(timeout=5)


# Control grid for Shire: vary limit and search strategy
SHIRE_CONTROL_GRID = [
    {"strategy": strat, "limit": lim}
    for strat in ["symbols", "files", "combined"]
    for lim in [5, 10, 20, 50]
]


def run_shire_phase(
    repo_path: Path,
    query_file: Path,
    repo_name: str = "local",
    results_path: Path | None = None,
) -> list[MetricRecord]:
    """Run Shire benchmark: sweep control grid, collect metrics per query per setting."""
    from bench.harness import append_results

    queries = load_queries(query_file)
    records: list[MetricRecord] = []

    client = ShireClient(repo_path)
    try:
        for query in queries:
            relevant_map = {r["path"]: r["relevance"] for r in query.relevant}

            for control in SHIRE_CONTROL_GRID:
                now = datetime.now(timezone.utc).isoformat(timespec="seconds")
                control_json = json.dumps(control, sort_keys=True)

                t0 = time.monotonic()

                file_paths: list[str] = []
                if control["strategy"] in ("symbols", "combined"):
                    symbols = client.search_symbols(query.question, limit=control["limit"])
                    for s in symbols:
                        fp = s.get("file_path", "")
                        if fp and fp not in file_paths:
                            file_paths.append(fp)

                if control["strategy"] in ("files", "combined"):
                    files = client.search_files(query.question, limit=control["limit"])
                    for f in files:
                        fp = f.get("path", "")
                        if fp and fp not in file_paths:
                            file_paths.append(fp)

                elapsed = time.monotonic() - t0
                ndcg = ndcg_at_k(file_paths, relevant_map, k=10)

                batch: list[MetricRecord] = []
                for metric, value in [
                    ("ndcg@10", ndcg),
                    ("cost_usd", 0.0),  # local, no API cost
                    ("latency_s", elapsed),
                    ("tokens_loaded", 0),  # Shire doesn't load tokens
                    ("llm_calls", 0),
                ]:
                    batch.append(MetricRecord(
                        now, "routing", repo_name, "shire",
                        query.id, control_json, metric, value,
                    ))

                records.extend(batch)
                if results_path:
                    append_results(results_path, batch)

    finally:
        client.close()

    return records
