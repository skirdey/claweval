#!/usr/bin/env python3
"""ClawEval Matrix Report Aggregator.

Reads per-agent report JSONs from a run directory and produces:
  - _matrix_summary.json  (machine-readable)
  - _matrix_summary.md    (markdown tables)

Usage:
    python matrix/report_aggregator.py reports/matrix/run_20260222_143000/
"""

import json
import sys
from pathlib import Path


def load_reports(run_dir: Path) -> dict[str, dict[str, dict]]:
    """Load all report JSONs. Returns {agent_name: {suite_name: report_data}}."""
    reports: dict[str, dict[str, dict]] = {}

    for json_file in sorted(run_dir.glob("*.json")):
        if json_file.name.startswith("_"):
            continue  # Skip our own output files

        # Filename format: agentname__suitename.json
        stem = json_file.stem
        if "__" not in stem:
            continue

        agent_name, suite_name = stem.split("__", 1)

        try:
            with open(json_file) as f:
                data = json.load(f)
        except (json.JSONDecodeError, OSError) as e:
            print(f"  [WARN] Failed to load {json_file}: {e}")
            continue

        reports.setdefault(agent_name, {})[suite_name] = data

    return reports


def compute_agent_stats(agent_reports: dict[str, dict]) -> dict:
    """Compute aggregate stats for one agent across all its suites."""
    total_episodes = 0
    passed_episodes = 0
    total_duration_ms = 0
    per_suite: dict[str, dict] = {}
    core_total = 0
    core_passed = 0
    advanced_total = 0
    advanced_passed = 0

    for suite_name, report in agent_reports.items():
        overall = report.get("overall", {})
        suite_total = overall.get("total_runs", 0)
        suite_passed = overall.get("passed_runs", 0)
        suite_duration = report.get("duration_ms", 0)
        scoring_class = report.get("scoring_class", "core")

        total_episodes += suite_total
        passed_episodes += suite_passed
        total_duration_ms += suite_duration
        if scoring_class == "advanced":
            advanced_total += suite_total
            advanced_passed += suite_passed
        else:
            core_total += suite_total
            core_passed += suite_passed

        per_suite[suite_name] = {
            "total": suite_total,
            "passed": suite_passed,
            "pass_rate": round(suite_passed / suite_total * 100, 1) if suite_total > 0 else 0,
            "duration_ms": suite_duration,
            "scoring_class": scoring_class,
        }

    pass_rate = round(passed_episodes / total_episodes * 100, 1) if total_episodes > 0 else 0
    avg_duration = round(total_duration_ms / len(agent_reports)) if agent_reports else 0

    return {
        "total_episodes": total_episodes,
        "passed_episodes": passed_episodes,
        "pass_rate": pass_rate,
        "total_duration_ms": total_duration_ms,
        "avg_suite_duration_ms": avg_duration,
        "per_suite": per_suite,
        "core_total_episodes": core_total,
        "core_passed_episodes": core_passed,
        "core_pass_rate": round(core_passed / core_total * 100, 1) if core_total > 0 else 0,
        "advanced_total_episodes": advanced_total,
        "advanced_passed_episodes": advanced_passed,
        "advanced_pass_rate": round(advanced_passed / advanced_total * 100, 1) if advanced_total > 0 else 0,
    }


def compute_episode_comparison(reports: dict[str, dict[str, dict]]) -> dict:
    """Build per-episode comparison across agents."""
    # Gather all episodes from all suites
    episode_results: dict[str, dict[str, dict]] = {}  # {suite::episode_id: {agent: result}}

    for agent_name, agent_reports in reports.items():
        for suite_name, report in agent_reports.items():
            for episode in report.get("episodes", []):
                ep_key = f"{suite_name}::{episode['id']}"
                summary = episode.get("summary", {})
                episode_results.setdefault(ep_key, {})[agent_name] = {
                    "passed": summary.get("passed_runs", 0),
                    "total": summary.get("total_runs", 0),
                    "pass_rate": summary.get("pass_rate", {}).get("percent", 0),
                    "avg_duration_ms": summary.get("avg_duration_ms", 0),
                }

    return episode_results


def build_summary(reports: dict[str, dict[str, dict]]) -> dict:
    """Build the full matrix summary."""
    leaderboard = []
    for agent_name, agent_reports in reports.items():
        stats = compute_agent_stats(agent_reports)
        leaderboard.append({"agent": agent_name, **stats})

    # Sort by pass_rate descending, then by avg_duration ascending
    leaderboard.sort(key=lambda x: (-x["pass_rate"], x["avg_suite_duration_ms"]))

    # Add rank
    for i, entry in enumerate(leaderboard):
        entry["rank"] = i + 1

    core_rank = sorted(
        leaderboard,
        key=lambda x: (-x["core_pass_rate"], x["avg_suite_duration_ms"]),
    )
    advanced_rank = sorted(
        leaderboard,
        key=lambda x: (-x["advanced_pass_rate"], x["avg_suite_duration_ms"]),
    )
    for i, entry in enumerate(core_rank):
        entry["core_rank"] = i + 1
    for i, entry in enumerate(advanced_rank):
        entry["advanced_rank"] = i + 1

    return {
        "leaderboard": leaderboard,
        "core_leaderboard": core_rank,
        "advanced_leaderboard": advanced_rank,
        "episode_comparison": compute_episode_comparison(reports),
    }


