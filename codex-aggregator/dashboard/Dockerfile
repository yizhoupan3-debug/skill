FROM rust:1.85-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml .
COPY src src
COPY dashboard/static dashboard/static
RUN cargo build --release --bin codex-dashboard

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/codex-dashboard /usr/local/bin/codex-dashboard
EXPOSE 5000
CMD ["codex-dashboard"]
