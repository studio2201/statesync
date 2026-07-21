#!/bin/sh
# Entrypoint (BusyBox ash). Zero required env vars:
#   PUID=99 PGID=100 UMASK=022 by default (Unraid nobody:users).
#
# 1. Apply UMASK
# 2. chown /config and /app (silent on read-only mounts)
# 3. exec daemon as PUID:PGID via su-exec
#
# Unraid console: docker exec -it statesync sh

set -e

PUID=${PUID:-99}
PGID=${PGID:-100}
UMASK=${UMASK:-022}

umask "$UMASK"

chown -R "$PUID:$PGID" /config 2>/dev/null || true
chown -R "$PUID:$PGID" /app 2>/dev/null || true

chmod +x /usr/local/bin/statesync

# Dashboard auth is intentionally disabled — no token / sign-in.
unset STATESYNC_WEB_AUTH

exec su-exec "$PUID:$PGID" /usr/local/bin/statesync "$@"
