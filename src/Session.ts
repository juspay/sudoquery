import type { SystemProperties } from "./types";
import { generateUuid } from "./Uuid";

let currentSessionId: string | null = null;

export function getSessionId(): string {
  if (!currentSessionId) {
    currentSessionId = generateId();
  }

  return currentSessionId;
}

export function resetSessionId(): void {
  currentSessionId = null;
}

export function getSystemProperties(): SystemProperties {
  return {
    geo: null,
    timezone: getTimezone(),
  };
}

function getTimezone(): string | null {
  if (typeof Intl === "undefined") {
    return null;
  }

  return Intl.DateTimeFormat().resolvedOptions().timeZone ?? null;
}

function generateId(): string {
  return generateUuid();
}
