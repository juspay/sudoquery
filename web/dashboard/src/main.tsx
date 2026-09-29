import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { ThemeProvider, CssBaseline } from '@mui/material'
import 'flowtoken/dist/styles.css'
import 'streamdown/styles.css';
import './index.css'
import App from './App.tsx'
import { AuthProvider } from './contexts/AuthContext'
import { OrganizationProvider } from './contexts/OrganizationContext'
import { ProjectProvider } from './contexts/ProjectContext'
import { ToastProvider } from './contexts/ToastContext'
import { ChatProvider } from './contexts/ChatContext'
import { SchemaProvider } from './contexts/SchemaContext'
import { createAppTheme } from './theme/theme'
import { initAnalytics } from './utils/analytics'
import MaintenanceModePage from './pages/MaintenanceModePage'

// Polyfill crypto.randomUUID for non-secure contexts (HTTP)
if (typeof crypto !== 'undefined' && typeof crypto.randomUUID !== 'function') {
  crypto.randomUUID = function () {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      const v = c === 'x' ? r : (r & 0x3) | 0x8;
      return v.toString(16);
    });
  };
}

// Initialize analytics SDK
initAnalytics();

const theme = createAppTheme();

const isMaintenanceMode = import.meta.env.VITE_MAINTENANCE_MODE === 'true';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {isMaintenanceMode ? (
      <ThemeProvider theme={theme}>
        <CssBaseline />
        <MaintenanceModePage />
      </ThemeProvider>
    ) : (
      <BrowserRouter>
        <AuthProvider>
          <ThemeProvider theme={theme}>
            <CssBaseline />
            <ToastProvider>
              <OrganizationProvider>
                <ProjectProvider>
                  <SchemaProvider>
                    <ChatProvider>
                      <App />
                    </ChatProvider>
                  </SchemaProvider>
                </ProjectProvider>
              </OrganizationProvider>
            </ToastProvider>
          </ThemeProvider>
        </AuthProvider>
      </BrowserRouter>
    )}
  </StrictMode>,
)
