# Stage 1: Build the static Rust application
FROM blackdex/rust-musl:1.85.0 as builder
WORKDIR /usr/src/statesync
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Final runtime container using RedHat UBI-minimal
FROM registry.access.redhat.com/ubi9/ubi-minimal:9.4
WORKDIR /app
COPY --from=builder /usr/src/statesync/target/x86_64-unknown-linux-musl/release/statesync /app/statesync
RUN microdnf install -y tzdata && microdnf clean all
ENV RUST_LOG=info
EXPOSE 8754
CMD ["/app/statesync"]
