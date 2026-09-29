import type { DynamicQuery } from '../types/metric';

/**
 * Substitute variables into a dynamic query template
 */
export function substituteVariables(variables: Record<string, string>, dynamicQuery: DynamicQuery): string {
  let result = dynamicQuery.query;

  Object.entries(dynamicQuery.variables).forEach(([key, config]) => {
    const value = variables[key] ?? config.default;
    const placeholder = `{${key}}`;

    // Wrap only string values in quotes for SQL
    // date, datetime, number, and direction (ASC/DESC) are used as-is without quotes
    let formattedValue = value;
    if (config.inputType === 'string') {
      formattedValue = `'${value}'`;
    }
    // date, datetime, number, direction are used as-is

    result = result.replace(new RegExp(placeholder, 'g'), formattedValue);
  });

  return result;
}
