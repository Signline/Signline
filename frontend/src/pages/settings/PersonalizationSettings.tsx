import React, { useState, useEffect } from 'react';
import { Palette, Upload, RotateCcw, Save, Info } from 'lucide-react';
import { useBranding, UpstreamAttribution } from '@/lib/branding';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const PersonalizationSettings: React.FC = () => {
  const { branding, refreshBranding, applyColorPreview } = useBranding();
  const { toast } = useToast();

  const [productName, setProductName] = useState(branding.product_name || 'Signline');
  const [primaryColor, setPrimaryColor] = useState(branding.primary_color || '#2161AD');
  const [accentColor, setAccentColor] = useState(branding.accent_color || '#253E8A');
  const [emailSenderName, setEmailSenderName] = useState(branding.email_sender_name || '');
  const [emailFooter, setEmailFooter] = useState(branding.email_footer || '');
  const [supportEmail, setSupportEmail] = useState(branding.support_email || '');
  const [supportUrl, setSupportUrl] = useState(branding.support_url || '');
  const [termsUrl, setTermsUrl] = useState(branding.terms_url || '');
  const [privacyUrl, setPrivacyUrl] = useState(branding.privacy_url || '');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .get<any>('/api/settings/personalization')
      .then((res) => {
        const b = res.branding || res.personalization || res.data || res;
        if (b.product_name) setProductName(b.product_name);
        if (b.primary_color) setPrimaryColor(b.primary_color);
        if (b.accent_color) setAccentColor(b.accent_color);
        if (b.email_sender_name !== undefined) setEmailSenderName(b.email_sender_name || '');
        if (b.email_footer !== undefined) setEmailFooter(b.email_footer || '');
        if (b.support_email !== undefined) setSupportEmail(b.support_email || '');
        if (b.support_url !== undefined) setSupportUrl(b.support_url || '');
        if (b.terms_url !== undefined) setTermsUrl(b.terms_url || '');
        if (b.privacy_url !== undefined) setPrivacyUrl(b.privacy_url || '');
      })
      .catch(() => {});
  }, []);

  const handleColorChange = (primary: string, accent: string) => {
    setPrimaryColor(primary);
    setAccentColor(accent);
    applyColorPreview(primary, accent);
  };

  const handleResetColors = () => {
    handleColorChange('#2161AD', '#253E8A');
    setProductName('Signline');
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await api.put('/api/settings/personalization', {
        product_name: productName.trim() || 'Signline',
        primary_color: primaryColor,
        accent_color: accentColor,
        email_sender_name: emailSenderName.trim(),
        email_footer: emailFooter.trim(),
        support_email: supportEmail.trim(),
        support_url: supportUrl.trim(),
        terms_url: termsUrl.trim(),
        privacy_url: privacyUrl.trim(),
      });
      await refreshBranding();
      toast({
        title: 'White-Label Personalization Saved',
        description: 'Applied brand colors, product name, and support links across the platform.',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Failed to save branding', description: err.message, variant: 'destructive' });
    } finally {
      setSaving(false);
    }
  };

  const handleUploadAsset = async (type: 'logo' | 'favicon', file: File) => {
    const formData = new FormData();
    formData.append('file', file);
    try {
      await api.post(`/api/settings/personalization/${type}`, formData);
      await refreshBranding();
      toast({
        title: `Custom ${type === 'logo' ? 'Logo' : 'Favicon'} Uploaded`,
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Upload failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleResetAsset = async (type: 'logo' | 'favicon') => {
    try {
      await api.delete(`/api/settings/personalization/${type}`);
      await refreshBranding();
      toast({ title: `Reverted to Default Signline ${type === 'logo' ? 'Logo' : 'Favicon'}` });
    } catch (err: any) {
      toast({ title: 'Reset failed', description: err.message, variant: 'destructive' });
    }
  };

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Palette className="h-5 w-5 text-primary" />
            White-Label Personalization &amp; Brand Theme
          </CardTitle>
          <CardDescription>
            Customize the product name, primary &amp; accent brand colors, logo, favicon, outgoing email footer, and support links.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSave} className="space-y-5">
            <div className="space-y-1.5">
              <Label htmlFor="brand-product-name">Product / Application Name</Label>
              <Input
                id="brand-product-name"
                value={productName}
                onChange={(e) => setProductName(e.target.value)}
                placeholder="Signline"
              />
            </div>

            {/* Color Pickers */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <Label htmlFor="brand-primary-color">Primary Brand Color (Hex)</Label>
                <div className="flex items-center gap-2">
                  <input
                    type="color"
                    value={primaryColor}
                    onChange={(e) => handleColorChange(e.target.value, accentColor)}
                    className="h-9 w-12 cursor-pointer rounded border border-input bg-card p-0.5"
                  />
                  <Input
                    id="brand-primary-color"
                    value={primaryColor}
                    onChange={(e) => handleColorChange(e.target.value, accentColor)}
                    placeholder="#2161AD"
                    className="font-mono"
                  />
                </div>
              </div>

              <div className="space-y-1.5">
                <Label htmlFor="brand-accent-color">Accent Brand Color (Hex)</Label>
                <div className="flex items-center gap-2">
                  <input
                    type="color"
                    value={accentColor}
                    onChange={(e) => handleColorChange(primaryColor, e.target.value)}
                    className="h-9 w-12 cursor-pointer rounded border border-input bg-card p-0.5"
                  />
                  <Input
                    id="brand-accent-color"
                    value={accentColor}
                    onChange={(e) => handleColorChange(primaryColor, e.target.value)}
                    placeholder="#253E8A"
                    className="font-mono"
                  />
                </div>
              </div>
            </div>

            <div className="flex justify-end">
              <Button type="button" variant="ghost" size="sm" onClick={handleResetColors}>
                <RotateCcw className="h-3.5 w-3.5" />
                Reset to Default Signline Palette (#2161AD / #253E8A)
              </Button>
            </div>

            {/* Logo & Favicon Upload */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-2 border-t border-border">
              <div className="rounded-lg border border-border bg-secondary/30 p-3.5 space-y-2.5">
                <div className="flex items-center justify-between">
                  <Label className="text-xs font-semibold">Application Logo</Label>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 px-2 text-xs"
                    onClick={() => handleResetAsset('logo')}
                  >
                    Reset
                  </Button>
                </div>
                <div className="h-14 rounded bg-card border border-border flex items-center justify-center p-2">
                  <img
                    src={`${branding.logo_url}?t=${Date.now()}`}
                    alt="Current Logo"
                    className="max-h-10 object-contain"
                    onError={(e) => {
                      (e.currentTarget as HTMLImageElement).style.display = 'none';
                    }}
                  />
                </div>
                <label className="block">
                  <span className="inline-flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-md border border-input bg-card px-3 py-1.5 text-xs font-medium hover:bg-secondary">
                    <Upload className="h-3.5 w-3.5" />
                    Upload Custom Logo (SVG/PNG)
                  </span>
                  <input
                    type="file"
                    accept="image/*"
                    className="hidden"
                    onChange={(e) => {
                      const f = e.target.files?.[0];
                      if (f) handleUploadAsset('logo', f);
                    }}
                  />
                </label>
              </div>

              <div className="rounded-lg border border-border bg-secondary/30 p-3.5 space-y-2.5">
                <div className="flex items-center justify-between">
                  <Label className="text-xs font-semibold">Browser Favicon</Label>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 px-2 text-xs"
                    onClick={() => handleResetAsset('favicon')}
                  >
                    Reset
                  </Button>
                </div>
                <div className="h-14 rounded bg-card border border-border flex items-center justify-center p-2">
                  <img
                    src={`${branding.favicon_url}?t=${Date.now()}`}
                    alt="Current Favicon"
                    className="h-8 w-8 object-contain"
                    onError={(e) => {
                      (e.currentTarget as HTMLImageElement).style.display = 'none';
                    }}
                  />
                </div>
                <label className="block">
                  <span className="inline-flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-md border border-input bg-card px-3 py-1.5 text-xs font-medium hover:bg-secondary">
                    <Upload className="h-3.5 w-3.5" />
                    Upload Custom Favicon (SVG/PNG/ICO)
                  </span>
                  <input
                    type="file"
                    accept="image/*,.ico"
                    className="hidden"
                    onChange={(e) => {
                      const f = e.target.files?.[0];
                      if (f) handleUploadAsset('favicon', f);
                    }}
                  />
                </label>
              </div>
            </div>

            {/* Email Branding */}
            <div className="space-y-3 pt-2 border-t border-border">
              <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Outgoing Email Branding
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="brand-sender-name">Email Sender Display Name</Label>
                <Input
                  id="brand-sender-name"
                  value={emailSenderName}
                  onChange={(e) => setEmailSenderName(e.target.value)}
                  placeholder="e.g. Acme Legal Contracts"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="brand-email-footer">Custom Email Footer Text</Label>
                <Textarea
                  id="brand-email-footer"
                  value={emailFooter}
                  onChange={(e) => setEmailFooter(e.target.value)}
                  placeholder="e.g. Acme Corp · 100 Market St, San Francisco, CA"
                  rows={2}
                />
              </div>
            </div>

            {/* Support & Legal Links */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 pt-2 border-t border-border">
              <div className="space-y-1.5">
                <Label htmlFor="brand-support-email">Support Email</Label>
                <Input
                  id="brand-support-email"
                  type="email"
                  value={supportEmail}
                  onChange={(e) => setSupportEmail(e.target.value)}
                  placeholder="support@example.com"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="brand-support-url">Support Help Center URL</Label>
                <Input
                  id="brand-support-url"
                  type="url"
                  value={supportUrl}
                  onChange={(e) => setSupportUrl(e.target.value)}
                  placeholder="https://help.example.com"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="brand-terms-url">Terms of Service URL</Label>
                <Input
                  id="brand-terms-url"
                  type="url"
                  value={termsUrl}
                  onChange={(e) => setTermsUrl(e.target.value)}
                  placeholder="https://example.com/terms"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="brand-privacy-url">Privacy Policy URL</Label>
                <Input
                  id="brand-privacy-url"
                  type="url"
                  value={privacyUrl}
                  onChange={(e) => setPrivacyUrl(e.target.value)}
                  placeholder="https://example.com/privacy"
                />
              </div>
            </div>

            {/* Mandatory AGPLv3 Attribution Notice */}
            <Alert variant="info">
              <AlertTitle className="flex items-center gap-1.5 text-xs">
                <Info className="h-4 w-4 text-primary" />
                AGPLv3 Section 7(b) Upstream Attribution Notice
              </AlertTitle>
              <AlertDescription className="text-xs space-y-2 mt-1">
                <p>
                  In compliance with the GNU Affero General Public License v3.0 Additional Terms, the
                  upstream credit is permanently preserved in all footers and outgoing emails:
                </p>
                <div className="rounded bg-card p-2 border border-border">
                  <UpstreamAttribution />
                </div>
              </AlertDescription>
            </Alert>

            <Button type="submit" disabled={saving}>
              <Save className="h-4 w-4" />
              {saving ? 'Saving Personalization...' : 'Save Personalization Settings'}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
};