def render_markdown(summary: dict, agents: list[str], suites: list[str]) -> str:
    """Render summary as markdown tables."""
    lines = ["# ClawEval Matrix Results\n"]

    # Leaderboard
    lines.append("## Leaderboard\n")
    lines.append("| Rank | Agent | Pass Rate | Passed/Total | Avg Duration |")
    lines.append("|------|-------|-----------|--------------|--------------|")
    for entry in summary["leaderboard"]:
        lines.append(
            f"| {entry['rank']} | {entry['agent']} | {entry['pass_rate']}% "
            f"| {entry['passed_episodes']}/{entry['total_episodes']} "
            f"| {entry['avg_suite_duration_ms']}ms |"
        )
    lines.append("")

    # Core reliability leaderboard
    lines.append("## Core Reliability\n")
    lines.append("| Rank | Agent | Core Pass Rate | Passed/Total |")
    lines.append("|------|-------|----------------|--------------|")
    for entry in sorted(summary["core_leaderboard"], key=lambda x: x["core_rank"]):
        lines.append(
            f"| {entry['core_rank']} | {entry['agent']} | {entry['core_pass_rate']}% "
            f"| {entry['core_passed_episodes']}/{entry['core_total_episodes']} |"
        )
    lines.append("")

    # Advanced capability leaderboard
    lines.append("## Advanced Capability\n")
    lines.append("| Rank | Agent | Advanced Pass Rate | Passed/Total |")
    lines.append("|------|-------|--------------------|--------------|")
    for entry in sorted(summary["advanced_leaderboard"], key=lambda x: x["advanced_rank"]):
        lines.append(
            f"| {entry['advanced_rank']} | {entry['agent']} | {entry['advanced_pass_rate']}% "
            f"| {entry['advanced_passed_episodes']}/{entry['advanced_total_episodes']} |"
        )
    lines.append("")

    # Per-Suite Breakdown
    if suites:
        lines.append("## Per-Suite Breakdown\n")
        header = "| Suite |" + " | ".join(agents) + " |"
        sep = "|-------|" + " | ".join(["-------"] * len(agents)) + " |"
        lines.append(header)
        lines.append(sep)

        for suite in suites:
            row = f"| {suite} |"
            for agent in agents:
                entry = next((e for e in summary["leaderboard"] if e["agent"] == agent), None)
                if entry and suite in entry.get("per_suite", {}):
                    ps = entry["per_suite"][suite]
                    row += f" {ps['passed']}/{ps['total']} |"
                else:
                    row += " - |"
            lines.append(row)
        lines.append("")

    # Per-Episode Comparison
    ep_comp = summary.get("episode_comparison", {})
    if ep_comp:
        lines.append("## Per-Episode Comparison\n")
        lines.append("| Episode |" + " | ".join(agents) + " |")
        lines.append("|---------|" + " | ".join(["-------"] * len(agents)) + " |")

        for ep_key in sorted(ep_comp.keys()):
            row = f"| {ep_key} |"
            for agent in agents:
                if agent in ep_comp[ep_key]:
                    r = ep_comp[ep_key][agent]
                    icon = "pass" if r["passed"] == r["total"] else "FAIL"
                    row += f" {icon} ({r['avg_duration_ms']:.0f}ms) |"
                else:
                    row += " - |"
            lines.append(row)
        lines.append("")

    return "\n".join(lines)


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <run_dir>")
        sys.exit(1)

    run_dir = Path(sys.argv[1])
    if not run_dir.is_dir():
        print(f"Not a directory: {run_dir}")
        sys.exit(1)

    print(f"Aggregating reports from {run_dir}")
    reports = load_reports(run_dir)

    if not reports:
        print("  No reports found.")
        sys.exit(0)

    agents = sorted(reports.keys())
    suites = sorted({s for agent_reports in reports.values() for s in agent_reports.keys()})
    print(f"  Found {len(agents)} agents, {len(suites)} suites")

    summary = build_summary(reports)

    # Write JSON summary
    json_path = run_dir / "_matrix_summary.json"
    with open(json_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"  Wrote {json_path}")

    # Write Markdown summary
    md_path = run_dir / "_matrix_summary.md"
    md_content = render_markdown(summary, agents, suites)
    with open(md_path, "w") as f:
        f.write(md_content)
    print(f"  Wrote {md_path}")


if __name__ == "__main__":
    main()
