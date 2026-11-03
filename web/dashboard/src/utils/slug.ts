// Org and workspace slugs, as dashboard-server takes them (see
// canonical_event::is_valid_slug). An org's slug is its id; a workspace's id
// is its org's id and its own slug joined by '-', e.g. north_star-web_shop.

export const SLUG_MIN_LENGTH = 3;
export const SLUG_MAX_LENGTH = 30;

const SLUG_PATTERN = /^[a-z][a-z0-9_]*[a-z0-9]$/;

export const SLUG_RULES =
  `${SLUG_MIN_LENGTH}–${SLUG_MAX_LENGTH} lowercase letters, digits and underscores, ` +
  'starting with a letter and ending with a letter or digit';

/** A suggested slug for `name`: "North Star" → "north_star". May be empty. */
export function slugify(name: string): string {
  return name
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^[0-9_]+/, '')
    .slice(0, SLUG_MAX_LENGTH)
    .replace(/_+$/, '');
}

/** Why `slug` is not a valid slug, or null if it is. */
export function slugError(slug: string): string | null {
  if (slug.length < SLUG_MIN_LENGTH || slug.length > SLUG_MAX_LENGTH || !SLUG_PATTERN.test(slug)) {
    return `Use ${SLUG_RULES}.`;
  }
  return null;
}

/** The id of `orgId`'s workspace with slug `slug`. */
export function workspaceId(orgId: string, slug: string): string {
  return `${orgId}-${slug}`;
}
