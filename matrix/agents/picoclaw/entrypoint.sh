#!/bin/sh
# Inject OPENROUTER_API_KEY into picoclaw model_list at startup
if [ -n "$OPENROUTER_API_KEY" ]; then
  python3 -c "
import json, os
cfg = json.load(open('$HOME/.picoclaw/config.json'))
for m in cfg.get('model_list', []):
    m['api_key'] = os.environ['OPENROUTER_API_KEY']
json.dump(cfg, open('$HOME/.picoclaw/config.json', 'w'), indent=2)
"
fi
exec python /app/adapter.py
