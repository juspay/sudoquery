import { describe, expect, it } from 'vitest';
import { slugError, slugify, workspaceId } from './slug';

describe('slugify', () => {
  it('suggests a slug from a name', () => {
    const cases: Array<[string, string]> = [
      ['North Star', 'north_star'],
      ['  Acme  Retail, Inc. ', 'acme_retail_inc'],
      ['Café Münster', 'cafe_munster'],
      ['123 Shop', 'shop'],
      ['web-shop', 'web_shop'],
      ['A very long organization name that keeps going', 'a_very_long_organization_name'],
      ['!!!', ''],
    ];

    for (const [name, expected] of cases) {
      expect(slugify(name), name).toBe(expected);
    }
  });

  it('suggests valid slugs for names long enough', () => {
    for (const name of ['North Star', 'Café Münster', 'A very long organization name that keeps going']) {
      expect(slugError(slugify(name)), name).toBeNull();
    }
  });
});

describe('slugError', () => {
  it('accepts valid slugs', () => {
    for (const slug of ['abc', 'north_star', 'north_star_123', 'a'.repeat(30)]) {
      expect(slugError(slug), slug).toBeNull();
    }
  });

  it('rejects anything else', () => {
    for (const slug of ['', 'ab', 'a'.repeat(31), '1abc', '_abc', 'abc_', 'Abc', 'web-shop', 'north star']) {
      expect(slugError(slug), slug).not.toBeNull();
    }
  });
});

describe('workspaceId', () => {
  it('joins the org id and the workspace slug', () => {
    expect(workspaceId('north_star', 'web_shop')).toBe('north_star-web_shop');
  });
});
