#!/bin/sh
# Wrapper: run picoclaw agent, strip ANSI codes, ASCII art, log lines, emoji, and blank lines.
picoclaw agent -m "$1" -s "$2" 2>/dev/null \
  | sed 's/\x1b\[[0-9;]*m//g' \
  | tr -d '\r' \
  | grep -v '██' \
  | grep -v '╔\|╗\|╚\|╝\|║\|═' \
  | grep -v ' INF ' \
  | grep -v ' WRN ' \
  | grep -v ' ERR ' \
  | grep -v ' DBG ' \
  | sed 's/^🦞 //' \
  | sed '/^$/d'
