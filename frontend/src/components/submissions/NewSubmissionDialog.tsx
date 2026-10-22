import React, { useState, useEffect } from 'react';
import { Send, Plus, Trash2, Upload, Copy, Check } from 'lucide-react';
import type { Template } from '@/types';
import { api } from '@/lib/api';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { useToast } from '@/components/ui/toast';

interface NewSubmissionDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  template: Template | null;
  templates?: Template[];
  onCreated: (submission: any) => void;
}

interface RecipientRow {
  role: string;
  submitter_uuid?: string;
  name: string;
  email: string;
  phone: string;
}

export const NewSubmissionDialog: React.FC<NewSubmissionDialogProps> = ({
  open,
  onOpenChange,
  template: initialTemplate,
  templates = [],
  onCreated,
}) => {
  const { toast } = useToast();
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>(
    initialTemplate?.id || templates[0]?.id || ''
  );
  const [order, setOrder] = useState<'preserved' | 'random'>('preserved');
  const [expireAt, setExpireAt] = useState<string>('');
  const [sendEmail, setSendEmail] = useState<boolean>(true);
  const [recipients, setRecipients] = useState<RecipientRow[]>([]);
  const [prefillValues, setPrefillValues] = useState<Record<string, string>>({});
  const [csvText, setCsvText] = useState<string>('');
  const [submitting, setSubmitting] = useState<boolean>(false);
  const [createdLinks, setCreatedLinks] = useState<Array<{ name: string; email: string; slug: string }> | null>(null);
  const [copiedSlug, setCopiedSlug] = useState<string | null>(null);

  const activeTemplate =
    initialTemplate ||
    templates.find((t) => t.id === selectedTemplateId) ||
    templates[0] ||
    null;

  useEffect(() => {
    if (initialTemplate) {
      setSelectedTemplateId(initialTemplate.id);
    } else if (!selectedTemplateId && templates[0]) {
      setSelectedTemplateId(templates[0].id);
    }
  }, [initialTemplate, templates, selectedTemplateId]);

  useEffect(() => {
    if (open) {
      setCreatedLinks(null);
      const roles =
        activeTemplate?.submitters && activeTemplate.submitters.length > 0
          ? activeTemplate.submitters
          : [{ uuid: 'party-1', name: 'First Party' }];
      setRecipients(
        roles.map((r) => ({
          role: r.name,
          submitter_uuid: r.uuid,
          name: '',
          email: '',
          phone: '',
        }))
      );
      setPrefillValues({});
    }
  }, [open, activeTemplate?.id]);

  const handleRecipientChange = (idx: number, patch: Partial<RecipientRow>) => {
    setRecipients((prev) => {
      const next = [...prev];
      next[idx] = { ...next[idx], ...patch };
      return next;
    });
  };

  const handleAddRecipient = () => {
    setRecipients((prev) => [
      ...prev,
      {
        role: `Party ${prev.length + 1}`,
        name: '',
        email: '',
        phone: '',
      },
    ]);
  };

  const handleRemoveRecipient = (idx: number) => {
    if (recipients.length <= 1) return;
    setRecipients((prev) => prev.filter((_, i) => i !== idx));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeTemplate) return;

    const validRecipients = recipients.filter((r) => r.email.trim() || r.name.trim());
    if (validRecipients.length === 0) {
      toast({
        title: 'Add at least one recipient',
        description: 'Please enter a name or email address for the signer.',
        variant: 'destructive',
      });
      return;
    }

    setSubmitting(true);
    try {
      const res = await api.post<any>('/api/submissions', {
        template_id: activeTemplate.id,
        send_email: sendEmail,
        order,
        expire_at: expireAt || undefined,
        submitters: validRecipients.map((r) => ({
          role: r.role,
          submitter_uuid: r.submitter_uuid,
          name: r.name.trim() || r.email.split('@')[0],
          email: r.email.trim(),
          phone: r.phone.trim() || undefined,
          values: prefillValues,
        })),
      });

      const submission = res.submission || res.data || res;
      const subs = submission.submitters || (Array.isArray(res) ? res : []);
      toast({
        title: 'Signature Request Created',
        description: `Created signing envelope for ${validRecipients.length} recipient(s).`,
        variant: 'success',
      });
      onCreated(submission);
      if (subs && subs.length > 0) {
        setCreatedLinks(
          subs.map((s: any) => ({
            name: s.name || s.role,
            email: s.email,
            slug: s.slug,
          }))
        );
      } else {
        onOpenChange(false);
      }
    } catch (err: any) {
      toast({
        title: 'Failed to create submission',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setSubmitting(false);
    }
  };

  const handleBulkCsvSubmit = async () => {
    if (!activeTemplate || !csvText.trim()) return;
    const lines = csvText
      .trim()
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter(Boolean);

    setSubmitting(true);
    try {
      let count = 0;
      for (const line of lines) {
        if (line.toLowerCase().startsWith('name,email')) continue;
        const [namePart, emailPart, phonePart] = line.split(',').map((s) => s.trim());
        const email = emailPart || namePart;
        const name = emailPart ? namePart : email.split('@')[0];
        if (!email || !email.includes('@')) continue;

        await api.post('/api/submissions', {
          template_id: activeTemplate.id,
          send_email: sendEmail,
          order,
          submitters: [
            {
              role: activeTemplate.submitters?.[0]?.name || 'First Party',
              name,
              email,
              phone: phonePart,
            },
          ],
        });
        count++;
      }
      toast({
        title: `Created ${count} Submissions from CSV`,
        variant: 'success',
      });
      onCreated({});
      onOpenChange(false);
    } catch (err: any) {
      toast({
        title: 'Bulk CSV error',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl" onClose={() => onOpenChange(false)}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Send className="h-5 w-5 text-primary" />
            New Signature Request
          </DialogTitle>
          <DialogDescription>
            Invite signers to complete and cryptographically sign{' '}
            <strong>{activeTemplate?.name || 'this template'}</strong>.
          </DialogDescription>
        </DialogHeader>

        {createdLinks ? (
          <div className="space-y-4">
            <div className="rounded-lg border border-emerald-500/40 bg-emerald-50 p-3 text-xs text-emerald-900">
              Signature request created! Share the direct signing links below or let recipients open their email invitation.
            </div>
            <div className="space-y-2">
              {createdLinks.map((item) => {
                const fullUrl = `${window.location.origin}/s/${item.slug}`;
                return (
                  <div
                    key={item.slug}
                    className="flex items-center justify-between gap-2 rounded-lg border border-border bg-secondary/40 p-2.5 text-xs"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="font-semibold">
                        {item.name} ({item.email})
                      </div>
                      <code className="text-primary break-all">{fullUrl}</code>
                    </div>
                    <div className="flex items-center gap-1.5 shrink-0">
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => {
                          navigator.clipboard.writeText(fullUrl);
                          setCopiedSlug(item.slug);
                          setTimeout(() => setCopiedSlug(null), 2000);
                        }}
                      >
                        {copiedSlug === item.slug ? (
                          <Check className="h-3.5 w-3.5 text-emerald-600" />
                        ) : (
                          <Copy className="h-3.5 w-3.5" />
                        )}
                        Copy Link
                      </Button>
                      <a href={`/s/${item.slug}`} target="_blank" rel="noopener noreferrer">
                        <Button size="sm">Sign Now</Button>
                      </a>
                    </div>
                  </div>
                );
              })}
            </div>
            <DialogFooter>
              <Button onClick={() => onOpenChange(false)}>Done</Button>
            </DialogFooter>
          </div>
        ) : (
          <Tabs defaultValue="invite">
            <TabsList className="grid grid-cols-2 w-full">
              <TabsTrigger value="invite">Invite Signers</TabsTrigger>
              <TabsTrigger value="bulk">Bulk CSV Upload</TabsTrigger>
            </TabsList>

            <TabsContent value="invite">
              <form onSubmit={handleSubmit} className="space-y-4">
                {!initialTemplate && templates.length > 0 && (
                  <div className="space-y-1.5">
                    <Label>Select Template</Label>
                    <Select
                      value={selectedTemplateId}
                      onChange={(e) => setSelectedTemplateId(e.target.value)}
                    >
                      {templates.map((t) => (
                        <option key={t.id} value={t.id}>
                          {t.name}
                        </option>
                      ))}
                    </Select>
                  </div>
                )}

                <div className="space-y-2.5">
                  <div className="flex items-center justify-between">
                    <Label className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Recipients / Signers
                    </Label>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={handleAddRecipient}
                      className="h-7 text-xs"
                    >
                      <Plus className="h-3.5 w-3.5" />
                      Add Signer
                    </Button>
                  </div>

                  {recipients.map((rec, idx) => (
                    <div
                      key={idx}
                      className="rounded-lg border border-border bg-secondary/30 p-3 space-y-2"
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-xs font-semibold text-primary">
                          #{idx + 1} · {rec.role}
                        </span>
                        {recipients.length > 1 && (
                          <button
                            type="button"
                            onClick={() => handleRemoveRecipient(idx)}
                            className="text-muted-foreground hover:text-destructive"
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </button>
                        )}
                      </div>
                      <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
                        <Input
                          placeholder="Full Name"
                          value={rec.name}
                          onChange={(e) => handleRecipientChange(idx, { name: e.target.value })}
                          required
                        />
                        <Input
                          type="email"
                          placeholder="Email Address"
                          value={rec.email}
                          onChange={(e) => handleRecipientChange(idx, { email: e.target.value })}
                          required
                        />
                        <Input
                          type="tel"
                          placeholder="Phone (Optional)"
                          value={rec.phone}
                          onChange={(e) => handleRecipientChange(idx, { phone: e.target.value })}
                        />
                      </div>
                    </div>
                  ))}
                </div>

                {/* Signing Order & Expiration */}
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div className="space-y-1">
                    <Label className="text-xs">Signing Order</Label>
                    <Select
                      value={order}
                      onChange={(e) => setOrder(e.target.value as 'preserved' | 'random')}
                    >
                      <option value="preserved">Preserved Sequential Order (1 → 2 → 3)</option>
                      <option value="random">Parallel / Any Order</option>
                    </Select>
                  </div>
                  <div className="space-y-1">
                    <Label className="text-xs">Expiration Date (Optional)</Label>
                    <Input
                      type="date"
                      value={expireAt}
                      onChange={(e) => setExpireAt(e.target.value)}
                    />
                  </div>
                </div>

                {/* Pre-fill Values if template has text/number/date fields */}
                {activeTemplate &&
                  activeTemplate.fields &&
                  activeTemplate.fields.filter(
                    (f) => f.type !== 'signature' && f.type !== 'initials'
                  ).length > 0 && (
                    <details className="rounded-lg border border-border p-3 text-xs">
                      <summary className="cursor-pointer font-semibold text-muted-foreground">
                        Pre-Fill Document Field Values (Optional)
                      </summary>
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 mt-3">
                        {activeTemplate.fields
                          .filter((f) => f.type !== 'signature' && f.type !== 'initials')
                          .map((f) => (
                            <div key={f.uuid} className="space-y-1">
                              <Label className="text-[11px]">{f.title || f.name}</Label>
                              <Input
                                value={prefillValues[f.name] || ''}
                                onChange={(e) =>
                                  setPrefillValues((prev) => ({
                                    ...prev,
                                    [f.name]: e.target.value,
                                    [f.uuid]: e.target.value,
                                  }))
                                }
                                placeholder={`Pre-fill ${f.name}...`}
                                className="h-7 text-xs"
                              />
                            </div>
                          ))}
                      </div>
                    </details>
                  )}

                <label className="flex items-center gap-2 text-xs cursor-pointer">
                  <input
                    type="checkbox"
                    checked={sendEmail}
                    onChange={(e) => setSendEmail(e.target.checked)}
                    className="accent-primary"
                  />
                  <span>Send signature invitation email immediately</span>
                </label>

                <DialogFooter>
                  <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                    Cancel
                  </Button>
                  <Button type="submit" disabled={submitting}>
                    <Send className="h-4 w-4" />
                    {submitting ? 'Sending...' : 'Send Signature Request'}
                  </Button>
                </DialogFooter>
              </form>
            </TabsContent>

            <TabsContent value="bulk" className="space-y-3">
              <p className="text-xs text-muted-foreground">
                Paste CSV rows (<code>Name,Email,Phone</code>) to send individual signature requests in bulk:
              </p>
              <textarea
                rows={6}
                value={csvText}
                onChange={(e) => setCsvText(e.target.value)}
                placeholder={'Name,Email,Phone\nAlice Smith,alice@example.com,+15550101\nBob Jones,bob@example.com,+15550102'}
                className="w-full rounded-md border border-input bg-card p-3 font-mono text-xs"
              />
              <DialogFooter>
                <Button variant="outline" onClick={() => onOpenChange(false)}>
                  Cancel
                </Button>
                <Button onClick={handleBulkCsvSubmit} disabled={submitting || !csvText.trim()}>
                  <Upload className="h-4 w-4" />
                  {submitting ? 'Processing CSV...' : 'Create Bulk Submissions'}
                </Button>
              </DialogFooter>
            </TabsContent>
          </Tabs>
        )}
      </DialogContent>
    </Dialog>
  );
};
