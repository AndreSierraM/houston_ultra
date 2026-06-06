import type { SessionRunStatus } from "../stores/session-status.ts";
import {
  getSessionStatusKey,
  isActiveSessionStatus,
  useSessionStatusStore,
} from "../stores/session-status.ts";
import type { BurstAgentPhase, BurstAgentSlot } from "./cloud-debug-burst.ts";

const BURST_RUN_STORAGE_KEY = "houston.cloudDebug.burstRun";

export interface SavedBurstRunState {
  running: boolean;
  slots: BurstAgentSlot[];
  configId: string;
  updatedAt: string;
}

export interface BurstRunSummary {
  total: number;
  running: number;
  done: number;
  error: number;
  pending: number;
  isActive: boolean;
}

const ACTIVE_BURST_PHASES: ReadonlySet<BurstAgentPhase> = new Set([
  "pending",
  "creating",
  "connecting",
  "mission",
  "listening",
]);

export function isActiveBurstPhase(phase: BurstAgentPhase): boolean {
  return ACTIVE_BURST_PHASES.has(phase);
}

export function summarizeBurstRun(
  slots: BurstAgentSlot[],
  running: boolean,
): BurstRunSummary {
  let done = 0;
  let error = 0;
  let pending = 0;
  let activeSlots = 0;

  for (const slot of slots) {
    if (slot.phase === "done") {
      done += 1;
    } else if (slot.phase === "error") {
      error += 1;
    } else if (slot.phase === "pending") {
      pending += 1;
    } else if (isActiveBurstPhase(slot.phase)) {
      activeSlots += 1;
    }
  }

  return {
    total: slots.length,
    running: activeSlots,
    done,
    error,
    pending,
    isActive: running || activeSlots > 0,
  };
}

export function refreshBurstSlotsFromSessions(slots: BurstAgentSlot[]): BurstAgentSlot[] {
  const statuses = useSessionStatusStore.getState().statuses;
  return slots.map((slot) => {
    if (!slot.agentPath || slot.phase === "done" || slot.phase === "error") {
      return slot;
    }
    const sessionStatus = statuses[getSessionStatusKey(slot.agentPath, slot.sessionKey)];
    const phase = inferBurstPhaseFromSession(slot.phase, sessionStatus);
    if (phase === slot.phase) return slot;
    return { ...slot, phase };
  });
}

function inferBurstPhaseFromSession(
  current: BurstAgentPhase,
  sessionStatus: SessionRunStatus | undefined,
): BurstAgentPhase {
  if (sessionStatus === "error") return "error";
  if (isActiveSessionStatus(sessionStatus)) {
    if (current === "mission") return "listening";
    if (current === "listening") return "listening";
    return "listening";
  }
  if (sessionStatus === "completed") {
    if (current === "listening" || current === "mission") return "done";
    if (current === "done") return "done";
  }
  return current;
}

export function loadBurstRunState(): SavedBurstRunState | null {
  if (typeof sessionStorage === "undefined") return null;
  try {
    const raw = sessionStorage.getItem(BURST_RUN_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as SavedBurstRunState;
    if (!Array.isArray(parsed.slots)) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function saveBurstRunState(state: SavedBurstRunState): void {
  if (typeof sessionStorage === "undefined") return;
  sessionStorage.setItem(BURST_RUN_STORAGE_KEY, JSON.stringify(state));
}

export function clearBurstRunState(): void {
  if (typeof sessionStorage === "undefined") return;
  sessionStorage.removeItem(BURST_RUN_STORAGE_KEY);
}

/** Drop orphaned `running` flags and revive tracking when slots are still active. */
export function reconcileBurstRunState(state: {
  running: boolean;
  slots: BurstAgentSlot[];
}): { running: boolean; slots: BurstAgentSlot[]; changed: boolean } {
  const slotSummary = summarizeBurstRun(state.slots, false);

  if (state.running && state.slots.length === 0) {
    return { running: false, slots: [], changed: true };
  }

  if (state.running && !slotSummary.isActive) {
    return { running: false, slots: state.slots, changed: true };
  }

  if (!state.running && slotSummary.running > 0) {
    return { running: true, slots: state.slots, changed: true };
  }

  return { running: state.running, slots: state.slots, changed: false };
}
