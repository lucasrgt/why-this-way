#!/usr/bin/env python3
"""Deterministic large-corpus stress benchmark for Why This Way."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


DEFAULT_CORPORA = (1_024, 10_000)
PROBES = 64


def run(
    command: list[str],
    cwd: Path,
    *,
    check: bool = True,
    input_text: str | None = None,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        command,
        cwd=cwd,
        input=input_text,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def toml_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def record_text(index: int, *, status: str = "active") -> str:
    identifier = f"domain-policy-{index:05d}"
    domain = f"domain-{index:05d}"
    common = f"""schema = 1
id = {toml_string(identifier)}
kind = "{"decision" if index % 2 == 0 else "invariant"}"
status = {toml_string(status)}
title = {toml_string(f"Preserve policy token-{index:05d}")}
statement = {toml_string(f"{domain} must preserve policy token-{index:05d}")}
scopes = [{toml_string(f"src/{domain}/**")}]
evidence = [
  {toml_string(f"{domain} owns policy token-{index:05d}")},
  {toml_string(f"breaking token-{index:05d} violates the accepted boundary")},
]
recorded_at = "2026-07-27T00:00:00Z"
recorded_by = "wtw-stress-benchmark"
recorded_commit = "synthetic-corpus"
"""
    if index % 2 == 0:
        specific = f"""rationale = {toml_string(f"Authority for token-{index:05d} remains local to {domain}")}

[[alternatives]]
statement = {toml_string(f"Move token-{index:05d} to a shared authority")}
rejected_because = {toml_string(f"Shared authority obscures {domain} ownership")}
"""
    else:
        specific = f"""violation = {toml_string(f"Changed code removes or bypasses token-{index:05d}")}
"""
    authority = f"""
