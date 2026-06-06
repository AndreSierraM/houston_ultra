//! Kubernetes manifest builders for per-agent engine runtimes.

use uuid::Uuid;

/// Must match `HOUSTON_UID` / `HOUSTON_GID` in `always-on/Dockerfile`.
pub const ENGINE_RUN_AS_UID: u32 = 10000;
pub const ENGINE_RUN_AS_GID: u32 = 10000;

pub fn org_namespace(org_id: Uuid) -> String {
    format!("hou-org-{org_id}")
}

pub fn agent_deployment_name(agent_id: Uuid) -> String {
    format!("hou-cloud-agent-{agent_id}")
}

pub fn agent_pvc_name(agent_id: Uuid) -> String {
    format!("hou-cloud-agent-{agent_id}-home")
}

pub fn agent_secret_name(agent_id: Uuid) -> String {
    format!("hou-cloud-agent-{agent_id}-token")
}

pub fn internal_service_url(agent_id: Uuid, org_id: Uuid) -> String {
    let ns = org_namespace(org_id);
    let name = agent_deployment_name(agent_id);
    format!("http://{name}.{ns}.svc.cluster.local:7777")
}

const PER_AGENT_CPU_REQUEST_MILLI: i32 = 250;
const PER_AGENT_MEM_REQUEST_MIB: i32 = 512;
const PER_AGENT_CPU_LIMIT_CORES: i32 = 2;
const PER_AGENT_MEM_LIMIT_GIB: i32 = 2;

fn milli_cpu_total(max_agents: i32) -> String {
    format!("{}m", max_agents.saturating_mul(PER_AGENT_CPU_REQUEST_MILLI))
}

fn mebi_total(max_agents: i32, per_agent_mib: i32) -> String {
    format!("{}Mi", max_agents.saturating_mul(per_agent_mib))
}

fn cpu_cores_total(max_agents: i32, per_agent_cores: i32) -> String {
    max_agents.saturating_mul(per_agent_cores).to_string()
}

fn gib_total(max_agents: i32, per_agent_gib: i32) -> String {
    format!("{}Gi", max_agents.saturating_mul(per_agent_gib))
}

pub fn org_quota_manifests(org_id: Uuid, max_cloud_agents: i32, max_storage_gb: i32) -> String {
    let ns = org_namespace(org_id);
    let req_cpu = milli_cpu_total(max_cloud_agents);
    let req_mem = mebi_total(max_cloud_agents, PER_AGENT_MEM_REQUEST_MIB);
    let lim_cpu = cpu_cores_total(max_cloud_agents, PER_AGENT_CPU_LIMIT_CORES);
    let lim_mem = gib_total(max_cloud_agents, PER_AGENT_MEM_LIMIT_GIB);
    format!(
        r#"apiVersion: v1
kind: ResourceQuota
metadata:
  name: hou-org-quota
  namespace: {ns}
spec:
  hard:
    requests.cpu: {req_cpu}
    requests.memory: {req_mem}
    limits.cpu: {lim_cpu}
    limits.memory: {lim_mem}
    persistentvolumeclaims: "{max_cloud_agents}"
    requests.storage: {max_storage_gb}Gi
---
apiVersion: v1
kind: LimitRange
metadata:
  name: hou-org-limits
  namespace: {ns}
spec:
  limits:
    - type: Container
      default:
        cpu: "2"
        memory: 2Gi
      defaultRequest:
        cpu: 250m
        memory: 512Mi
"#
    )
}

pub fn namespace_manifest(org_id: Uuid) -> String {
    let ns = org_namespace(org_id);
    format!(
        r#"apiVersion: v1
kind: Namespace
metadata:
  name: {ns}
  labels:
    houston.ai/managed: "true"
    houston.ai/org-id: "{org_id}"
"#
    )
}

