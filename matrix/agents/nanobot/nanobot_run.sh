#!/bin/sh
# Wrapper: run nanobot agent and strip branding header (empty line + "🐈 nanobot").
nanobot agent -m "$1" -s "eval:$2" --no-markdown 2>/dev/null | sed '/^$/d; /nanobot$/d'
