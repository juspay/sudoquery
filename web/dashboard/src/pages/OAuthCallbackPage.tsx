// OAuthCallbackPage wraps the existing OAuthCallback component as a route-compatible page.
// Phase 2 will update OAuthCallback to redirect based on org count.
import { useAuth } from '../contexts/AuthContext';
import { authService } from '../services/authService';
import { OAuthCallback } from '../components/Auth/OAuthCallback';
import { HyperAnalytics } from '../utils/analytics';

export default function OAuthCallbackPage() {
  const { setAuthState } = useAuth();

  const handleAuthSuccess = () => {
    const stored = authService.getStoredAuth();
    if (stored) {
      setAuthState(stored);
      // Identify user in analytics
      if (stored.user?.sub) {
        HyperAnalytics.setUser(stored.user.sub);
      }
    }
  };

  return <OAuthCallback onAuthSuccess={handleAuthSuccess} />;
}
