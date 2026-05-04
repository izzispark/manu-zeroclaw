#!/bin/bash
set -e

OVERRIDES_DIR="/zeroclaw-data/.zeroclaw/overrides"

if [ -d "$OVERRIDES_DIR" ] && [ "$(ls -A $OVERRIDES_DIR 2>/dev/null)" ]; then
    echo "[startup] Applying config overlays from $OVERRIDES_DIR"
    zeroclaw-apply-overrides \
        --base /zeroclaw-data/.zeroclaw/config.toml \
        --overrides-dir "$OVERRIDES_DIR"
    echo "[startup] Overlays applied"
else
    echo "[startup] No override files found, starting daemon directly"
fi

exec zeroclaw daemon "$@"
