# Stage 1: Build the static Rust application using Musl target
FROM clux/muslrust:1.85.0-stable as builder
WORKDIR /usr/src/statesync
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Scratch runtime container (Zero Attack Surface)
FROM scratch
WORKDIR /app
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /app/statesync
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /bin/sh
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /bin/bash
COPY --from=builder /usr/share/zoneinfo /usr/share/zoneinfo
ENV RUST_LOG=info
ENV TZ=UTC
EXPOSE 8754
ENTRYPOINT ["/app/statesync"]