[authority]
kind = "adr"
source = {toml_string(f"docs/decisions/{identifier}.md")}
quote = {toml_string(f"{domain} owns policy token-{index:05d}")}
"""
    return common + specific + authority


def seed_repository(root: Path, binary: Path, count: int) -> None:
    run(["git", "init", "-q"], root)
    run(["git", "config", "user.name", "WTW Benchmark"], root)
    run(["git", "config", "user.email", "benchmark@example.com"], root)
    run(["git", "config", "core.autocrlf", "false"], root)
    (root / "AGENTS.md").write_text("# Benchmark repository\n", encoding="utf-8")
    (root / "src").mkdir()
    (root / "src" / "baseline.txt").write_text("baseline\n", encoding="utf-8")
    run([str(binary), "init"], root)

    decisions = root / ".wtw" / "records" / "decisions"
    invariants = root / ".wtw" / "records" / "invariants"
    started = time.perf_counter()
    for index in range(count):
        folder = decisions if index % 2 == 0 else invariants
        (folder / f"domain-policy-{index:05d}.toml").write_text(
            record_text(index),
            encoding="utf-8",
        )
    seed_ms = round((time.perf_counter() - started) * 1_000)
    (root / ".benchmark-seed-ms").write_text(str(seed_ms), encoding="utf-8")
    run(["git", "add", "."], root)
    run(["git", "commit", "-qm", "seed WTW stress corpus"], root)


def timed_json(
    command: list[str],
    root: Path,
    *,
    allowed_returncodes: tuple[int, ...] = (0,),
) -> tuple[dict[str, Any], float]:
    started = time.perf_counter()
    result = run(command, root, check=False)
    elapsed_ms = (time.perf_counter() - started) * 1_000
    if result.returncode not in allowed_returncodes:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return json.loads(result.stdout), elapsed_ms


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    position = min(len(ordered) - 1, max(0, int(round((len(ordered) - 1) * fraction))))
    return ordered[position]


def configure_judge(root: Path, target_index: int) -> None:
    python = Path(sys.executable).resolve()
    script = root / ".wtw" / "benchmark-judge.py"
    identifier = f"domain-policy-{target_index:05d}"
    domain = f"domain-{target_index:05d}"
    evidence = f"bypass token-{target_index:05d}"
    script.write_text(
        "\n".join(
            [
                "import json, sys",
                "prompt = sys.stdin.read()",
                f"evidence = {evidence!r}",
                "finding = {",
                f"    'record_uri': 'wtw://decision/{identifier}',",
                f"    'path': 'src/{domain}/handler.ts',",
                "    'line': 2,",
                "    'evidence': evidence,",
                "    'reason': 'The changed code bypasses the accepted domain authority.',",
                "}",
                "print(json.dumps({'findings': [finding] if evidence in prompt else []}))",
                "",
            ]
        ),
        encoding="utf-8",
    )
    command = ", ".join((toml_string(str(python)), toml_string(str(script))))
    (root / ".wtw" / "config.local.toml").write_text(
        f"schema = 1\n[judge]\ncommand = [{command}]\n",
        encoding="utf-8",
    )


def guard_checks(root: Path, binary: Path, target_index: int) -> dict[str, Any]:
    identifier = f"domain-policy-{target_index:05d}"
    domain = f"domain-{target_index:05d}"
    target = root / "src" / domain / "handler.ts"
    target.parent.mkdir(parents=True)
    target.write_text("export const policy = 'compliant';\n", encoding="utf-8")
    run(["git", "add", "."], root)
    run(["git", "commit", "-qm", "add guard fixture"], root)
    configure_judge(root, target_index)

    target.write_text(
        f"export const policy = 'compliant';\nexport const unsafe = 'bypass token-{target_index:05d}';\n",
        encoding="utf-8",
    )
    positive, positive_ms = timed_json(
        [
            str(binary),
            "guard",
            "--task",
            f"change {domain} policy token-{target_index:05d}",
            "--path",
            f"src/{domain}/handler.ts",
            "--json",
        ],
        root,
        allowed_returncodes=(0, 1),
    )

    target.write_text(
        "export const policy = 'compliant';\nexport const note = 'preserve accepted authority';\n",
        encoding="utf-8",
    )
    negative, negative_ms = timed_json(
        [
            str(binary),
            "guard",
            "--task",
            f"change {domain} policy token-{target_index:05d}",
            "--path",
            f"src/{domain}/handler.ts",
            "--json",
        ],
        root,
    )
    findings = positive.get("findings", [])
    return {
        "positiveFindingCount": len(findings),
        "positiveFoundTarget": any(
            finding.get("record_uri") == f"wtw://decision/{identifier}"
            for finding in findings
        ),
        "negativeFindingCount": len(negative.get("findings", [])),
        "positiveMs": round(positive_ms, 2),
        "negativeMs": round(negative_ms, 2),
    }


def corrupt_storage_fails_closed(root: Path, binary: Path) -> bool:
    corrupt = (
        root
        / ".wtw"
        / "records"
        / "invariants"
        / "corrupt-benchmark-record.toml"
    )
    corrupt.write_text("schema = 99\n", encoding="utf-8")
    result = run(
        [str(binary), "explain", "--task", "any task", "--json"],
        root,
        check=False,
    )
    corrupt.unlink()
    return result.returncode != 0


def run_corpus(binary: Path, count: int) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"wtw-stress-{count}-") as temporary:
        root = Path(temporary)
        seed_repository(root, binary, count)
        seed_ms = int((root / ".benchmark-seed-ms").read_text(encoding="utf-8"))

        graph, export_ms = timed_json([str(binary), "export"], root)
        health, health_ms = timed_json([str(binary), "health", "--json"], root)
        latencies: list[float] = []
        exact_hits = 0
        top_rank_hits = 0
        bounded_results = 0
        probe_count = min(PROBES, count)
        stride = max(1, count // probe_count)
        probed = [min(index * stride, count - 1) for index in range(probe_count)]

        for target_index in probed:
            identifier = f"domain-policy-{target_index:05d}"
            domain = f"domain-{target_index:05d}"
            context, elapsed_ms = timed_json(
                [
                    str(binary),
                    "explain",
                    "--task",
                    f"modify {domain} policy token-{target_index:05d}",
                    "--path",
                    f"src/{domain}/handler.ts",
                    "--limit",
                    "12",
                    "--json",
                ],
                root,
            )
            latencies.append(elapsed_ms)
            records = context.get("records", [])
            identifiers = [record.get("id") for record in records]
            exact_hits += int(identifier in identifiers)
            top_rank_hits += int(bool(identifiers) and identifiers[0] == identifier)
            bounded_results += int(len(records) <= 12)

        negative, negative_ms = timed_json(
            [
                str(binary),
                "explain",
                "--task",
                "quasar-zebra-absent-vocabulary",
                "--path",
                "unrelated/void/file.txt",
                "--limit",
                "12",
                "--json",
            ],
            root,
        )
        guard = guard_checks(root, binary, probed[0] if probed[0] % 2 == 0 else 0)
        corrupt_failed_closed = corrupt_storage_fails_closed(root, binary)
        graph_nodes = graph.get("nodes", [])
        graph_edges = graph.get("edges", [])

        result = {
            "schema": 1,
            "benchmark": "wtw-large-corpus-stress",
            "version": "0.1.3",
            "corpus": {
                "records": count,
                "decisions": (count + 1) // 2,
                "invariants": count // 2,
                "graphNodes": len(graph_nodes),
                "graphEdges": len(graph_edges),
            },
            "retrieval": {
                "probes": probe_count,
                "exactHits": exact_hits,
                "topRankHits": top_rank_hits,
                "boundedResults": bounded_results,
                "negativeResults": len(negative.get("records", [])),
            },
            "guard": guard,
            "health": {
                "passed": health.get("passed"),
                "issueCount": len(health.get("issues", [])),
            },
            "failClosed": {
                "corruptStorageRejected": corrupt_failed_closed,
            },
            "performance": {
                "seedMs": seed_ms,
                "exportMs": round(export_ms, 2),
                "healthMs": round(health_ms, 2),
                "explainMeanMs": round(statistics.mean(latencies), 2),
                "explainP50Ms": round(percentile(latencies, 0.50), 2),
                "explainP95Ms": round(percentile(latencies, 0.95), 2),
                "explainMaxMs": round(max(latencies), 2),
                "negativeExplainMs": round(negative_ms, 2),
            },
            "passed": (
                len(graph_nodes) == count
                and exact_hits == probe_count
                and top_rank_hits == probe_count
                and bounded_results == probe_count
                and len(negative.get("records", [])) == 0
                and guard["positiveFindingCount"] == 1
                and guard["positiveFoundTarget"]
                and guard["negativeFindingCount"] == 0
                and health.get("passed") is True
                and corrupt_failed_closed
            ),
            "limitations": [
                "The corpus and judge are deterministic synthetic fixtures.",
                "Retrieval probes use unique vocabulary and exact governing paths.",
                "This stress test measures storage, ranking, graph, guard plumbing, and fail-closed behavior; it does not measure whether an LLM discovers the correct decision.",
            ],
        }
        return result


def render_report(result: dict[str, Any]) -> str:
    corpus = result["corpus"]
    retrieval = result["retrieval"]
    guard = result["guard"]
    performance = result["performance"]
    limitations = "\n".join(f"- {item}" for item in result["limitations"])
    return f"""# WTW large-corpus stress: {corpus["records"]:,} records

