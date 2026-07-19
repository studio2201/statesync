# Stage 1: Build the static Rust application using Musl target
FROM clux/muslrust:1.85.0-stable as builder
WORKDIR /usr/src/statesync
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Alpine Linux runtime container (Safe, lightweight, shell and TUI support)
FROM alpine:3.20
RUN apk add --no-cache tzdata
WORKDIR /app
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /app/statesync
ENV RUST_LOG=info
ENV TZ=UTC
EXPOSE 8754
ENTRYPOINT ["/app/statesync"]
