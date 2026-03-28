#!/bin/sh
# Multi-turn session-aware wrapper for hermes.
# claweval passes the same {session_id} for all steps in an episode.
# On first call we create a new hermes session; on subsequent calls we resume it.

MESSAGE="$1"
CLAWEVAL_SID="$2"
SESSION_MAP="/tmp/hermes_sessions"
mkdir -p "$SESSION_MAP"

HERMES_SID_FILE="$SESSION_MAP/$CLAWEVAL_SID"

if [ -f "$HERMES_SID_FILE" ]; then
  # Resume existing hermes session
  HERMES_SID=$(cat "$HERMES_SID_FILE")
  RAW=$(hermes chat -q "$MESSAGE" -Q --resume "$HERMES_SID" --model "$HERMES_MODEL" --provider openrouter --yolo 2>/dev/null)
else
  # First message — new session
  RAW=$(hermes chat -q "$MESSAGE" -Q --model "$HERMES_MODEL" --provider openrouter --yolo 2>/dev/null)
  # Extract and save hermes session ID for future calls
  HERMES_SID=$(echo "$RAW" | tr -d '\r' | grep '^session_id:' | tail -1 | sed 's/^session_id: *//')
  if [ -n "$HERMES_SID" ]; then
    echo "$HERMES_SID" > "$HERMES_SID_FILE"
  fi
fi

# Clean output: strip carriage returns, banner, session_id line, blank lines
echo "$RAW" \
  | tr -d '\r' \
  | grep -v 'Hermes' \
  | grep -v '^session_id:' \
  | grep -v 'preparing' \
  | grep -v 'recall ' \
  | grep -v 'memory ' \
  | grep -v 'Resumed session' \
  | sed '/^$/d'
