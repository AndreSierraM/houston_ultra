/**
 * Houston Cloud control plane client.
 * Auth: `VITE_HOUSTON_CLOUD_TOKEN` (local dev) or Supabase session bearer.
 */
import type { AgentBootstrapBundle } from "@houston-ai/engine-client";
import type { Agent } from "./types";

export type AgentRuntimeMode = "local" | "cloud_24_7";

export interface CloudEntitlement {
  orgId: string;
  status: "active" | "past_due" | "canceled";
  maxCloudAgents: number;
  maxStorageGb: number;
  maxMembers: number;
}

export interface CreateCloudAgentInput {
  name: string;
  configId: string;
  color?: string;
  claudeMd?: string;
  provider?: string;
  model?: string;
  bootstrapBundle?: AgentBootstrapBundle;
  syncProviderCredentials?: boolean;
}

export interface PatchCloudAgentInput {
  name?: string;
  color?: string;
}

declare const __HOUSTON_CLOUD_BASE__: string | undefined;

function cloudEnvBase(): string | undefined {
  const fromVite = (import.meta as ImportMeta & { env?: Record<string, string> }).env
    ?.VITE_HOUSTON_CLOUD_BASE?.trim();
  const baked =
    typeof __HOUSTON_CLOUD_BASE__ !== "undefined" ? __HOUSTON_CLOUD_BASE__?.trim() : "";
  return (fromVite || baked || "").replace(/\/$/, "") || undefined;
}

/** Public control plane URL (set via `VITE_HOUSTON_CLOUD_BASE` or `HOUSTON_CLOUD_BASE` at build). */
export function getCloudBaseUrl(): string {
  return cloudEnvBase() ?? "";
}

export function isCloudConfigured(): boolean {
  return Boolean(cloudEnvBase());
}

function cloudTokenFromEnv(): string | undefined {
  const fromVite = (import.meta as ImportMeta & { env?: Record<string, string> }).env
    ?.VITE_HOUSTON_CLOUD_TOKEN?.trim();
  return fromVite || undefined;
}

/** True when a static cloud token is configured (no Supabase sign-in needed). */
export function hasCloudToken(): boolean {
  return Boolean(cloudTokenFromEnv());
}

export async function pingCloudServer(): Promise<{
  ok: boolean;
  latencyMs?: number;
}> {
  const base = cloudEnvBase();
  if (!base) {
    return { ok: false };
  }
  const start = performance.now();
  try {
    const res = await fetch(`${base}/health`);
    const latencyMs = Math.round(performance.now() - start);
    if (!res.ok) {
      return { ok: false, latencyMs };
    }
    const body = (await res.text()).trim();
    return body === "ok" ? { ok: true, latencyMs } : { ok: false, latencyMs };
  } catch {
    return { ok: false, latencyMs: Math.round(performance.now() - start) };
  }
}

export async function isCloudAvailable(): Promise<boolean> {
  if (!isCloudConfigured()) return false;
  if (cloudTokenFromEnv()) return true;
  const { supabase } = await import("./supabase");
  const { data, error } = await supabase.auth.getSession();
  if (error) return false;
  return Boolean(data.session?.access_token);
}

function cloudBaseUrl(): string {
  const base = cloudEnvBase();
  if (!base) {
    throw new Error("Cloud control plane URL is not configured");
  }
  return base.replace(/\/$/, "");
}

export async function cloudBearerToken(): Promise<string> {
  const envToken = cloudTokenFromEnv();
  if (envToken) return envToken;
  const { supabase } = await import("./supabase");
  const { data, error } = await supabase.auth.getSession();
  if (error) throw error;
  const token = data.session?.access_token;
  if (!token) {
    throw new Error("Cloud token missing: set VITE_HOUSTON_CLOUD_TOKEN or sign in");
  }
  return token;
}

