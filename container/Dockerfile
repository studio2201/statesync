# Stage 1: static musl binary (zero glibc dependency at runtime)
FROM docker.io/clux/muslrust:stable AS builder
WORKDIR /usr/src/statesync
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Alpine Linux runtime (NOT scratch; NOT UBI unless compliance requires it).
#
# Unraid console:  docker exec -it statesync sh
# Shell is BusyBox ash with essential utilities (wget for healthcheck).
#
# Permission model for the persistent /config volume:
#   - ENTRYPOINT runs as root, chowns /config to PUID:PGID (default 99:100),
#     then exec's the daemon via su-exec. No required user env vars.
#   - Daemon binds 0.0.0.0:4601 (ports >= 1024 need no root).
FROM alpine:3.20

# Keep ash + core BusyBox tools; add only essentials for health + TLS + Unraid.
RUN apk add --no-cache \
        ca-certificates \
        su-exec \
        tzdata \
        wget \
    && mkdir -p /config /app \
    && test -x /bin/sh \
    && (test -x /bin/ash || ln -sf /bin/busybox /bin/ash)

WORKDIR /app
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /usr/local/bin/statesync
COPY container/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/statesync

# Zero required environment variables at runtime.
ENV RUST_LOG=info \
    TZ=UTC \
    PUID=99 \
    PGID=100 \
    UMASK=022

EXPOSE 4601
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD wget -qO- http://127.0.0.1:4601/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
