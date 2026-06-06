-- Worker burst: parent link, TTL, runtime mode on cloud_agents.

ALTER TABLE cloud_agents
    ADD COLUMN IF NOT EXISTS parent_agent_id UUID REFERENCES cloud_agents(id) ON DELETE SET NULL;

ALTER TABLE cloud_agents
    ADD COLUMN IF NOT EXISTS worker_ttl_seconds INT;

ALTER TABLE cloud_agents
    ADD COLUMN IF NOT EXISTS runtime_mode TEXT NOT NULL DEFAULT 'cloud_24_7';

ALTER TABLE cloud_agents DROP CONSTRAINT IF EXISTS cloud_agents_runtime_mode_check;

ALTER TABLE cloud_agents
    ADD CONSTRAINT cloud_agents_runtime_mode_check
    CHECK (runtime_mode IN ('cloud_24_7', 'cloud_on_demand', 'cloud_worker'));

CREATE INDEX IF NOT EXISTS cloud_agents_parent_idx
    ON cloud_agents(parent_agent_id)
    WHERE deleted_at IS NULL;
