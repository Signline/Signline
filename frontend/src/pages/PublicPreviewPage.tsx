import React, { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { Download, FileCheck, ShieldCheck, CheckCircle2 } from 'lucide-react';
import type { Submission } from '@/types';
import { formatDate } from '@/lib/utils';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';

export const PublicPreviewPage: React.FC = () => {
  const { slug } = useParams<{ slug: string }>();
  const [submission, setSubmission] = useState<Submission | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!slug) return;
    api
      .get<any>(`/api/e/${slug}`)
      .then((res) => {
        setSubmission(res.submission || res.data || res);
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [slug]);

  if (loading) {
    return (
      <div className="py-16 text-center text-sm text-muted-foreground">
        Loading document preview...
      </div>
    );
  }

  if (!submission) {
    return (
      <div className="py-16 text-center text-sm text-muted-foreground">
        Submission record not found.
      </div>
    );
  }

  return (
    <div className="container max-w-3xl mx-auto py-12 px-4 space-y-6">
      <Card>
        <CardHeader className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <CheckCircle2 className="h-5 w-5 text-emerald-600" />
              <CardTitle>{submission.name || submission.template_name || 'Signed Document'}</CardTitle>
              <Badge variant={submission.status === 'completed' ? 'success' : 'secondary'}>
                {submission.status}
              </Badge>
            </div>
            <CardDescription>
              Created {formatDate(submission.created_at)}
              {submission.completed_at && ` · Completed ${formatDate(submission.completed_at)}`}
            </CardDescription>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <a
              href={`/api/submissions/${submission.id}/download`}
              target="_blank"
              rel="noopener noreferrer"
            >
              <Button size="sm">
                <Download className="h-4 w-4" />
                Signed PDF
              </Button>
            </a>
            <a
              href={`/api/submissions/${submission.id}/download?audit=true`}
              target="_blank"
              rel="noopener noreferrer"
            >
              <Button variant="outline" size="sm">
                <FileCheck className="h-4 w-4" />
                Audit Trail PDF
              </Button>
            </a>
          </div>
        </CardHeader>

        <CardContent className="space-y-4">
          <div className="rounded-lg border border-border bg-secondary/40 p-4 space-y-2 text-xs">
            <div className="font-semibold flex items-center gap-1.5">
              <ShieldCheck className="h-4 w-4 text-primary" />
              Participants &amp; Signatures
            </div>
            <div className="divide-y divide-border">
              {(submission.submitters || []).map((s) => (
                <div key={s.id || s.slug} className="py-2 flex items-center justify-between">
                  <div>
                    <span className="font-semibold">{s.name || s.role}</span>{' '}
                    <span className="text-muted-foreground">({s.email})</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant={s.status === 'completed' ? 'success' : 'secondary'}>
                      {s.status}
                    </Badge>
                    {s.completed_at && (
                      <span className="text-muted-foreground">{formatDate(s.completed_at)}</span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="text-center pt-2">
            <Link to="/verify" className="text-xs text-primary hover:underline">
              Verify this PDF's cryptographic PKCS#7 signature &amp; SHA-256 digest →
            </Link>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
