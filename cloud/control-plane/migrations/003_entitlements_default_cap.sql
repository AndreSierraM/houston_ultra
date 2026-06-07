-- Cap orgs still on legacy unlimited dev defaults. Idempotent on every boot.
UPDATE cloud_entitlements
SET max_cloud_agents = 5,
    max_storage_gb = GREATEST(max_storage_gb, 50),
    updated_at = now()
WHERE max_cloud_agents IN (4, 8, 100000);
