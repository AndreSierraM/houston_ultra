export interface CloudWsHandle {
  disconnect(): void;
}

/** Per-agent cloud WS cache with LRU eviction (regression-tested). */
export class CloudAgentWsRegistry<T extends CloudWsHandle> {
  private readonly byAgentId = new Map<string, T>();
  private maxSize: number;

  constructor(maxSize: number) {
    this.maxSize = maxSize;
  }

  setMaxSize(maxSize: number): void {
    this.maxSize = Math.max(1, maxSize);
    while (this.byAgentId.size > this.maxSize) {
      const oldest = this.byAgentId.keys().next().value;
      if (!oldest) break;
      this.byAgentId.delete(oldest);
    }
  }

  maxSlots(): number {
    return this.maxSize;
  }

  get(agentId: string): T | undefined {
    const ws = this.byAgentId.get(agentId);
    if (ws) {
      this.touch(agentId, ws);
    }
    return ws;
  }

  set(agentId: string, ws: T): void {
    this.byAgentId.delete(agentId);
    this.byAgentId.set(agentId, ws);
  }

  touch(agentId: string, ws: T): void {
    this.byAgentId.delete(agentId);
    this.byAgentId.set(agentId, ws);
  }

  remove(agentId: string): T | undefined {
    const ws = this.byAgentId.get(agentId);
    if (!ws) return undefined;
    this.byAgentId.delete(agentId);
    return ws;
  }

  clear(): T[] {
    const all = [...this.byAgentId.values()];
    this.byAgentId.clear();
    return all;
  }

  size(): number {
    return this.byAgentId.size;
  }

  agentIds(): string[] {
    return [...this.byAgentId.keys()];
  }

  evictOldestIfNeeded(onEvict: (ws: T) => void): void {
    if (this.byAgentId.size < this.maxSize) return;
    const oldest = this.byAgentId.keys().next().value;
    if (!oldest) return;
    const ws = this.remove(oldest);
    if (ws) onEvict(ws);
  }
}