async function cloudFetch<T>(
  path: string,
  init?: RequestInit,
  label = "cloud",
): Promise<T> {
  const token = await cloudBearerToken();
  const res = await fetch(`${cloudBaseUrl()}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
      ...(init?.headers ?? {}),
    },
  });
  const body = await res.json().catch(() => null);
  if (!res.ok) {
    const message =
      (body as { error?: { message?: string } } | null)?.error?.message ??
      `Cloud request failed (${res.status})`;
    const err = new Error(message);
    const { showErrorToast } = await import("./error-toast");
    showErrorToast(label, message, err);
    throw err;
  }
  return body as T;
}

export async function fetchCloudMe(): Promise<{
  userId: string;
  email?: string;
  orgId: string;
  orgRole: string;
}> {
  return cloudFetch("/v1/cloud/me");
}

export async function fetchCloudEntitlements(): Promise<CloudEntitlement> {
  return cloudFetch("/v1/cloud/entitlements");
}

/** Normalize control-plane JSON (camelCase or legacy snake_case) into app Agent. */
export function normalizeCloudAgent(raw: Agent & Record<string, unknown>): Agent {
  const id = String(raw.id);
  const folderPath =
    (typeof raw.folderPath === "string" && raw.folderPath) ||
    (typeof raw.folder_path === "string" && raw.folder_path) ||
    `cloud://${id}`;
  return {
    id,
    name: String(raw.name),
    folderPath,
    configId:
      (typeof raw.configId === "string" && raw.configId) ||
      (typeof raw.config_id === "string" && raw.config_id) ||
      "",
    color: typeof raw.color === "string" ? raw.color : undefined,
    createdAt:
      (typeof raw.createdAt === "string" && raw.createdAt) ||
      (typeof raw.created_at === "string" && raw.created_at) ||
      new Date().toISOString(),
    lastOpenedAt:
      (typeof raw.lastOpenedAt === "string" && raw.lastOpenedAt) ||
      (typeof raw.last_opened_at === "string" && raw.last_opened_at) ||
      undefined,
    runtime: "cloud_24_7",
  };
}

export async function listCloudAgents(): Promise<Agent[]> {
  const agents = await cloudFetch<(Agent & Record<string, unknown>)[]>("/v1/cloud/agents");
  return agents.map(normalizeCloudAgent);
}

export async function createCloudAgent(
  input: CreateCloudAgentInput,
): Promise<Agent> {
  const { syncProviderCredentials: _sync, ...payload } = input;
  const agent = await cloudFetch<Agent & Record<string, unknown>>("/v1/cloud/agents", {
    method: "POST",
    body: JSON.stringify(payload),
  });
  return normalizeCloudAgent(agent);
}

export async function patchCloudAgent(
  id: string,
  input: PatchCloudAgentInput,
): Promise<Agent> {
  const agent = await cloudFetch<Agent & Record<string, unknown>>(
    `/v1/cloud/agents/${id}`,
    { method: "PATCH", body: JSON.stringify(input) },
    "update cloud agent",
  );
  return normalizeCloudAgent(agent);
}

export async function deleteCloudAgent(id: string): Promise<void> {
  await cloudFetch(`/v1/cloud/agents/${id}`, { method: "DELETE" }, "delete cloud agent");
}

export function cloudEngineBaseUrl(agentId: string): string {
  return `${cloudBaseUrl()}/v1/cloud/agents/${agentId}/proxy`;
}

export function cloudEngineWsUrl(agentId: string): string {
  const base = cloudBaseUrl().replace(/^http/, "ws");
  return `${base}/v1/cloud/agents/${agentId}/ws`;
}

export type CloudShareRole = "viewer" | "operator" | "admin";

export interface CloudAgentShare {
  userId: string;
  role: CloudShareRole;
}

export interface UpsertCloudAgentShareInput {
  userId: string;
  role: CloudShareRole;
}

export async function listCloudAgentShares(
  agentId: string,
): Promise<CloudAgentShare[]> {
  return cloudFetch(`/v1/cloud/agents/${agentId}/shares`);
}

export async function upsertCloudAgentShare(
  agentId: string,
  input: UpsertCloudAgentShareInput,
): Promise<CloudAgentShare> {
  return cloudFetch(
    `/v1/cloud/agents/${agentId}/shares`,
    { method: "POST", body: JSON.stringify(input) },
    "share cloud agent",
  );
}

export async function revokeCloudAgentShare(
  agentId: string,
  userId: string,
): Promise<void> {
  await cloudFetch(
    `/v1/cloud/agents/${agentId}/shares/${userId}`,
    { method: "DELETE" },
    "revoke cloud agent share",
  );
}
