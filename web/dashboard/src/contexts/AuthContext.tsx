import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { authService, type AuthState } from '../services/authService';
import { HyperAnalytics } from '../utils/analytics';

interface AuthContextType extends AuthState {
  login: (username: string, password: string, isAdmin?: boolean, realm?: string) => Promise<void>;
  googleLogin: (idpAlias?: string) => void;
  logout: () => void;
  loading: boolean;
  setAuthState: React.Dispatch<React.SetStateAction<AuthState>>;
}

const AuthContext = createContext<AuthContextType | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [authState, setAuthState] = useState<AuthState>({
    isAuthenticated: false,
    isAdmin: false,
    token: null,
    user: null,
  });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const stored = authService.getStoredAuth();
    if (stored) {
      setAuthState(stored);
      // Identify user in analytics
      if (stored.user?.sub) {
        HyperAnalytics.setUser(stored.user.sub);
      }
    }
    setLoading(false);
  }, []);

  const login = async (username: string, password: string, isAdmin = false, realm?: string) => {
    let state: AuthState;
    if (isAdmin) {
      state = await authService.adminLogin(username, password);
    } else if (realm) {
      state = await authService.userLogin(username, password, realm);
    } else {
      throw new Error('Realm is required for user login');
    }
    setAuthState(state);
    // Identify user in analytics
    if (state.user?.sub) {
      HyperAnalytics.setUser(state.user.sub);
    }
  };

  const googleLogin = (idpAlias?: string) => {
    authService.initiateGoogleLogin(idpAlias);
  };

  const logout = () => {
    authService.logout();
    setAuthState({
      isAuthenticated: false,
      isAdmin: false,
      token: null,
      user: null,
    });
    // Remove user from analytics
    HyperAnalytics.removeUser();
  };

  return (
    <AuthContext.Provider value={{ ...authState, login, googleLogin, logout, loading, setAuthState }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
