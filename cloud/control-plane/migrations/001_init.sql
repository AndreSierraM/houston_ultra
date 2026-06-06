-- Houston Cloud control plane schema (MVP).

CREATE TABLE IF NOT EXISTS organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS organization_members (
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);

CREATE TABLE IF NOT EXISTS cloud_entitlements (
    org_id UUID PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('active', 'past_due', 'canceled')),
    max_cloud_agents INT NOT NULL DEFAULT 4,
    max_storage_gb INT NOT NULL DEFAULT 10,
    max_members INT NOT NULL DEFAULT 5,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS cloud_agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL,
    name TEXT NOT NULL,
    config_id TEXT NOT NULL,
    color TEXT,
    folder_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_opened_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS cloud_agents_org_idx ON cloud_agents(org_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS cloud_agents_owner_idx ON cloud_agents(owner_user_id) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS cloud_agent_runtimes (
    agent_id UUID PRIMARY KEY REFERENCES cloud_agents(id) ON DELETE CASCADE,
    container_name TEXT NOT NULL,
    internal_url TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    engine_token TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('provisioning', 'running', 'stopped', 'error')),
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS cloud_agent_shares (
    agent_id UUID NOT NULL REFERENCES cloud_agents(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('viewer', 'operator', 'admin')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (agent_id, user_id)
);

CREATE TABLE IF NOT EXISTS audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID,
    agent_id UUID,
    user_id UUID,
    action TEXT NOT NULL,
    detail JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS audit_events_agent_idx ON audit_events(agent_id, created_at DESC);

-- Local k3d principal (HOUSTON_CLOUD_LOCAL_USER_ID default). Idempotent.
INSERT INTO organizations (id, name)
VALUES ('00000000-0000-0000-0000-000000000010', 'dev@local.test''s org')
ON CONFLICT (id) DO NOTHING;

INSERT INTO organization_members (org_id, user_id, role)
VALUES (
    '00000000-0000-0000-0000-000000000010',
    '00000000-0000-0000-0000-000000000001',
    'owner'
)
ON CONFLICT (org_id, user_id) DO NOTHING;

INSERT INTO cloud_entitlements (org_id, status, max_cloud_agents, max_storage_gb, max_members)
VALUES ('00000000-0000-0000-0000-000000000010', 'active', 4, 10, 5)
ON CONFLICT (org_id) DO NOTHING;
