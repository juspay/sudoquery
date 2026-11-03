-- chosen_slug_identifiers: an org's id is the slug given when it is created
-- (^[a-z][a-z0-9_]{1,28}[a-z0-9]$, e.g. north_star), and a workspace's id is
-- its org's id and its own slug joined by '-' (e.g. north_star-web_shop), so
-- workspace ids are unique across orgs and name the workspace's topic and
-- databases on their own. Rows with the old generated ids fail the new
-- checks: recreate them.

ALTER TABLE public.organizations DROP CONSTRAINT organizations_id_slug_check;
ALTER TABLE public.organizations
    ADD CONSTRAINT organizations_id_slug_check CHECK (id ~ '^[a-z][a-z0-9_]{1,28}[a-z0-9]$');

ALTER TABLE public.workspaces DROP CONSTRAINT workspaces_id_slug_check;
ALTER TABLE public.workspaces ALTER COLUMN organization_id SET NOT NULL;
ALTER TABLE public.workspaces
    ADD CONSTRAINT workspaces_id_slug_check CHECK (
        id ~ '^[a-z][a-z0-9_]{1,28}[a-z0-9]-[a-z][a-z0-9_]{1,28}[a-z0-9]$'
        AND starts_with(id, organization_id || '-')
    );
