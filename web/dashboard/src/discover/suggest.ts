import { fromKueryExpression } from '../vendor/kuery';
import type { FieldDef } from './types';

/** The marker the DQL grammar recognises as the caret; it cannot be changed. */
const CURSOR = '@kuery-cursor@';

const MAX_SUGGESTIONS = 10;

export type SuggestionKind = 'field' | 'operator' | 'value' | 'conjunction';

export interface Suggestion {
  kind: SuggestionKind;
  /** Inserted in place of `query.slice(start, end)`. */
  text: string;
  label: string;
  detail?: string;
  start: number;
  end: number;
}

/** What the grammar expects at the caret. Offsets index the original query. */
export interface CursorContext {
  start: number;
  end: number;
  prefix: string;
  suffix: string;
  /** The field the caret's value belongs to, or the field name being typed. */
  fieldName: string;
  /** The caret follows a finished clause and a space, where `and` / `or` fit. */
  afterClause: boolean;
  kinds: SuggestionKind[];
}

/** `null` when the query cannot be parsed up to the caret. */
export function cursorContext(query: string, caret: number): CursorContext | null {
  const marked = `${query.slice(0, caret)}${CURSOR}${query.slice(caret)}`;
  let node: Record<string, unknown>;
  try {
    node = fromKueryExpression(marked, {
      parseCursor: true,
      cursorSymbol: CURSOR,
    }) as unknown as Record<string, unknown>;
  } catch {
    return null;
  }
  if (node.type !== 'cursor') return null;

  const nestedPath = typeof node.nestedPath === 'string' ? node.nestedPath : '';
  const fieldName = typeof node.fieldName === 'string' ? node.fieldName.trim() : '';
  return {
    start: Number(node.start),
    end: Number(node.end),
    prefix: String(node.prefix ?? ''),
    suffix: String(node.suffix ?? ''),
    fieldName: nestedPath && fieldName ? `${nestedPath}.${fieldName}` : fieldName,
    // Not right after an operator or an opening bracket: a value belongs there.
    afterClause: /[^\s:<>=({]\s+$/.test(query.slice(0, caret)),
    kinds: Array.isArray(node.suggestionTypes) ? (node.suggestionTypes as SuggestionKind[]) : [],
  };
}

export function fieldSuggestions(context: CursorContext, fields: FieldDef[]): Suggestion[] {
  if (!context.kinds.includes('field')) return [];
  const search = `${context.prefix}${context.suffix}`.trim().toLowerCase();
  return fields
    // A name typed in full needs an operator next, not itself again.
    .filter((field) => field.name.toLowerCase().includes(search) && field.name.toLowerCase() !== search)
    .sort((a, b) => {
      // Names starting with what was typed come first.
      const rank = (field: FieldDef) => (field.name.toLowerCase().startsWith(search) ? 0 : 1);
      return rank(a) - rank(b);
    })
    .slice(0, MAX_SUGGESTIONS)
    .map((field) => ({
      kind: 'field',
      text: field.name,
      label: field.name,
      detail: field.type,
      start: context.start,
      end: context.end,
    }));
}

const COMPARISONS: Array<[string, string]> = [
  ['<', 'is less than'],
  ['<=', 'is less than or equal to'],
  ['>', 'is greater than'],
  ['>=', 'is greater than or equal to'],
];

/** Offered once a complete field name has been typed. */
export function operatorSuggestions(context: CursorContext, fields: FieldDef[]): Suggestion[] {
  if (!context.kinds.includes('operator')) return [];
  const field = fields.find((candidate) => candidate.name === context.fieldName);
  if (!field) return [];

  const operators: Array<[string, string]> = [[':', 'equals some value']];
  if (field.type === 'number' || field.type === 'date' || field.type === 'ip') {
    operators.push(...COMPARISONS);
  }
  operators.push([': *', 'exists in any form']);

  return operators.map(([operator, detail]) => ({
    kind: 'operator',
    text: operator === ': *' ? `${operator} ` : operator,
    label: operator,
    detail,
    start: context.end,
    end: context.end,
  }));
}

/** Offered after a complete clause and a space. */
export function conjunctionSuggestions(context: CursorContext): Suggestion[] {
  if (!context.kinds.includes('conjunction') || !context.afterClause) return [];
  return [
    ['and', 'requires both arguments to be true'],
    ['or', 'requires one or more arguments to be true'],
  ].map(([word, detail]) => ({
    kind: 'conjunction',
    text: `${word} `,
    label: word,
    detail,
    start: context.end,
    end: context.end,
  }));
}

/** The field and typed prefix to look values up for, when a value is expected. */
export function valueLookup(context: CursorContext): { field: string; prefix: string } | null {
  if (!context.kinds.includes('value') || context.fieldName === '') return null;
  return { field: context.fieldName, prefix: `${context.prefix}${context.suffix}`.trim() };
}

function quote(value: string): string {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

/** Strings are quoted, so values with spaces or reserved characters stay one value. */
export function valueSuggestions(
  context: CursorContext,
  values: string[],
  field: FieldDef | undefined,
): Suggestion[] {
  const raw = field?.type === 'number' || field?.type === 'boolean';
  return values.slice(0, MAX_SUGGESTIONS).map((value) => ({
    kind: 'value',
    text: `${raw ? value : quote(value)} `,
    label: value,
    start: context.start,
    end: context.end,
  }));
}

export function applySuggestion(
  query: string,
  suggestion: Suggestion,
): { query: string; caret: number } {
  const next = `${query.slice(0, suggestion.start)}${suggestion.text}${query.slice(suggestion.end)}`;
  return { query: next, caret: suggestion.start + suggestion.text.length };
}
