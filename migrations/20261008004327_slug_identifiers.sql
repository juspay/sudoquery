-- slug_identifiers: org/project ids UUID -> text slugs (^[a-z][a-z0-9-]{4,28}[a-z0-9]$).
-- Destructive drop/recreate of the 12 dependent tables (approved: dev data is
-- disposable, nothing runs in production, no backward compatibility). `users`
-- and the enum types survive untouched. user_project_consoles.proj_id is renamed
-- to project_id and its legacy index/FK names move to PG default naming style.

DROP TABLE IF EXISTS public.messages CASCADE;
DROP TABLE IF EXISTS public.chats CASCADE;
DROP TABLE IF EXISTS public.event_descriptions CASCADE;
DROP TABLE IF EXISTS public.property_descriptions CASCADE;
DROP TABLE IF EXISTS public.live_dashboards CASCADE;
DROP TABLE IF EXISTS public.user_project_consoles CASCADE;
DROP TABLE IF EXISTS public.project_tokens CASCADE;
DROP TABLE IF EXISTS public.project_memberships CASCADE;
DROP TABLE IF EXISTS public.organization_memberships CASCADE;
DROP TABLE IF EXISTS public.invitations CASCADE;
DROP TABLE IF EXISTS public.projects CASCADE;
DROP TABLE IF EXISTS public.organizations CASCADE;

--
-- Name: organizations; Type: TABLE
--

