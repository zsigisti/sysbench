#!/usr/bin/env sh
# Fetch the CRUCIBLE score-server build runbook (web.md) to the current dir,
# then hand it to an AI coding agent (e.g. `claude`) to stand up the server.
#
# One-liner (no clone needed):
#   curl -fsSL https://raw.githubusercontent.com/zsigisti/crucible/main/web.md -o web.md
set -eu
URL="${CRUX_WEB_MD_URL:-https://raw.githubusercontent.com/zsigisti/crucible/main/web.md}"
OUT="${1:-web.md}"
echo "Fetching $URL -> $OUT"
curl -fsSL "$URL" -o "$OUT"
echo "Saved $OUT. Next: open it with your coding agent, e.g.  claude \"follow $OUT\""
