#!/bin/sh
# Entrypoint for the statesync container.
#
# Runs as root (Docker default for ENTRYPOINT).
#  1. Applies the user-configured UMASK
#  2. Chowns /config and /app to PUID:PGID (default 99:100, Unraid's
#     'nobody' user). Fails silently on read-only mounts; the daemon
#     falls back to /app/config.json in that case.
#  3. Execs the daemon as PUID:PGID via su-exec.

set -e

PUID=${PUID:-99}
PGID=${PGID:-100}
UMASK=${UMASK:-022}

umask "$UMASK"

chown -R "$PUID:$PGID" /config 2>/dev/null || true
chown -R "$PUID:$PGID" /app 2>/dev/null || true

chmod +x /usr/local/bin/statesync

# Dashboard auth is intentionally disabled — no token / sign-in.
# Clear any leftover auto-generated token so restarts stay open.
unset STATESYNC_WEB_AUTH

exec su-exec "$PUID:$PGID" /usr/local/bin/statesync "$@"
