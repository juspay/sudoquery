# kuery (DQL parser), vendored from OpenSearch Dashboards

Parses a DQL string (`name:checkout and not actor_id:*`) and turns it into OpenSearch query DSL, in the browser.

| | |
|---|---|
| Source | https://github.com/opensearch-project/OpenSearch-Dashboards |
| Commit | `950d8884fd66ae82f880e967eb1defae1cee5230` (3.9.0 development, 2026-09-29) |
| Path | `src/plugins/data/common/opensearch_query/kuery/` |
| Licence | Apache-2.0: `LICENSE.txt`, with the attribution in `NOTICE.txt` |

Upstream file names, layout and licence headers are kept. Every file that differs from upstream says so under its header.

## Use

```ts
import { fromKueryExpression, toOpenSearchQuery, DQLSyntaxError } from '../vendor/kuery';

const dsl = toOpenSearchQuery(
  fromKueryExpression('name:checkout and occured_at > "2026-09-01"'),
  { fields: [{ name: 'occured_at', type: 'date' }] }, // optional index pattern
  { dateFormatTZ: 'Browser' } // optional; adds `time_zone` to date fields
);
```

- A syntax error is thrown as `DQLSyntaxError`; `shortMessage` is the one-line text for the user.
- The index pattern is optional. Without one, every `field:value` becomes a `match` on the field as typed. With one, fields of type `date` become ranges, wildcard field names are expanded over the listed fields, and fields with `subType.nested` are checked and wrapped in `nested`.
- Whole numbers beyond `Number.MAX_SAFE_INTEGER` come out of the parser as `BigInt`, which `JSON.stringify` refuses. Convert them before sending the query.
- Cursor mode, for autocomplete: `fromKueryExpression('name:che@kuery-cursor@', { parseCursor: true, cursorSymbol: '@kuery-cursor@' })` returns a `cursor` node with `suggestionTypes`, `fieldName`, `prefix`, `suffix`, `start` and `end`. The symbol must be exactly `@kuery-cursor@`; the grammar matches that text.

## Local changes

**The generated parser** (`ast/_generated_/kuery.js`, Peggy 4.2.0 output of `ast/kuery.peg`). Only the module wrapper changed: `module.exports = (function () { … })()` became ES exports (`parse`, `SyntaxError`, `StartRules`). `ast/_generated_/kuery.d.ts` is new; upstream imports the parser under `@ts-ignore`. `ast/kuery.peg` is kept for reference and is not compiled here. To regenerate: `peggy --format es --allowed-start-rules start,Literal ast/kuery.peg`.

**Imports that reached outside this directory** were replaced:

| Upstream import | Replacement |
|---|---|
| `IIndexPattern`, `IFieldType`, `JsonObject`, `JsonValue`, `RangeFilterParams` from the data plugin | `standalone_types.ts` (new), reduced to what kuery reads. `IIndexPattern.title` is optional here |
| `lodash` (`get`, `isUndefined`, `pick`, `map`, `mapValues`, `repeat`) | native code |
| `@osd/i18n` | the same English strings, inline |
| `getTimeZoneFromSettings` from `../utils` (moment-timezone) | `utils.ts` (new): `'Browser'` resolves through `Intl.DateTimeFormat`, anything else passes through |

**Removed:**

- `functions/geo_bounding_box.ts` and `functions/geo_polygon.ts`, with their tests and their names in `FunctionName`. The grammar cannot produce them.
- Script queries for scripted fields in `functions/is.ts` and `functions/range.ts` (upstream's `getPhraseScript` / `getRangeScript`, which pull in the whole filters module). A field marked `scripted` is now queried like any other field. `functions/exists.ts` still refuses scripted fields, as upstream does.

**For this app's TypeScript and ESLint settings** (`strict`, `verbatimModuleSyntax`, `noUnusedParameters`):

- type-only imports and re-exports use `import type` / `export type`;
- `catch` variables are annotated `any`;
- three `@ts-ignore` comments were replaced by casts, `parse: Function` by `typeof parseKuery`, and `'function' as 'function'` by `as const`;
- the unused `config` parameter of `exists.toOpenSearchQuery` is named `_config`.

`web/dashboard/eslint.config.js` turns `@typescript-eslint/no-explicit-any` off for this directory, because upstream types its AST nodes as `any` throughout.

## Tests

Upstream's jest tests run here under vitest (`npm test`). Changes to them: imports (`vitest`, local types, the fixture), two `(result as any)` casts and one lodash `get` in assertions' setup, and descriptions on two `@ts-expect-error` comments. No assertion was changed.

- `test_fixtures/fields.ts` is upstream's `index_patterns/fields/fields.mocks.ts`, unchanged apart from its type import.
- Dropped with the code they covered: `functions/geo_bounding_box.test.ts`, `functions/geo_polygon.test.ts`, and the `should support scripted fields` case in each of `functions/is.test.ts` and `functions/range.test.ts`.
- `index.test.ts` is new: the public surface, DSL without an index pattern, the `'Browser'` time zone, and cursor mode.
