import React, { useState, useEffect, useCallback } from 'react';
import { FileKey2, Upload, RefreshCw, Clock, Save } from 'lucide-react';
import { formatDate } from '@/lib/utils';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';
import { VerifyPdfPage } from '@/pages/VerifyPdfPage';

export const EsignSettings: React.FC = () => {
  const { toast } = useToast();
  const [cert, setCert] = useState<any>(null);
  const [tsaUrl, setTsaUrl] = useState('');
  const [p12File, setP12File] = useState<File | null>(null);
  const [p12Password, setP12Password] = useState('');
  const [loading, setLoading] = useState(false);

  const loadCert = useCallback(async () => {
    try {
      const res = await api.get<any>('/api/settings/esign');
      const data = res.certificate || res.esign || res.data || res;
      setCert(data);
      if (res.timestamp_server_url !== undefined) {
        setTsaUrl(res.timestamp_server_url || '');
      } else if (data.timestamp_server_url !== undefined) {
        setTsaUrl(data.timestamp_server_url || '');
      }
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadCert();
  }, [loadCert]);

  const handleRegenerateCert = async () => {
    setLoading(true);
    try {
      await api.post('/api/settings/esign', { action: 'regenerate' });
      toast({
        title: 'Generated New 2048-Bit RSA X.509 Certificate',
        variant: 'success',
      });
      await loadCert();
    } catch (err: any) {
      toast({ title: 'Failed to regenerate', description: err.message, variant: 'destructive' });
    } finally {
      setLoading(false);
    }
  };

  const handleUploadP12 = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!p12File) return;
    setLoading(true);
    try {
      const formData = new FormData();
      formData.append('file', p12File);
      formData.append('password', p12Password);
      await api.post('/api/settings/esign', formData);
      toast({
        title: 'Custom PKCS#12 Certificate Installed',
        variant: 'success',
      });
      setP12File(null);
      setP12Password('');
      await loadCert();
    } catch (err: any) {
      toast({ title: 'PKCS#12 upload failed', description: err.message, variant: 'destructive' });
    } finally {
      setLoading(false);
    }
  };

  const handleSaveTsa = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      await api.post('/api/settings/esign', {
        timestamp_server_url: tsaUrl.trim(),
      });
      toast({
        title: 'RFC 3161 Timestamp Authority URL Saved',
        variant: 'success',
      });
      await loadCert();
    } catch (err: any) {
      toast({ title: 'Save failed', description: err.message, variant: 'destructive' });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <CardTitle className="flex items-center gap-2">
                <FileKey2 className="h-5 w-5 text-primary" />
                Active X.509 PDF Signing Certificate
              </CardTitle>
              <CardDescription>
                Every completed PDF is sealed with a detached PKCS#7 / CMS digital signature (`/SubFilter /adbe.pkcs7.detached`).
              </CardDescription>
            </div>
            <Badge variant="success">Active &amp; Ready</Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="rounded-lg border border-border bg-secondary/40 p-4 text-xs space-y-2">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
              <div>
                <span className="text-muted-foreground">Subject:</span>{' '}
                <strong className="text-foreground">
                  {cert?.subject || 'CN=signline.local, O=Signline'}
                </strong>
              </div>
              <div>
                <span className="text-muted-foreground">Issuer:</span>{' '}
                <strong className="text-foreground">
                  {cert?.issuer || 'CN=signline.local, O=Signline'}
                </strong>
              </div>
              <div>
                <span className="text-muted-foreground">Serial Number:</span>{' '}
                <code className="font-mono">{cert?.serial_number || cert?.serial || '01'}</code>
              </div>
              <div>
                <span className="text-muted-foreground">Validity:</span>{' '}
                <span>
                  {formatDate(cert?.not_before)} → {formatDate(cert?.not_after)}
                </span>
              </div>
            </div>
            {cert?.fingerprint_sha256 && (
              <div className="pt-1 border-t border-border">
                <span className="text-muted-foreground">SHA-256 Fingerprint:</span>{' '}
                <code className="font-mono break-all">{cert.fingerprint_sha256}</code>
              </div>
            )}
          </div>

          <div className="flex flex-wrap items-center justify-between gap-3">
            <Button variant="outline" size="sm" onClick={handleRegenerateCert} disabled={loading}>
              <RefreshCw className="h-3.5 w-3.5" />
              Regenerate Self-Signed 2048-Bit RSA Certificate
            </Button>
          </div>

          {/* Custom PKCS#12 Upload */}
          <form onSubmit={handleUploadP12} className="space-y-3 pt-4 border-t border-border">
            <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Upload Custom PKCS#12 (.p12 / .pfx) Signing Certificate
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div className="space-y-1">
                <Label htmlFor="p12-file" className="text-xs">
                  PKCS#12 Certificate Bundle (.p12 / .pfx)
                </Label>
                <Input
                  id="p12-file"
                  type="file"
                  accept=".p12,.pfx"
                  onChange={(e) => setP12File(e.target.files?.[0] || null)}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="p12-pass" className="text-xs">
                  PKCS#12 Password
                </Label>
                <Input
                  id="p12-pass"
                  type="password"
                  value={p12Password}
                  onChange={(e) => setP12Password(e.target.value)}
                  placeholder="Certificate bundle passphrase"
                />
              </div>
            </div>
            <Button type="submit" size="sm" disabled={!p12File || loading}>
              <Upload className="h-3.5 w-3.5" />
              Upload &amp; Activate PKCS#12 Certificate
            </Button>
          </form>

          {/* RFC 3161 Timestamp Authority */}
          <form onSubmit={handleSaveTsa} className="space-y-3 pt-4 border-t border-border">
            <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground flex items-center gap-1.5">
              <Clock className="h-3.5 w-3.5 text-primary" />
              RFC 3161 Trusted Timestamp Server (TSA)
            </div>
            <div className="flex items-center gap-2">
              <Input
                value={tsaUrl}
                onChange={(e) => setTsaUrl(e.target.value)}
                placeholder="http://timestamp.digicert.com"
              />
              <Button type="submit" disabled={loading}>
                <Save className="h-4 w-4" />
                Save TSA
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>

      {/* Embedded PDF Signature Verifier */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Test &amp; Verify Signed PDF</CardTitle>
          <CardDescription>
            Verify any PDF's cryptographic signature directly inside E-Sign settings.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <VerifyPdfPage embedded />
        </CardContent>
      </Card>
    </div>
  );
};
