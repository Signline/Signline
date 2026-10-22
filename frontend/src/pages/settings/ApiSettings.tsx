import React, { useState, useEffect, useCallback } from 'react';
import { KeyRound, Copy, Check, RefreshCw, FlaskConical } from 'lucide-react';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const ApiSettings: React.FC = () => {
  const { toast } = useToast();
  const [token, setToken] = useState('');
  const [sandboxMode, setSandboxMode] = useState(false);
  const [copied, setCopied] = useState(false);
  const [rotating, setRotating] = useState(false);

  const loadApiConfig = useCallback(async () => {
    try {
      const res = await api.get<any>('/api/settings/api');
      setToken(res.token || res.api_token || res.x_auth_token || '');
      if (res.testing !== undefined) setSandboxMode(Boolean(res.testing));
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadApiConfig();
  }, [loadApiConfig]);

  const handleRotateToken = async () => {
    setRotating(true);
    try {
      const res = await api.post<any>('/api/settings/api', { action: 'rotate' });
      const newToken = res.token || res.api_token || res.x_auth_token;
      if (newToken) setToken(newToken);
      toast({
        title: 'API Key Rotated',
        description: 'Generated a new X-Auth-Token secret.',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Rotation failed', description: err.message, variant: 'destructive' });
    } finally {
      setRotating(false);
    }
  };

  const handleToggleSandbox = async (checked: boolean) => {
    setSandboxMode(checked);
    try {
      await api.post('/api/settings/api', { testing: checked });
      toast({
        title: checked ? 'Sandbox Testing Mode Enabled' : 'Production Mode Active',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Failed to toggle mode', description: err.message, variant: 'destructive' });
    }
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <KeyRound className="h-5 w-5 text-primary" />
            REST API Key (`X-Auth-Token`)
          </CardTitle>
          <CardDescription>
            Authenticate Signline REST API clients using the `X-Auth-Token` HTTP header.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <code className="flex-1 rounded-lg border border-border bg-secondary/40 px-3.5 py-2 text-xs font-mono select-all break-all">
              {token || 'Loading token...'}
            </code>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                navigator.clipboard.writeText(token);
                setCopied(true);
                setTimeout(() => setCopied(false), 2000);
              }}
            >
              {copied ? <Check className="h-4 w-4 text-emerald-600" /> : <Copy className="h-4 w-4" />}
              Copy Key
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={handleRotateToken}
              disabled={rotating}
            >
              <RefreshCw className="h-4 w-4" />
              Rotate Key
            </Button>
          </div>

          <div className="flex items-center justify-between rounded-lg border border-border bg-secondary/20 p-3.5">
            <div className="text-xs">
              <div className="font-semibold flex items-center gap-1.5">
                <FlaskConical className="h-4 w-4 text-primary" />
                Sandbox / Testing Mode (`X-Account-Testing`)
              </div>
              <div className="text-muted-foreground">
                Test API integrations and webhooks without affecting production metrics.
              </div>
            </div>
            <Switch checked={sandboxMode} onCheckedChange={handleToggleSandbox} />
          </div>

          <div className="space-y-2 pt-2">
            <div className="text-xs font-semibold">Quick API Reference Examples:</div>
            <pre className="rounded-lg bg-slate-950 text-slate-100 p-4 text-xs font-mono overflow-x-auto">
{`# List Templates
curl -H "X-Auth-Token: ${token || 'YOUR_TOKEN'}" \\
  ${window.location.origin}/api/templates

# Create Signature Submission
curl -X POST ${window.location.origin}/api/submissions \\
  -H "X-Auth-Token: ${token || 'YOUR_TOKEN'}" \\
  -H "Content-Type: application/json" \\
  -d '{"template_id": "TEMPLATE_ID", "submitters": [{"role": "First Party", "email": "signer@example.com"}]}'`}
            </pre>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
