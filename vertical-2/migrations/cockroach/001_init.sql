-- Vertical 2 Organizational Context Graph
-- Apply against database `context_graph` (create DB first if needed).

CREATE TABLE IF NOT EXISTS graph_node (
    tenant_id         STRING NOT NULL,
    node_id           STRING NOT NULL,
    node_type         STRING NOT NULL,
    display_name      STRING NOT NULL DEFAULT '',
    resource_id       STRING NOT NULL DEFAULT '',
    properties_json   JSONB NOT NULL DEFAULT '{}'::JSONB,
    is_private        BOOL NOT NULL DEFAULT false,
    allowed_group_ids STRING[] NOT NULL DEFAULT ARRAY[],
    acl_version       INT8 NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, node_id)
);

CREATE INDEX IF NOT EXISTS idx_node_type ON graph_node (tenant_id, node_type);
CREATE INDEX IF NOT EXISTS idx_node_resource ON graph_node (tenant_id, resource_id);

CREATE TABLE IF NOT EXISTS graph_edge (
    tenant_id         STRING NOT NULL,
    edge_id           STRING NOT NULL,
    edge_type         STRING NOT NULL,
    from_node_id      STRING NOT NULL,
    to_node_id        STRING NOT NULL,
    valid_from        TIMESTAMPTZ NOT NULL,
    valid_to          TIMESTAMPTZ NULL,
    event_id          STRING NOT NULL,
    properties_json   JSONB NOT NULL DEFAULT '{}'::JSONB,
    is_private        BOOL NOT NULL DEFAULT false,
    allowed_group_ids STRING[] NOT NULL DEFAULT ARRAY[],
    acl_version       INT8 NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, edge_id)
);

CREATE INDEX IF NOT EXISTS idx_edge_from ON graph_edge (tenant_id, from_node_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edge_to ON graph_edge (tenant_id, to_node_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edge_event ON graph_edge (tenant_id, event_id);

CREATE TABLE IF NOT EXISTS entity_state (
    tenant_id         STRING NOT NULL,
    node_id           STRING NOT NULL,
    state_key         STRING NOT NULL,
    state_value       STRING NOT NULL,
    as_of             TIMESTAMPTZ NOT NULL,
    event_id          STRING NOT NULL,
    is_private        BOOL NOT NULL DEFAULT false,
    allowed_group_ids STRING[] NOT NULL DEFAULT ARRAY[],
    PRIMARY KEY (tenant_id, node_id, state_key)
);

CREATE TABLE IF NOT EXISTS projector_offsets (
    consumer_group STRING NOT NULL,
    topic          STRING NOT NULL,
    partition_id   INT8 NOT NULL,
    next_offset    INT8 NOT NULL,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (consumer_group, topic, partition_id)
);

CREATE TABLE IF NOT EXISTS projector_applied_events (
    tenant_id  STRING NOT NULL,
    event_id   STRING NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, event_id)
);

CREATE TABLE IF NOT EXISTS user_membership (
    tenant_id       STRING NOT NULL,
    global_user_id  STRING NOT NULL,
    group_id        STRING NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, global_user_id, group_id)
);

CREATE INDEX IF NOT EXISTS idx_membership_user ON user_membership (tenant_id, global_user_id);
