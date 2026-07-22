-- Vertical 3: status twins, ledgers, draft/veto state, publish records
-- Apply against database `status_twins` on the shared Cockroach cluster:
--   CREATE DATABASE IF NOT EXISTS status_twins;
--   cockroach sql --insecure -d status_twins < migrations/cockroach/001_init.sql

CREATE TABLE IF NOT EXISTS twin (
    tenant_id         STRING NOT NULL,
    twin_id           STRING NOT NULL,  -- twin:person:gu_… or twin:team:…
    twin_kind         STRING NOT NULL,  -- person|team
    subject_id        STRING NOT NULL,  -- global_user_id or team node_id
    display_name      STRING NOT NULL DEFAULT '',
    timezone          STRING NOT NULL DEFAULT 'UTC',
    channel_id        STRING NOT NULL DEFAULT '',  -- Slack channel for publish
    shadow_until      TIMESTAMPTZ NULL,
    high_auto_publish BOOL NOT NULL DEFAULT false,
    enabled           BOOL NOT NULL DEFAULT true,
    config_json       JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, twin_id)
);

CREATE TABLE IF NOT EXISTS slack_user_map (
    tenant_id       STRING NOT NULL,
    global_user_id  STRING NOT NULL,
    slack_user_id   STRING NOT NULL,
    slack_team_id   STRING NOT NULL DEFAULT '',
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, global_user_id),
    UNIQUE (tenant_id, slack_user_id)
);

CREATE TABLE IF NOT EXISTS ledger_snapshot (
    tenant_id          STRING NOT NULL,
    ledger_id          STRING NOT NULL,
    twin_id            STRING NOT NULL,
    period_start       TIMESTAMPTZ NOT NULL,
    period_end         TIMESTAMPTZ NOT NULL,
    confidence_rollup  STRING NOT NULL,  -- high|medium|blocker
    ledger_json        JSONB NOT NULL,
    graph_as_of        TIMESTAMPTZ NOT NULL,
    compiled_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, ledger_id),
    UNIQUE (tenant_id, twin_id, period_start, period_end)
);

CREATE TABLE IF NOT EXISTS draft_delivery (
    tenant_id        STRING NOT NULL,
    draft_id         STRING NOT NULL,
    ledger_id        STRING NOT NULL,
    twin_id          STRING NOT NULL,
    status           STRING NOT NULL,
    -- shadow|pending|edited|vetoed|publish_queued|published|expired|force_human
    slack_dm_channel STRING NOT NULL DEFAULT '',
    slack_dm_ts      STRING NOT NULL DEFAULT '',
    draft_text       STRING NOT NULL DEFAULT '',
    edited_text      STRING NULL,
    veto_deadline    TIMESTAMPTZ NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, draft_id),
    UNIQUE (tenant_id, ledger_id)
);

CREATE TABLE IF NOT EXISTS publish_record (
    tenant_id    STRING NOT NULL,
    publish_id   STRING NOT NULL,
    ledger_id    STRING NOT NULL,
    draft_id     STRING NOT NULL,
    channel_id   STRING NOT NULL,
    slack_ts     STRING NOT NULL DEFAULT '',
    body_hash    STRING NOT NULL,
    published_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, publish_id),
    UNIQUE (tenant_id, ledger_id)
);

CREATE TABLE IF NOT EXISTS compile_run (
    tenant_id   STRING NOT NULL,
    run_id      STRING NOT NULL,
    twin_id     STRING NOT NULL,
    status      STRING NOT NULL, -- ok|error|skipped_shadow
    error_text  STRING NOT NULL DEFAULT '',
    started_at  TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NULL,
    PRIMARY KEY (tenant_id, run_id)
);

CREATE INDEX IF NOT EXISTS idx_ledger_twin ON ledger_snapshot (tenant_id, twin_id, compiled_at DESC);
CREATE INDEX IF NOT EXISTS idx_draft_status ON draft_delivery (tenant_id, status, veto_deadline);
CREATE INDEX IF NOT EXISTS idx_publish_ledger ON publish_record (tenant_id, ledger_id);
