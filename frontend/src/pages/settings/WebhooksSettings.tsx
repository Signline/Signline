import React, { useState, useEffect, useCallback } from 'react';
import { Webhook, Plus, Trash2, Send, RefreshCw, Activity } from 'lucide-react';
import type { WebhookEndpoint, WebhookEventRecord } from '@/types';
import { WEBHOOK_EVENTS_LIST, formatDate } from '@/lib/utils';
import { api, unwrapList } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const WebhooksSettings: React.FC = () => {
  const { toast } = useToast();
  const [webhooks, setWebhooks] = useState<WebhookEndpoint[]>([]);
  const [eventsLog, setEventsLog] = useState<WebhookEventRecord[]>([]);
  const [url, setUrl] = useState('');
  const [selectedEvents, setSelectedEvents] = useState<string[]>([
    'form.completed',
    'submission.completed',
  ]);
  const [hmacSecret, setHmacSecret] = useState('');
  const [saving, setSaving] = useState(false);

  const loadWebhooks = useCallback(async () => {
    try {
      const res = await api.get<any>('/api/settings/webhooks');
      setWebhooks(unwrapList<WebhookEndpoint>(res, 'webhooks'));
      if (res.events) {
        setEventsLog(unwrapList<WebhookEventRecord>(res.events));
      }
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadWebhooks();
  }, [loadWebhooks]);

  const toggleEvent = (ev: string) => {
    setSelectedEvents((prev) =>
      prev.includes(ev) ? prev.filter((item) => item !== ev) : [...prev, ev]
    );
  };

  const handleCreateWebhook = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    setSaving(true);
    try {
      await api.post('/api/settings/webhooks', {
        url: url.trim(),
        events: selectedEvents,
        hmac_secret: hmacSecret.trim() || undefined,
      });
      setUrl('');
      setHmacSecret('');
      toast({
        title: 'Webhook Endpoint Registered',
        description: 'Payloads will be signed with the X-Signline-Signature HMAC-SHA256 header.',
        variant: 'success',
      });
      await loadWebhooks();
    } catch (err: any) {
      toast({ title: 'Failed to create webhook', description: err.message, variant: 'destructive' });
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteWebhook = async (id: string) => {
    try {
      await api.delete(`/api/settings/webhooks/${id}`);
      toast({ title: 'Webhook Deleted' });
      await loadWebhooks();
    } catch (err: any) {
      toast({ title: 'Delete failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleTestWebhook = async (id: string) => {
    try {
      await api.post(`/api/settings/webhooks/${id}/test`, {});
      toast({
        title: 'Test Webhook Dispatched',
        description: 'Sent signed event payload with X-Signline-Signature HMAC.',
        variant: 'success',
      });
      await loadWebhooks();
    } catch (err: any) {
      toast({ title: 'Webhook test failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleRetryEvent = async (eventId: string) => {
    try {
      await api.post(`/api/settings/webhooks/events/${eventId}/resend`, {});
      toast({ title: 'Webhook Retried', variant: 'success' });
      await loadWebhooks();
    } catch (err: any) {
      toast({ title: 'Retry failed', description: err.message, variant: 'destructive' });
    }
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Webhook className="h-5 w-5 text-primary" />
            Webhooks &amp; Event Subscriptions
          </CardTitle>
          <CardDescription>
            Receive real-time HTTP POST callbacks signed with the `X-Signline-Signature` HMAC-SHA256 header.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-5">
          <form onSubmit={handleCreateWebhook} className="space-y-4 rounded-xl border border-border bg-secondary/20 p-4">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="wh-url">Webhook Receiver URL</Label>
                <Input
                  id="wh-url"
                  type="url"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="https://api.example.com/webhooks/signline"
                  required
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="wh-secret">
                  HMAC Signing Secret (`X-Signline-Signature`, optional auto-generated)
                </Label>
                <Input
                  id="wh-secret"
                  value={hmacSecret}
                  onChange={(e) => setHmacSecret(e.target.value)}
                  placeholder="Auto-generated if left blank"
                  className="font-mono"
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label className="text-xs font-semibold">Subscribe to Events (11 Available)</Label>
              <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-2">
                {WEBHOOK_EVENTS_LIST.map((ev) => (
                  <label
                    key={ev}
                    className="flex items-center gap-2 rounded border border-border bg-card px-2.5 py-1.5 text-xs cursor-pointer"
                  >
                    <input
                      type="checkbox"
                      checked={selectedEvents.includes(ev)}
                      onChange={() => toggleEvent(ev)}
                      className="accent-primary"
                    />
                    <span className="font-mono text-[11px]">{ev}</span>
                  </label>
                ))}
              </div>
            </div>

            <Button type="submit" disabled={saving}>
              <Plus className="h-4 w-4" />
              {saving ? 'Adding...' : 'Add Webhook Endpoint'}
            </Button>
          </form>

          {/* Configured Webhooks List */}
          <div className="space-y-3">
            {webhooks.map((wh) => (
              <div
                key={wh.id}
                className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 rounded-lg border border-border bg-card p-4"
              >
                <div className="space-y-1.5">
                  <div className="font-mono text-sm font-semibold break-all">{wh.url}</div>
                  <div className="flex flex-wrap gap-1">
                    {(wh.events || []).map((ev) => (
                      <Badge key={ev} variant="secondary" className="font-mono text-[10px]">
                        {ev}
                      </Badge>
                    ))}
                  </div>
                  {wh.hmac_secret && (
                    <div className="text-[11px] text-muted-foreground">
                      HMAC Secret (`X-Signline-Signature`):{' '}
                      <code className="font-mono text-foreground">{wh.hmac_secret}</code>
                    </div>
                  )}
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <Button variant="secondary" size="sm" onClick={() => handleTestWebhook(wh.id)}>
                    <Send className="h-3.5 w-3.5" />
                    Send Test
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDeleteWebhook(wh.id)}
                    className="text-destructive"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Delivery Log */}
      {eventsLog.length > 0 && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base flex items-center gap-2">
              <Activity className="h-4 w-4 text-primary" />
              Recent Webhook Deliveries ({eventsLog.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2 text-xs">
              {eventsLog.map((ev) => (
                <div
                  key={ev.id}
                  className="flex items-center justify-between rounded-lg border border-border bg-secondary/20 px-3 py-2"
                >
                  <div className="flex items-center gap-2">
                    <Badge
                      variant={
                        ev.status === 'success'
                          ? 'success'
                          : ev.status === 'error'
                          ? 'destructive'
                          : 'warning'
                      }
                    >
                      {ev.status} {ev.last_status_code ? `(${ev.last_status_code})` : ''}
                    </Badge>
                    <span className="font-mono font-semibold">{ev.event_type}</span>
                    <span className="text-muted-foreground">{formatDate(ev.created_at)}</span>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleRetryEvent(ev.id)}
                  >
                    <RefreshCw className="h-3.5 w-3.5" />
                    Retry
                  </Button>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
};
