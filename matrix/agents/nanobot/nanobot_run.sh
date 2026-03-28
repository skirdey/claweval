#!/bin/sh
# Wrapper: run nanobot agent, strip branding header, init messages, and blank lines.
nanobot agent -m "$1" -s "eval:$2" --no-markdown 2>/dev/null \
  | sed '/^$/d; /nanobot$/d; /^  Created /d; /^Hint:/d; /^[*][*]/d'
