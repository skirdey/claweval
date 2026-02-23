#!/bin/sh
# Inject OPENROUTER_API_KEY into nanobot config at startup
if [ -n "$OPENROUTER_API_KEY" ]; then
  python3 -c "
import json, os
cfg = json.load(open('$HOME/.nanobot/config.json'))
cfg.setdefault('providers', {}).setdefault('openrouter', {})['apiKey'] = os.environ['OPENROUTER_API_KEY']
json.dump(cfg, open('$HOME/.nanobot/config.json', 'w'), indent=2)
"
fi
exec python /app/adapter.py
