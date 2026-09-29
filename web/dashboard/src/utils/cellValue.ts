/** True for values that should render as an expandable JSON cell (objects and arrays). */
export const isExpandableValue = (value: unknown): boolean =>
  typeof value === 'object' && value !== null;

/** JSON.stringify that never throws: falls back to String() for circular/undefined values. */
export const safeStringify = (value: unknown, space?: number): string => {
  try {
    return JSON.stringify(value, null, space) ?? String(value);
  } catch {
    return String(value);
  }
};
