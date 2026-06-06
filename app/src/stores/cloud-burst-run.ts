import { create } from "zustand";
import {
  clearBurstRunState,
  loadBurstRunState,
  reconcileBurstRunState,
  refreshBurstSlotsFromSessions,
  saveBurstRunState,
} from "../lib/cloud-debug-burst-state";
import type { BurstAgentSlot } from "../lib/cloud-debug-burst";
import { setCloudDebugWsCap } from "../lib/runtime-router";
import { useSessionStatusStore } from "../stores/session-status";

function burstSlotsPhaseChanged(
  before: BurstAgentSlot[],
  after: BurstAgentSlot[],
): boolean {
  if (before.length !== after.length) return true;
  return before.some((slot, index) => slot.phase !== after[index]?.phase);
}

interface CloudBurstRunStore {
  hydrated: boolean;
  running: boolean;
  slots: BurstAgentSlot[];
  configId: string;
  hydrate: () => void;
  setRunning: (running: boolean) => void;
  setConfigId: (configId: string) => void;
  setSlots: (
    slots: BurstAgentSlot[] | ((prev: BurstAgentSlot[]) => BurstAgentSlot[]),
  ) => void;
  patchSlot: (index: number, patch: Partial<BurstAgentSlot>) => void;
  reset: () => void;
}

function patchSlotList(
  slots: BurstAgentSlot[],
  index: number,
  patch: Partial<BurstAgentSlot>,
): BurstAgentSlot[] {
  return slots.map((s) => (s.index === index ? { ...s, ...patch } : s));
}

function persistFromState(state: CloudBurstRunStore): void {
  if (state.slots.length === 0 && !state.running) {
    clearBurstRunState();
    return;
  }
  saveBurstRunState({
    running: state.running,
    slots: state.slots,
    configId: state.configId,
    updatedAt: new Date().toISOString(),
  });
}

export const useCloudBurstRunStore = create<CloudBurstRunStore>((set, get) => ({
  hydrated: false,
  running: false,
  slots: [],
  configId: "",
  hydrate: () => {
    if (get().hydrated) return;
    const saved = loadBurstRunState();
    if (saved) {
      const reconciled = reconcileBurstRunState({
        running: saved.running,
        slots: saved.slots,
      });
      if (reconciled.changed && !reconciled.running && reconciled.slots.length === 0) {
        clearBurstRunState();
      }
      set({
        hydrated: true,
        slots: reconciled.slots,
        configId: saved.configId,
        running: reconciled.running,
      });
      return;
    }
    set({ hydrated: true });
  },
  setRunning: (running) => {
    set({ running });
    persistFromState(get());
  },
  setConfigId: (configId) => {
    set({ configId });
    persistFromState(get());
  },
  setSlots: (next) => {
    set((state) => ({
      slots: typeof next === "function" ? next(state.slots) : next,
    }));
    persistFromState(get());
  },
  patchSlot: (index, patch) => {
    set((state) => ({ slots: patchSlotList(state.slots, index, patch) }));
    persistFromState(get());
  },
  reset: () => {
    clearBurstRunState();
    setCloudDebugWsCap(import.meta.env.DEV ? 128 : 4);
    set({ running: false, slots: [], configId: "" });
  },
}));

useSessionStatusStore.subscribe(() => {
  const state = useCloudBurstRunStore.getState();
  if (state.slots.length === 0) return;
  const next = refreshBurstSlotsFromSessions(state.slots);
  if (!burstSlotsPhaseChanged(state.slots, next)) return;
  state.setSlots(next);
});
