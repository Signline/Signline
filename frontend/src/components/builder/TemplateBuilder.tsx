import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  ArrowLeft,
  Save,
  Sparkles,
  Code2,
  Send,
  Upload,
  History,
} from 'lucide-react';
import type { FieldType, SubmitterRole, Template, TemplateField } from '@/types';
import { ALL_FIELD_TYPES, generateUuid } from '@/lib/utils';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useToast } from '@/components/ui/toast';
import { DocumentCanvas } from '@/components/builder/DocumentCanvas';
import { FieldPalette } from '@/components/builder/FieldPalette';
import { FieldInspector } from '@/components/builder/FieldInspector';
import { HtmlTemplateModal } from '@/components/builder/HtmlTemplateModal';

interface TemplateBuilderProps {
  template: Template;
  onTemplateSaved: (updated: Template) => void;
  onOpenSendModal?: () => void;
}

export const TemplateBuilder: React.FC<TemplateBuilderProps> = ({
  template,
  onTemplateSaved,
  onOpenSendModal,
}) => {
  const navigate = useNavigate();
  const { toast } = useToast();

  const [name, setName] = useState(template.name);
  const [submitters, setSubmitters] = useState<SubmitterRole[]>(() =>
    template.submitters && template.submitters.length > 0
      ? template.submitters
      : [{ uuid: generateUuid(), name: 'First Party' }]
  );
  const [selectedSubmitterUuid, setSelectedSubmitterUuid] = useState<string>(
    () => submitters[0]?.uuid || ''
  );
  const [fields, setFields] = useState<TemplateField[]>(() => template.fields || []);
  const [selectedFieldUuid, setSelectedFieldUuid] = useState<string | null>(null);
  const [activePage, setActivePage] = useState<number>(0);
  const [saving, setSaving] = useState(false);
  const [detecting, setDetecting] = useState(false);
  const [htmlModalOpen, setHtmlModalOpen] = useState(false);

  const selectedField = fields.find((f) => f.uuid === selectedFieldUuid) || null;

  const createFieldObject = (
    type: FieldType,
    page: number,
    x: number,
    y: number
  ): TemplateField => {
    const meta = ALL_FIELD_TYPES.find((f) => f.type === type);
    const sameTypeCount = fields.filter((f) => f.type === type).length + 1;
    const defaultOptions =
      type === 'select' || type === 'radio' || type === 'multiple'
        ? [
            { uuid: generateUuid(), value: 'Option 1' },
            { uuid: generateUuid(), value: 'Option 2' },
          ]
        : undefined;

    return {
      uuid: generateUuid(),
      submitter_uuid: selectedSubmitterUuid || submitters[0]?.uuid || 'party-1',
      name: `${type}_${sameTypeCount}`,
      type,
      required: type !== 'note' && type !== 'heading' && type !== 'checkbox',
      options: defaultOptions,
      areas: [
        {
          x,
          y,
          w: meta?.defaultW || 0.22,
          h: meta?.defaultH || 0.04,
          page,
        },
      ],
    };
  };

  const handleAddFieldFromClick = (type: FieldType) => {
    const pageFieldsCount = fields.filter(
      (f) => (f.areas?.[0]?.page ?? 0) === activePage
    ).length;
    const offset = (pageFieldsCount % 8) * 0.055;
    const newField = createFieldObject(
      type,
      activePage,
      Number((0.12 + (pageFieldsCount % 3) * 0.05).toFixed(3)),
      Number(Math.min(0.85, 0.12 + offset).toFixed(3))
    );
    setFields((prev) => [...prev, newField]);
    setSelectedFieldUuid(newField.uuid);
  };

  const handleDropNewField = (type: FieldType, page: number, x: number, y: number) => {
    const newField = createFieldObject(type, page, x, y);
    setFields((prev) => [...prev, newField]);
    setSelectedFieldUuid(newField.uuid);
  };

  const handleUpdateField = (updated: TemplateField) => {
    setFields((prev) => prev.map((f) => (f.uuid === updated.uuid ? updated : f)));
  };

  const handleDeleteField = (uuid: string) => {
    setFields((prev) => prev.filter((f) => f.uuid !== uuid));
    if (selectedFieldUuid === uuid) {
      setSelectedFieldUuid(null);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const res = await api.put<any>(`/api/templates/${template.id}`, {
        name,
        submitters,
        fields,
        schema: template.schema,
      });
      const updated = res.template || res.data || res;
      onTemplateSaved({ ...template, ...updated, name, submitters, fields });
      toast({
        title: 'Template Saved',
        description: `Saved ${fields.length} fields across ${submitters.length} signer roles.`,
        variant: 'success',
      });
    } catch (err: any) {
      toast({
        title: 'Failed to save template',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setSaving(false);
    }
  };

  const handleAutoDetectFields = async () => {
    setDetecting(true);
    try {
      const res = await api.post<any>(`/api/templates/${template.id}/detect_fields`, {});
      const detected: TemplateField[] = res.fields || res.detected_fields || [];
      if (detected.length > 0) {
        setFields(detected);
        toast({
          title: `Auto-Detected ${detected.length} Fields`,
          description: 'Extracted form fields from PDF text runs and visual lines.',
          variant: 'success',
        });
      } else {
        toast({
          title: 'Auto-Detection Complete',
          description: 'No additional unspoken text tags found; existing fields preserved.',
        });
      }
    } catch (err: any) {
      toast({
        title: 'Auto-Detect Fields failed',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setDetecting(false);
    }
  };

  const handleReplaceDocument = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const formData = new FormData();
    formData.append('file', file);
    formData.append('template_id', template.id);
    try {
      const res = await api.post<any>('/api/templates/upload', formData);
      const updated = res.template || res.data || res;
      if (updated) {
        onTemplateSaved(updated);
        toast({
          title: 'Document Updated',
          description: 'Uploaded and rendered PDF pages.',
          variant: 'success',
        });
      }
    } catch (err: any) {
      toast({
        title: 'Upload failed',
        description: err.message,
        variant: 'destructive',
      });
    }
  };

  const handleSaveHtml = async (htmlName: string, htmlBody: string) => {
    const res = await api.post<any>('/api/templates/html', {
      template_id: template.id,
      name: htmlName,
      html: htmlBody,
    });
    const updated = res.template || res.data || res;
    if (updated) {
      setName(updated.name || htmlName);
      if (updated.fields) setFields(updated.fields);
      if (updated.submitters) setSubmitters(updated.submitters);
      onTemplateSaved(updated);
      toast({
        title: 'Dynamic HTML Compiled to PDF',
        description: 'Extracted tags and updated document preview.',
        variant: 'success',
      });
    }
  };

  return (
    <div className="flex flex-col h-[calc(100vh-8.5rem)] rounded-xl border border-border bg-card shadow-sm overflow-hidden">
      {/* Top Builder Action Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-card px-4 py-2.5">
        <div className="flex items-center gap-3">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate(`/templates/${template.id}`)}
          >
            <ArrowLeft className="h-4 w-4" />
            Back
          </Button>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="h-8 w-64 font-semibold text-sm"
          />
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <label className="cursor-pointer">
            <span className="inline-flex h-8 items-center gap-1.5 rounded-md border border-input bg-card px-3 text-xs font-medium hover:bg-secondary">
              <Upload className="h-3.5 w-3.5" />
              Add / Replace PDF
            </span>
            <input
              type="file"
              accept=".pdf,image/*"
              onChange={handleReplaceDocument}
              className="hidden"
            />
          </label>

          <Button
            variant="outline"
            size="sm"
            onClick={() => setHtmlModalOpen(true)}
            className="h-8 text-xs"
          >
            <Code2 className="h-3.5 w-3.5" />
            HTML Editor
          </Button>

          <Button
            variant="outline"
            size="sm"
            onClick={handleAutoDetectFields}
            disabled={detecting}
            className="h-8 text-xs"
          >
            <Sparkles className="h-3.5 w-3.5 text-primary" />
            {detecting ? 'Detecting...' : 'Auto-Detect Fields'}
          </Button>

          {onOpenSendModal && (
            <Button
              variant="secondary"
              size="sm"
              onClick={onOpenSendModal}
              className="h-8 text-xs"
            >
              <Send className="h-3.5 w-3.5" />
              Send to Sign
            </Button>
          )}

          <Button size="sm" onClick={handleSave} disabled={saving} className="h-8">
            <Save className="h-3.5 w-3.5" />
            {saving ? 'Saving...' : 'Save Template'}
          </Button>
        </div>
      </div>

      {/* Main 3-Column Builder Workspace */}
      <div className="flex-1 grid grid-cols-1 lg:grid-cols-12 min-h-0">
        {/* Center/Left Document Canvas */}
        <div className="lg:col-span-8 xl:col-span-9 h-full min-h-0 border-r border-border">
          <DocumentCanvas
            templateId={template.id}
            documents={template.documents || []}
            fields={fields}
            submitters={submitters}
            selectedFieldUuid={selectedFieldUuid}
            activePage={activePage}
            onSelectPage={setActivePage}
            onSelectField={setSelectedFieldUuid}
            onUpdateField={handleUpdateField}
            onDropNewField={handleDropNewField}
          />
        </div>

        {/* Right Sidebar: Field Inspector + Roles & 20-Field Palette */}
        <div className="lg:col-span-4 xl:col-span-3 h-full overflow-y-auto p-4 space-y-5 bg-card">
          <FieldInspector
            field={selectedField}
            allFields={fields}
            submitters={submitters}
            onChange={handleUpdateField}
            onDelete={handleDeleteField}
          />

          <FieldPalette
            submitters={submitters}
            selectedSubmitterUuid={selectedSubmitterUuid}
            onSelectSubmitter={setSelectedSubmitterUuid}
            onUpdateSubmitters={setSubmitters}
            onAddField={handleAddFieldFromClick}
          />

          <div className="pt-2 border-t border-border flex items-center justify-between text-[11px] text-muted-foreground">
            <span className="inline-flex items-center gap-1">
              <History className="h-3 w-3" />
              Total Fields: {fields.length}
            </span>
            <span>Slug: {template.slug}</span>
          </div>
        </div>
      </div>

      <HtmlTemplateModal
        open={htmlModalOpen}
        onOpenChange={setHtmlModalOpen}
        initialName={name}
        initialHtml={template.html_body}
        onSave={handleSaveHtml}
      />
    </div>
  );
};
