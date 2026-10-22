import React, { useState } from 'react';
import { Link } from 'react-router-dom';
import {
  Download,
  FileCheck,
  Mail,
  ExternalLink,
  Copy,
  Check,
  Clock,
  ShieldCheck,
  Users,
  Activity,
} from 'lucide-react';
import type { Submission } from '@/types';
import { formatDate } from '@/lib/utils';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table';
import { useToast } from '@/components/ui/toast';

interface SubmissionDetailViewProps {
  submission: Submission;
  onRefresh: () => Promise<void>;
}

export const SubmissionDetailView: React.FC<SubmissionDetailViewProps> = ({
  submission,
  onRefresh,
}) => {
  const { toast } = useToast();
  const [copiedSlug, setCopiedSlug] = useState<string | null>(null);
  const [resendingId, setResendingId] = useState<string | null>(null);

  const handleResendEmail = async (submitterId: string) => {
    setResendingId(submitterId);
    try {
      await api.post(`/api/submissions/${submission.id}/resend_email`, {
        submitter_id: submitterId,
      });
      toast({
        title: 'Invitation Email Resent',
        description: 'Sent a fresh signature request email and logged the event.',
        variant: 'success',
      });
      await onRefresh();
    } catch (err: any) {
      toast({
        title: 'Failed to resend email',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setResendingId(null);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'completed':
        return <Badge variant="success">Completed</Badge>;
      case 'declined':
        return <Badge variant="destructive">Declined</Badge>;
      case 'opened':
        return <Badge variant="info">Opened</Badge>;
      case 'sent':
        return <Badge variant="warning">Sent</Badge>;
      default:
        return <Badge variant="secondary">Awaiting</Badge>;
    }
  };

  const submitters = submission.submitters || [];
  const events = submission.events || [];

  // Aggregate all filled values across submitters
  const allValuesEntries: Array<{
    signer: string;
    key: string;
    value: any;
  }> = [];
  for (const sub of submitters) {
    if (sub.values && typeof sub.values === 'object') {
      for (const [k, v] of Object.entries(sub.values)) {
        // Avoid duplicating UUID keys if a readable field name also exists
        if (/^[0-9a-f-]{36}$/i.test(k)) continue;
        allValuesEntries.push({
          signer: sub.name || sub.email || sub.role,
          key: k,
          value: v,
        });
      }
    }
  }

  return (
    <div className="space-y-6">
      {/* Header Banner */}
      <div className="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-border bg-card p-5 shadow-sm">
        <div className="space-y-1">
          <div className="flex items-center gap-2.5">
            <h1 className="text-xl font-bold">{submission.name || submission.template_name || 'Submission'}</h1>
            {getStatusBadge(submission.status)}
          </div>
          <p className="text-xs text-muted-foreground">
            Envelope ID: <code>{submission.id}</code> · Created {formatDate(submission.created_at)} ·
            Signing Order: <strong className="capitalize">{submission.submitters_order || 'preserved'}</strong>
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <a
            href={`/api/submissions/${submission.id}/download`}
            target="_blank"
            rel="noopener noreferrer"
          >
            <Button size="sm" className="gap-1.5">
              <Download className="h-4 w-4" />
              Download Signed PDF
            </Button>
          </a>
          <a
            href={`/api/submissions/${submission.id}/download?audit=true`}
            target="_blank"
            rel="noopener noreferrer"
          >
            <Button variant="outline" size="sm" className="gap-1.5">
              <FileCheck className="h-4 w-4" />
              Audit Trail PDF
            </Button>
          </a>
          <Link to={`/e/${submission.slug}`}>
            <Button variant="secondary" size="sm" className="gap-1.5">
              <ExternalLink className="h-3.5 w-3.5" />
              Public Preview
            </Button>
          </Link>
        </div>
      </div>

      {/* Submitters Table */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base flex items-center gap-2">
            <Users className="h-4 w-4 text-primary" />
            Signers &amp; Recipients ({submitters.length})
          </CardTitle>
          <CardDescription>
            Track real-time signing progress, copy direct signing URLs, or resend email invitations.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Role</TableHead>
                <TableHead>Signer</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Completed / Opened</TableHead>
                <TableHead className="text-right">Signing Link &amp; Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {submitters.map((sub) => {
                const signUrl = `${window.location.origin}/s/${sub.slug}`;
                return (
                  <TableRow key={sub.id || sub.slug}>
                    <TableCell className="font-medium">{sub.role || 'First Party'}</TableCell>
                    <TableCell>
                      <div className="font-semibold">{sub.name || '—'}</div>
                      <div className="text-xs text-muted-foreground">{sub.email}</div>
                    </TableCell>
                    <TableCell>{getStatusBadge(sub.status)}</TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {sub.completed_at
                        ? `Signed ${formatDate(sub.completed_at)}`
                        : sub.declined_at
                        ? `Declined ${formatDate(sub.declined_at)}`
                        : sub.opened_at
                        ? `Opened ${formatDate(sub.opened_at)}`
                        : sub.sent_at
                        ? `Sent ${formatDate(sub.sent_at)}`
                        : 'Pending'}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="inline-flex items-center gap-1.5">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => {
                            navigator.clipboard.writeText(signUrl);
                            setCopiedSlug(sub.slug);
                            setTimeout(() => setCopiedSlug(null), 2000);
                          }}
                        >
                          {copiedSlug === sub.slug ? (
                            <Check className="h-3.5 w-3.5 text-emerald-600" />
                          ) : (
                            <Copy className="h-3.5 w-3.5" />
                          )}
                          Copy Link
                        </Button>
                        <Link to={`/s/${sub.slug}`}>
                          <Button variant="secondary" size="sm">
                            <ExternalLink className="h-3.5 w-3.5" />
                            Open Form
                          </Button>
                        </Link>
                        {sub.status !== 'completed' && (
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={resendingId === sub.id}
                            onClick={() => handleResendEmail(sub.id)}
                          >
                            <Mail className="h-3.5 w-3.5" />
                            Resend
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      {/* Filled Field Values & Chronological Audit Trail */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base flex items-center gap-2">
              <ShieldCheck className="h-4 w-4 text-primary" />
              Submitted Field Data
            </CardTitle>
          </CardHeader>
          <CardContent>
            {allValuesEntries.length === 0 ? (
              <p className="text-xs text-muted-foreground py-4">
                No field values have been submitted yet.
              </p>
            ) : (
              <div className="space-y-2">
                {allValuesEntries.map((entry, idx) => (
                  <div
                    key={idx}
                    className="flex items-center justify-between gap-4 rounded-lg border border-border bg-secondary/30 px-3 py-2 text-xs"
                  >
                    <div>
                      <span className="font-semibold text-foreground">{entry.key}</span>
                      <span className="text-muted-foreground ml-1.5">({entry.signer})</span>
                    </div>
                    <div className="font-mono text-right truncate max-w-[240px]">
                      {typeof entry.value === 'string' && entry.value.startsWith('data:image') ? (
                        <img
                          src={entry.value}
                          alt={entry.key}
                          className="h-8 object-contain inline-block bg-white rounded px-1 border"
                        />
                      ) : (
                        String(entry.value)
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base flex items-center gap-2">
              <Activity className="h-4 w-4 text-primary" />
              Immutable Audit Trail Timeline ({events.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            {events.length === 0 ? (
              <p className="text-xs text-muted-foreground py-4">
                Audit events will appear here as signers view and complete the document.
              </p>
            ) : (
              <div className="relative pl-4 border-l-2 border-primary/30 space-y-3">
                {events.map((ev) => (
                  <div key={ev.id} className="relative text-xs">
                    <span className="absolute -left-[21px] top-1 h-2.5 w-2.5 rounded-full bg-primary" />
                    <div className="flex items-center justify-between">
                      <span className="font-semibold uppercase tracking-wide text-foreground">
                        {ev.event_type.replace(/_/g, ' ')}
                      </span>
                      <span className="text-muted-foreground inline-flex items-center gap-1">
                        <Clock className="h-3 w-3" />
                        {formatDate(ev.event_timestamp)}
                      </span>
                    </div>
                    {(ev.submitter_name || ev.submitter_email) && (
                      <div className="text-muted-foreground">
                        Participant: {ev.submitter_name} ({ev.submitter_email})
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
};
