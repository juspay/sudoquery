/*
 * Copyright OpenSearch Contributors
 * SPDX-License-Identifier: Apache-2.0
 */

// Not an upstream file. Covers what the dashboard relies on and upstream's
// tests don't: the public surface, DSL built without an index pattern, and
// the parser's cursor mode.

import { describe, test, expect } from 'vitest';
import {
  DQLSyntaxError,
  doesKueryExpressionHaveLuceneSyntaxError,
  fromKueryExpression,
  fromLiteralExpression,
  nodeTypes,
  toOpenSearchQuery,
} from './index';
import type { IIndexPattern } from './index';

const CURSOR = '@kuery-cursor@';

function suggestionsAt(expression: string) {
  return fromKueryExpression(expression, { parseCursor: true, cursorSymbol: CURSOR });
}

describe('standalone kuery', () => {
  test('builds query DSL without an index pattern', () => {
    const node = fromKueryExpression(
      'name:checkout_viewed and properties.plan:pro and not actor_id:*'
    );

    expect(toOpenSearchQuery(node)).toEqual({
      bool: {
        filter: [
          { bool: { should: [{ match: { name: 'checkout_viewed' } }], minimum_should_match: 1 } },
          { bool: { should: [{ match: { 'properties.plan': 'pro' } }], minimum_should_match: 1 } },
          {
            bool: {
              must_not: {
                bool: { should: [{ exists: { field: 'actor_id' } }], minimum_should_match: 1 },
              },
            },
          },
        ],
      },
    });
  });

  test('turns an exact value on a date field into a range in the given time zone', () => {
    const indexPattern: IIndexPattern = { fields: [{ name: 'occured_at', type: 'date' }] };
    const node = fromKueryExpression('occured_at:"2026-09-01"');

    expect(toOpenSearchQuery(node, indexPattern, { dateFormatTZ: 'Asia/Kolkata' })).toEqual({
      bool: {
        should: [
          {
            range: {
              occured_at: { gte: '2026-09-01', lte: '2026-09-01', time_zone: 'Asia/Kolkata' },
            },
          },
        ],
        minimum_should_match: 1,
      },
    });
  });

  test("resolves the 'Browser' time zone without moment-timezone", () => {
    const indexPattern: IIndexPattern = { fields: [{ name: 'occured_at', type: 'date' }] };
    const node = fromKueryExpression('occured_at > "2026-09-01"');

    expect(toOpenSearchQuery(node, indexPattern, { dateFormatTZ: 'Browser' })).toEqual({
      bool: {
        should: [
          {
            range: {
              occured_at: {
                gt: '2026-09-01',
                time_zone: Intl.DateTimeFormat().resolvedOptions().timeZone,
              },
            },
          },
        ],
        minimum_should_match: 1,
      },
    });
  });

  test('reports a syntax error with a short message', () => {
    let thrown: unknown;
    try {
      fromKueryExpression('name:');
    } catch (error) {
      thrown = error;
    }

    expect(thrown).toBeInstanceOf(DQLSyntaxError);
    expect((thrown as DQLSyntaxError).shortMessage).toBe(
      'Expected whitespace, "{", "(", value but end of input found.'
    );
    expect((thrown as DQLSyntaxError).message).toBe(
      'Expected whitespace, "{", "(", value but end of input found.\nname:\n-----^'
    );
  });

  test('tells Lucene syntax from DQL', () => {
    expect(doesKueryExpressionHaveLuceneSyntaxError('a:[1 TO 5]')).toBe(true);
    expect(doesKueryExpressionHaveLuceneSyntaxError('a:1 and b:2')).toBe(false);
  });

  test('parses a literal on its own', () => {
    expect(fromLiteralExpression('true')).toEqual(nodeTypes.literal.buildNode(true));
    expect(fromLiteralExpression('"quoted"')).toEqual(nodeTypes.literal.buildNode('quoted'));
  });

  describe('cursor mode', () => {
    test('suggests values after a field', () => {
      expect(suggestionsAt(`name:che${CURSOR}`)).toEqual({
        type: 'cursor',
        start: 5,
        end: 8,
        prefix: 'che',
        suffix: '',
        text: 'che',
        fieldName: 'name',
        suggestionTypes: ['value', 'conjunction'],
      });
    });

    test('suggests fields and operators for a bare word', () => {
      const cursor = suggestionsAt(`na${CURSOR}`);

      expect(cursor).toMatchObject({ type: 'cursor', start: 0, end: 2, prefix: 'na' });
      expect(cursor.fieldName).toBe('na');
      expect(cursor.suggestionTypes).toEqual(['field', 'operator', 'conjunction']);
    });

    test('suggests a new clause after a conjunction', () => {
      const cursor = suggestionsAt(`name:checkout and ${CURSOR}`);

      expect(cursor).toMatchObject({ type: 'cursor', prefix: '', suffix: '' });
      expect(cursor.suggestionTypes).toEqual(['field', 'operator', 'conjunction']);
    });
  });
});
