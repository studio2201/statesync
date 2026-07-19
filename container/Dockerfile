# Stage 1: Build the static Rust application using Musl target
FROM docker.io/clux/muslrust:stable AS builder
WORKDIR /usr/src/statesync
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Alpine Linux runtime container (Safe, lightweight, shell and TUI support)
FROM alpine:3.20
RUN apk add --no-cache tzdata wget && \
    addgroup -S -g 1000 statesync && \
    adduser -S -u 1000 -G statesync statesync
WORKDIR /app
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /usr/local/bin/statesync
COPY container/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chown -R statesync:statesync /usr/local/bin /app && \
    chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/statesync
USER statesync
ENV RUST_LOG=info
ENV TZ=UTC
EXPOSE 8754
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD wget -qO- http://127.0.0.1:8754/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
