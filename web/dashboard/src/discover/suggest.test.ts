import { describe, expect, it } from 'vitest';
import { EVENT_FIELDS } from './fields';
import {
  applySuggestion,
  conjunctionSuggestions,
  cursorContext,
  fieldSuggestions,
  operatorSuggestions,
  valueLookup,
  valueSuggestions,
  type CursorContext,
} from './suggest';
import type { FieldDef } from './types';

const fields: FieldDef[] = [
  ...EVENT_FIELDS,
  { name: 'properties.plan', type: 'string', facetable: false },
  { name: 'properties.amount', type: 'number', facetable: false },
];

/** The context with the caret at the end of `query`. */
function at(query: string): CursorContext {
  const context = cursorContext(query, query.length);
  if (!context) throw new Error(`no context for ${query}`);
  return context;
}

const labels = (suggestions: Array<{ label: string }>) => suggestions.map((entry) => entry.label);

describe('cursorContext', () => {
  it('knows a value is expected after a field and colon', () => {
    expect(at('name:che')).toMatchObject({ start: 5, end: 8, prefix: 'che', fieldName: 'name' });
    expect(at('name:che').kinds).toContain('value');
  });

  it('knows a field is expected at the start and after a conjunction', () => {
    expect(at('').kinds).toContain('field');
    expect(at('name:checkout and so')).toMatchObject({ start: 18, end: 20, prefix: 'so' });
  });

  it('gives up on text the grammar cannot read yet', () => {
    expect(cursorContext('name:"chec', 10)).toBeNull();
    expect(cursorContext('(name:a or ', 11)).toBeNull();
  });

  it('works with the caret in the middle of the query', () => {
    expect(cursorContext('name:che and source:web', 8)).toMatchObject({
      start: 5,
      end: 8,
      fieldName: 'name',
    });
  });
});

describe('fieldSuggestions', () => {
  it('lists fields containing what was typed, prefix matches first', () => {
    expect(labels(fieldSuggestions(at('na'), fields))).toEqual(['name']);
    const forC = labels(fieldSuggestions(at('c'), fields));
    expect(forC[0]).toBe('correlation_id');
    expect(forC).toContain('source');
  });

  it('finds properties by any part of their name', () => {
    expect(labels(fieldSuggestions(at('plan'), fields))).toEqual(['properties.plan']);
  });

  it('does not offer a name that is already typed in full', () => {
    expect(labels(fieldSuggestions(at('name'), fields))).toEqual([]);
  });

  it('offers nothing where a value is expected', () => {
    expect(fieldSuggestions(at('name:che'), fields)).toEqual([]);
  });

  it('replaces the typed text', () => {
    const [suggestion] = fieldSuggestions(at('name:a and so'), fields);

    expect(applySuggestion('name:a and so', suggestion)).toEqual({
      query: 'name:a and source',
      caret: 17,
    });
  });
});

describe('operatorSuggestions', () => {
  it('offers equality and existence for a known field', () => {
    expect(labels(operatorSuggestions(at('name'), fields))).toEqual([':', ': *']);
  });

  it('adds comparisons for numbers and dates', () => {
    expect(labels(operatorSuggestions(at('properties.amount'), fields))).toEqual([
      ':',
      '<',
      '<=',
      '>',
      '>=',
      ': *',
    ]);
    expect(labels(operatorSuggestions(at('occured_at'), fields))).toContain('>=');
  });

  it('offers nothing until the field name is complete', () => {
    expect(operatorSuggestions(at('na'), fields)).toEqual([]);
  });

  it('is appended after the field', () => {
    const [colon] = operatorSuggestions(at('name'), fields);

    expect(applySuggestion('name', colon)).toEqual({ query: 'name:', caret: 5 });
  });
});

describe('conjunctionSuggestions', () => {
  it('offers and / or after a finished clause and a space', () => {
    expect(labels(conjunctionSuggestions(at('name:checkout ')))).toEqual(['and', 'or']);
  });

  it('offers nothing mid-word, or where a value is still missing', () => {
    expect(conjunctionSuggestions(at('name:checkout'))).toEqual([]);
    expect(conjunctionSuggestions(at('name : '))).toEqual([]);
    expect(conjunctionSuggestions(at('occured_at > '))).toEqual([]);
    expect(conjunctionSuggestions(at(''))).toEqual([]);
  });
});

describe('value suggestions', () => {
  it('looks values up for the field before the colon', () => {
    expect(valueLookup(at('name:che'))).toEqual({ field: 'name', prefix: 'che' });
    expect(valueLookup(at('name:'))).toEqual({ field: 'name', prefix: '' });
    expect(valueLookup(at('na'))).toBeNull();
  });

  it('quotes strings and replaces what was typed', () => {
    const context = at('name:che');
    const [suggestion] = valueSuggestions(context, ['checkout "viewed"'], fields[0]);

    expect(suggestion.label).toBe('checkout "viewed"');
    expect(applySuggestion('name:che', suggestion).query).toBe('name:"checkout \\"viewed\\"" ');
  });

  it('leaves numbers and booleans unquoted', () => {
    const amount = fields.find((field) => field.name === 'properties.amount');
    const [suggestion] = valueSuggestions(at('properties.amount:1'), ['12'], amount);

    expect(suggestion.text).toBe('12 ');
  });
});
