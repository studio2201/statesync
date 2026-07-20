# Stage 1: Build the static Rust application using Musl target
FROM docker.io/clux/muslrust:stable AS builder
WORKDIR /usr/src/statesync
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Alpine Linux runtime container.
#
# Permission model for the persistent /config volume:
#   - Container ENTRYPOINT runs as root (Docker default). It chowns
#     /config and /app to nobody:nogroup (uid 65534, Unraid's appdata
#     convention), then exec's the daemon as nobody.
#   - Daemon itself runs as nobody. It can write to /config (now owned
#     by nobody) and /app. Binds to 0.0.0.0:4407 (no root needed for
#     ports >= 1024).
#   - If /config is on a read-only mount, chown fails and the daemon
#     falls back to /app/config.json (ephemeral), then to an in-memory
#     default, each with a clear warning logged.
#   - No user intervention is ever required.
FROM alpine:3.20
RUN apk add --no-cache tzdata wget su-exec && \
    mkdir -p /config /app
WORKDIR /app
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /usr/local/bin/statesync
COPY container/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/statesync
ENV RUST_LOG=info
ENV TZ=UTC
EXPOSE 4407
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD wget -qO- http://127.0.0.1:4407/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]