/*
 * Copyright OpenSearch Contributors
 * SPDX-License-Identifier: Apache-2.0
 */

// Typings for the generated parser in ./kuery.js. Not part of upstream, which
// imports the parser under `@ts-ignore`; see README.md.

import type { KueryNode, KueryParseOptions } from '../../types';

/** The rules `options.startRule` may name. */
export const StartRules: string[];

/** Thrown by `parse`; its `name` is `'SyntaxError'`. */
export class SyntaxError extends Error {
  expected: Array<{ type: string; description?: string; text?: string }> | null;
  found: string | null;
  location: {
    source?: unknown;
    start: { offset: number; line: number; column: number };
    end: { offset: number; line: number; column: number };
  };
}

export function parse(expression: any, options?: Partial<KueryParseOptions>): KueryNode;
