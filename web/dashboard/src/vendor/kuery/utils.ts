/*
 * Copyright OpenSearch Contributors
 * SPDX-License-Identifier: Apache-2.0
 */

// Replaces src/plugins/data/common/opensearch_query/utils.ts, which asks
// moment-timezone for the browser's time zone; see README.md.

export function getTimeZoneFromSettings(dateFormatTZ: string) {
  return dateFormatTZ === 'Browser'
    ? Intl.DateTimeFormat().resolvedOptions().timeZone
    : dateFormatTZ;
}
