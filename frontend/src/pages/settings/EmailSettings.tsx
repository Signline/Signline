import React, { useState, useEffect, useCallback } from 'react';
import {
  Mail,
  Save,
  Send,
  Link2,
  Unlink,
  CheckCircle2,
  FileText,
  Activity,
} from 'lucide-react';
import { useSearchParams } from 'react-router-dom';
import { formatDate } from '@/lib/utils';
import { api, unwrapList } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const EmailSettings: React.FC = () => {
  const { toast } = useToast();
  const [searchParams, setSearchParams] = useSearchParams();

  // Gmail OAuth 2.0 state
  const [gmailClientId, setGmailClientId] = useState('');
  const [gmailClientSecret, setGmailClientSecret] = useState('');
  const [gmailConnected, setGmailConnected] = useState(false);
  const [gmailEmail, setGmailEmail] = useState('');
  const [gmailConnectedAt, setGmailConnectedAt] = useState('');
  const [savingGmail, setSavingGmail] = useState(false);

  // SMTP state
  const [smtpHost, setSmtpHost] = useState('');
  const [smtpPort, setSmtpPort] = useState('587');
  const [smtpUser, setSmtpUser] = useState('');
  const [smtpPassword, setSmtpPassword] = useState('');
  const [smtpFrom, setSmtpFrom] = useState('noreply@signline.local');
  const [smtpSecurity, setSmtpSecurity] = useState('starttls');
  const [testRecipient, setTestRecipient] = useState('');
  const [savingSmtp, setSavingSmtp] = useState(false);

  // Custom Email Templates
  const [tplType, setTplType] = useState('invitation');
  const [tplSubject, setTplSubject] = useState('You have been invited to sign {template.name}');
  const [tplBody, setTplBody] = useState(
    'Hello {submitter.name},\n\nPlease review and sign "{template.name}" using the link below:\n{submitter.link}\n\nThank you!'
  );

  // Email delivery events
  const [emailEvents, setEmailEvents] = useState<any[]>([]);

  const redirectUri = `${window.location.origin}/api/settings/email/gmail/callback`;

  const loadSettings = useCallback(async () => {
    try {
      const [smtpRes, gmailRes, tplRes] = await Promise.all([
        api.get<any>('/api/settings/email/smtp').catch(() => ({})),
        api.get<any>('/api/settings/email/gmail').catch(() => ({})),
        api.get<any>('/api/settings/email/templates').catch(() => ({})),
      ]);

      const smtp = smtpRes.smtp || smtpRes.data || smtpRes;
      if (smtp.host) setSmtpHost(smtp.host);
      if (smtp.port) setSmtpPort(String(smtp.port));
      if (smtp.username || smtp.user_name) setSmtpUser(smtp.username || smtp.user_name);
      if (smtp.from || smtp.from_email) setSmtpFrom(smtp.from || smtp.from_email);
      if (smtp.security || smtp.authentication) setSmtpSecurity(smtp.security || 'starttls');
      if (smtpRes.events) setEmailEvents(unwrapList(smtpRes.events));

      const gm = gmailRes.gmail || gmailRes.data || gmailRes;
      if (gm.client_id) setGmailClientId(gm.client_id);
      setGmailConnected(Boolean(gm.connected));
      setGmailEmail(gm.email || '');
      setGmailConnectedAt(gm.connected_at || '');

      const tpls = unwrapList(tplRes, 'templates');
      if (tpls.length > 0 && tpls[0].subject) {
        setTplSubject(tpls[0].subject);
        setTplBody(tpls[0].body);
      }
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // Result of the Google OAuth 2.0 redirect back to this page (?gmail=connected|error)
  useEffect(() => {
    const result = searchParams.get('gmail');
    if (!result) return;
    if (result === 'connected') {
      toast({
        title: 'Gmail Connected',
        description: 'Gmail OAuth 2.0 is now used to send emails.',
        variant: 'success',
      });
    } else {
      toast({
        title: 'Connect Gmail failed',
        description: searchParams.get('message') || 'Google authorization was not completed.',
        variant: 'destructive',
      });
    }
    const next = new URLSearchParams(searchParams);
    next.delete('gmail');
    next.delete('message');
    setSearchParams(next, { replace: true });
  }, [searchParams, setSearchParams, toast]);

  const handleSaveGmail = async (e: React.FormEvent) => {
    e.preventDefault();
    setSavingGmail(true);
    try {
      const res = await api.post<any>('/api/settings/email/gmail', {
        client_id: gmailClientId.trim(),
        client_secret: gmailClientSecret.trim() || undefined,
      });
      toast({
        title: 'Gmail OAuth 2.0 Credentials Saved',
        description: 'You can now connect your Google Workspace or Gmail mailbox.',
        variant: 'success',
      });
      if (res.connected !== undefined) setGmailConnected(Boolean(res.connected));
      await loadSettings();
    } catch (err: any) {
      toast({ title: 'Failed to save Gmail config', description: err.message, variant: 'destructive' });
    } finally {
      setSavingGmail(false);
    }
  };

  const handleConnectGmail = async () => {
    try {
      const res = await api.get<any>('/api/settings/email/gmail/connect');
      if (!res.authorization_url) {
        throw new Error('No authorization URL was returned');
      }
      // Google redirects back to /api/settings/email/gmail/callback, which returns to this page
      window.location.assign(res.authorization_url);
    } catch (err: any) {
      toast({ title: 'Connect Gmail failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleDisconnectGmail = async () => {
    try {
      await api.delete('/api/settings/email/gmail');
      setGmailConnected(false);
      setGmailEmail('');
      toast({ title: 'Disconnected Gmail Mailbox' });
      await loadSettings();
    } catch (err: any) {
      toast({ title: 'Disconnect failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleSaveSmtp = async (e: React.FormEvent) => {
    e.preventDefault();
    setSavingSmtp(true);
    try {
      await api.put('/api/settings/email/smtp', {
        host: smtpHost.trim(),
        port: Number(smtpPort) || 587,
        username: smtpUser.trim(),
        password: smtpPassword || undefined,
        from_email: smtpFrom.trim(),
        security: smtpSecurity,
      });
      toast({
        title: 'SMTP Configuration Saved',
        variant: 'success',
      });
      await loadSettings();
    } catch (err: any) {
      toast({ title: 'Failed to save SMTP', description: err.message, variant: 'destructive' });
    } finally {
      setSavingSmtp(false);
    }
  };

  const handleSendTestEmail = async () => {
    try {
      await api.post('/api/settings/email/test', {
        to: testRecipient.trim() || undefined,
      });
      toast({
        title: 'Test Email Dispatched',
        description: `Sent via ${gmailConnected ? 'Gmail OAuth 2.0 API' : 'SMTP'}.`,
        variant: 'success',
      });
      await loadSettings();
    } catch (err: any) {
      toast({ title: 'Test email failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleSaveTemplate = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await api.post('/api/settings/email/templates', {
        tag: tplType,
        subject: tplSubject,
        body: tplBody,
      });
      toast({
        title: 'Email Template Saved',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Save failed', description: err.message, variant: 'destructive' });
    }
  };

  return (
    <div className="space-y-6">
      {/* 1. Gmail OAuth 2.0 Card */}
      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <CardTitle className="flex items-center gap-2">
                <Mail className="h-5 w-5 text-primary" />
                Gmail &amp; Google Workspace OAuth 2.0 Integration
              </CardTitle>
              <CardDescription>
                Send signature requests, automated reminders, and signed document copies through the Gmail REST API (`gmail.send`).
              </CardDescription>
            </div>
            <Badge variant={gmailConnected ? 'success' : 'secondary'}>
              {gmailConnected ? `Connected (${gmailEmail || 'Gmail API'})` : 'Not Connected'}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {gmailConnected && (
            <Alert variant="success">
              <AlertTitle className="flex items-center gap-1.5 text-xs">
                <CheckCircle2 className="h-4 w-4 text-emerald-700" />
                Gmail API Active — Takes Precedence Over SMTP
              </AlertTitle>
              <AlertDescription className="text-xs mt-1">
                Connected mailbox: <strong>{gmailEmail || 'Configured Gmail Account'}</strong>
                {gmailConnectedAt && ` since ${formatDate(gmailConnectedAt)}`}. All outgoing emails
                are dispatched via OAuth 2.0 MIME over HTTPS.
              </AlertDescription>
            </Alert>
          )}

          <form onSubmit={handleSaveGmail} className="space-y-4">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="gmail-client-id">Google OAuth Client ID</Label>
                <Input
                  id="gmail-client-id"
                  value={gmailClientId}
                  onChange={(e) => setGmailClientId(e.target.value)}
                  placeholder="1234567890-abc.apps.googleusercontent.com"
                  required
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="gmail-client-secret">Google OAuth Client Secret</Label>
                <Input
                  id="gmail-client-secret"
                  type="password"
                  value={gmailClientSecret}
                  onChange={(e) => setGmailClientSecret(e.target.value)}
                  placeholder="GOCSPX-••••••••••••"
                />
              </div>
            </div>

            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">Authorized Redirect URI</Label>
              <code className="block rounded border border-border bg-secondary/40 px-3 py-1.5 text-xs font-mono select-all">
                {redirectUri}
              </code>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <Button type="submit" disabled={savingGmail}>
                <Save className="h-4 w-4" />
                Save Credentials
              </Button>
              {gmailClientId && (
                <Button type="button" variant="secondary" onClick={handleConnectGmail}>
                  <Link2 className="h-4 w-4" />
                  {gmailConnected ? 'Reconnect Gmail OAuth' : 'Connect Gmail'}
                </Button>
              )}
              {gmailConnected && (
                <Button type="button" variant="destructive" onClick={handleDisconnectGmail}>
                  <Unlink className="h-4 w-4" />
                  Disconnect
                </Button>
              )}
            </div>
          </form>
        </CardContent>
      </Card>

      {/* 2. SMTP Delivery Configuration */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">SMTP Server Configuration</CardTitle>
          <CardDescription>
            Standard SMTP relay settings (used automatically whenever Gmail OAuth 2.0 is not connected).
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <form onSubmit={handleSaveSmtp} className="space-y-4">
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <div className="space-y-1.5 sm:col-span-2">
                <Label htmlFor="smtp-host">SMTP Host</Label>
                <Input
                  id="smtp-host"
                  value={smtpHost}
                  onChange={(e) => setSmtpHost(e.target.value)}
                  placeholder="smtp.example.com"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="smtp-port">Port</Label>
                <Input
                  id="smtp-port"
                  type="number"
                  value={smtpPort}
                  onChange={(e) => setSmtpPort(e.target.value)}
                  placeholder="587"
                />
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="smtp-user">SMTP Username</Label>
                <Input
                  id="smtp-user"
                  value={smtpUser}
                  onChange={(e) => setSmtpUser(e.target.value)}
                  placeholder="user@example.com"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="smtp-pass">SMTP Password</Label>
                <Input
                  id="smtp-pass"
                  type="password"
                  value={smtpPassword}
                  onChange={(e) => setSmtpPassword(e.target.value)}
                  placeholder="••••••••••••"
                />
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="smtp-from">Sender From Address</Label>
                <Input
                  id="smtp-from"
                  type="email"
                  value={smtpFrom}
                  onChange={(e) => setSmtpFrom(e.target.value)}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="smtp-sec">Encryption / TLS</Label>
                <Select value={smtpSecurity} onChange={(e) => setSmtpSecurity(e.target.value)}>
                  <option value="starttls">STARTTLS (Port 587)</option>
                  <option value="tls">Implicit SSL/TLS (Port 465)</option>
                  <option value="none">None / Plain (Port 25 / 1025)</option>
                </Select>
              </div>
            </div>

            <div className="flex flex-wrap items-center justify-between gap-3 pt-2 border-t border-border">
              <Button type="submit" disabled={savingSmtp}>
                <Save className="h-4 w-4" />
                Save SMTP Settings
              </Button>

              <div className="flex items-center gap-2">
                <Input
                  type="email"
                  value={testRecipient}
                  onChange={(e) => setTestRecipient(e.target.value)}
                  placeholder="test@example.com"
                  className="w-48 h-9"
                />
                <Button type="button" variant="secondary" onClick={handleSendTestEmail}>
                  <Send className="h-4 w-4" />
                  Send Test Email
                </Button>
              </div>
            </div>
          </form>
        </CardContent>
      </Card>

      {/* 3. Custom Email Templates Editor */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base flex items-center gap-2">
            <FileText className="h-4 w-4 text-primary" />
            Custom Email Templates
          </CardTitle>
          <CardDescription>
            Customize subject lines and body text with variables (`{'{template.name}'}`, `{'{submitter.name}'}`, `{'{submitter.link}'}`).
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSaveTemplate} className="space-y-4">
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <div className="space-y-1.5">
                <Label>Email Type</Label>
                <Select value={tplType} onChange={(e) => setTplType(e.target.value)}>
                  <option value="invitation">Signature Request Invitation</option>
                  <option value="reminder">Automated Reminder</option>
                  <option value="completed">Completed Document Copy</option>
                </Select>
              </div>
              <div className="space-y-1.5 sm:col-span-2">
                <Label>Subject Line</Label>
                <Input value={tplSubject} onChange={(e) => setTplSubject(e.target.value)} />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label>Message Body</Label>
              <Textarea
                rows={4}
                value={tplBody}
                onChange={(e) => setTplBody(e.target.value)}
              />
            </div>
            <Button type="submit" size="sm">
              <Save className="h-3.5 w-3.5" />
              Save Email Template
            </Button>
          </form>
        </CardContent>
      </Card>

      {/* 4. Outgoing Email Delivery Log */}
      {emailEvents.length > 0 && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base flex items-center gap-2">
              <Activity className="h-4 w-4 text-primary" />
              Recent Outgoing Email Events ({emailEvents.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2 text-xs">
              {emailEvents.slice(0, 15).map((ev, i) => (
                <div
                  key={ev.id || i}
                  className="flex flex-wrap items-center justify-between gap-2 rounded border border-border bg-secondary/20 px-3 py-2"
                >
                  <div className="flex flex-wrap items-center gap-1.5">
                    <Badge variant="info">
                      {ev.event_type || 'sent'}
                    </Badge>
                    <span className="font-medium">{ev.email}</span>
                    {ev.tag && <span className="text-muted-foreground">({ev.tag})</span>}
                    {ev.data?.from_email && (
                      <span className="text-muted-foreground">from {ev.data.from_email}</span>
                    )}
                    {Array.isArray(ev.data?.attachments) &&
                      ev.data.attachments.map((att: any, idx: number) => (
                        <Badge key={idx} variant="secondary" className="font-mono text-[10px]">
                          📎 {att.filename} ({att.byte_size} B)
                        </Badge>
                      ))}
                  </div>
                  <span className="text-muted-foreground">
                    {formatDate(ev.event_datetime || ev.created_at)}
                  </span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
};
