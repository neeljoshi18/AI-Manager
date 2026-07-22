# Build egress-proxy
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config ./config
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/egress-proxy /usr/local/bin/egress-proxy
COPY config /app/config
# Mount real secrets at runtime; example only for image defaults
COPY secrets/dev_secrets.example.json /app/secrets/dev_secrets.example.json

ENV EGRESS_BIND=0.0.0.0:18090 \
    TOOL_REGISTRY=/app/config/tool_registry.yaml \
    SECRETS_FILE=/app/secrets/dev_secrets.json

EXPOSE 18090
CMD ["egress-proxy"]
