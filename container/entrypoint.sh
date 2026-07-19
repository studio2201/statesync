#!/bin/sh
# Entrypoint wrapper for the statesync container.
#
# Fixes ownership of the persistent /config volume before the daemon
# starts (best-effort). This handles the common case where the host-side
# directory was created as root (UID 0), which prevents the non-root
# 'statesync' user inside the container from writing to it.
#
# The binary itself also handles this case (falls back to /app/config.json
# then in-memory), but fixing it here means the user's intended mount
# path actually works.

set -e

# Only attempt the chown if /config exists as a directory and we have
# permission to modify it (i.e. running as root before USER statesync).
if [ -d /config ]; then
    if command -v chown >/dev/null 2>&1; then
        # Best-effort: ignore failures (read-only mount, etc.). The
        # daemon will surface a clearer warning if writes still fail.
        chown -R statesync:statesync /config 2>/dev/null || true
    fi
    # Also ensure the directory itself exists if the volume mount is empty.
    if [ ! -d /config ]; then
        mkdir -p /config 2>/dev/null || true
    fi
fi

exec /usr/local/bin/statesync "$@"