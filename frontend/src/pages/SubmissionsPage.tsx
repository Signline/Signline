import React, { useState, useEffect, useCallback } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import {
  Send,
  Search,
  Download,
  FileCheck,
  ArrowLeft,
  Archive,
  RotateCcw,
  ExternalLink,
} from 'lucide-react';
import type { Submission, Template } from '@/types';
import { formatDate } from '@/lib/utils';
import { api, unwrapList } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent } from '@/components/ui/card';
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
import { SubmissionDetailView } from '@/components/submissions/SubmissionDetailView';

export const SubmissionsPage: React.FC = () => {
  const { id } = useParams<{ id?: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();

  const [submissions, setSubmissions] = useState<Submission[]>([]);
  const [templates, setTemplates] = useState<Template[]>([]);
  const [detailSubmission, setDetailSubmission] = useState<Submission | null>(null);
  const [statusFilter, setStatusFilter] = useState<string>('all');
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [loading, setLoading] = useState<boolean>(true);
  const [newModalOpen, setNewModalOpen] = useState<boolean>(false);

  const loadList = useCallback(async () => {
    setLoading(true);
    try {
      const [subRes, tplRes] = await Promise.all([
        api.get('/api/submissions'),
        api.get('/api/templates').catch(() => []),
      ]);
      setSubmissions(unwrapList<Submission>(subRes, 'submissions'));
      setTemplates(unwrapList<Template>(tplRes, 'templates'));
    } catch (err: any) {
      toast({
        title: 'Failed to load submissions',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  }, [toast]);

  const loadDetail = useCallback(async (subId: string) => {
    setLoading(true);
    try {
      const res = await api.get<any>(`/api/submissions/${subId}`);
      setDetailSubmission(res.submission || res.data || res);
    } catch (err: any) {
      toast({
        title: 'Failed to load submission',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    if (id) {
      loadDetail(id);
    } else {
      setDetailSubmission(null);
      loadList();
    }
  }, [id, loadDetail, loadList]);

  if (id && detailSubmission) {
    return (
      <div className="space-y-4">
        <Button variant="ghost" size="sm" onClick={() => navigate('/submissions')}>
          <ArrowLeft className="h-4 w-4" />
          Back to All Submissions
        </Button>
        <SubmissionDetailView
          submission={detailSubmission}
          onRefresh={() => loadDetail(id)}
        />
      </div>
    );
  }

  const handleArchiveToggle = async (sub: Submission) => {
    try {
      if (sub.archived_at) {
        await api.post(`/api/submissions/${sub.id}/unarchive`, {});
        toast({ title: 'Submission Restored', variant: 'success' });
      } else {
        await api.delete(`/api/submissions/${sub.id}`);
        toast({ title: 'Submission Archived' });
      }
      await loadList();
    } catch (err: any) {
      toast({ title: 'Action failed', description: err.message, variant: 'destructive' });
    }
  };

  const filtered = submissions.filter((s) => {
    if (statusFilter === 'archived') {
      if (!s.archived_at) return false;
    } else {
      if (s.archived_at) return false;
      if (statusFilter !== 'all' && s.status !== statusFilter) return false;
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      const matchName =
        s.name?.toLowerCase().includes(q) || s.template_name?.toLowerCase().includes(q);
      const matchSub = (s.submitters || []).some(
        (sub) =>
          sub.name?.toLowerCase().includes(q) || sub.email?.toLowerCase().includes(q)
      );
      return matchName || matchSub;
    }
    return true;
  });

  const statusTabs = [
    { key: 'all', label: 'All Active' },
    { key: 'pending', label: 'Pending' },
    { key: 'completed', label: 'Completed' },
    { key: 'declined', label: 'Declined' },
    { key: 'expired', label: 'Expired' },
    { key: 'archived', label: 'Archived' },
  ];

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Signature Submissions</h1>
          <p className="text-sm text-muted-foreground">
            Monitor envelope statuses, download signed PDFs and Audit Trail Certificates, or export data.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <a href="/api/submissions/export" target="_blank" rel="noopener noreferrer">
            <Button variant="outline">
              <Download className="h-4 w-4" />
              Export CSV
            </Button>
          </a>
          <Button onClick={() => setNewModalOpen(true)}>
            <Send className="h-4 w-4" />
            New Submission
          </Button>
        </div>
      </div>

      {/* Filter Bar */}
      <div className="flex flex-col md:flex-row items-stretch md:items-center justify-between gap-3 rounded-xl border border-border bg-card p-3 shadow-sm">
        <div className="flex flex-wrap items-center gap-1">
          {statusTabs.map((tab) => (
            <button
              key={tab.key}
              type="button"
              onClick={() => setStatusFilter(tab.key)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                statusFilter === tab.key
                  ? 'bg-primary text-primary-foreground'
                  : 'text-muted-foreground hover:bg-secondary hover:text-foreground'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        <div className="relative w-full md:w-72">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search signer name, email, template..."
            className="pl-8 h-9"
          />
        </div>
      </div>

      {/* Table */}
      <Card>
        <CardContent className="p-0">
          {loading ? (
            <div className="py-12 text-center text-sm text-muted-foreground">
              Loading submissions...
            </div>
          ) : filtered.length === 0 ? (
            <div className="py-14 text-center space-y-2">
              <Send className="h-8 w-8 text-muted-foreground mx-auto" />
              <div className="text-sm font-medium">No submissions match your filter</div>
              <p className="text-xs text-muted-foreground">
                Send a signature request from any template to start collecting signatures.
              </p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Template / Envelope</TableHead>
                  <TableHead>Signers</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((sub) => (
                  <TableRow key={sub.id}>
                    <TableCell>
                      <Link
                        to={`/submissions/${sub.id}`}
                        className="font-semibold text-foreground hover:text-primary"
                      >
                        {sub.name || sub.template_name || 'Document Envelope'}
                      </Link>
                      <div className="text-[11px] text-muted-foreground">
                        Slug: {sub.slug}
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        {(sub.submitters || []).map((signer) => (
                          <div key={signer.id || signer.slug} className="flex items-center gap-2 text-xs">
                            <Badge
                              variant={
                                signer.status === 'completed'
                                  ? 'success'
                                  : signer.status === 'declined'
                                  ? 'destructive'
                                  : 'secondary'
                              }
                              className="text-[10px] px-1.5 py-0"
                            >
                              {signer.status}
                            </Badge>
                            <span className="font-medium">{signer.name || signer.role}</span>
                            <span className="text-muted-foreground">({signer.email})</span>
                            <Link
                              to={`/s/${signer.slug}`}
                              className="text-primary hover:underline inline-flex items-center gap-0.5"
                              title="Open signing link"
                            >
                              <ExternalLink className="h-3 w-3" />
                            </Link>
                          </div>
                        ))}
                      </div>
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
                      <div className="inline-flex items-center gap-1.5">
                        <Link to={`/submissions/${sub.id}`}>
                          <Button variant="outline" size="sm">
                            Details &amp; Audit
                          </Button>
                        </Link>
                        <a
                          href={`/api/submissions/${sub.id}/download`}
                          target="_blank"
                          rel="noopener noreferrer"
                          title="Download Signed PDF"
                        >
                          <Button variant="secondary" size="sm">
                            <Download className="h-3.5 w-3.5" />
                          </Button>
                        </a>
                        <a
                          href={`/api/submissions/${sub.id}/download?audit=true`}
                          target="_blank"
                          rel="noopener noreferrer"
                          title="Download Audit Trail PDF"
                        >
                          <Button variant="ghost" size="sm">
                            <FileCheck className="h-3.5 w-3.5" />
                          </Button>
                        </a>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleArchiveToggle(sub)}
                          title={sub.archived_at ? 'Restore' : 'Archive'}
                        >
                          {sub.archived_at ? (
                            <RotateCcw className="h-3.5 w-3.5" />
                          ) : (
                            <Archive className="h-3.5 w-3.5" />
                          )}
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <NewSubmissionDialog
        open={newModalOpen}
        onOpenChange={setNewModalOpen}
        template={null}
        templates={templates}
        onCreated={() => loadList()}
      />
    </div>
  );
};
