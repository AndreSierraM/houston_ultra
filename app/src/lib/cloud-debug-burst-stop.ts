import type { BurstAgentSlot } from "./cloud-debug-burst.ts";
import { isActiveBurstPhase } from "./cloud-debug-burst-state.ts";
import { tauriChat } from "./tauri.ts";

export const BURST_STOPPED_MESSAGE = "Stopped by user";

let stopRequested = false;
const burstEventUnsubs: Array<() => void> = [];

export function registerBurstEventUnsub(unsub: () => void): void {
  burstEventUnsubs.push(unsub);
}

export function detachBurstEventBridges(): void {
  while (burstEventUnsubs.length > 0) {
    const unsub = burstEventUnsubs.pop();
    unsub?.();
  }
}

export function beginBurstRun(): void {
  stopRequested = false;
  detachBurstEventBridges();
}

export function requestBurstStop(): void {
  stopRequested = true;
}

export function isBurstStopRequested(): boolean {
  return stopRequested;
}

export function burstStoppedError(): Error {
  return new Error(BURST_STOPPED_MESSAGE);
}

export function assertBurstNotStopped(): void {
  if (stopRequested) {
    throw burstStoppedError();
  }
}

export async function stopBurstSessions(slots: BurstAgentSlot[]): Promise<void> {
  const stops = slots
    .filter(
      (slot) =>
        slot.agentPath &&
        slot.sessionKey &&
        isActiveBurstPhase(slot.phase),
    )
    .map(async (slot) => {
      await tauriChat.stop(slot.agentPath!, slot.sessionKey);
    });
  await Promise.allSettled(stops);
}

export function markStoppedBurstSlots(slots: BurstAgentSlot[]): BurstAgentSlot[] {
  return slots.map((slot) =>
    isActiveBurstPhase(slot.phase)
      ? { ...slot, phase: "error", error: BURST_STOPPED_MESSAGE }
      : slot,
  );
}
