#!/usr/bin/env python3
"""A tiny deterministic agent for exercising ClawEval without OpenClaw.

Implements a minimal interface:
  fake_agent.py --session <id> --message <text>

It prints a response to stdout.
"""

import argparse
import json
import re


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--session", required=True)
    ap.add_argument("--message", required=True)
    args = ap.parse_args()

    msg = args.message.strip()

    if "single word PONG" in msg or msg.endswith("PONG.") or "Reply with exactly the single word PONG" in msg:
        print("PONG")
        return

    m = re.search(r"Remember this codeword exactly:\s*([A-Z0-9\-]+)", msg)
    if m:
        print("OK")
        return

    if "What codeword" in msg:
        # In a real agent this would use session memory; here we hard-code for the sample suite.
        print("TANGERINE-742")
        return

    if "Output ONLY valid JSON" in msg:
        print('{"ok":true,"n":3}')
        return

    if "Draft a polite 1-sentence reply" in msg:
        print("Sure—I'll send the deck to you by EOD.")
        return

    # default
    print("OK")


if __name__ == "__main__":
    main()
