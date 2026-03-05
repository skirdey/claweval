#!/usr/bin/env python3
"""ClawEval Matrix Runner — orchestrates Docker containers and claweval runs.

Usage:
    python matrix/matrix_runner.py
    python matrix/matrix_runner.py --agents openclaw openai_direct --suites matrix_basic.json
    python matrix/matrix_runner.py --no-docker --keep-containers --jobs 4
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path

MATRIX_DIR = Path(__file__).parent.resolve()
PROJECT_ROOT = MATRIX_DIR.parent
COMPOSE_FILE = MATRIX_DIR / "docker-compose.yml"
DEFAULT_CONFIG = MATRIX_DIR / "matrix.json"
SUITES_DIR = MATRIX_DIR / "suites"


def load_dotenv(path: Path):
    """Load KEY=VALUE lines from a .env file into os.environ."""
    if not path.exists():
        return
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "=" in line:
                key, _, value = line.partition("=")
                os.environ.setdefault(key.strip(), value.strip())


def substitute_env_vars(obj):
    """Recursively replace ${VAR} patterns in strings with os.environ values."""
    if isinstance(obj, str):
        return re.sub(r'\$\{(\w+)\}', lambda m: os.environ.get(m.group(1), m.group(0)), obj)
    if isinstance(obj, dict):
        return {k: substitute_env_vars(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [substitute_env_vars(v) for v in obj]
    return obj


def load_config(config_path: Path) -> dict:
    with open(config_path) as f:
        return json.load(f)


def docker_compose(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    env_file = MATRIX_DIR / ".env"
    cmd = ["docker", "compose", "-f", str(COMPOSE_FILE)]
    if env_file.exists():
        cmd.extend(["--env-file", str(env_file)])
    cmd.extend(list(args))
    print(f"  $ {' '.join(cmd)}")
    return subprocess.run(cmd, capture_output=True, text=True, check=check, cwd=str(PROJECT_ROOT))


def start_containers(agents: list[dict], build: bool = True, global_sidecars: list[str] | None = None):
    """Start Docker containers for the given agents."""
    service_names = []
    for agent in agents:
        service_names.append(agent["service"])
        # Include sidecar services
        for sidecar in agent.get("sidecars", []):
            service_names.append(sidecar)

    for sidecar in global_sidecars or []:
        service_names.append(sidecar)
    service_names = sorted(set(service_names))

    print(f"\n--- Starting containers: {', '.join(service_names)}")
    build_flag = ["--build"] if build else []
    docker_compose("up", "-d", *build_flag, *service_names)


def stop_containers():
    """Stop and remove all matrix containers."""
    print("\n--- Stopping containers")
    docker_compose("down", "--remove-orphans", check=False)


def wait_for_health(agents: list[dict], timeout: int = 60, interval: float = 2.0):
    """Poll /health on each agent until all are ready or timeout."""
    import requests

    print(f"\n--- Waiting for agents to be healthy (timeout={timeout}s)")
    deadline = time.time() + timeout
    pending = {a["name"]: a["port"] for a in agents}

    while pending and time.time() < deadline:
        for name, port in list(pending.items()):
            try:
                resp = requests.get(f"http://localhost:{port}/health", timeout=3)
                if resp.status_code == 200:
                    data = resp.json()
                    print(f"  [OK] {name} (port {port}): {data}")
                    del pending[name]
            except Exception:
                pass
        if pending:
            time.sleep(interval)

    if pending:
        failed = ", ".join(pending.keys())
        print(f"  [FAIL] Agents not ready: {failed}")
        sys.exit(1)

    print("  All agents healthy.")


def run_claweval(
    suite_path: Path,
    report_path: Path,
    enable_llm_judge: bool = False,
    jobs: int = 1,
) -> bool:
    """Run claweval on a suite file, writing the report to report_path."""
    # Find claweval binary
    claweval_bin = shutil.which("claweval")
    if not claweval_bin:
        # Try cargo target
        release_bin = PROJECT_ROOT / "target" / "release" / "claweval"
        debug_bin = PROJECT_ROOT / "target" / "debug" / "claweval"
        if release_bin.exists():
            claweval_bin = str(release_bin)
        elif debug_bin.exists():
            claweval_bin = str(debug_bin)
        else:
            # Try with .exe on Windows
            release_exe = PROJECT_ROOT / "target" / "release" / "claweval.exe"
            debug_exe = PROJECT_ROOT / "target" / "debug" / "claweval.exe"
            if release_exe.exists():
                claweval_bin = str(release_exe)
            elif debug_exe.exists():
                claweval_bin = str(debug_exe)
            else:
                print("  [ERROR] claweval binary not found. Run 'cargo build --release' first.")
                return False

    cmd = [claweval_bin, "run", str(suite_path), "--out", str(report_path)]
    if enable_llm_judge:
        cmd.append("--enable-llm-judge")
    if jobs > 1:
        cmd.extend(["--jobs", str(jobs)])

    print(f"  $ {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=str(PROJECT_ROOT))
    return result.returncode == 0


def make_concrete_suite(
    template_path: Path,
    agent: dict,
    judge_config: dict | None,
    suite_metadata: dict | None,
    tmp_dir: Path,
) -> Path:
    """Deep-copy a suite template and override the backend to point at the agent's port."""
    with open(template_path) as f:
        suite = json.load(f)

    # Override backend to HTTP pointing at this agent
    suite["backend"] = {
        "type": "http",
        "url": f"http://localhost:{agent['port']}/chat",
        "session_field": "session_id",
        "message_field": "message",
        "response_field": "response",
    }

    # Override suite name to include agent
    suite["name"] = f"{suite['name']}__{agent['name']}"

    # Inject judge_backend if provided
    if judge_config:
        suite["judge_backend"] = substitute_env_vars(judge_config)

    if suite_metadata:
        md = suite_metadata.get(template_path.stem, {})
        if md.get("capability_tags") is not None:
            suite["capability_tags"] = md.get("capability_tags")
        if md.get("scoring_class") is not None:
            suite["scoring_class"] = md.get("scoring_class")

    out_path = tmp_dir / f"{agent['name']}__{template_path.stem}.json"
    with open(out_path, "w") as f:
        json.dump(suite, f, indent=2)
    return out_path


