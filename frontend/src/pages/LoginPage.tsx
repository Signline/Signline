import React, { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { KeyRound, Lock, ShieldCheck } from 'lucide-react';
import { BrandLogo, UpstreamAttribution } from '@/lib/branding';
import { useAuth } from '@/lib/auth';
import { api, setStoredToken } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const LoginPage: React.FC = () => {
  const navigate = useNavigate();
  const { refreshAuth } = useAuth();
  const { toast } = useToast();

  const [mode, setMode] = useState<'login' | 'otp' | 'reset'>('login');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [otpAttempt, setOtpAttempt] = useState('');
  const [resetToken, setResetToken] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [loading, setLoading] = useState(false);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      const res = await api.post<any>('/api/auth/login', {
        email: email.trim(),
        password,
        otp_attempt: otpAttempt.trim() || undefined,
      });

      if (res.requires_otp || res.otp_required) {
        setMode('otp');
        toast({
          title: 'Two-Factor Authentication Required',
          description: 'Please enter the 6-digit code from your authenticator app.',
        });
        setLoading(false);
        return;
      }

      if (res.token) {
        setStoredToken(res.token);
      }
      await refreshAuth();
      toast({
        title: 'Signed In',
        variant: 'success',
      });
      navigate('/');
    } catch (err: any) {
      if (err.data?.requires_otp || err.message?.toLowerCase().includes('otp')) {
        setMode('otp');
      } else {
        toast({
          title: 'Sign in failed',
          description: err.message,
          variant: 'destructive',
        });
      }
    } finally {
      setLoading(false);
    }
  };

  const handleRequestReset = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      const res = await api.post<any>('/api/auth/password/reset', { email: email.trim() });
      if (res.reset_token) {
        setResetToken(res.reset_token);
      }
      toast({
        title: 'Password Reset Initiated',
        description: 'Check your email or enter your reset token below.',
        variant: 'success',
      });
    } catch (err: any) {
      toast({
        title: 'Reset request failed',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  };

  const handleConfirmReset = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      await api.put('/api/auth/password/reset', {
        token: resetToken.trim(),
        password: newPassword,
      });
      toast({
        title: 'Password Updated',
        description: 'You can now sign in with your new password.',
        variant: 'success',
      });
      setMode('login');
    } catch (err: any) {
      toast({
        title: 'Failed to reset password',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex flex-col justify-between bg-background px-4 py-8">
      <div className="mx-auto w-full max-w-md my-auto space-y-6">
        <div className="text-center space-y-2">
          <div className="inline-flex justify-center">
            <BrandLogo className="h-10 w-10" />
          </div>
          <p className="text-sm text-muted-foreground">
            Sign in to manage templates, signature requests, and audit trails
          </p>
        </div>

        <Card className="shadow-lg">
          {mode === 'login' && (
            <>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Lock className="h-5 w-5 text-primary" />
                  Sign In to Your Account
                </CardTitle>
                <CardDescription>
                  Enter your email and password to access your workspace.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <form onSubmit={handleLogin} className="space-y-4">
                  <div className="space-y-1.5">
                    <Label htmlFor="login-email">Email Address</Label>
                    <Input
                      id="login-email"
                      type="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      placeholder="you@example.com"
                      required
                    />
                  </div>
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <Label htmlFor="login-password">Password</Label>
                      <button
                        type="button"
                        onClick={() => setMode('reset')}
                        className="text-xs text-primary hover:underline"
                      >
                        Forgot password?
                      </button>
                    </div>
                    <Input
                      id="login-password"
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      placeholder="••••••••••••"
                      required
                    />
                  </div>

                  <Button type="submit" className="w-full" disabled={loading}>
                    {loading ? 'Signing In...' : 'Sign In'}
                  </Button>
                </form>
              </CardContent>
            </>
          )}

          {mode === 'otp' && (
            <>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <KeyRound className="h-5 w-5 text-primary" />
                  Two-Factor Authentication (TOTP)
                </CardTitle>
                <CardDescription>
                  Enter the 6-digit verification code from your authenticator app.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <form onSubmit={handleLogin} className="space-y-4">
                  <div className="space-y-1.5">
                    <Label htmlFor="login-otp">6-Digit Authenticator Code</Label>
                    <Input
                      id="login-otp"
                      value={otpAttempt}
                      onChange={(e) => setOtpAttempt(e.target.value)}
                      placeholder="123456"
                      maxLength={6}
                      autoFocus
                      required
                    />
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => setMode('login')}
                      className="flex-1"
                    >
                      Back
                    </Button>
                    <Button type="submit" className="flex-1" disabled={loading}>
                      {loading ? 'Verifying...' : 'Verify & Sign In'}
                    </Button>
                  </div>
                </form>
              </CardContent>
            </>
          )}

          {mode === 'reset' && (
            <>
              <CardHeader>
                <CardTitle>Reset Password</CardTitle>
                <CardDescription>
                  Request a password reset token or enter an existing token below.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <form onSubmit={handleRequestReset} className="space-y-3">
                  <div className="space-y-1.5">
                    <Label htmlFor="reset-email">Account Email</Label>
                    <Input
                      id="reset-email"
                      type="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      placeholder="you@example.com"
                      required
                    />
                  </div>
                  <Button type="submit" variant="secondary" className="w-full" disabled={loading}>
                    Send Password Reset Link
                  </Button>
                </form>

                <form onSubmit={handleConfirmReset} className="space-y-3 pt-3 border-t border-border">
                  <div className="space-y-1.5">
                    <Label htmlFor="reset-token">Reset Token</Label>
                    <Input
                      id="reset-token"
                      value={resetToken}
                      onChange={(e) => setResetToken(e.target.value)}
                      placeholder="Paste reset token..."
                      required
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="reset-new-pw">New Password</Label>
                    <Input
                      id="reset-new-pw"
                      type="password"
                      value={newPassword}
                      onChange={(e) => setNewPassword(e.target.value)}
                      placeholder="••••••••••••"
                      required
                    />
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => setMode('login')}
                      className="flex-1"
                    >
                      Back to Login
                    </Button>
                    <Button type="submit" className="flex-1" disabled={loading}>
                      Update Password
                    </Button>
                  </div>
                </form>
              </CardContent>
            </>
          )}
        </Card>

        <div className="text-center text-xs text-muted-foreground">
          <Link to="/verify" className="inline-flex items-center gap-1 hover:text-foreground hover:underline">
            <ShieldCheck className="h-3.5 w-3.5 text-primary" />
            Verify a signed PDF document without signing in
          </Link>
        </div>
      </div>

      <footer className="pt-8 text-center">
        <UpstreamAttribution />
      </footer>
    </div>
  );
};
