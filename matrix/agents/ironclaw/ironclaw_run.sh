#!/bin/bash
# Wrapper: run ironclaw in single-message mode.
# ironclaw -m doesn't exit after responding, so we use timeout to force it.
# Decorative output (Processing..., separator) goes to stderr, suppressed.
timeout 30 ironclaw --no-onboard -m "$1" 2>/dev/null
