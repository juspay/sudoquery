/**
 * Segment rules as the builder edits them — one flat group of conditions,
 * matched all or any — and their CDP predicate JSON (see the CDP's
 * docs/segment-predicate-dsl.md). The builder covers the flat case; any
 * predicate, nested or not, can still be described with `describePredicate`.
 */

import type { MetricDefinition } from '../services/cdpService';

export type Op =
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte'
  | 'eq'
  | 'neq'
  | 'between'
  | 'exists'
  | 'missing'
  | 'within_days'
  | 'beyond_days';

export interface Condition {
  metric: string;
  op: Op;
  /** Text as typed; `between` uses `value` and `valueTo`. */
  value: string;
  valueTo: string;
}

export interface Rule {
  match: 'all' | 'any';
  conditions: Condition[];
}

export type Leaf = { metric: string; op: Op; value?: number | [number, number] };
export type Predicate = { all: Predicate[] } | { any: Predicate[] } | { not: Predicate } | Leaf;

export const OP_LABELS: Record<Op, string> = {
  gt: 'greater than',
  gte: 'at least',
  lt: 'less than',
  lte: 'at most',
  eq: 'equals',
  neq: 'does not equal',
  between: 'between',
  exists: 'exists',
  missing: 'is missing',
  within_days: 'within the last … days',
  beyond_days: 'more than … days ago',
};

const NUMERIC_OPS: Op[] = ['gte', 'gt', 'lte', 'lt', 'eq', 'neq', 'between'];
const PRESENCE_OPS: Op[] = ['exists', 'missing'];
const TEMPORAL_OPS: Op[] = ['within_days', 'beyond_days'];

/** The operators a metric of this kind accepts (DSL §2). */
export function opsFor(valueKind: string | undefined): Op[] {
  switch (valueKind) {
    case 'occurred_at_epoch':
      return [...TEMPORAL_OPS, ...PRESENCE_OPS, ...NUMERIC_OPS];
    case 'property_string':
      return [...TEMPORAL_OPS, ...PRESENCE_OPS];
    default:
      return [...NUMERIC_OPS, ...PRESENCE_OPS];
  }
}

/** One entry per metric name (a metric may be claimed from several events). */
export function metricChoices(definitions: MetricDefinition[]): MetricDefinition[] {
  const seen = new Map<string, MetricDefinition>();
  for (const d of definitions) {
    if (!seen.has(d.metric_name)) seen.set(d.metric_name, d);
  }
  return [...seen.values()].sort((a, b) =>
    (a.display_name ?? a.metric_name).localeCompare(b.display_name ?? b.metric_name),
  );
}

export function emptyCondition(metric = '', op: Op = 'gte'): Condition {
  return { metric, op, value: '', valueTo: '' };
}

function number(text: string): number | null {
  const trimmed = text.trim();
  if (trimmed === '') return null;
  const n = Number(trimmed);
  return Number.isFinite(n) ? n : null;
}

/** The leaf for one condition, or why it can't be one yet. */
export function toLeaf(c: Condition): Leaf | string {
  if (!c.metric) return 'choose a metric';
  if (PRESENCE_OPS.includes(c.op)) return { metric: c.metric, op: c.op };
  if (c.op === 'between') {
    const lo = number(c.value);
    const hi = number(c.valueTo);
    if (lo === null || hi === null) return 'enter both ends of the range';
    if (lo > hi) return 'the range must go from low to high';
    return { metric: c.metric, op: c.op, value: [lo, hi] };
  }
  const value = number(c.value);
  if (value === null) return 'enter a number';
  if (TEMPORAL_OPS.includes(c.op) && (!Number.isInteger(value) || value <= 0)) {
    return 'days must be a whole number above 0';
  }
  return { metric: c.metric, op: c.op, value };
}

/** The rule's predicate, or the first problem with it. */
export function toPredicate(rule: Rule): { predicate: Predicate } | { error: string } {
  if (rule.conditions.length === 0) return { error: 'add at least one condition' };
  const leaves: Leaf[] = [];
  for (const [i, c] of rule.conditions.entries()) {
    const leaf = toLeaf(c);
    if (typeof leaf === 'string') return { error: `condition ${i + 1}: ${leaf}` };
    leaves.push(leaf);
  }
  // A single condition is a bare leaf — the DSL needs no wrapping group.
  if (leaves.length === 1) return { predicate: leaves[0] };
  return { predicate: rule.match === 'all' ? { all: leaves } : { any: leaves } };
}

function describeLeaf(leaf: Leaf, name: (metric: string) => string): string {
  const metric = name(leaf.metric);
  switch (leaf.op) {
    case 'exists':
      return `${metric} exists`;
    case 'missing':
      return `${metric} is missing`;
    case 'between': {
      const [lo, hi] = leaf.value as [number, number];
      return `${metric} between ${lo} and ${hi}`;
    }
    case 'within_days':
      return `${metric} within the last ${leaf.value} days`;
    case 'beyond_days':
      return `${metric} more than ${leaf.value} days ago`;
    default:
      return `${metric} ${OP_LABELS[leaf.op]} ${leaf.value}`;
  }
}

/** A readable sentence for any predicate, nested groups included. */
export function describePredicate(
  predicate: Predicate,
  name: (metric: string) => string = (m) => m,
): string {
  if ('all' in predicate) {
    return predicate.all.map((p) => wrap(p, name)).join(' and ');
  }
  if ('any' in predicate) {
    return predicate.any.map((p) => wrap(p, name)).join(' or ');
  }
  if ('not' in predicate) {
    return `not ${wrap(predicate.not, name)}`;
  }
  return describeLeaf(predicate, name);
}

function wrap(p: Predicate, name: (metric: string) => string): string {
  const text = describePredicate(p, name);
  return 'all' in p || 'any' in p ? `(${text})` : text;
}

/** A segment id from its name: lowercase letters, digits and hyphens. */
export function slugify(name: string): string {
  return name
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64);
}
