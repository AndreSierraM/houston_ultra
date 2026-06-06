//! Kubernetes manifest builders for per-agent engine runtimes.

use uuid::Uuid;

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

pub fn agent_manifests(
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
---
apiVersion: v1
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
  minReadySeconds: 5
  selector:
    matchLabels:
      app: {deploy}
  template:
    metadata:
      labels:
        app: {deploy}
        houston.ai/agent-id: "{agent_id}"
    spec:
      containers:
        - name: engine
          image: {engine_image}
          imagePullPolicy: IfNotPresent
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
            periodSeconds: 2
            failureThreshold: 30
          readinessProbe:
            httpGet:
              path: /v1/health
              port: 7777
              httpHeaders:
                - name: Authorization
                  value: Bearer {engine_token}
            periodSeconds: 5
            timeoutSeconds: 5
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
        assert!(yaml.contains("Authorization"));
        assert!(yaml.contains("Bearer abc123"));
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
}
