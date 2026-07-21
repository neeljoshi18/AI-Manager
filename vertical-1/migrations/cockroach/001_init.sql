-- Vertical 1 ACL + identity mapping store (Spec §3.3)
-- CockroachDB (Postgres wire protocol).

CREATE TABLE IF NOT EXISTS tenants (
    tenant_id STRING PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    config_json JSONB NOT NULL DEFAULT '{}'::JSONB
);

-- UserMap: tenant_id ∥ provider_user_id → global_user_id
CREATE TABLE IF NOT EXISTS user_identity_map (
    tenant_id STRING NOT NULL,
    provider STRING NOT NULL,
    provider_user_id STRING NOT NULL,
    global_user_id STRING NOT NULL,
    email STRING NOT NULL DEFAULT '',
    display_name STRING NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, provider, provider_user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_identity_global
    ON user_identity_map (tenant_id, global_user_id);

-- GroupMap: global_user_id → {group_id...}
CREATE TABLE IF NOT EXISTS user_group_membership (
    tenant_id STRING NOT NULL,
    global_user_id STRING NOT NULL,
    group_id STRING NOT NULL,
    acl_version INT8 NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, global_user_id, group_id)
);

CREATE INDEX IF NOT EXISTS idx_group_members
    ON user_group_membership (tenant_id, group_id);

-- Monotonic ACL version per tenant (revocation ordering).
CREATE TABLE IF NOT EXISTS tenant_acl_version (
    tenant_id STRING PRIMARY KEY,
    acl_version INT8 NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Resource → allow-list mapping (repo teams, project roles).
CREATE TABLE IF NOT EXISTS resource_acl (
    tenant_id STRING NOT NULL,
    resource_id STRING NOT NULL,
    is_private BOOL NOT NULL DEFAULT true,
    allowed_group_ids STRING[] NOT NULL DEFAULT ARRAY[],
    acl_version INT8 NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, resource_id)
);

-- Audit log of ACL revocations for debugging <200ms sync.
CREATE TABLE IF NOT EXISTS acl_revocation_audit (
    event_id STRING PRIMARY KEY,
    tenant_id STRING NOT NULL,
    global_user_id STRING NOT NULL,
    group_id STRING NOT NULL,
    change_type STRING NOT NULL,
    acl_version INT8 NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
