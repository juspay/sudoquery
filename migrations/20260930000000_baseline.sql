-- baseline: generated from pg_schema.sql (pg_dump). Frozen — add new migrations as new files instead of editing.

--
-- PostgreSQL database dump
--


-- Dumped from database version 17.9
-- Dumped by pg_dump version 18.3 (Homebrew)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: chat_type; Type: TYPE; Schema: public; Owner: hyper_analytics_user
--

CREATE TYPE public.chat_type AS ENUM (
    'general',
    'create_dashboard'
);


ALTER TYPE public.chat_type OWNER TO hyper_analytics_user;

--
-- Name: invitation_status; Type: TYPE; Schema: public; Owner: hyper_analytics_user
--

CREATE TYPE public.invitation_status AS ENUM (
    'pending',
    'accepted',
    'revoked',
    'expired'
);


ALTER TYPE public.invitation_status OWNER TO hyper_analytics_user;

--
-- Name: invitation_type; Type: TYPE; Schema: public; Owner: hyper_analytics_user
--

CREATE TYPE public.invitation_type AS ENUM (
    'organization',
    'project'
);


ALTER TYPE public.invitation_type OWNER TO hyper_analytics_user;

--
-- Name: org_role; Type: TYPE; Schema: public; Owner: hyper_analytics_user
--

CREATE TYPE public.org_role AS ENUM (
    'org_admin',
    'org_user'
);


ALTER TYPE public.org_role OWNER TO hyper_analytics_user;

--
-- Name: project_role; Type: TYPE; Schema: public; Owner: hyper_analytics_user
--

CREATE TYPE public.project_role AS ENUM (
    'project_admin',
    'project_user'
);


ALTER TYPE public.project_role OWNER TO hyper_analytics_user;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: chats; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.chats (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
    user_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    title text NOT NULL,
    version integer NOT NULL,
    chat_type public.chat_type NOT NULL
);


ALTER TABLE public.chats OWNER TO hyper_analytics_user;

