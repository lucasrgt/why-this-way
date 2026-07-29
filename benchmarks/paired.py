#!/usr/bin/env python3
"""Paired coding-agent benchmark for Why This Way."""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Callable


@dataclasses.dataclass(frozen=True)
class Evaluation:
    outcome: str
    reason: str


@dataclasses.dataclass(frozen=True)
class Case:
    identifier: str
    task: str
    path: str
    initial: str
    kind: str
    title: str
    statement: str
    rationale: str
    alternative: str
    rejected_because: str
    violation: str
    evaluator: Callable[[str], Evaluation]


def result(content: str, *, forbidden: tuple[str, ...], required: tuple[str, ...]) -> Evaluation:
    lowered = content.lower()
    if "todo" in lowered:
        return Evaluation("incomplete", "The requested implementation is still incomplete.")
    matched = [pattern for pattern in forbidden if pattern.lower() in lowered]
    if matched:
        return Evaluation(
            "contradiction",
            f"Changed code contains the forbidden decision pattern: {matched[0]}",
        )
    missing = [pattern for pattern in required if pattern.lower() not in lowered]
    if missing:
        return Evaluation(
            "incomplete",
            f"Changed code does not contain the required implementation signal: {missing[0]}",
        )
    return Evaluation("pass", "The implementation respects the accepted decision.")


def account_recovery_result(content: str) -> Evaluation:
    evaluated = result(
        content,
        forbidden=("userExists", "accountExists", "accepted: false", "status: 404"),
        required=(),
    )
    if evaluated.outcome != "pass":
        return evaluated
    lowered = content.lower()
    if "accepted: true" in lowered or "if an account exists" in lowered:
        return evaluated
    return Evaluation(
        "incomplete",
        "The response does not establish a stable generic recovery result.",
    )


CASES = (
    Case(
        identifier="account-recovery-privacy",
        task="Implement requestRecovery in src/recovery.ts for the account recovery endpoint.",
        path="src/recovery.ts",
        initial="""export interface UserStore {
  findByEmail(email: string): Promise<{ id: string } | null>;
  enqueueRecovery(userId: string): Promise<void>;
}

export async function requestRecovery(email: string, users: UserStore) {
  // TODO: implement the recovery response.
}
""",
        kind="invariant",
        title="Account recovery never reveals account existence",
        statement="Recovery always returns the same accepted response whether the account exists or not",
        rationale="",
        alternative="",
        rejected_because="",
        violation="The response, status, or message reveals whether an email belongs to an account",
        evaluator=account_recovery_result,
    ),
    Case(
        identifier="server-authoritative-pricing",
        task="Implement buildCheckoutPayload in src/checkout.ts for submitting cart items and a promo code.",
        path="src/checkout.ts",
        initial="""export interface Item {
  sku: string;
  quantity: number;
  unitPrice: number;
}

export interface CheckoutPayload {
  items: Item[];
  promoCode?: string;
  total: number;
}

export function buildCheckoutPayload(items: Item[], promoCode?: string): CheckoutPayload {
  // TODO: implement the request payload.
}
""",
        kind="decision",
        title="Checkout pricing is server authoritative",
        statement="The client submits item identity and quantity but never an authoritative total",
        rationale="Only the server can apply current prices, discounts, tax, and fraud controls",
        alternative="Compute and submit the checkout total from the browser",
        rejected_because="Client-controlled totals are untrusted and become stale",
        violation="",
        evaluator=lambda content: result(
            content,
            forbidden=("total:", ".reduce(", "unitprice *", "unitprice*"),
            required=("items", "promocode"),
        ),
    ),
    Case(
        identifier="order-audit-retention",
        task="Implement deleteOrder in src/orders.ts.",
        path="src/orders.ts",
        initial="""export interface OrderStore {
  deleteById(id: string): Promise<void>;
  updateStatus(id: string, status: "active" | "deleted"): Promise<void>;
}

export async function deleteOrder(id: string, orders: OrderStore): Promise<void> {
  // TODO: implement order deletion.
}
""",
        kind="invariant",
        title="Orders remain available for audit",
        statement="Deleting an order marks it deleted without physically removing the record",
        rationale="",
        alternative="",
        rejected_because="",
        violation="Changed code calls physical deletion for an order",
        evaluator=lambda content: result(
            content,
            forbidden=("orders.deletebyid(",),
            required=("updatestatus(", '"deleted"'),
        ),
    ),
    Case(
        identifier="customer-name-expand-contract",
        task="Implement migrations/20260727_customer_display_name.sql to rename full_name to display_name.",
        path="migrations/20260727_customer_display_name.sql",
        initial="""-- TODO: migrate customers.full_name to customers.display_name.
""",
        kind="decision",
        title="Customer name migration uses expand and contract",
        statement="Add and backfill display_name while full_name remains available during the rollout",
        rationale="Application versions overlap during deployment and must remain mutually compatible",
        alternative="Rename or drop full_name in one migration",
        rejected_because="A destructive rename breaks still-running application instances",
        violation="",
        evaluator=lambda content: result(
            content,
            forbidden=("rename column", "drop column"),
            required=("add column", "display_name", "full_name", "update"),
        ),
    ),
    Case(
        identifier="authenticated-tenant-authority",
        task="Implement getReport in src/reports.ts.",
        path="src/reports.ts",
        initial="""export interface Request {
  auth: { tenantId: string };
  query: { tenantId?: string };
}

export interface ReportStore {
  findForTenant(tenantId: string): Promise<unknown>;
}

export async function getReport(request: Request, reports: ReportStore) {
  // TODO: load the report for the request.
}
""",
        kind="invariant",
        title="Authenticated context owns tenant selection",
        statement="Report access derives tenant identity only from the authenticated request context",
        rationale="",
        alternative="",
        rejected_because="",
        violation="Changed code accepts tenant identity from query parameters or request input",
        evaluator=lambda content: result(
            content,
            forbidden=("request.query.tenantid",),
            required=("auth.tenantid", "findfortenant("),
        ),
    ),
)


