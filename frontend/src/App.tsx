import React from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { BrandingProvider } from '@/lib/branding';
import { AuthProvider, useAuth } from '@/lib/auth';
import { ToastProvider } from '@/components/ui/toast';
import { AppLayout } from '@/components/layout/AppLayout';
import { SettingsLayout } from '@/components/layout/SettingsLayout';
import { PublicLayout } from '@/components/layout/PublicLayout';

import { SetupPage } from '@/pages/SetupPage';
import { LoginPage } from '@/pages/LoginPage';
import { DashboardPage } from '@/pages/DashboardPage';
import { TemplateViewPage } from '@/pages/TemplateViewPage';
import { TemplateEditPage } from '@/pages/TemplateEditPage';
import { SubmissionsPage } from '@/pages/SubmissionsPage';
import { PublicSignPage } from '@/pages/PublicSignPage';
import { PublicStartFormPage } from '@/pages/PublicStartFormPage';
import { PublicPreviewPage } from '@/pages/PublicPreviewPage';
import { VerifyPdfPage } from '@/pages/VerifyPdfPage';

import { ProfileSettings } from '@/pages/settings/ProfileSettings';
import { AccountSettings } from '@/pages/settings/AccountSettings';
import { PersonalizationSettings } from '@/pages/settings/PersonalizationSettings';
import { EmailSettings } from '@/pages/settings/EmailSettings';
import { NotificationsSettings } from '@/pages/settings/NotificationsSettings';
import { EsignSettings } from '@/pages/settings/EsignSettings';
import { UsersSettings } from '@/pages/settings/UsersSettings';
import { WebhooksSettings } from '@/pages/settings/WebhooksSettings';
import { StorageSettings } from '@/pages/settings/StorageSettings';
import { ApiSettings } from '@/pages/settings/ApiSettings';
import { McpSettings } from '@/pages/settings/McpSettings';

const RequireAuth: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { user, needsSetup, loading } = useAuth();

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background text-sm text-muted-foreground">
        Loading Signline...
      </div>
    );
  }

  if (needsSetup) {
    return <Navigate to="/setup" replace />;
  }

  if (!user) {
    return <Navigate to="/sign_in" replace />;
  }

  return <>{children}</>;
};

export const App: React.FC = () => {
  return (
    <BrandingProvider>
      <ToastProvider>
        <AuthProvider>
          <BrowserRouter>
            <Routes>
              {/* Setup & Login */}
              <Route path="/setup" element={<SetupPage />} />
              <Route path="/sign_in" element={<LoginPage />} />
              <Route path="/login" element={<Navigate to="/sign_in" replace />} />

              {/* Public Signing, Shared Forms, Preview & Cryptographic Verifier */}
              <Route element={<PublicLayout />}>
                <Route path="/s/:slug" element={<PublicSignPage />} />
                <Route path="/d/:slug" element={<PublicStartFormPage />} />
                <Route path="/e/:slug" element={<PublicPreviewPage />} />
                <Route path="/verify" element={<VerifyPdfPage />} />
              </Route>

              {/* Authenticated Dashboard, Builder, Submissions & Settings */}
              <Route
                element={
                  <RequireAuth>
                    <AppLayout />
                  </RequireAuth>
                }
              >
                <Route path="/" element={<DashboardPage />} />
                <Route path="/templates/:id" element={<TemplateViewPage />} />
                <Route path="/templates/:id/edit" element={<TemplateEditPage />} />
                <Route path="/submissions" element={<SubmissionsPage />} />
                <Route path="/submissions/:id" element={<SubmissionsPage />} />

                <Route path="/settings" element={<SettingsLayout />}>
                  <Route index element={<Navigate to="/settings/profile" replace />} />
                  <Route path="profile" element={<ProfileSettings />} />
                  <Route path="account" element={<AccountSettings />} />
                  <Route path="personalization" element={<PersonalizationSettings />} />
                  <Route path="email" element={<EmailSettings />} />
                  <Route path="notifications" element={<NotificationsSettings />} />
                  <Route path="esign" element={<EsignSettings />} />
                  <Route path="users" element={<UsersSettings />} />
                  <Route path="webhooks" element={<WebhooksSettings />} />
                  <Route path="storage" element={<StorageSettings />} />
                  <Route path="api" element={<ApiSettings />} />
                  <Route path="mcp" element={<McpSettings />} />
                </Route>
              </Route>

              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </BrowserRouter>
        </AuthProvider>
      </ToastProvider>
    </BrandingProvider>
  );
};

export default App;