pub fn agent_pvc_manifest(agent_id: Uuid, org_id: Uuid) -> String {
    let ns = org_namespace(org_id);
    let pvc = agent_pvc_name(agent_id);
    format!(
        r#"apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: {pvc}
  namespace: {ns}
  labels:
    houston.ai/agent-id: "{agent_id}"
spec:
  accessModes: [ReadWriteOnce]
  resources:
    requests:
      storage: 10Gi
"#
    )
}

pub fn agent_workload_manifests(
    agent_id: Uuid,
    org_id: Uuid,
    engine_image: &str,
    engine_token: &str,
) -> String {
    let ns = org_namespace(org_id);
    let deploy = agent_deployment_name(agent_id);
    let pvc = agent_pvc_name(agent_id);
    let secret = agent_secret_name(agent_id);
    format!(
        r#"apiVersion: v1
kind: Secret
metadata:
  name: {secret}
  namespace: {ns}
type: Opaque
stringData:
  HOUSTON_ENGINE_TOKEN: "{engine_token}"
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: {deploy}
  namespace: {ns}
  labels:
    houston.ai/agent-id: "{agent_id}"
spec:
  replicas: 1
  progressDeadlineSeconds: 300
  minReadySeconds: 0
  selector:
    matchLabels:
      app: {deploy}
  template:
    metadata:
      labels:
        app: {deploy}
        houston.ai/agent-id: "{agent_id}"
    spec:
      securityContext:
        fsGroup: {ENGINE_RUN_AS_GID}
        fsGroupChangePolicy: OnRootMismatch
      containers:
        - name: engine
          image: {engine_image}
          imagePullPolicy: IfNotPresent
          securityContext:
            runAsUser: {ENGINE_RUN_AS_UID}
            runAsGroup: {ENGINE_RUN_AS_GID}
            runAsNonRoot: true
          ports:
            - containerPort: 7777
              name: http
          env:
            - name: HOME
              value: /data
            - name: HOUSTON_HOME
              value: /data/.houston
            - name: HOUSTON_DOCS
              value: /data/workspace
            - name: HOUSTON_BIND
              value: 0.0.0.0:7777
            - name: HOUSTON_BIND_ALL
              value: "1"
            - name: HOUSTON_NO_PARENT_WATCHDOG
              value: "1"
            - name: HOUSTON_TUNNEL_URL
              value: http://127.0.0.1:1
            - name: HOUSTON_ENGINE_TOKEN
              valueFrom:
                secretKeyRef:
                  name: {secret}
                  key: HOUSTON_ENGINE_TOKEN
          volumeMounts:
            - name: home
              mountPath: /data
          startupProbe:
            httpGet:
              path: /v1/health
              port: 7777
              httpHeaders:
                - name: Authorization
                  value: Bearer {engine_token}
            periodSeconds: 1
            timeoutSeconds: 3
            failureThreshold: 120
          readinessProbe:
            httpGet:
              path: /v1/health
              port: 7777
              httpHeaders:
                - name: Authorization
                  value: Bearer {engine_token}
            periodSeconds: 2
            timeoutSeconds: 3
            failureThreshold: 3
          resources:
            requests:
              cpu: 250m
              memory: 512Mi
            limits:
              cpu: "2"
              memory: 2Gi
      volumes:
        - name: home
          persistentVolumeClaim:
            claimName: {pvc}
---
apiVersion: v1
kind: Service
metadata:
  name: {deploy}
  namespace: {ns}
  labels:
    houston.ai/agent-id: "{agent_id}"
spec:
  type: ClusterIP
  selector:
    app: {deploy}
  ports:
    - name: http
      port: 7777
      targetPort: 7777
"#
    )
}

