import React, { useState, useEffect, useCallback } from 'react';
import { Cpu, Plus, Trash2, Copy, Check } from 'lucide-react';
import type { McpTokenRecord } from '@/types';
import { formatDate } from '@/lib/utils';
import { api, unwrapList } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const McpSettings: React.FC = () => {
  const { toast } = useToast();
  const [enabled, setEnabled] = useState(true);
  const [tokens, setTokens] = useState<McpTokenRecord[]>([]);
  const [tokenName, setTokenName] = useState('');
  const [newlyCreatedToken, setNewlyCreatedToken] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const loadMcp = useCallback(async () => {
    try {
      const res = await api.get<any>('/api/settings/mcp');
      if (res.enabled !== undefined) setEnabled(Boolean(res.enabled));
      setTokens(unwrapList<McpTokenRecord>(res, 'tokens'));
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadMcp();
  }, [loadMcp]);

  const handleToggleMcp = async (checked: boolean) => {
    setEnabled(checked);
    try {
      await api.post('/api/settings/mcp', { enabled: checked });
      toast({
        title: checked ? 'MCP Server Enabled' : 'MCP Server Disabled',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Update failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleCreateToken = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!tokenName.trim()) return;
    try {
      const res = await api.post<any>('/api/settings/mcp', {
        name: tokenName.trim(),
      });
      const rawToken = res.token || res.mcp_token?.token;
      if (rawToken) {
        setNewlyCreatedToken(rawToken);
      }
      setTokenName('');
      toast({
        title: 'MCP Bearer Token Created',
        variant: 'success',
      });
      await loadMcp();
    } catch (err: any) {
      toast({ title: 'Creation failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleDeleteToken = async (id: string) => {
    try {
      await api.delete(`/api/settings/mcp/${id}`);
      toast({ title: 'MCP Token Revoked' });
      await loadMcp();
    } catch (err: any) {
      toast({ title: 'Revoke failed', description: err.message, variant: 'destructive' });
    }
  };

  const mcpEndpoint = `${window.location.origin}/mcp`;
  const configSnippet = JSON.stringify(
    {
      mcpServers: {
        signline: {
          url: mcpEndpoint,
          headers: {
            Authorization: `Bearer ${newlyCreatedToken || 'YOUR_MCP_TOKEN'}`,
          },
        },
      },
    },
    null,
    2
  );

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              <Cpu className="h-5 w-5 text-primary" />
              Model Context Protocol (MCP) Server
            </CardTitle>
            <CardDescription>
              Connect Model Context Protocol (MCP) clients to Signline over JSON-RPC 2.0 (`POST /mcp`).
            </CardDescription>
          </div>
          <Switch checked={enabled} onCheckedChange={handleToggleMcp} />
        </div>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="rounded-lg border border-border bg-secondary/30 p-3 text-xs space-y-1.5">
          <div className="font-semibold">7 Exposed MCP Tools:</div>
          <div className="flex flex-wrap gap-1.5">
            {[
              'list_templates',
              'get_template',
              'create_submission',
              'list_submissions',
              'get_submission',
              'create_template_from_html',
              'create_submission_from_html',
            ].map((tool) => (
              <Badge key={tool} variant="secondary" className="font-mono text-[11px]">
                {tool}
              </Badge>
            ))}
          </div>
        </div>

        <form onSubmit={handleCreateToken} className="flex items-end gap-2">
          <div className="flex-1 space-y-1">
            <Label htmlFor="mcp-token-name" className="text-xs">
              New MCP Token Name
            </Label>
            <Input
              id="mcp-token-name"
              value={tokenName}
              onChange={(e) => setTokenName(e.target.value)}
              placeholder="e.g. Desktop MCP Client"
              required
            />
          </div>
          <Button type="submit">
            <Plus className="h-4 w-4" />
            Create MCP Token
          </Button>
        </form>

        {newlyCreatedToken && (
          <div className="rounded-lg border border-emerald-500/50 bg-emerald-50 p-3 text-xs space-y-2">
            <div className="font-semibold text-emerald-950">
              New MCP Token Created — Copy it now:
            </div>
            <div className="flex items-center gap-2">
              <code className="flex-1 rounded bg-white p-2 border font-mono break-all">
                {newlyCreatedToken}
              </code>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  navigator.clipboard.writeText(newlyCreatedToken);
                  setCopied(true);
                  setTimeout(() => setCopied(false), 2000);
                }}
              >
                {copied ? <Check className="h-4 w-4 text-emerald-600" /> : <Copy className="h-4 w-4" />}
              </Button>
            </div>
          </div>
        )}

        {/* Active Tokens List */}
        {tokens.length > 0 && (
          <div className="space-y-2">
            <Label className="text-xs font-semibold">Active MCP Tokens</Label>
            {tokens.map((t) => (
              <div
                key={t.id}
                className="flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2 text-xs"
              >
                <div>
                  <span className="font-semibold">{t.name}</span>
                  <code className="ml-2 text-muted-foreground font-mono">
                    {t.token_prefix}...
                  </code>
                  <span className="ml-2 text-muted-foreground">{formatDate(t.created_at)}</span>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => handleDeleteToken(t.id)}
                  className="text-destructive"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  Revoke
                </Button>
              </div>
            ))}
          </div>
        )}

        {/* JSON Config Snippet */}
        <div className="space-y-1.5 pt-2 border-t border-border">
          <Label className="text-xs font-semibold">MCP Client Configuration:</Label>
          <pre className="rounded-lg bg-slate-950 text-slate-100 p-3 text-xs font-mono overflow-x-auto">
            {configSnippet}
          </pre>
        </div>
      </CardContent>
    </Card>
  );
};
