import React, { useState, useEffect, useCallback } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import {
  ArrowLeft,
  Edit3,
  Send,
  Share2,
  Code,
  Download,
  Copy,
  Check,
  ExternalLink,
  FileText,
} from 'lucide-react';
import type { Submission, Template } from '@/types';
import { formatDate } from '@/lib/utils';
import { api, unwrapList } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog';
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table';
import { useToast } from '@/components/ui/toast';
import { NewSubmissionDialog } from '@/components/submissions/NewSubmissionDialog';

export const TemplateViewPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();

  const [template, setTemplate] = useState<Template | null>(null);
  const [submissions, setSubmissions] = useState<Submission[]>([]);
  const [loading, setLoading] = useState(true);
  const [sendOpen, setSendOpen] = useState(false);
  const [shareOpen, setShareOpen] = useState(false);
  const [embedOpen, setEmbedOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  const loadTemplate = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      const [tplRes, subRes] = await Promise.all([
        api.get<any>(`/api/templates/${id}`),
        api.get<any>(`/api/submissions?template_id=${id}`).catch(() => []),
      ]);
      const resolvedTpl = tplRes.template || tplRes.data || tplRes;
      setTemplate(resolvedTpl);
      setSubmissions(unwrapList<Submission>(subRes, 'submissions'));
    } catch (err: any) {
      toast({
        title: 'Failed to load template',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  }, [id, toast]);

  useEffect(() => {
    loadTemplate();
  }, [loadTemplate]);

  const handleToggleSharedLink = async (checked: boolean) => {
    if (!template) return;
    try {
      const res = await api.put<any>(`/api/templates/${template.id}`, {
        shared_link: checked,
      });
      const updated = res.template || res.data || res;
      setTemplate({ ...template, ...updated, shared_link: checked });
      toast({
        title: checked ? 'Public Share Link Enabled' : 'Public Share Link Disabled',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Update failed', description: err.message, variant: 'destructive' });
    }
  };

  if (loading || !template) {
    return <div className="py-12 text-center text-sm text-muted-foreground">Loading template...</div>;
  }

  const shareUrl = `${window.location.origin}/d/${template.slug}`;
  const embedSnippet = `<script src="${window.location.origin}/js/form.js"></script>\n<signline-form data-src="${shareUrl}"></signline-form>`;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-border bg-card p-5 shadow-sm">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" onClick={() => navigate('/')}>
              <ArrowLeft className="h-4 w-4" />
            </Button>
            <h1 className="text-xl font-bold">{template.name}</h1>
            {template.shared_link && <Badge variant="info">Shared Link Active</Badge>}
          </div>
          <p className="text-xs text-muted-foreground pl-9">
            Template ID: <code>{template.id}</code> · Slug: <code>{template.slug}</code> ·{' '}
            {template.fields?.length || 0} fields across {template.submitters?.length || 1} role(s)
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => setShareOpen(true)}>
            <Share2 className="h-4 w-4" />
            Share Link
          </Button>
          <Button variant="outline" size="sm" onClick={() => setEmbedOpen(true)}>
            <Code className="h-4 w-4" />
            API &amp; Embed
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => navigate(`/templates/${template.id}/edit`)}
          >
            <Edit3 className="h-4 w-4" />
            Edit Template
          </Button>
          <Button size="sm" onClick={() => setSendOpen(true)}>
            <Send className="h-4 w-4" />
            Send to Sign
          </Button>
        </div>
      </div>

      {/* Template Fields & Roles Overview */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <Card className="lg:col-span-1">
          <CardHeader className="pb-3">
            <CardTitle className="text-base">Configured Signer Roles &amp; Fields</CardTitle>
            <CardDescription>
              Summary of fields mapped on this template.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {(template.submitters || [{ uuid: '1', name: 'First Party' }]).map((role) => {
              const roleFields = (template.fields || []).filter(
                (f) => f.submitter_uuid === role.uuid
              );
              return (
                <div
                  key={role.uuid}
                  className="rounded-lg border border-border bg-secondary/30 p-3 space-y-2"
                >
                  <div className="flex items-center justify-between">
                    <span className="font-semibold text-xs">{role.name}</span>
                    <Badge variant="secondary">{roleFields.length} fields</Badge>
                  </div>
                  <div className="flex flex-wrap gap-1">
                    {roleFields.map((f) => (
                      <span
                        key={f.uuid}
                        className="rounded bg-card px-2 py-0.5 text-[11px] border border-border"
                      >
                        {f.name} ({f.type})
                      </span>
                    ))}
                  </div>
                </div>
              );
            })}
          </CardContent>
        </Card>

        {/* Submissions for this Template */}
        <Card className="lg:col-span-2">
          <CardHeader className="pb-3 flex flex-row items-center justify-between">
            <div>
              <CardTitle className="text-base">
                Submissions ({submissions.length})
              </CardTitle>
              <CardDescription>
                Envelopes and signers invited using this template.
              </CardDescription>
            </div>
            <a
              href={`/api/submissions/export?template_id=${template.id}`}
              target="_blank"
              rel="noopener noreferrer"
            >
              <Button variant="outline" size="sm">
                <Download className="h-3.5 w-3.5" />
                Export CSV
              </Button>
            </a>
          </CardHeader>
          <CardContent>
            {submissions.length === 0 ? (
              <div className="py-10 text-center space-y-2">
                <FileText className="h-8 w-8 text-muted-foreground mx-auto" />
                <p className="text-xs text-muted-foreground">
                  No submissions yet for this template. Click <strong>Send to Sign</strong> to invite recipients.
                </p>
              </div>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Recipients</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {submissions.map((sub) => (
                    <TableRow key={sub.id}>
                      <TableCell>
                        {(sub.submitters || []).map((s) => (
                          <div key={s.id || s.slug} className="text-xs">
                            <span className="font-semibold">{s.name || s.role}</span>{' '}
                            <span className="text-muted-foreground">({s.email})</span>
                          </div>
                        ))}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant={
                            sub.status === 'completed'
                              ? 'success'
                              : sub.status === 'declined'
                              ? 'destructive'
                              : 'warning'
                          }
                        >
                          {sub.status}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {formatDate(sub.created_at)}
                      </TableCell>
                      <TableCell className="text-right">
                        <Link to={`/submissions/${sub.id}`}>
                          <Button variant="outline" size="sm">
                            View Audit &amp; PDF
                          </Button>
                        </Link>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Share Link Modal */}
      <Dialog open={shareOpen} onOpenChange={setShareOpen}>
        <DialogContent onClose={() => setShareOpen(false)}>
          <DialogHeader>
            <DialogTitle>Public Self-Serve Signing Link (/d/{template.slug})</DialogTitle>
            <DialogDescription>
              Anyone with this link can initiate and sign their own copy of this template.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="flex items-center justify-between rounded-lg border border-border bg-secondary/40 p-3">
              <div className="text-xs">
                <div className="font-semibold">Enable Public Shared Link</div>
                <div className="text-muted-foreground">Allow self-serve signing at /d/{template.slug}</div>
              </div>
              <Switch
                checked={Boolean(template.shared_link)}
                onCheckedChange={handleToggleSharedLink}
              />
            </div>

            <div className="flex items-center gap-2">
              <code className="flex-1 rounded border border-border bg-card px-3 py-2 text-xs break-all">
                {shareUrl}
              </code>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  navigator.clipboard.writeText(shareUrl);
                  setCopied(true);
                  setTimeout(() => setCopied(false), 2000);
                }}
              >
                {copied ? <Check className="h-4 w-4 text-emerald-600" /> : <Copy className="h-4 w-4" />}
              </Button>
              <Link to={`/d/${template.slug}`} target="_blank">
                <Button size="sm">
                  <ExternalLink className="h-4 w-4" />
                  Open
                </Button>
              </Link>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      {/* API & Embed Code Modal */}
      <Dialog open={embedOpen} onOpenChange={setEmbedOpen}>
        <DialogContent className="max-w-xl" onClose={() => setEmbedOpen(false)}>
          <DialogHeader>
            <DialogTitle>Embed &amp; REST API Integration</DialogTitle>
            <DialogDescription>
              100% compatible with Signline SDKs, <code>&lt;signline-form&gt;</code> web components, and <code>X-Auth-Token</code> API calls.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3 text-xs">
            <div className="font-semibold">HTML Web Component Embed:</div>
            <pre className="rounded-lg bg-slate-950 text-slate-100 p-3 overflow-x-auto font-mono">
              {embedSnippet}
            </pre>
            <div className="font-semibold">REST API cURL Example:</div>
            <pre className="rounded-lg bg-slate-950 text-slate-100 p-3 overflow-x-auto font-mono">
{`curl -X POST ${window.location.origin}/api/submissions \\
  -H "X-Auth-Token: YOUR_API_TOKEN" \\
  -H "Content-Type: application/json" \\
  -d '{
    "template_id": "${template.id}",
    "send_email": true,
    "submitters": [
      { "role": "${template.submitters?.[0]?.name || 'First Party'}", "email": "signer@example.com" }
    ]
  }'`}
            </pre>
          </div>
        </DialogContent>
      </Dialog>

      <NewSubmissionDialog
        open={sendOpen}
        onOpenChange={setSendOpen}
        template={template}
        onCreated={() => loadTemplate()}
      />
    </div>
  );
};
