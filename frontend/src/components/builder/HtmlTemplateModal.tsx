import React, { useState } from 'react';
import { Code2, Eye } from 'lucide-react';
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
import { Textarea } from '@/components/ui/textarea';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';

interface HtmlTemplateModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialName?: string;
  initialHtml?: string;
  onSave: (name: string, html: string) => Promise<void>;
}

const DEFAULT_HTML_SAMPLE = `<div style="font-family: sans-serif; padding: 32px; color: #1e293b; line-height: 1.6;">
  <h1 style="color: #2161AD; border-bottom: 2px solid #2161AD; padding-bottom: 8px;">
    Master Services & Non-Disclosure Agreement
  </h1>
  <p>
    This Agreement is entered into as of <strong>{{effective_date;role=First Party;type=date}}</strong>
    by and between <strong>Signline Organization</strong> and the undersigned Counterparty.
  </p>
  <h3>1. Scope of Confidentiality</h3>
  <p>
    Both parties agree to hold all proprietary technical and business information in strict confidence.
  </p>
  <table style="width: 100%; margin-top: 32px; border-collapse: collapse;">
    <tr>
      <td style="width: 50%; padding: 12px; border: 1px solid #cbd5e1;">
        <strong>First Party Name:</strong><br/>
        {{first_party_name;role=First Party;type=text}}
      </td>
      <td style="width: 50%; padding: 12px; border: 1px solid #cbd5e1;">
        <strong>Second Party Name:</strong><br/>
        {{second_party_name;role=Second Party;type=text}}
      </td>
    </tr>
    <tr>
      <td style="width: 50%; padding: 12px; border: 1px solid #cbd5e1;">
        <strong>First Party Signature:</strong><br/>
        {{first_party_signature;role=First Party;type=signature}}
      </td>
      <td style="width: 50%; padding: 12px; border: 1px solid #cbd5e1;">
        <strong>Second Party Signature:</strong><br/>
        {{second_party_signature;role=Second Party;type=signature}}
      </td>
    </tr>
  </table>
</div>`;

export const HtmlTemplateModal: React.FC<HtmlTemplateModalProps> = ({
  open,
  onOpenChange,
  initialName = 'Dynamic HTML Agreement',
  initialHtml = DEFAULT_HTML_SAMPLE,
  onSave,
}) => {
  const [name, setName] = useState(initialName);
  const [html, setHtml] = useState(initialHtml || DEFAULT_HTML_SAMPLE);
  const [saving, setSaving] = useState(false);

  const insertTag = (tag: string) => {
    setHtml((prev) => `${prev}\n${tag}`);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !html.trim()) return;
    setSaving(true);
    try {
      await onSave(name.trim(), html);
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl" onClose={() => onOpenChange(false)}>
        <form onSubmit={handleSubmit} className="space-y-4">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Code2 className="h-5 w-5 text-primary" />
              Dynamic HTML-to-PDF Template Editor
            </DialogTitle>
            <DialogDescription>
              Author rich HTML documents with embedded{' '}
              <code>{'{{field_name;role=First Party;type=signature}}'}</code> field tags that
              automatically compile into interactive PDF fields.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-1.5">
            <Label htmlFor="html-tpl-name">Template Name</Label>
            <Input
              id="html-tpl-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
            />
          </div>

          <div className="flex flex-wrap items-center gap-1.5 text-xs">
            <span className="text-muted-foreground mr-1">Quick Insert Tag:</span>
            {[
              { label: '+ Signature', tag: '{{signature;role=First Party;type=signature}}' },
              { label: '+ Text Field', tag: '{{full_name;role=First Party;type=text}}' },
              { label: '+ Date Field', tag: '{{signed_date;role=First Party;type=date}}' },
              { label: '+ Checkbox', tag: '{{accepted_terms;role=First Party;type=checkbox}}' },
            ].map((item) => (
              <Button
                key={item.label}
                type="button"
                variant="secondary"
                size="sm"
                className="h-6 px-2 text-xs"
                onClick={() => insertTag(item.tag)}
              >
                {item.label}
              </Button>
            ))}
          </div>

          <Tabs defaultValue="code">
            <TabsList>
              <TabsTrigger value="code" className="gap-1.5">
                <Code2 className="h-3.5 w-3.5" />
                HTML Source
              </TabsTrigger>
              <TabsTrigger value="preview" className="gap-1.5">
                <Eye className="h-3.5 w-3.5" />
                Live Preview
              </TabsTrigger>
            </TabsList>

            <TabsContent value="code">
              <Textarea
                value={html}
                onChange={(e) => setHtml(e.target.value)}
                rows={13}
                className="font-mono text-xs"
              />
            </TabsContent>

            <TabsContent value="preview">
              <div
                className="rounded-lg border border-border bg-white p-4 max-h-80 overflow-y-auto shadow-inner"
                dangerouslySetInnerHTML={{ __html: html }}
              />
            </TabsContent>
          </Tabs>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={saving}>
              {saving ? 'Generating PDF & Saving...' : 'Save HTML Template'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
