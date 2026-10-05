/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * The OpenSearch Contributors require contributions made to
 * this file be licensed under the Apache-2.0 license or a
 * compatible open source license.
 *
 * Any modifications Copyright OpenSearch Contributors. See
 * GitHub history for details.
 */

/*
 * Licensed to Elasticsearch B.V. under one or more contributor
 * license agreements. See the NOTICE file distributed with
 * this work for additional information regarding copyright
 * ownership. Elasticsearch B.V. licenses this file to you under
 * the Apache License, Version 2.0 (the "License"); you may
 * not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *    http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

// Modified for standalone use in web/dashboard; see README.md.

import { nodeTypes } from '../node_types';
import * as ast from '../ast';
import { getFields } from './utils/get_fields';
import { getTimeZoneFromSettings } from '../utils';
import { getFullFieldNameNode } from './utils/get_full_field_name_node';
import type { KueryNode } from '../types';
import type { IIndexPattern, IFieldType, RangeFilterParams } from '../standalone_types';

// The order lodash's `pick` produced upstream, which fixes the order of the node's arguments.
const RANGE_PARAMS = ['gt', 'lt', 'gte', 'lte', 'format'] as const;

export function buildNodeParams(fieldName: string, params: RangeFilterParams) {
  const paramsToMap = RANGE_PARAMS.filter((key) => params != null && key in params);
  const fieldNameArg =
    typeof fieldName === 'string'
      ? ast.fromLiteralExpression(fieldName)
      : nodeTypes.literal.buildNode(fieldName);

  const args = paramsToMap.map((key) => {
    return nodeTypes.namedArg.buildNode(key, params[key]);
  });

  return {
    arguments: [fieldNameArg, ...args],
  };
}

export function toOpenSearchQuery(
  node: KueryNode,
  indexPattern?: IIndexPattern,
  config: Record<string, any> = {},
  context: Record<string, any> = {}
) {
  const [fieldNameArg, ...args] = node.arguments;
  const fullFieldNameArg = getFullFieldNameNode(
    fieldNameArg,
    indexPattern,
    context?.nested ? context.nested.path : undefined
  );
  const fields = indexPattern ? getFields(fullFieldNameArg, indexPattern) : [];
  const namedArgs = extractArguments(args);
  const queryParams = Object.fromEntries(
    Object.entries(namedArgs).map(([name, arg]) => {
      return [name, ast.toOpenSearchQuery(arg as KueryNode)];
    })
  );

  // If no fields are found in the index pattern we send through the given field name as-is. We do this to preserve
  // the behaviour of lucene on dashboards where there are panels based on different index patterns that have different
  // fields. If a user queries on a field that exists in one pattern but not the other, the index pattern without the
  // field should return no results. It's debatable whether this is desirable, but it's been that way forever, so we'll
  // keep things familiar for now.
  if (fields && fields.length === 0) {
    fields.push({
      name: ast.toOpenSearchQuery(fullFieldNameArg) as unknown as string,
      scripted: false,
      type: '',
    });
  }

  const queries = fields!.map((field: IFieldType) => {
    const wrapWithNestedQuery = (query: any) => {
      // Wildcards can easily include nested and non-nested fields. There isn't a good way to let
      // users handle this themselves so we automatically add nested queries in this scenario.
      if (
        !(fullFieldNameArg.type === 'wildcard') ||
        !field.subType?.nested ||
        context!.nested
      ) {
        return query;
      } else {
        return {
          nested: {
            path: field.subType!.nested!.path,
            query,
            score_mode: 'none',
          },
        };
      }
    };

    if (field.type === 'date') {
      const timeZoneParam = config.dateFormatTZ
        ? { time_zone: getTimeZoneFromSettings(config!.dateFormatTZ) }
        : {};
      return wrapWithNestedQuery({
        range: {
          [field.name]: {
            ...queryParams,
            ...timeZoneParam,
          },
        },
      });
    }
    return wrapWithNestedQuery({
      range: {
        [field.name]: queryParams,
      },
    });
  });

  return {
    bool: {
      should: queries,
      minimum_should_match: 1,
    },
  };
}

function extractArguments(args: any) {
  if ((args.gt && args.gte) || (args.lt && args.lte)) {
    throw new Error('range ends cannot be both inclusive and exclusive');
  }

  const unnamedArgOrder = ['gte', 'lte', 'format'];

  return args.reduce((acc: any, arg: any, index: number) => {
    if (arg.type === 'namedArg') {
      acc[arg.name] = arg.value;
    } else {
      acc[unnamedArgOrder[index]] = arg;
    }

    return acc;
  }, {});
}