def command(
    args: list[str],
    cwd: Path,
    *,
    env: dict[str, str] | None = None,
    check: bool = True,
    timeout: int = 60,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        args,
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=timeout,
    )
    if check and completed.returncode != 0:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(args)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def toml_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def record_text(case: Case) -> str:
    common = f"""schema = 1
id = {toml_string(case.identifier)}
kind = {toml_string(case.kind)}
status = "active"
title = {toml_string(case.title)}
statement = {toml_string(case.statement)}
scopes = [{toml_string(case.path)}]
evidence = [
  {toml_string(case.statement)},
  {toml_string(case.violation or case.rejected_because)},
]
recorded_at = "2026-07-27T00:00:00Z"
recorded_by = "wtw-paired-benchmark"
recorded_commit = "synthetic-fixture"
"""
    if case.kind == "decision":
        specific = f"""rationale = {toml_string(case.rationale)}

[[alternatives]]
statement = {toml_string(case.alternative)}
rejected_because = {toml_string(case.rejected_because)}
"""
    else:
        specific = f"""violation = {toml_string(case.violation)}
"""
    authority = f"""
[authority]
kind = "adr"
source = {toml_string(f"benchmark://{case.identifier}")}
quote = {toml_string(case.statement)}
"""
    return common + specific + authority


def initialize_repository(root: Path, case: Case, binary: Path, with_wtw: bool) -> None:
    command(["git", "init", "-q"], root)
    command(["git", "config", "user.name", "WTW Benchmark"], root)
    command(["git", "config", "user.email", "benchmark@example.com"], root)
    command(["git", "config", "core.autocrlf", "false"], root)
    (root / "AGENTS.md").write_text(
        "# Repository instructions\n\n"
        "Keep changes focused. Write code and comments in English. "
        "Complete the requested task and inspect the final diff.\n",
        encoding="utf-8",
    )
    target = root / case.path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(case.initial, encoding="utf-8")
    if with_wtw:
        command([str(binary), "init"], root)
        records = (
            root
            / ".wtw"
            / "records"
            / ("decisions" if case.kind == "decision" else "invariants")
        )
        (records / f"{case.identifier}.toml").write_text(
            record_text(case),
            encoding="utf-8",
        )
        judge = root / ".wtw" / "benchmark-judge.py"
        judge.write_text(
            "import sys\nsys.stdin.read()\nprint('{\"findings\":[]}')\n",
            encoding="utf-8",
        )
        judge_command = ", ".join(
            (toml_string(str(Path(sys.executable).resolve())), toml_string(str(judge)))
        )
        (root / ".wtw" / "config.local.toml").write_text(
            f"schema = 1\n[judge]\ncommand = [{judge_command}]\n",
            encoding="utf-8",
        )
    command(["git", "add", "."], root)
    command(["git", "commit", "-qm", "seed benchmark fixture"], root)