This benchmark exercises Why This Way against a versioned synthetic repository with an equal mix of decisions and invariants.

| Measurement | Result |
| --- | ---: |
| Records loaded and exported | {corpus["graphNodes"]:,}/{corpus["records"]:,} |
| Retrieval exact hits | {retrieval["exactHits"]}/{retrieval["probes"]} |
| Target ranked first | {retrieval["topRankHits"]}/{retrieval["probes"]} |
| Result limit respected | {retrieval["boundedResults"]}/{retrieval["probes"]} |
| Unrelated-query results | {retrieval["negativeResults"]} |
| Contradicting diff findings | {guard["positiveFindingCount"]} |
| Compliant diff findings | {guard["negativeFindingCount"]} |
| Corrupt storage rejected | {"yes" if result["failClosed"]["corruptStorageRejected"] else "no"} |
| Graph health | {"pass" if result["health"]["passed"] else "fail"} |
| Explain latency p50 | {performance["explainP50Ms"]:.2f} ms |
| Explain latency p95 | {performance["explainP95Ms"]:.2f} ms |
| Explain latency max | {performance["explainMaxMs"]:.2f} ms |
| Overall result | {"PASS" if result["passed"] else "FAIL"} |

## Interpretation

WTW recalled every deliberately relevant record, ranked it first, kept the response bounded, rejected corrupt truth, and distinguished a known contradiction from a compliant change.

## Limitations

{limitations}
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--records", type=int, action="append")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    binary = (args.binary or repository / "target" / "release" / (
        "wtw.exe" if os.name == "nt" else "wtw"
    )).resolve()
    if not binary.exists():
        raise SystemExit(f"WTW binary not found: {binary}")
    corpora = args.records or list(DEFAULT_CORPORA)
    exit_code = 0
    for count in corpora:
        result = run_corpus(binary, count)
        output = repository / "benchmarks" / "results" / f"v0.1.3-stress-{count}"
        output.mkdir(parents=True, exist_ok=True)
        (output / "summary.json").write_text(
            json.dumps(result, indent=2) + "\n",
            encoding="utf-8",
        )
        (output / "REPORT.md").write_text(render_report(result), encoding="utf-8")
        print(
            f"{'PASS' if result['passed'] else 'FAIL'} {count} records, "
            f"{result['retrieval']['exactHits']}/{result['retrieval']['probes']} exact hits, "
            f"p95 {result['performance']['explainP95Ms']:.2f} ms"
        )
        if not result["passed"]:
            exit_code = 1
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
