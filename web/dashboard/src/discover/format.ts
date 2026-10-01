/** "850ms", "3.2s", "4m 05s", "2h 07m", "3d 4h". */
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '—';
  if (ms < 1_000) return `${Math.round(ms)}ms`;
  const seconds = ms / 1_000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${String(Math.floor(seconds % 60)).padStart(2, '0')}s`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${String(minutes % 60).padStart(2, '0')}m`;
  return `${Math.floor(hours / 24)}d ${hours % 24}h`;
}

/** Offset from the start of a session: "+0ms", "+3.2s", "+4m 05s". */
export function formatOffset(ms: number): string {
  return `+${formatDuration(Math.max(0, ms))}`;
}

// Distinct hues that sit well on the cream background; the first is the app's blue.
const NAME_COLORS = [
  '#4287f5',
  '#2AA99E',
  '#A5402C',
  '#B7791F',
  '#7C5CBF',
  '#3F7D3A',
  '#C2497A',
  '#47698C',
];

/**
 * One colour per distinct name, in order of first appearance, so names only
 * share a colour once there are more of them than colours.
 */
export function nameColors(names: string[]): Map<string, string> {
  const colors = new Map<string, string>();
  for (const name of names) {
    if (!colors.has(name)) {
      colors.set(name, NAME_COLORS[colors.size % NAME_COLORS.length]);
    }
  }
  return colors;
}

export function formatCount(count: number): string {
  return count.toLocaleString('en-US');
}

/** DOM id of an event in the session timeline, to scroll to it. */
export function eventElementId(index: number): string {
  return `session-event-${index}`;
}
