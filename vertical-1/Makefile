.PHONY: build test verify unit fmt clippy run-ingest run-ingest-prod run-query docker-up docker-down clean

export PATH := $(HOME)/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$(PATH)

build:
	cargo build --workspace

release:
	cargo build --workspace --release

unit:
	cargo test -p telemetry-core --lib -- --nocapture

verify:
	cargo run -p telemetry-verify -- --load-count 2000 --replay-count 5000 --workers 32

test: unit verify

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

run-ingest:
	SKIP_AUTH=true RUNTIME_MODE=embedded cargo run -p telemetry-ingestion

# Requires: docker compose up -d  and .env (see .env.example)
run-ingest-prod:
	set -a; [ -f .env ] && . ./.env; set +a; \
	SKIP_AUTH=$${SKIP_AUTH:-true} RUNTIME_MODE=production cargo run -p telemetry-ingestion

run-query:
	RUNTIME_MODE=embedded cargo run -p telemetry-query

docker-up:
	docker compose up -d
	@echo "Applying migrations…"
	@docker compose exec -T cockroach ./cockroach sql --insecure < migrations/cockroach/001_init.sql || true
	@docker compose exec -T clickhouse clickhouse-client --password vertical1 --multiquery < migrations/clickhouse/001_init.sql || true

docker-down:
	docker compose down -v

clean:
	cargo clean
