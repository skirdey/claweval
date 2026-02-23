#!/bin/sh
# Wrapper: run picoclaw agent and strip the "🦞 " logo prefix.
picoclaw agent -m "$1" -s "$2" 2>/dev/null | sed 's/^🦞 //'
