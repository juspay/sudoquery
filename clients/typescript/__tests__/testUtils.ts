import type { Event } from "../src/types";

export function createMockEvent(id: number, overrides: Partial<Event> = {}): Event {
  return {
    envelop_version: "1.0",
    id: crypto.randomUUID(),
    name: `event_${id}`,
    tenant_id: "tenant-1",
    workspace_id: null,
    session_id: "session-1",
    anon_id: crypto.randomUUID(),
    actor_id: `user_${id}`,
    source: "typescript",
    occured_at: new Date(1_700_000_000_000 + id).toISOString(),
    properties: { id },
    correlation_id: null,
    trace_id: null,
    system_properties: null,
    ...overrides,
  };
}
