import React, { useState } from 'react';
import {
  ShieldCheck,
  ShieldAlert,
  Upload,
  FileKey2,
  CheckCircle2,
  Clock,
  Hash,
  UserCheck,
} from 'lucide-react';
import type { PdfSignatureVerification } from '@/types';
import { formatBytes, formatDate } from '@/lib/utils';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const VerifyPdfPage: React.FC<{ embedded?: boolean }> = ({ embedded = false }) => {
  const { toast } = useToast();
  const [verifying, setVerifying] = useState(false);
  const [report, setReport] = useState<PdfSignatureVerification | null>(null);
  const [dragOver, setDragOver] = useState(false);

  const handleFileVerify = async (file: File) => {
    setVerifying(true);
    try {
      const formData = new FormData();
      formData.append('file', file);
      const res = await api.post<any>('/api/verify', formData);
      const data: PdfSignatureVerification = res.verification || res.data || res;
      setReport(data);
      toast({
        title: data.verified
          ? 'PDF Signature Cryptographically Verified'
          : 'PDF Verification Completed',
        description: `SHA-256: ${data.sha256?.slice(0, 24)}...`,
        variant: data.verified ? 'success' : 'default',
      });
    } catch (err: any) {
      toast({
        title: 'Verification failed',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setVerifying(false);
    }
  };

  return (
    <div className={embedded ? 'space-y-4' : 'container max-w-4xl mx-auto py-8 px-4 space-y-6'}>
      {!embedded && (
        <div className="space-y-1">
          <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
            <ShieldCheck className="h-6 w-6 text-primary" />
            Cryptographic PDF Signature Verifier
          </h1>
          <p className="text-sm text-muted-foreground">
            Inspect any PDF file for detached PKCS#7 / CMS digital signatures, `/ByteRange` tamper resistance, X.509 certificate chains, RFC 3161 timestamps, and matching Signline SHA-256 audit records.
          </p>
        </div>
      )}

      {/* Drag & Drop Upload Box */}
      <div
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragOver(false);
          const f = e.dataTransfer.files?.[0];
          if (f) handleFileVerify(f);
        }}
        className={`rounded-xl border-2 border-dashed p-8 text-center transition-colors ${
          dragOver
            ? 'border-primary bg-primary/10'
            : 'border-border bg-card hover:border-primary/50'
        }`}
      >
        <Upload className="h-10 w-10 text-primary mx-auto mb-3" />
        <div className="font-semibold text-sm">
          Drag and drop a signed PDF file here, or click to select a file
        </div>
        <p className="text-xs text-muted-foreground mt-1 mb-4">
          Computes the SHA-256 digest and verifies `/ByteRange` PKCS#7 digital signatures against X.509 certificates.
        </p>
        <label className="cursor-pointer inline-block">
          <span className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-sm hover:opacity-95">
            {verifying ? 'Analyzing Cryptographic Signatures...' : 'Select PDF to Verify'}
          </span>
          <input
            type="file"
            accept=".pdf,application/pdf"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) handleFileVerify(f);
            }}
          />
        </label>
      </div>

      {/* Verification Report */}
      {report && (
        <div className="space-y-4">
          <Card
            className={
              report.verified
                ? 'border-emerald-500/50 bg-emerald-50/20'
                : 'border-amber-500/50 bg-amber-50/20'
            }
          >
            <CardHeader className="pb-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2.5">
                  {report.verified ? (
                    <CheckCircle2 className="h-6 w-6 text-emerald-600" />
                  ) : (
                    <ShieldAlert className="h-6 w-6 text-amber-600" />
                  )}
                  <div>
                    <CardTitle className="text-lg">
                      {report.verified
                        ? 'Authentic & Cryptographically Verified PDF'
                        : `PDF Status: ${report.integrity_status || 'Unsigned / Unmatched'}`}
                    </CardTitle>
                    <CardDescription>
                      {report.filename} ({formatBytes(report.byte_size)})
                    </CardDescription>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <Badge variant={report.verified ? 'success' : 'warning'}>
                    Integrity: {report.integrity_status || (report.verified ? 'Valid' : 'Unsigned')}
                  </Badge>
                  {report.pkcs7_present && (
                    <Badge variant="info">PKCS#7 / CMS Signed</Badge>
                  )}
                </div>
              </div>
            </CardHeader>
            <CardContent className="space-y-4 text-xs">
              <div className="rounded-lg border border-border bg-card p-3 flex flex-col sm:flex-row sm:items-center justify-between gap-2">
                <span className="font-semibold flex items-center gap-1.5 text-muted-foreground">
                  <Hash className="h-4 w-4 text-primary" />
                  Document SHA-256 Digest:
                </span>
                <code className="font-mono text-foreground break-all">{report.sha256}</code>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {/* X.509 Certificate Details */}
                <div className="rounded-lg border border-border bg-card p-4 space-y-2">
                  <div className="font-semibold text-sm flex items-center gap-1.5">
                    <FileKey2 className="h-4 w-4 text-primary" />
                    X.509 Digital Certificate &amp; PKCS#7
                  </div>
                  {report.certificate ? (
                    <div className="space-y-1 text-muted-foreground">
                      <div>
                        Subject: <strong className="text-foreground">{report.certificate.subject}</strong>
                      </div>
                      <div>
                        Issuer: <strong className="text-foreground">{report.certificate.issuer}</strong>
                      </div>
                      <div>
                        Serial: <code className="text-foreground">{report.certificate.serial_number}</code>
                      </div>
                      <div>
                        Validity Window:{' '}
                        <span className="text-foreground">
                          {formatDate(report.certificate.not_before)} →{' '}
                          {formatDate(report.certificate.not_after)}
                        </span>
                      </div>
                      <div>
                        ByteRange Integrity:{' '}
                        <Badge variant={report.byte_range_valid ? 'success' : 'destructive'}>
                          {report.byte_range_valid ? 'Valid / Untampered' : 'Invalid'}
                        </Badge>
                      </div>
                    </div>
                  ) : (
                    <p className="text-muted-foreground">
                      No embedded X.509 certificate metadata found in PDF dictionary.
                    </p>
                  )}
                </div>

                {/* Matching Signline Audit Trail Record */}
                <div className="rounded-lg border border-border bg-card p-4 space-y-2">
                  <div className="font-semibold text-sm flex items-center gap-1.5">
                    <UserCheck className="h-4 w-4 text-primary" />
                    Signline Audit Registry Match
                  </div>
                  {report.matching_submission ? (
                    <div className="space-y-1.5 text-muted-foreground">
                      <div>
                        Template:{' '}
                        <strong className="text-foreground">
                          {report.matching_submission.template_name}
                        </strong>
                      </div>
                      <div>
                        Envelope Slug:{' '}
                        <code className="text-foreground">
                          {report.matching_submission.submission_slug}
                        </code>
                      </div>
                      <div>
                        Completed At:{' '}
                        <strong className="text-foreground">
                          {formatDate(report.matching_submission.completed_at)}
                        </strong>
                      </div>
                      <div className="pt-1">
                        <div className="font-semibold text-foreground mb-1">Verified Signers:</div>
                        {(report.matching_submission.submitters || []).map((s, i) => (
                          <div key={i} className="text-[11px]">
                            • <strong>{s.name}</strong> ({s.email}) — {s.role}
                          </div>
                        ))}
                      </div>
                    </div>
                  ) : (
                    <p className="text-muted-foreground">
                      No matching `completed_documents` SHA-256 record found in this instance's registry.
                    </p>
                  )}

                  {report.timestamp && (
                    <div className="pt-2 border-t border-border flex items-center justify-between">
                      <span className="inline-flex items-center gap-1 text-muted-foreground">
                        <Clock className="h-3.5 w-3.5" />
                        RFC 3161 Timestamp:
                      </span>
                      <Badge variant={report.timestamp.present ? 'info' : 'secondary'}>
                        {report.timestamp.status || (report.timestamp.present ? 'Verified' : 'None')}
                      </Badge>
                    </div>
                  )}
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
};
