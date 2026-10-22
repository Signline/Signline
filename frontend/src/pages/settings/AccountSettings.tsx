import React, { useState, useEffect } from 'react';
import { Building2, Save } from 'lucide-react';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const AccountSettings: React.FC = () => {
  const { toast } = useToast();

  const [name, setName] = useState('Signline Organization');
  const [timezone, setTimezone] = useState('UTC');
  const [locale, setLocale] = useState('en-US');
  const [filenameFormat, setFilenameFormat] = useState('{template.name} - {submission.status}');

  const [prefs, setPrefs] = useState({
    flatten_result_pdf: true,
    with_signature_id: true,
    with_submitter_timezone: true,
    combine_pdf_result: true,
    allow_to_resubmit: true,
    allow_to_decline: true,
    allow_typed_signature: true,
    verify_submitter_email: false,
    enforce_signing_order: true,
  });
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .get<any>('/api/settings/account')
      .then((res) => {
        const data = res.account || res.data || res;
        if (data.name) setName(data.name);
        if (data.timezone) setTimezone(data.timezone);
        if (data.locale) setLocale(data.locale);
        const cfg = res.configs || data.configs || data.preferences || data;
        if (cfg.document_filename_format) setFilenameFormat(cfg.document_filename_format);
        setPrefs((prev) => ({
          ...prev,
          ...(typeof cfg === 'object' ? cfg : {}),
        }));
      })
      .catch(() => {});
  }, []);

  const togglePref = (key: keyof typeof prefs, val: boolean) => {
    setPrefs((prev) => ({ ...prev, [key]: val }));
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await api.put('/api/settings/account', {
        name: name.trim(),
        timezone,
        locale,
        document_filename_format: filenameFormat,
        ...prefs,
      });
      toast({
        title: 'Account & Signing Settings Saved',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Save failed', description: err.message, variant: 'destructive' });
    } finally {
      setSaving(false);
    }
  };

  const preferenceItems: Array<{
    key: keyof typeof prefs;
    title: string;
    description: string;
  }> = [
    {
      key: 'flatten_result_pdf',
      title: 'Flatten Result PDF Documents',
      description: 'Permanently flatten form annotations into the PDF content stream upon signing.',
    },
    {
      key: 'with_signature_id',
      title: 'Include Cryptographic Signature ID Stamp',
      description: 'Stamp a tamper-evident border, UUID, and timestamp beneath signatures and initials.',
    },
    {
      key: 'with_submitter_timezone',
      title: 'Use Submitter Local Timezone in Timestamps',
      description: 'Display date & time stamps in the signer’s detected timezone alongside UTC.',
    },
    {
      key: 'combine_pdf_result',
      title: 'Combine Multi-Document Submissions & Audit Trail',
      description: 'Merge all template documents into a single signed PDF package on download.',
    },
    {
      key: 'allow_to_decline',
      title: 'Allow Signers to Decline Documents',
      description: 'Show the "Decline" button in the public signing interface (/s/:slug).',
    },
    {
      key: 'allow_typed_signature',
      title: 'Allow Typed Script Signatures',
      description: 'Allow signers to type their signature in addition to drawing or uploading.',
    },
    {
      key: 'allow_to_resubmit',
      title: 'Allow Resubmitting Completed Forms',
      description: 'Permit signers to re-open and update their submission if needed.',
    },
    {
      key: 'verify_submitter_email',
      title: 'Require Email 2FA OTP Before Signing',
      description: 'Prompt signers to verify a 6-digit email verification code before opening `/s/:slug`.',
    },
    {
      key: 'enforce_signing_order',
      title: 'Enforce Sequential Signing Order by Default',
      description: 'Default new multi-party submissions to preserved sequential signing order.',
    },
  ];

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Building2 className="h-5 w-5 text-primary" />
          Organization &amp; Signing Preferences
        </CardTitle>
        <CardDescription>
          Configure organization details, timezone, locale, and global document signing behavior.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSave} className="space-y-5">
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <div className="space-y-1.5">
              <Label htmlFor="acc-name">Company / Organization Name</Label>
              <Input
                id="acc-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="acc-tz">Default Timezone</Label>
              <Input
                id="acc-tz"
                value={timezone}
                onChange={(e) => setTimezone(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="acc-locale">Default Locale</Label>
              <Select value={locale} onChange={(e) => setLocale(e.target.value)}>
                <option value="en-US">English (US)</option>
                <option value="en-GB">English (UK)</option>
                <option value="es-ES">Español</option>
                <option value="fr-FR">Français</option>
                <option value="de-DE">Deutsch</option>
              </Select>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="acc-filename">Downloaded Document Filename Format</Label>
            <Input
              id="acc-filename"
              value={filenameFormat}
              onChange={(e) => setFilenameFormat(e.target.value)}
              placeholder="{template.name} - {submitter.name}"
            />
          </div>

          <div className="space-y-2.5 pt-2 border-t border-border">
            <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Global E-Signature Controls
            </div>
            {preferenceItems.map((item) => (
              <div
                key={item.key}
                className="flex items-center justify-between gap-4 rounded-lg border border-border bg-secondary/20 p-3"
              >
                <div className="text-xs">
                  <div className="font-semibold text-foreground">{item.title}</div>
                  <div className="text-muted-foreground">{item.description}</div>
                </div>
                <Switch
                  checked={Boolean(prefs[item.key])}
                  onCheckedChange={(checked) => togglePref(item.key, checked)}
                />
              </div>
            ))}
          </div>

          <Button type="submit" disabled={saving}>
            <Save className="h-4 w-4" />
            {saving ? 'Saving...' : 'Save Account Settings'}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
};