def main():
    load_dotenv(MATRIX_DIR / ".env")

    parser = argparse.ArgumentParser(description="ClawEval Matrix Runner")
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG, help="matrix.json path")
    parser.add_argument("--agents", nargs="*", help="Agent names to run (default: all enabled)")
    parser.add_argument("--suites", nargs="*", help="Suite filenames to run (default: all)")
    parser.add_argument("--no-docker", action="store_true", help="Skip docker compose up/down")
    parser.add_argument("--keep-containers", action="store_true", help="Don't stop containers after run")
    parser.add_argument("--no-build", action="store_true", help="Skip --build flag on docker compose up")
    parser.add_argument("--jobs", type=int, default=1, help="Parallel jobs for claweval")
    parser.add_argument("--enable-llm-judge", action="store_true", help="Enable LLM judge checks")
    parser.add_argument("--timeout", type=int, default=60, help="Health check timeout in seconds")
    args = parser.parse_args()

    # Load config
    config = load_config(args.config)
    all_agents = config["agents"]
    judge_config = config.get("judge_backend")
    suite_metadata = config.get("suite_metadata", {})
    suite_files = config.get("suites", [])

    # Filter agents
    if args.agents:
        agents = [a for a in all_agents if a["name"] in args.agents]
    else:
        agents = [a for a in all_agents if a.get("enabled", True)]

    if not agents:
        print("No agents selected. Check --agents or matrix.json.")
        sys.exit(1)

    # Filter suites
    if args.suites:
        suite_files = args.suites

    suite_paths = []
    for sf in suite_files:
        p = SUITES_DIR / sf
        if p.exists():
            suite_paths.append(p)
        else:
            print(f"  [WARN] Suite not found: {p}")

    if not suite_paths:
        print("No suites found. Check --suites or matrix.json.")
        sys.exit(1)

    print(f"Matrix: {len(agents)} agents x {len(suite_paths)} suites = {len(agents) * len(suite_paths)} runs")
    print(f"  Agents: {', '.join(a['name'] for a in agents)}")
    print(f"  Suites: {', '.join(p.stem for p in suite_paths)}")

    # Create run directory for reports
    run_id = datetime.now().strftime("%Y%m%d_%H%M%S")
    reports_dir = PROJECT_ROOT / "reports" / "matrix" / f"run_{run_id}"
    reports_dir.mkdir(parents=True, exist_ok=True)
    print(f"  Reports: {reports_dir}")

    global_sidecars = config.get("options", {}).get("global_sidecars", [])

    # Docker compose up
    if not args.no_docker:
        start_containers(agents, build=not args.no_build, global_sidecars=global_sidecars)
        wait_for_health(agents, timeout=args.timeout)

    # Run matrix
    results = {}
    with tempfile.TemporaryDirectory(prefix="claweval_matrix_") as tmp_dir:
        tmp_path = Path(tmp_dir)

        for agent in agents:
            agent_reports = []
            for suite_path in suite_paths:
                combo_name = f"{agent['name']}__{suite_path.stem}"
                print(f"\n=== Running: {combo_name} ===")

                concrete_suite = make_concrete_suite(
                    suite_path, agent, judge_config, suite_metadata, tmp_path
                )
                report_path = reports_dir / f"{combo_name}.json"

                success = run_claweval(
                    concrete_suite,
                    report_path,
                    enable_llm_judge=args.enable_llm_judge,
                    jobs=args.jobs,
                )

                status = "PASS" if success else "FAIL"
                print(f"  Result: {status}  Report: {report_path}")
                agent_reports.append({
                    "suite": suite_path.stem,
                    "report": str(report_path),
                    "success": success,
                })

            results[agent["name"]] = agent_reports

    # Aggregate reports
    print("\n=== Aggregating reports ===")
    aggregator = MATRIX_DIR / "report_aggregator.py"
    if aggregator.exists():
        subprocess.run(
            [sys.executable, str(aggregator), str(reports_dir)],
            cwd=str(PROJECT_ROOT),
        )
    else:
        print("  [WARN] report_aggregator.py not found, skipping aggregation")

    # Docker compose down
    if not args.no_docker and not args.keep_containers:
        stop_containers()

    # Summary
    print("\n=== Matrix Complete ===")
    for agent_name, reports in results.items():
        passed = sum(1 for r in reports if r["success"])
        total = len(reports)
        print(f"  {agent_name}: {passed}/{total} suite runs succeeded")
    print(f"  Reports: {reports_dir}")


if __name__ == "__main__":
    main()