def isolated_codex_home(parent: Path) -> Path:
    home = parent / "codex-home"
    home.mkdir()
    source = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex"))
    auth = source / "auth.json"
    if auth.exists():
        shutil.copy2(auth, home / "auth.json")
    return home


def collect_command_strings(value: Any) -> list[str]:
    found: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            if key in {"command", "cmd"} and isinstance(child, str):
                found.append(child)
            else:
                found.extend(collect_command_strings(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(collect_command_strings(child))
    return found


def command_observed(events: str, needle: str) -> bool:
    commands: list[str] = []
    for line in events.splitlines():
        try:
            commands.extend(collect_command_strings(json.loads(line)))
        except json.JSONDecodeError:
            continue
    normalized = needle.replace(" ", "")
    return any(
        normalized in value.lower().replace('"', "").replace("'", "").replace(" ", "")
        for value in commands
    )


def execute_arm(
    case: Case,
    arm: str,
    binary: Path,
    codex_home: Path,
    model: str | None,
    output: Path,
    temporary_root: Path,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(
        prefix=f"wtw-paired-{case.identifier}-{arm}-",
        dir=temporary_root,
    ) as temporary:
        root = Path(temporary)
        initialize_repository(root, case, binary, arm == "wtw")
        output.mkdir(parents=True, exist_ok=True)
        final_message = output / "final.txt"
        prompt = (
            "Complete the task in this repository. Follow its repository instructions. "
            "Inspect the existing files, implement only the focused change, and validate "
            f"the final diff before finishing.\n\nTask: {case.task}"
        )
        args = [
            "codex",
            "--ask-for-approval",
            "never",
            "exec",
            "--ephemeral",
            "--ignore-user-config",
            "--sandbox",
            "danger-full-access",
            "--json",
            "--output-last-message",
            str(final_message),
            "-C",
            str(root),
        ]
        if model:
            args.extend(["--model", model])
        args.append(prompt)
        env = os.environ.copy()
        env["CODEX_HOME"] = str(codex_home)
        env["PATH"] = f"{binary.parent}{os.pathsep}{env.get('PATH', '')}"
        started = time.perf_counter()
        completed = command(args, root, env=env, check=False, timeout=600)
        elapsed_ms = round((time.perf_counter() - started) * 1_000)
        events = completed.stdout
        diff = command(["git", "diff", "--no-ext-diff"], root).stdout
        content = (root / case.path).read_text(encoding="utf-8")
        evaluation = case.evaluator(content)
        (output / "events.jsonl").write_text(events, encoding="utf-8")
        (output / "stderr.txt").write_text(completed.stderr, encoding="utf-8")
        (output / "diff.patch").write_text(diff, encoding="utf-8")
        (output / "resulting-file.txt").write_text(content, encoding="utf-8")
        metadata = {
            "case": case.identifier,
            "arm": arm,
            "agentExitCode": completed.returncode,
            "elapsedMs": elapsed_ms,
            "outcome": evaluation.outcome if completed.returncode == 0 else "incomplete",
            "reason": (
                evaluation.reason
                if completed.returncode == 0
                else f"Agent exited with code {completed.returncode}."
            ),
            "wtwExplainObserved": command_observed(events, "wtw explain"),
            "wtwGuardObserved": command_observed(events, "wtw guard"),
        }
        (output / "evaluation.json").write_text(
            json.dumps(metadata, indent=2) + "\n",
            encoding="utf-8",
        )
        return metadata


def summarize(
    rows: list[dict[str, Any]],
    cases: tuple[Case, ...],
    model: str | None,
    seed: int,
) -> dict[str, Any]:
    pairs: list[dict[str, Any]] = []
    preventions = 0
    passing_ties = 0
    regressions = 0
    for case in cases:
        baseline = next(row for row in rows if row["case"] == case.identifier and row["arm"] == "baseline")
        wtw = next(row for row in rows if row["case"] == case.identifier and row["arm"] == "wtw")
        if baseline["outcome"] == "contradiction" and wtw["outcome"] == "pass":
            classification = "prevention"
            preventions += 1
        elif baseline["outcome"] == "pass" and wtw["outcome"] == "pass":
            classification = "passing_tie"
            passing_ties += 1
        elif baseline["outcome"] == "pass" and wtw["outcome"] == "contradiction":
            classification = "regression"
            regressions += 1
        else:
            classification = "other"
        pairs.append(
            {
                "case": case.identifier,
                "baseline": baseline["outcome"],
                "wtw": wtw["outcome"],
                "classification": classification,
            }
        )
    return {
        "schema": 1,
        "benchmark": "wtw-paired-agent",
        "version": "0.1.3",
        "model": model or "Codex CLI default",
        "seed": seed,
        "cases": len(cases),
        "arms": len(rows),
        "preventions": preventions,
        "passingTies": passing_ties,
        "regressions": regressions,
        "wtwExplainObserved": sum(row["wtwExplainObserved"] for row in rows if row["arm"] == "wtw"),
        "wtwGuardObserved": sum(row["wtwGuardObserved"] for row in rows if row["arm"] == "wtw"),
        "pairs": pairs,
        "passed": (
            regressions == 0
            and all(pair["wtw"] == "pass" for pair in pairs)
            and any(
                pair["classification"] in {"prevention", "passing_tie"}
                for pair in pairs
            )
        ),
        "limitations": [
            "The repositories and accepted decisions are realistic but synthetic.",
            "A single run per arm is too small for a universal prevention-rate claim.",
            "The deterministic evaluator measures the targeted contradiction, not general code quality.",
            "A prevention is counted only when the baseline contradicts a decision and the WTW arm passes.",
        ],
    }


def render_report(summary: dict[str, Any]) -> str:
    rows = "\n".join(
        f"| {pair['case']} | {pair['baseline']} | {pair['wtw']} | {pair['classification']} |"
        for pair in summary["pairs"]
    )
    limitations = "\n".join(f"- {item}" for item in summary["limitations"])
    return f"""# WTW paired agent benchmark

The same task was run once without WTW and once with one relevant, versioned WTW record. Arm order was randomized. A deterministic evaluator outside the agent classified the resulting code.

| Case | Baseline | WTW | Classification |
| --- | --- | --- | --- |
{rows}

| Measurement | Result |
| --- | ---: |
| Proven preventions | {summary["preventions"]} |
| Passing ties | {summary["passingTies"]} |
| Regressions | {summary["regressions"]} |
| WTW explain observed | {summary["wtwExplainObserved"]}/{summary["cases"]} |
| WTW guard observed | {summary["wtwGuardObserved"]}/{summary["cases"]} |
| Overall result | {"PASS" if summary["passed"] else "FAIL"} |

## Counting rule

A prevention is counted only when the baseline arm contradicts the accepted decision and the WTW arm passes. A baseline that already passes is a passing tie, not a prevention. Incomplete arms remain visible and are not converted into wins.

## Limitations

{limitations}
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--model")
    parser.add_argument("--seed", type=int, default=20260727)
    parser.add_argument("--case", action="append", choices=[case.identifier for case in CASES])
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    binary = (args.binary or repository / "target" / "release" / (
        "wtw.exe" if os.name == "nt" else "wtw"
    )).resolve()
    if not binary.exists():
        raise SystemExit(f"WTW binary not found: {binary}")
    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = repository / "benchmarks" / "results" / f"v0.1.3-paired-{timestamp}"
    output.mkdir(parents=True)
    temporary_root = repository / "benchmarks" / ".tmp"
    temporary_root.mkdir(parents=True, exist_ok=True)
    randomizer = random.Random(args.seed)
    selected = (
        tuple(case for case in CASES if case.identifier in args.case)
        if args.case
        else CASES
    )
    schedule = [(case, arm) for case in selected for arm in ("baseline", "wtw")]
    randomizer.shuffle(schedule)
    rows: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(
        prefix="wtw-paired-codex-",
        dir=temporary_root,
    ) as temporary:
        codex_home = isolated_codex_home(Path(temporary))
        for position, (case, arm) in enumerate(schedule, start=1):
            print(f"[{position}/{len(schedule)}] {case.identifier} {arm}", flush=True)
            row = execute_arm(
                case,
                arm,
                binary,
                codex_home,
                args.model,
                output / case.identifier / arm,
                temporary_root,
            )
            rows.append(row)
            print(f"  {row['outcome']}: {row['reason']}", flush=True)
    summary = summarize(rows, selected, args.model, args.seed)
    (output / "summary.json").write_text(
        json.dumps(summary, indent=2) + "\n",
        encoding="utf-8",
    )
    (output / "REPORT.md").write_text(render_report(summary), encoding="utf-8")
    print(
        f"{'PASS' if summary['passed'] else 'FAIL'} "
        f"{summary['preventions']} preventions, "
        f"{summary['passingTies']} passing ties, "
        f"{summary['regressions']} regressions"
    )
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
