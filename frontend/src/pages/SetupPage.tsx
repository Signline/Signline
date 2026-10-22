import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ShieldCheck, Sparkles, ArrowRight } from 'lucide-react';
import { BrandLogo, UpstreamAttribution } from '@/lib/branding';
import { useAuth } from '@/lib/auth';
import { api, setStoredToken } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const SetupPage: React.FC = () => {
  const navigate = useNavigate();
  const { refreshAuth } = useAuth();
  const { toast } = useToast();

  const [firstName, setFirstName] = useState('');
  const [lastName, setLastName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [accountName, setAccountName] = useState('Signline Organization');
  const [timezone, setTimezone] = useState(
    Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  );
  const [locale, setLocale] = useState('en-US');
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      const res = await api.post<any>('/api/setup', {
        first_name: firstName.trim(),
        last_name: lastName.trim(),
        email: email.trim(),
        password,
        account_name: accountName.trim(),
        timezone,
        locale,
      });
      if (res.token) {
        setStoredToken(res.token);
      }
      await refreshAuth();
      toast({
        title: 'Signline Setup Complete',
        description: 'Provisioned admin account, Default folder, API key, and X.509 signing certificate.',
        variant: 'success',
      });
      navigate('/');
    } catch (err: any) {
      toast({
        title: 'Setup Error',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex flex-col justify-between bg-background px-4 py-8">
      <div className="mx-auto w-full max-w-lg my-auto space-y-6">
        <div className="text-center space-y-2">
          <div className="inline-flex justify-center">
            <BrandLogo className="h-10 w-10" />
          </div>
          <p className="text-sm text-muted-foreground">
            Initial Instance Setup &amp; Cryptographic Key Provisioning
          </p>
        </div>

        <Card className="shadow-lg">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Sparkles className="h-5 w-5 text-primary" />
              Create Administrator &amp; Organization
            </CardTitle>
            <CardDescription>
              Set up your primary administrator profile. A 2048-bit RSA X.509 signing certificate and
              REST API access token will be generated automatically.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleSubmit} className="space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label htmlFor="setup-first-name">First Name</Label>
                  <Input
                    id="setup-first-name"
                    value={firstName}
                    onChange={(e) => setFirstName(e.target.value)}
                    placeholder="Jane"
                    required
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="setup-last-name">Last Name</Label>
                  <Input
                    id="setup-last-name"
                    value={lastName}
                    onChange={(e) => setLastName(e.target.value)}
                    placeholder="Doe"
                    required
                  />
                </div>
              </div>

              <div className="space-y-1.5">
                <Label htmlFor="setup-email">Administrator Email</Label>
                <Input
                  id="setup-email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="admin@example.com"
                  required
                />
              </div>

              <div className="space-y-1.5">
                <Label htmlFor="setup-password">Password (min 8 characters)</Label>
                <Input
                  id="setup-password"
                  type="password"
                  minLength={6}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="••••••••••••"
                  required
                />
              </div>

              <div className="space-y-1.5">
                <Label htmlFor="setup-org">Organization / Account Name</Label>
                <Input
                  id="setup-org"
                  value={accountName}
                  onChange={(e) => setAccountName(e.target.value)}
                  placeholder="Acme Corp"
                  required
                />
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label htmlFor="setup-tz">Timezone</Label>
                  <Input
                    id="setup-tz"
                    value={timezone}
                    onChange={(e) => setTimezone(e.target.value)}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="setup-locale">Locale</Label>
                  <Select value={locale} onChange={(e) => setLocale(e.target.value)}>
                    <option value="en-US">English (US)</option>
                    <option value="en-GB">English (UK)</option>
                    <option value="es-ES">Español</option>
                    <option value="fr-FR">Français</option>
                    <option value="de-DE">Deutsch</option>
                  </Select>
                </div>
              </div>

              <div className="rounded-lg border border-border bg-secondary/40 p-3 text-xs text-muted-foreground flex items-start gap-2">
                <ShieldCheck className="h-4 w-4 text-primary shrink-0 mt-0.5" />
                <span>
                  All documents signed on this instance will be sealed with PKCS#7 / CMS digital
                  signatures and SHA-256 tamper verification.
                </span>
              </div>

              <Button type="submit" className="w-full" size="lg" disabled={submitting}>
                {submitting ? 'Initializing Signline...' : 'Complete Setup & Launch Dashboard'}
                <ArrowRight className="h-4 w-4" />
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>

      <footer className="pt-8 text-center">
        <UpstreamAttribution />
      </footer>
    </div>
  );
};