--
-- Name: event_descriptions; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.event_descriptions (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
    event_name text NOT NULL,
    description text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.event_descriptions OWNER TO hyper_analytics_user;

--
-- Name: invitations; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.invitations (
    id uuid NOT NULL,
    email character varying(255) NOT NULL,
    invitation_type public.invitation_type NOT NULL,
    target_id uuid NOT NULL,
    role character varying(50) NOT NULL,
    invited_by uuid NOT NULL,
    status public.invitation_status DEFAULT 'pending'::public.invitation_status NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    expires_at timestamp with time zone DEFAULT (now() + '7 days'::interval)
);


ALTER TABLE public.invitations OWNER TO hyper_analytics_user;

--
-- Name: live_dashboards; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.live_dashboards (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
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


ALTER TABLE public.live_dashboards OWNER TO hyper_analytics_user;

--
-- Name: messages; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.messages (
    id uuid NOT NULL,
    chat_id uuid NOT NULL,
    message jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    role text NOT NULL,
    updated_at timestamp with time zone
);


ALTER TABLE public.messages OWNER TO hyper_analytics_user;

--
-- Name: organization_memberships; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.organization_memberships (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    organization_id uuid NOT NULL,
    role public.org_role DEFAULT 'org_user'::public.org_role NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.organization_memberships OWNER TO hyper_analytics_user;

--
-- Name: organizations; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.organizations (
    id uuid NOT NULL,
    name character varying(255) NOT NULL,
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.organizations OWNER TO hyper_analytics_user;

--
-- Name: project_memberships; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.project_memberships (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    role public.project_role DEFAULT 'project_user'::public.project_role NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.project_memberships OWNER TO hyper_analytics_user;

--
-- Name: project_tokens; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.project_tokens (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
    token uuid NOT NULL,
    name character varying(255),
    last_used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.project_tokens OWNER TO hyper_analytics_user;

--
-- Name: projects; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.projects (
    id uuid NOT NULL,
    organization_id uuid,
    name character varying(255) NOT NULL,
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    timezone text
);


ALTER TABLE public.projects OWNER TO hyper_analytics_user;

--
-- Name: property_descriptions; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.property_descriptions (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
    event_name text NOT NULL,
    property_name text NOT NULL,
    property_type text NOT NULL,
    description text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.property_descriptions OWNER TO hyper_analytics_user;

--
-- Name: user_project_consoles; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.user_project_consoles (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    proj_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    console text,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.user_project_consoles OWNER TO hyper_analytics_user;

--
-- Name: users; Type: TABLE; Schema: public; Owner: hyper_analytics_user
--

CREATE TABLE public.users (
    id uuid NOT NULL,
    keycloak_user_id character varying(255) NOT NULL,
    email character varying(255) NOT NULL,
    username character varying(255) NOT NULL,
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.users OWNER TO hyper_analytics_user;

--
-- Name: chats chats_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.chats
    ADD CONSTRAINT chats_pkey PRIMARY KEY (id);


--
-- Name: event_descriptions event_descriptions_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.event_descriptions
    ADD CONSTRAINT event_descriptions_pkey PRIMARY KEY (id);


--
-- Name: event_descriptions event_descriptions_project_id_event_name_key; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.event_descriptions
    ADD CONSTRAINT event_descriptions_project_id_event_name_key UNIQUE (project_id, event_name);


--
-- Name: invitations invitations_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.invitations
    ADD CONSTRAINT invitations_pkey PRIMARY KEY (id);


--
-- Name: live_dashboards live_dashboards_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.live_dashboards
    ADD CONSTRAINT live_dashboards_pkey PRIMARY KEY (id);


--
-- Name: messages messages_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_pkey PRIMARY KEY (id);


--
-- Name: organization_memberships organization_memberships_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_pkey PRIMARY KEY (id);


--
-- Name: organization_memberships organization_memberships_user_id_organization_id_key; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_user_id_organization_id_key UNIQUE (user_id, organization_id);


--
-- Name: organizations organizations_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.organizations
    ADD CONSTRAINT organizations_pkey PRIMARY KEY (id);


--
-- Name: project_memberships project_memberships_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_pkey PRIMARY KEY (id);


--
-- Name: project_memberships project_memberships_user_id_project_id_key; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_user_id_project_id_key UNIQUE (user_id, project_id);


--
-- Name: project_tokens project_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.project_tokens
    ADD CONSTRAINT project_tokens_pkey PRIMARY KEY (id);


--
-- Name: project_tokens project_tokens_token_key; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.project_tokens
    ADD CONSTRAINT project_tokens_token_key UNIQUE (token);


--
-- Name: projects projects_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_pkey PRIMARY KEY (id);


--
-- Name: property_descriptions property_descriptions_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.property_descriptions
    ADD CONSTRAINT property_descriptions_pkey PRIMARY KEY (id);


--
-- Name: property_descriptions property_descriptions_project_id_event_name_property_name_key; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.property_descriptions
    ADD CONSTRAINT property_descriptions_project_id_event_name_property_name_key UNIQUE (project_id, event_name, property_name);


--
-- Name: user_project_consoles user_project_consoles_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.user_project_consoles
    ADD CONSTRAINT user_project_consoles_pkey PRIMARY KEY (id);


--
-- Name: users users_keycloak_user_id_key; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_keycloak_user_id_key UNIQUE (keycloak_user_id);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: idx_chats_chat_type; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_chats_chat_type ON public.chats USING btree (chat_type);


--
-- Name: idx_chats_project_user; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_chats_project_user ON public.chats USING btree (project_id, user_id);


--
-- Name: idx_event_descriptions_project_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_event_descriptions_project_id ON public.event_descriptions USING btree (project_id);


--
-- Name: idx_invitations_email; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_invitations_email ON public.invitations USING btree (email);


--
-- Name: idx_invitations_invited_by; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_invitations_invited_by ON public.invitations USING btree (invited_by);


--
-- Name: idx_invitations_status; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_invitations_status ON public.invitations USING btree (status);


--
-- Name: idx_invitations_target_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_invitations_target_id ON public.invitations USING btree (target_id);


--
-- Name: idx_live_dashboards_project_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_live_dashboards_project_id ON public.live_dashboards USING btree (project_id);


--
-- Name: idx_messages_chat_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_messages_chat_id ON public.messages USING btree (chat_id, created_at DESC);


--
-- Name: idx_organization_memberships_organization_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_organization_memberships_organization_id ON public.organization_memberships USING btree (organization_id);


--
-- Name: idx_organization_memberships_user_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_organization_memberships_user_id ON public.organization_memberships USING btree (user_id);


--
-- Name: idx_organizations_deleted_at; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_organizations_deleted_at ON public.organizations USING btree (deleted_at);


--
-- Name: idx_project_id_source; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_project_id_source ON public.live_dashboards USING btree (project_id, creation_source);


--
-- Name: idx_project_memberships_project_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_project_memberships_project_id ON public.project_memberships USING btree (project_id);


--
-- Name: idx_project_memberships_user_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_project_memberships_user_id ON public.project_memberships USING btree (user_id);


--
-- Name: idx_project_tokens_project_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_project_tokens_project_id ON public.project_tokens USING btree (project_id);


--
-- Name: idx_project_tokens_token; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_project_tokens_token ON public.project_tokens USING btree (token);


--
-- Name: idx_projects_deleted_at; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_projects_deleted_at ON public.projects USING btree (deleted_at);


--
-- Name: idx_projects_organization_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_projects_organization_id ON public.projects USING btree (organization_id);


--
-- Name: idx_property_descriptions_event_name; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_property_descriptions_event_name ON public.property_descriptions USING btree (project_id, event_name);


--
-- Name: idx_property_descriptions_project_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_property_descriptions_project_id ON public.property_descriptions USING btree (project_id);


--
-- Name: idx_user_proj_consoles_on_all_three; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_user_proj_consoles_on_all_three ON public.user_project_consoles USING btree (id, user_id, proj_id);


--
-- Name: idx_user_proj_consoles_on_user_and_proj; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_user_proj_consoles_on_user_and_proj ON public.user_project_consoles USING btree (user_id, proj_id);


--
-- Name: idx_users_deleted_at; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_users_deleted_at ON public.users USING btree (deleted_at);


--
-- Name: idx_users_email; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_users_email ON public.users USING btree (email);


--
-- Name: idx_users_keycloak_user_id; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE INDEX idx_users_keycloak_user_id ON public.users USING btree (keycloak_user_id);


--
-- Name: invitations_email_target_key; Type: INDEX; Schema: public; Owner: hyper_analytics_user
--

CREATE UNIQUE INDEX invitations_email_target_key ON public.invitations USING btree (email, target_id) WHERE (status = 'pending'::public.invitation_status);


--
-- Name: chats chats_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.chats
    ADD CONSTRAINT chats_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: chats chats_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.chats
    ADD CONSTRAINT chats_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: event_descriptions event_descriptions_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.event_descriptions
    ADD CONSTRAINT event_descriptions_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: user_project_consoles fk_project; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.user_project_consoles
    ADD CONSTRAINT fk_project FOREIGN KEY (proj_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: user_project_consoles fk_user; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.user_project_consoles
    ADD CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: invitations invitations_invited_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.invitations
    ADD CONSTRAINT invitations_invited_by_fkey FOREIGN KEY (invited_by) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: live_dashboards live_dashboards_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.live_dashboards
    ADD CONSTRAINT live_dashboards_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: messages messages_chat_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_chat_id_fkey FOREIGN KEY (chat_id) REFERENCES public.chats(id) ON DELETE CASCADE;


--
-- Name: organization_memberships organization_memberships_organization_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;


--
-- Name: organization_memberships organization_memberships_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: project_memberships project_memberships_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: project_memberships project_memberships_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.project_memberships
    ADD CONSTRAINT project_memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: project_tokens project_tokens_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.project_tokens
    ADD CONSTRAINT project_tokens_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: projects projects_organization_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES public.organizations(id);


--
-- Name: property_descriptions property_descriptions_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: hyper_analytics_user
--

ALTER TABLE ONLY public.property_descriptions
    ADD CONSTRAINT property_descriptions_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--