pub fn agent_manifests(
    agent_id: Uuid,
    org_id: Uuid,
    engine_image: &str,
    engine_token: &str,
) -> String {
    format!(
        "{}\n---\n{}",
        agent_pvc_manifest(agent_id, org_id),
        agent_workload_manifests(agent_id, org_id, engine_image, engine_token)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_url_uses_cluster_dns() {
        let id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let org = Uuid::parse_str("11111111-1111-1111-1111-111111111101").unwrap();
        let url = internal_service_url(id, org);
        assert!(url.contains("hou-cloud-agent-"));
        assert!(url.contains(".svc.cluster.local:7777"));
    }

    #[test]
    fn agent_manifest_includes_cloud_boot_tuning() {
        let id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let org = Uuid::parse_str("11111111-1111-1111-1111-111111111101").unwrap();
        let yaml = agent_manifests(id, org, "houston/engine:dev", "abc123");
        assert!(yaml.contains("HOUSTON_TUNNEL_URL"));
        assert!(yaml.contains("http://127.0.0.1:1"));
        assert!(yaml.contains("startupProbe:"));
        assert!(yaml.contains("periodSeconds: 1"));
        assert!(yaml.contains("minReadySeconds: 0"));
        assert!(yaml.contains("Authorization"));
        assert!(yaml.contains("Bearer abc123"));
    }

    #[test]
    fn agent_pvc_manifest_is_standalone() {
        let id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let org = Uuid::parse_str("11111111-1111-1111-1111-111111111101").unwrap();
        let yaml = agent_pvc_manifest(id, org);
        assert!(yaml.contains("kind: PersistentVolumeClaim"));
        assert!(!yaml.contains("kind: Deployment"));
    }

    #[test]
    fn agent_manifest_mounts_full_data_home() {
        let id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let org = Uuid::parse_str("11111111-1111-1111-1111-111111111101").unwrap();
        let yaml = agent_manifests(id, org, "houston/engine:dev", "tok");
        assert!(yaml.contains("name: HOME\n              value: /data"));
        assert!(yaml.contains("mountPath: /data\n"));
        assert!(!yaml.contains("subPath:"));
    }

    #[test]
    fn agent_manifest_sets_volume_ownership() {
        let id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let org = Uuid::parse_str("11111111-1111-1111-1111-111111111101").unwrap();
        let yaml = agent_manifests(id, org, "houston/engine:dev", "tok");
        assert!(yaml.contains(&format!("fsGroup: {ENGINE_RUN_AS_GID}")));
        assert!(yaml.contains(&format!("runAsUser: {ENGINE_RUN_AS_UID}")));
        assert!(yaml.contains("fsGroupChangePolicy: OnRootMismatch"));
    }

    #[test]
    fn org_quota_manifest_includes_resource_quota_and_limit_range() {
        let org = Uuid::parse_str("11111111-1111-1111-1111-111111111101").unwrap();
        let yaml = org_quota_manifests(org, 3, 30);
        assert!(yaml.contains("kind: ResourceQuota"));
        assert!(yaml.contains("name: hou-org-quota"));
        assert!(yaml.contains("kind: LimitRange"));
        assert!(yaml.contains("name: hou-org-limits"));
        assert!(yaml.contains("requests.cpu: 750m"));
        assert!(yaml.contains("requests.memory: 1536Mi"));
        assert!(yaml.contains("limits.cpu: 6"));
        assert!(yaml.contains("limits.memory: 6Gi"));
        assert!(yaml.contains("persistentvolumeclaims: \"3\""));
        assert!(yaml.contains("requests.storage: 30Gi"));
        assert!(yaml.contains("namespace: hou-org-11111111-1111-1111-1111-111111111101"));
    }

    #[test]
    fn org_quota_manifest_scales_with_max_cloud_agents() {
        let org = Uuid::new_v4();
        let small = org_quota_manifests(org, 1, 10);
        let large = org_quota_manifests(org, 10, 100);
        assert!(small.contains("requests.cpu: 250m"));
        assert!(large.contains("requests.cpu: 2500m"));
        assert!(small.contains("persistentvolumeclaims: \"1\""));
        assert!(large.contains("persistentvolumeclaims: \"10\""));
        assert!(small.contains("requests.storage: 10Gi"));
        assert!(large.contains("requests.storage: 100Gi"));
    }
}
