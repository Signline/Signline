import React, { useState, useEffect } from 'react';
import { HardDrive, Save } from 'lucide-react';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const StorageSettings: React.FC = () => {
  const { toast } = useToast();
  const [service, setService] = useState('disk');
  const [bucket, setBucket] = useState('');
  const [region, setRegion] = useState('us-east-1');
  const [endpoint, setEndpoint] = useState('');
  const [accessKeyId, setAccessKeyId] = useState('');
  const [secretAccessKey, setSecretAccessKey] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .get<any>('/api/settings/storage')
      .then((res) => {
        const s = res.storage || res.data || res;
        if (s.service || s.provider) setService(s.service || s.provider);
        if (s.bucket) setBucket(s.bucket);
        if (s.region) setRegion(s.region);
        if (s.endpoint) setEndpoint(s.endpoint);
        if (s.access_key_id) setAccessKeyId(s.access_key_id);
      })
      .catch(() => {});
  }, []);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await api.put('/api/settings/storage', {
        service,
        bucket: bucket.trim(),
        region: region.trim(),
        endpoint: endpoint.trim(),
        access_key_id: accessKeyId.trim(),
        secret_access_key: secretAccessKey || undefined,
      });
      toast({
        title: 'Storage Settings Saved',
        description: `Configured active storage engine: ${service}.`,
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Save failed', description: err.message, variant: 'destructive' });
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <HardDrive className="h-5 w-5 text-primary" />
          Document &amp; Attachment Storage
        </CardTitle>
        <CardDescription>
          Configure where uploaded PDF templates, page previews, and signed documents are stored.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSave} className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="storage-provider">Storage Provider</Label>
            <Select
              id="storage-provider"
              value={service}
              onChange={(e) => setService(e.target.value)}
            >
              <option value="disk">Local Disk + PocketBase Storage (`pb_data/storage`)</option>
              <option value="aws_s3">Amazon S3 / S3-Compatible (MinIO, Cloudflare R2)</option>
              <option value="google_gcs">Google Cloud Storage (GCS)</option>
              <option value="azure_blob">Azure Blob Storage</option>
            </Select>
          </div>

          {service !== 'disk' && (
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-2">
              <div className="space-y-1.5">
                <Label>Bucket / Container Name</Label>
                <Input
                  value={bucket}
                  onChange={(e) => setBucket(e.target.value)}
                  placeholder="signline-documents"
                  required
                />
              </div>
              <div className="space-y-1.5">
                <Label>Region</Label>
                <Input
                  value={region}
                  onChange={(e) => setRegion(e.target.value)}
                  placeholder="us-east-1"
                />
              </div>
              <div className="space-y-1.5 sm:col-span-2">
                <Label>Custom S3 Endpoint (Optional, for R2 / MinIO / DigitalOcean)</Label>
                <Input
                  value={endpoint}
                  onChange={(e) => setEndpoint(e.target.value)}
                  placeholder="https://s3.us-east-1.amazonaws.com"
                />
              </div>
              <div className="space-y-1.5">
                <Label>Access Key ID / Client ID</Label>
                <Input
                  value={accessKeyId}
                  onChange={(e) => setAccessKeyId(e.target.value)}
                />
              </div>
              <div className="space-y-1.5">
                <Label>Secret Access Key</Label>
                <Input
                  type="password"
                  value={secretAccessKey}
                  onChange={(e) => setSecretAccessKey(e.target.value)}
                  placeholder="••••••••••••"
                />
              </div>
            </div>
          )}

          <Button type="submit" disabled={saving}>
            <Save className="h-4 w-4" />
            {saving ? 'Saving...' : 'Save Storage Configuration'}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
};
