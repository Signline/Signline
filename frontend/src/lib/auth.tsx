import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import type { User, Account } from '@/types';
import { api, setStoredToken } from '@/lib/api';

interface AuthContextValue {
  user: User | null;
  account: Account | null;
  needsSetup: boolean;
  loading: boolean;
  refreshAuth: () => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue>({
  user: null,
  account: null,
  needsSetup: false,
  loading: true,
  refreshAuth: async () => {},
  logout: async () => {},
});

export const AuthProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [user, setUser] = useState<User | null>(null);
  const [account, setAccount] = useState<Account | null>(null);
  const [needsSetup, setNeedsSetup] = useState(false);
  const [loading, setLoading] = useState(true);

  const refreshAuth = useCallback(async () => {
    try {
      const statusRes = await api.get<{ needs_setup?: boolean }>('/api/setup/status').catch(() => ({
        needs_setup: false,
      }));
      if (statusRes && statusRes.needs_setup) {
        setNeedsSetup(true);
        setUser(null);
        setAccount(null);
        setLoading(false);
        return;
      }
      setNeedsSetup(false);

      const meRes = await api.get<any>('/api/auth/me');
      const resolvedUser = meRes.user || (meRes.email ? meRes : null);
      const resolvedAccount = meRes.account || null;
      setUser(resolvedUser);
      setAccount(resolvedAccount);
    } catch {
      setUser(null);
      setAccount(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshAuth();
  }, [refreshAuth]);

  const logout = useCallback(async () => {
    try {
      await api.post('/api/auth/logout');
    } catch {
      // ignore
    }
    setStoredToken(null);
    setUser(null);
    setAccount(null);
  }, []);

  return (
    <AuthContext.Provider value={{ user, account, needsSetup, loading, refreshAuth, logout }}>
      {children}
    </AuthContext.Provider>
  );
};

export function useAuth() {
  return useContext(AuthContext);
}
