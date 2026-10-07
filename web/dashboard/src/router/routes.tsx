import React, { lazy, Suspense } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { RequireAuth } from './RequireAuth';
import { LoadingSkeleton } from '../components/shared/LoadingSkeleton';
import { DEMO_CONFIG } from '../config/api';

// ─── Layouts ─────────────────────────────────────────────────────────────────
// Lazy-loaded to keep Phase 1 working even when layouts are stubs
const PublicLayout = lazy(() => import('../layouts/PublicLayout'));
const OnboardingShell = lazy(() => import('../layouts/OnboardingShell'));
const AppShell = lazy(() => import('../layouts/AppShell'));
const DocsLayout = lazy(() => import('../layouts/DocsLayout'));

// ─── Public pages ─────────────────────────────────────────────────────────────
const LandingPage = lazy(() => import('../pages/LandingPage'));
const LoginPage = lazy(() => import('../pages/LoginPage'));
const NotFoundPage = lazy(() => import('../pages/NotFoundPage'));
const DemoStorePage = lazy(() => import('../pages/DemoStorePage'));

// ─── Docs pages ───────────────────────────────────────────────────────────────
const GettingStartedPage = lazy(() => import('../pages/docs/GettingStartedPage'));
const UsagePage = lazy(() => import('../pages/docs/UsagePage'));
const ConfigurationPage = lazy(() => import('../pages/docs/ConfigurationPage'));
const ApiReferencePage = lazy(() => import('../pages/docs/ApiReferencePage'));

// ─── OAuth Callback ───────────────────────────────────────────────────────────
const OAuthCallbackPage = lazy(() => import('../pages/OAuthCallbackPage'));

// ─── Onboarding pages ────────────────────────────────────────────────────────
const OnboardingInvitationsPage = lazy(() => import('../pages/OnboardingInvitationsPage'));
const OnboardingOrgPage = lazy(() => import('../pages/OnboardingOrgPage'));
const OnboardingProjectPage = lazy(() => import('../pages/OnboardingProjectPage'));

// ─── App pages ────────────────────────────────────────────────────────────────
const ChatPage = lazy(() => import('../pages/ChatPageV1'));
const DashboardPage = lazy(() => import('../pages/DashboardPage'));
const MetricsPage = lazy(() => import('../pages/MetricsPage'));
const EventSchemaPage = lazy(() => import('../pages/EventSchemaPage'));
const SettingsLayout = lazy(() => import('../layouts/SettingsLayout'));
const AccountSettingsPage = lazy(() => import('../pages/settings/AccountSettingsPage'));
const InvitationsSettingsPage = lazy(() => import('../pages/settings/InvitationsSettingsPage'));
const ProjectSettingsPage = lazy(() => import('../pages/settings/ProjectSettingsPage'));
const OrganizationSettingsPage = lazy(() => import('../pages/settings/OrganizationSettingsPage'));
const SQLConsole = lazy(() => import('../pages/SQLConsole'));
const DiscoverPage = lazy(() => import('../pages/DiscoverPage'));
const SessionsPage = lazy(() => import('../pages/SessionsPage'));
const SessionDetailPage = lazy(() => import('../pages/SessionDetailPage'));

function PageLoader() {
  return <LoadingSkeleton variant="page" />;
}

export function AppRoutes() {
  return (
    <Suspense fallback={<PageLoader />}>
      <Routes>
        {/* Public routes */}
        <Route element={<PublicLayout />}>
          <Route index element={<LandingPage />} />
          <Route path="login" element={<LoginPage />} />
          <Route path="callback" element={<OAuthCallbackPage />} />
        </Route>

        {/* Demo store; set VITE_DEMO_ENABLED=true to turn it on */}
        {DEMO_CONFIG.ENABLED && <Route path="demo" element={<DemoStorePage />} />}

        {/* Docs routes */}
        <Route path="docs" element={<DocsLayout />}>
          <Route index element={<Navigate to="getting-started" replace />} />
          <Route path="getting-started" element={<GettingStartedPage />} />
          <Route path="usage" element={<UsagePage />} />
          <Route path="configuration" element={<ConfigurationPage />} />
          <Route path="api-reference" element={<ApiReferencePage />} />
        </Route>

        {/* Onboarding routes */}
        <Route path="onboarding" element={<OnboardingShell />}>
          <Route index element={<Navigate to="invitations" replace />} />
          <Route path="invitations" element={<OnboardingInvitationsPage />} />
          <Route path="org" element={<OnboardingOrgPage />} />
          <Route path="project" element={<OnboardingProjectPage />} />
        </Route>

        {/* Authenticated app routes */}
        <Route
          path="app"
          element={
            <RequireAuth>
              <AppShell />
            </RequireAuth>
          }
        >
          <Route index element={<Navigate to="chat" replace />} />
          <Route path="chat/:chatId?" element={<ChatPage />} />
          <Route path="dashboard" element={<DashboardPage />} />
          <Route path="metrics" element={<MetricsPage />} />
          <Route path="events-schema" element={<EventSchemaPage />} />
          <Route path="sql-console" element={<SQLConsole />} />
          <Route path="discover" element={<DiscoverPage />} />
          <Route path="discover/sessions" element={<SessionsPage />} />
          <Route path="discover/sessions/:sessionId" element={<SessionDetailPage />} />
          <Route path="settings" element={<SettingsLayout />}>
            <Route index element={<Navigate to="account" replace />} />
            <Route path="account" element={<AccountSettingsPage />} />
            <Route path="invitations" element={<InvitationsSettingsPage />} />
            <Route path="project" element={<ProjectSettingsPage />} />
            <Route path="organization" element={<OrganizationSettingsPage />} />
          </Route>
        </Route>

        {/* 404 */}
        <Route path="*" element={<NotFoundPage />} />
      </Routes>
    </Suspense>
  );
}
