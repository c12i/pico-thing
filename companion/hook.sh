#!/bin/sh
# Claude Code hook: forward the event JSON (stdin) to the pico-thing companion.
# Never blocks or fails Claude: short timeout, output discarded, always exits 0.
DIR=$(cd "$(dirname "$0")" && pwd)
TOKEN=$(sed -n 's/.*"token": *"\([0-9a-f]*\)".*/\1/p' "$DIR/config.json" 2>/dev/null)
PORT=$(sed -n 's/.*"port": *\([0-9]*\).*/\1/p' "$DIR/config.json" 2>/dev/null | head -1)
curl -s -m 2 -o /dev/null -X POST \
    -H "X-Token: $TOKEN" -H "X-Term: ${TERM_PROGRAM:-}" \
    --data-binary @- "http://127.0.0.1:${PORT:-8765}/hook" 2>/dev/null
exit 0
