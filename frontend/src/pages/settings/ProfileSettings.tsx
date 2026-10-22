import React, { useState, useEffect } from 'react';
import { User, Shield, PenTool, Save, QrCode } from 'lucide-react';
import { useAuth } from '@/lib/auth';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';
import { SignaturePadModal } from '@/components/signing/SignaturePadModal';

export const ProfileSettings: React.FC = () => {
  const { user, refreshAuth } = useAuth();
  const { toast } = useToast();

  const [firstName, setFirstName] = useState(user?.first_name || '');
  const [lastName, setLastName] = useState(user?.last_name || '');
  const [email, setEmail] = useState(user?.email || '');
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [signatureDataUrl, setSignatureDataUrl] = useState(user?.signature_url || '');
  const [initialsDataUrl, setInitialsDataUrl] = useState(user?.initials_url || '');
  const [emailAlerts, setEmailAlerts] = useState(true);
  const [saving, setSaving] = useState(false);

  // Modals for signature & initials
  const [sigModalOpen, setSigModalOpen] = useState(false);
  const [initialsModalOpen, setInitialsModalOpen] = useState(false);

  // 2FA TOTP Setup
  const [totpSecret, setTotpSecret] = useState('');
  const [totpUri, setTotpUri] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [otpEnabled, setOtpEnabled] = useState(Boolean(user?.otp_required_for_login));

  useEffect(() => {
    api
      .get<any>('/api/settings/profile')
      .then((res) => {
        const u = res.user || res.data || res;
        if (u.first_name !== undefined) setFirstName(u.first_name);
        if (u.last_name !== undefined) setLastName(u.last_name);
        if (u.email !== undefined) setEmail(u.email);
        if (u.signature_url) setSignatureDataUrl(u.signature_url);
        if (u.initials_url) setInitialsDataUrl(u.initials_url);
        if (u.otp_required_for_login !== undefined) setOtpEnabled(Boolean(u.otp_required_for_login));
        if (res.receive_completed_email !== undefined) {
          setEmailAlerts(Boolean(res.receive_completed_email));
        }
      })
      .catch(() => {});
  }, []);

  const handleSaveProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await api.put('/api/settings/profile', {
        first_name: firstName.trim(),
        last_name: lastName.trim(),
        email: email.trim(),
        current_password: currentPassword || undefined,
        new_password: newPassword || undefined,
        signature_data_url: signatureDataUrl || undefined,
        initials_data_url: initialsDataUrl || undefined,
        receive_completed_email: emailAlerts,
      });
      setCurrentPassword('');
      setNewPassword('');
      await refreshAuth();
      toast({
        title: 'Profile Updated',
        description: 'Your personal profile and signature preferences have been saved.',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Update failed', description: err.message, variant: 'destructive' });
    } finally {
      setSaving(false);
    }
  };

  const handleSetup2fa = async () => {
    try {
      const res = await api.post<any>('/api/auth/2fa/setup', {});
      setTotpSecret(res.secret || res.otp_secret || '');
      setTotpUri(res.provisioning_uri || res.otpauth_url || '');
      toast({
        title: '2FA Secret Generated',
        description: 'Add the secret to your authenticator app and enter the 6-digit code to enable.',
      });
    } catch (err: any) {
      toast({ title: '2FA Setup failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleVerify2fa = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await api.post('/api/auth/2fa/verify', {
        code: totpCode.trim(),
        secret: totpSecret || undefined,
      });
      setOtpEnabled(true);
      setTotpSecret('');
      setTotpCode('');
      await refreshAuth();
      toast({
        title: 'Two-Factor Authentication Enabled',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Invalid 2FA Code', description: err.message, variant: 'destructive' });
    }
  };

  const handleDisable2fa = async () => {
    try {
      await api.delete('/api/auth/2fa');
      setOtpEnabled(false);
      setTotpSecret('');
      await refreshAuth();
      toast({ title: 'Two-Factor Authentication Disabled' });
    } catch (err: any) {
      toast({ title: 'Failed to disable 2FA', description: err.message, variant: 'destructive' });
    }
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <User className="h-5 w-5 text-primary" />
            Profile &amp; Contact Information
          </CardTitle>
          <CardDescription>
            Update your name, email address, password, and saved signature &amp; initials.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSaveProfile} className="space-y-5">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="prof-first">First Name</Label>
                <Input
                  id="prof-first"
                  value={firstName}
                  onChange={(e) => setFirstName(e.target.value)}
                  required
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="prof-last">Last Name</Label>
                <Input
                  id="prof-last"
                  value={lastName}
                  onChange={(e) => setLastName(e.target.value)}
                />
              </div>
            </div>

            <div className="space-y-1.5">
              <Label htmlFor="prof-email">Email Address</Label>
              <Input
                id="prof-email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-2 border-t border-border">
              <div className="space-y-1.5">
                <Label htmlFor="prof-cur-pw">Current Password (only to change password)</Label>
                <Input
                  id="prof-cur-pw"
                  type="password"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  placeholder="••••••••••••"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="prof-new-pw">New Password</Label>
                <Input
                  id="prof-new-pw"
                  type="password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  placeholder="Leave blank to keep existing"
                />
              </div>
            </div>

            {/* Saved Signature & Initials */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-2 border-t border-border">
              <div className="rounded-lg border border-border bg-secondary/30 p-3 space-y-2">
                <div className="flex items-center justify-between">
                  <Label className="text-xs font-semibold">Saved Signature</Label>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => setSigModalOpen(true)}
                  >
                    <PenTool className="h-3.5 w-3.5" />
                    Update
                  </Button>
                </div>
                <div className="h-20 rounded bg-white border border-border flex items-center justify-center p-2">
                  {signatureDataUrl ? (
                    <img
                      src={signatureDataUrl}
                      alt="Saved signature"
                      className="max-h-full object-contain"
                    />
                  ) : (
                    <span className="text-xs text-muted-foreground">No signature saved</span>
                  )}
                </div>
              </div>

              <div className="rounded-lg border border-border bg-secondary/30 p-3 space-y-2">
                <div className="flex items-center justify-between">
                  <Label className="text-xs font-semibold">Saved Initials</Label>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => setInitialsModalOpen(true)}
                  >
                    <PenTool className="h-3.5 w-3.5" />
                    Update
                  </Button>
                </div>
                <div className="h-20 rounded bg-white border border-border flex items-center justify-center p-2">
                  {initialsDataUrl ? (
                    <img
                      src={initialsDataUrl}
                      alt="Saved initials"
                      className="max-h-full object-contain"
                    />
                  ) : (
                    <span className="text-xs text-muted-foreground">No initials saved</span>
                  )}
                </div>
              </div>
            </div>

            <div className="flex items-center justify-between rounded-lg border border-border p-3">
              <div className="text-xs">
                <div className="font-semibold">Email Notifications on Completion</div>
                <div className="text-muted-foreground">
                  Receive an email alert when signers complete or decline your documents
                </div>
              </div>
              <Switch checked={emailAlerts} onCheckedChange={setEmailAlerts} />
            </div>

            <Button type="submit" disabled={saving}>
              <Save className="h-4 w-4" />
              {saving ? 'Saving...' : 'Save Profile Settings'}
            </Button>
          </form>
        </CardContent>
      </Card>

      {/* Two-Factor Authentication Card */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2 text-base">
              <Shield className="h-5 w-5 text-primary" />
              Two-Factor Authentication (TOTP 2FA)
            </CardTitle>
            <Badge variant={otpEnabled ? 'success' : 'secondary'}>
              {otpEnabled ? 'Enabled' : 'Disabled'}
            </Badge>
          </div>
          <CardDescription>
            Protect your account with RFC 6238 time-based one-time passwords (Google Authenticator, 1Password, Authy).
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {otpEnabled ? (
            <div className="flex items-center justify-between rounded-lg border border-emerald-500/40 bg-emerald-50 p-3 text-xs text-emerald-900">
              <span>Two-Factor Authentication is currently active on your account.</span>
              <Button variant="destructive" size="sm" onClick={handleDisable2fa}>
                Disable 2FA
              </Button>
            </div>
          ) : !totpSecret ? (
            <Button variant="outline" onClick={handleSetup2fa}>
              <QrCode className="h-4 w-4" />
              Set Up Two-Factor Authentication
            </Button>
          ) : (
            <form onSubmit={handleVerify2fa} className="space-y-3 rounded-lg border border-border bg-secondary/30 p-4">
              <div className="text-xs space-y-1">
                <div className="font-semibold">Authenticator Secret Key:</div>
                <code className="block rounded bg-card p-2 border border-border font-mono select-all">
                  {totpSecret}
                </code>
                {totpUri && (
                  <div className="text-[11px] text-muted-foreground break-all">
                    URI: <code>{totpUri}</code>
                  </div>
                )}
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="totp-verify-input" className="text-xs">
                  Enter 6-Digit Verification Code from App
                </Label>
                <div className="flex items-center gap-2">
                  <Input
                    id="totp-verify-input"
                    value={totpCode}
                    onChange={(e) => setTotpCode(e.target.value)}
                    placeholder="123456"
                    maxLength={6}
                    className="max-w-[180px]"
                    required
                  />
                  <Button type="submit">Verify &amp; Activate 2FA</Button>
                </div>
              </div>
            </form>
          )}
        </CardContent>
      </Card>

      <SignaturePadModal
        open={sigModalOpen}
        onOpenChange={setSigModalOpen}
        title="Update Saved Signature"
        defaultTypedName={`${firstName} ${lastName}`.trim()}
        onSave={setSignatureDataUrl}
      />
      <SignaturePadModal
        open={initialsModalOpen}
        onOpenChange={setInitialsModalOpen}
        title="Update Saved Initials"
        defaultTypedName={`${firstName.charAt(0)}${lastName.charAt(0)}`.toUpperCase()}
        onSave={setInitialsDataUrl}
      />
    </div>
  );
};
