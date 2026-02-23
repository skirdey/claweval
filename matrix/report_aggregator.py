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

    for suite_name, report in agent_reports.items():
        overall = report.get("overall", {})
        suite_total = overall.get("total_runs", 0)
        suite_passed = overall.get("passed_runs", 0)
        suite_duration = report.get("duration_ms", 0)

        total_episodes += suite_total
        passed_episodes += suite_passed
        total_duration_ms += suite_duration

        per_suite[suite_name] = {
            "total": suite_total,
            "passed": suite_passed,
            "pass_rate": round(suite_passed / suite_total * 100, 1) if suite_total > 0 else 0,
            "duration_ms": suite_duration,
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

    return {
        "leaderboard": leaderboard,
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
