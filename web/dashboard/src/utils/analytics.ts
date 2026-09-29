import { SudoQuery } from 'sudo-query';

// Initialize analytics with configuration from environment
export const initAnalytics = () => {
  const token = import.meta.env.VITE_ANALYTICS_TOKEN;
  const flushInterval = parseInt(import.meta.env.VITE_ANALYTICS_FLUSH_INTERVAL || '5000', 10);

  SudoQuery.init({
    token,
    flushInterval,
    batchSize: 10,
    endpoint: 'https://sudoquery.juspay.io/api/push_batch'
  });
};

// Export the SudoQuery class for direct access if needed
export { SudoQuery as HyperAnalytics };

// Event names constants
const EVENTS = {
  // Navigation
  TAB_CHANGED: 'tab_changed',
  SIDEBAR_TOGGLED: 'sidebar_toggled',

  // Chat
  CHAT_CREATED: 'chat_created',
  CHAT_DELETED: 'chat_deleted',
  CHAT_SELECTED: 'chat_selected',
  MESSAGE_SENT: 'message_sent',

  // Metrics
  METRIC_SAVED: 'metric_saved',
  METRIC_DELETED: 'metric_deleted',
  METRIC_PRESENTED: 'metric_presented',
  METRIC_REFRESHED: 'metric_refreshed',

  // Funnels
  FUNNEL_TAB_OPENED: 'funnel_tab_opened',

  // Dashboard
  DASHBOARD_CREATION_OPENED: 'dashboard_creation_opened',
  DASHBOARD_CREATION_CLOSED: 'dashboard_creation_closed',
  DASHBOARD_CREATED: 'dashboard_created',

  // Theme
  THEME_CHANGED: 'theme_changed',

  // Queries
  QUERY_EXECUTED: 'query_executed',
  QUERY_ERROR: 'query_error',
} as const;

// Track an event with properties
export const trackEvent = (eventName: string, properties: Record<string, unknown> = {}) => {
  SudoQuery.track(eventName, properties);
};

// Convenience functions for common events

const trackTabChanged = (tab: string, previousTab?: string) => {
  trackEvent(EVENTS.TAB_CHANGED, { tab, previous_tab: previousTab });
};

const trackSidebarToggled = (collapsed: boolean) => {
  trackEvent(EVENTS.SIDEBAR_TOGGLED, { collapsed });
};

const trackThemeChanged = (mode: 'light' | 'dark') => {
  trackEvent(EVENTS.THEME_CHANGED, { mode });
};

const trackChatCreated = () => {
  trackEvent(EVENTS.CHAT_CREATED);
};

const trackChatDeleted = () => {
  trackEvent(EVENTS.CHAT_DELETED);
};

const trackChatSelected = () => {
  trackEvent(EVENTS.CHAT_SELECTED);
};

const trackMessageSent = (messageLength: number) => {
  trackEvent(EVENTS.MESSAGE_SENT, { message_length: messageLength });
};

const trackMetricSaved = (metricId: string, label: string) => {
  trackEvent(EVENTS.METRIC_SAVED, { metric_id: metricId, metric_label: label });
};

export const trackMetricDeleted = (metricId: string, label: string) => {
  trackEvent(EVENTS.METRIC_DELETED, { metric_id: metricId, metric_label: label });
};

const trackMetricPresented = (metricId: string, label: string, query?: string) => {
  trackEvent(EVENTS.METRIC_PRESENTED, { metric_id: metricId, metric_label: label, ...(query && { query }) });
};

export const trackMetricRefreshed = (queryHash: string) => {
  trackEvent(EVENTS.METRIC_REFRESHED, { query_hash: queryHash });
};

const trackFunnelTabOpened = () => {
  trackEvent(EVENTS.FUNNEL_TAB_OPENED, { timestamp: Date.now() });
};

export const trackQueryExecuted = (queryLength: number, resultRows: number, latencyMs?: number) => {
  trackEvent(EVENTS.QUERY_EXECUTED, { query_length: queryLength, result_rows: resultRows, latency_ms: latencyMs });
};

export const trackQueryError = (errorMessage: string, latencyMs?: number) => {
  trackEvent(EVENTS.QUERY_ERROR, { error_message: errorMessage, latency_ms: latencyMs });
};

export const trackDashboardCreationOpened = () => {
  trackEvent(EVENTS.DASHBOARD_CREATION_OPENED);
};

export const trackDashboardCreationClosed = () => {
  trackEvent(EVENTS.DASHBOARD_CREATION_CLOSED);
};

const trackDashboardCreated = (dashboardId: string, description: string) => {
  trackEvent(EVENTS.DASHBOARD_CREATED, { dashboard_id: dashboardId, description });
};