CREATE TABLE public.organizations (
    id text NOT NULL,
    name character varying(255) NOT NULL,
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: projects; Type: TABLE
--

CREATE TABLE public.projects (
    id text NOT NULL,
    organization_id text,
    name character varying(255) NOT NULL,
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    timezone text
);


--
-- Name: chats; Type: TABLE
--

CREATE TABLE public.chats (
    id uuid NOT NULL,
    project_id text NOT NULL,
    user_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    title text NOT NULL,
    version integer NOT NULL,
    chat_type public.chat_type NOT NULL
);


--
-- Name: messages; Type: TABLE
--

CREATE TABLE public.messages (
    id uuid NOT NULL,
    chat_id uuid NOT NULL,
    message jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    role text NOT NULL,
    updated_at timestamp with time zone
);


--
-- Name: event_descriptions; Type: TABLE
--

CREATE TABLE public.event_descriptions (
    id uuid NOT NULL,
    project_id text NOT NULL,
    event_name text NOT NULL,
    description text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: property_descriptions; Type: TABLE
--

CREATE TABLE public.property_descriptions (
    id uuid NOT NULL,
    project_id text NOT NULL,
    event_name text NOT NULL,
    property_name text NOT NULL,
    property_type text NOT NULL,
    description text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: live_dashboards; Type: TABLE
--

CREATE TABLE public.live_dashboards (
    id uuid NOT NULL,
    project_id text NOT NULL,
    query text NOT NULL,
    description text NOT NULL,
    chart_config text,
    last_ran_at timestamp with time zone,
    response jsonb,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    title text,
    creation_source text
);


--
-- Name: user_project_consoles; Type: TABLE
--

CREATE TABLE public.user_project_consoles (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    project_id text NOT NULL,
    name character varying(255) NOT NULL,
    console text,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP
);


--
-- Name: project_tokens; Type: TABLE
--

CREATE TABLE public.project_tokens (
    id uuid NOT NULL,
    project_id text NOT NULL,
    token uuid NOT NULL,
    name character varying(255),
    last_used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: project_memberships; Type: TABLE
--

CREATE TABLE public.project_memberships (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    project_id text NOT NULL,
    role public.project_role DEFAULT 'project_user'::public.project_role NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: organization_memberships; Type: TABLE
--

CREATE TABLE public.organization_memberships (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    organization_id text NOT NULL,
    role public.org_role DEFAULT 'org_user'::public.org_role NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: invitations; Type: TABLE
--

CREATE TABLE public.invitations (
    id uuid NOT NULL,
    email character varying(255) NOT NULL,
    invitation_type public.invitation_type NOT NULL,
    target_id text NOT NULL,
    role character varying(50) NOT NULL,
    invited_by uuid NOT NULL,
    status public.invitation_status DEFAULT 'pending'::public.invitation_status NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    expires_at timestamp with time zone DEFAULT (now() + '7 days'::interval)
);


--
-- Name: CONSTRAINTS
--

ALTER TABLE ONLY public.organizations
    ADD CONSTRAINT organizations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.organizations
    ADD CONSTRAINT organizations_id_slug_check CHECK (id ~ '^[a-z][a-z0-9-]{4,28}[a-z0-9]$');

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_id_slug_check CHECK (id ~ '^[a-z][a-z0-9-]{4,28}[a-z0-9]$');

ALTER TABLE ONLY public.chats
    ADD CONSTRAINT chats_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.event_descriptions
    ADD CONSTRAINT event_descriptions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.event_descriptions
    ADD CONSTRAINT event_descriptions_project_id_event_name_key UNIQUE (project_id, event_name);

ALTER TABLE ONLY public.property_descriptions
    ADD CONSTRAINT property_descriptions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.property_descriptions
    ADD CONSTRAINT property_descriptions_project_id_event_name_property_name_key UNIQUE (project_id, event_name, property_name);

ALTER TABLE ONLY public.live_dashboards
    ADD CONSTRAINT live_dashboards_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_project_consoles
    ADD CONSTRAINT user_project_consoles_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.project_tokens
    ADD CONSTRAINT project_tokens_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.project_tokens
    ADD CONSTRAINT project_tokens_token_key UNIQUE (token);

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_user_id_project_id_key UNIQUE (user_id, project_id);

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_user_id_organization_id_key UNIQUE (user_id, organization_id);

ALTER TABLE ONLY public.invitations
    ADD CONSTRAINT invitations_pkey PRIMARY KEY (id);

--
-- Name: INDEXES
--

CREATE INDEX idx_organizations_deleted_at ON public.organizations USING btree (deleted_at);

CREATE INDEX idx_projects_deleted_at ON public.projects USING btree (deleted_at);

CREATE INDEX idx_projects_organization_id ON public.projects USING btree (organization_id);

CREATE INDEX idx_chats_chat_type ON public.chats USING btree (chat_type);

CREATE INDEX idx_chats_project_user ON public.chats USING btree (project_id, user_id);

CREATE INDEX idx_messages_chat_id ON public.messages USING btree (chat_id, created_at DESC);

CREATE INDEX idx_event_descriptions_project_id ON public.event_descriptions USING btree (project_id);

CREATE INDEX idx_property_descriptions_project_id ON public.property_descriptions USING btree (project_id);

CREATE INDEX idx_property_descriptions_event_name ON public.property_descriptions USING btree (project_id, event_name);

CREATE INDEX idx_live_dashboards_project_id ON public.live_dashboards USING btree (project_id);

CREATE INDEX idx_project_id_source ON public.live_dashboards USING btree (project_id, creation_source);

CREATE INDEX idx_user_project_consoles_all_three ON public.user_project_consoles USING btree (id, user_id, project_id);

CREATE INDEX idx_user_project_consoles_user_and_proj ON public.user_project_consoles USING btree (user_id, project_id);

CREATE INDEX idx_project_tokens_project_id ON public.project_tokens USING btree (project_id);

CREATE INDEX idx_project_tokens_token ON public.project_tokens USING btree (token);

CREATE INDEX idx_project_memberships_project_id ON public.project_memberships USING btree (project_id);

CREATE INDEX idx_project_memberships_user_id ON public.project_memberships USING btree (user_id);

CREATE INDEX idx_organization_memberships_organization_id ON public.organization_memberships USING btree (organization_id);

CREATE INDEX idx_organization_memberships_user_id ON public.organization_memberships USING btree (user_id);

CREATE INDEX idx_invitations_email ON public.invitations USING btree (email);

CREATE INDEX idx_invitations_invited_by ON public.invitations USING btree (invited_by);

CREATE INDEX idx_invitations_status ON public.invitations USING btree (status);

CREATE INDEX idx_invitations_target_id ON public.invitations USING btree (target_id);

CREATE UNIQUE INDEX invitations_email_target_key ON public.invitations USING btree (email, target_id) WHERE (status = 'pending'::public.invitation_status);

--
-- Name: FK CONSTRAINTS
--

ALTER TABLE ONLY public.chats
    ADD CONSTRAINT chats_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.chats
    ADD CONSTRAINT chats_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_chat_id_fkey FOREIGN KEY (chat_id) REFERENCES public.chats(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.event_descriptions
    ADD CONSTRAINT event_descriptions_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.property_descriptions
    ADD CONSTRAINT property_descriptions_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.live_dashboards
    ADD CONSTRAINT live_dashboards_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_project_consoles
    ADD CONSTRAINT user_project_consoles_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_project_consoles
    ADD CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.project_tokens
    ADD CONSTRAINT project_tokens_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.invitations
    ADD CONSTRAINT invitations_invited_by_fkey FOREIGN KEY (invited_by) REFERENCES public.users(id) ON DELETE CASCADE;
