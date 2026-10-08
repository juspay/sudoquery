/**
 * Sequence numbers let the collector measure delivery: every event carries the
 * stream it came from (one per page load in browsers, one per process in
 * Node.js) and its position in that stream. Gaps in `seq` are lost events;
 * repeated event ids are duplicates.
 */

import { generateUuid } from "./Uuid";

let streamId: string | null = null;
let nextSeq = 0;

export function nextSequence(): { stream_id: string; seq: number } {
  if (!streamId) {
    streamId = generateUuid();
  }

  return { stream_id: streamId, seq: nextSeq++ };
}

/**
 * Start a new stream. Useful for testing.
 */
export function resetSequence(): void {
  streamId = null;
  nextSeq = 0;
}
