import React from 'react';
import { Plus, Trash2, Sliders, Calculator, GitBranch } from 'lucide-react';
import type { FieldCondition, FieldOption, SubmitterRole, TemplateField } from '@/types';
import { generateUuid } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { Separator } from '@/components/ui/separator';

interface FieldInspectorProps {
  field: TemplateField | null;
  allFields: TemplateField[];
  submitters: SubmitterRole[];
  onChange: (updated: TemplateField) => void;
  onDelete: (uuid: string) => void;
}

export const FieldInspector: React.FC<FieldInspectorProps> = ({
  field,
  allFields,
  submitters,
  onChange,
  onDelete,
}) => {
  if (!field) {
    return (
      <div className="rounded-lg border border-dashed border-border p-4 text-center text-xs text-muted-foreground">
        Click any field on the document canvas to configure its properties, validation, formulas, or conditional rules.
      </div>
    );
  }

  const hasOptions =
    field.type === 'select' || field.type === 'radio' || field.type === 'multiple';

  const handleAddOption = () => {
    const opts: FieldOption[] = field.options || [];
    onChange({
      ...field,
      options: [...opts, { uuid: generateUuid(), value: `Option ${opts.length + 1}` }],
    });
  };

  const handleUpdateOption = (uuid: string, value: string) => {
    onChange({
      ...field,
      options: (field.options || []).map((o) => (o.uuid === uuid ? { ...o, value } : o)),
    });
  };

  const handleRemoveOption = (uuid: string) => {
    onChange({
      ...field,
      options: (field.options || []).filter((o) => o.uuid !== uuid),
    });
  };

  const handleAddCondition = () => {
    const otherFields = allFields.filter((f) => f.uuid !== field.uuid);
    const cond: FieldCondition = {
      field_uuid: otherFields[0]?.uuid || '',
      operator: 'equal',
      value: '',
      action: 'show',
    };
    onChange({
      ...field,
      conditions: [...(field.conditions || []), cond],
    });
  };

  const handleUpdateCondition = (idx: number, patch: Partial<FieldCondition>) => {
    const list = [...(field.conditions || [])];
    list[idx] = { ...list[idx], ...patch };
    onChange({ ...field, conditions: list });
  };

  const handleRemoveCondition = (idx: number) => {
    const list = [...(field.conditions || [])];
    list.splice(idx, 1);
    onChange({ ...field, conditions: list });
  };

  return (
    <div className="space-y-4 rounded-xl border border-border bg-card p-3.5 shadow-sm">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-primary">
          <Sliders className="h-3.5 w-3.5" />
          <span>Field Inspector ({field.type})</span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onDelete(field.uuid)}
          className="h-7 px-2 text-destructive hover:bg-destructive/10"
        >
          <Trash2 className="h-3.5 w-3.5" />
          Delete
        </Button>
      </div>

      <div className="space-y-3 text-xs">
        <div className="space-y-1">
          <Label className="text-xs">Field Name / Variable Key</Label>
          <Input
            value={field.name}
            onChange={(e) => onChange({ ...field, name: e.target.value })}
            placeholder="e.g. full_name"
            className="h-8 text-xs"
          />
        </div>

        <div className="space-y-1">
          <Label className="text-xs">Assigned Role</Label>
          <Select
            value={field.submitter_uuid}
            onChange={(e) => onChange({ ...field, submitter_uuid: e.target.value })}
            className="h-8 text-xs"
          >
            {submitters.map((s) => (
              <option key={s.uuid} value={s.uuid}>
                {s.name}
              </option>
            ))}
          </Select>
        </div>

        <div className="flex items-center justify-between py-1">
          <Label className="text-xs">Required Field</Label>
          <Switch
            checked={field.required}
            onCheckedChange={(checked) => onChange({ ...field, required: checked })}
          />
        </div>

        <div className="flex items-center justify-between py-1">
          <Label className="text-xs">Read-Only</Label>
          <Switch
            checked={Boolean(field.readonly)}
            onCheckedChange={(checked) => onChange({ ...field, readonly: checked })}
          />
        </div>

        <div className="space-y-1">
          <Label className="text-xs">Display Label / Title</Label>
          <Input
            value={field.title || ''}
            onChange={(e) => onChange({ ...field, title: e.target.value })}
            placeholder="Optional label shown to signer"
            className="h-8 text-xs"
          />
        </div>

        <div className="space-y-1">
          <Label className="text-xs">Default Value</Label>
          <Input
            value={field.default_value || ''}
            onChange={(e) => onChange({ ...field, default_value: e.target.value })}
            placeholder="Pre-populated value"
            className="h-8 text-xs"
          />
        </div>

        <div className="space-y-1">
          <Label className="text-xs">Description / Instructions</Label>
          <Textarea
            value={field.description || ''}
            onChange={(e) => onChange({ ...field, description: e.target.value })}
            placeholder="Helper tooltip or note..."
            rows={2}
            className="text-xs min-h-[52px]"
          />
        </div>

        {/* Options list for select/radio/multiple */}
        {hasOptions && (
          <>
            <Separator />
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label className="text-xs font-semibold">Choices / Options</Label>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleAddOption}
                  className="h-6 px-2 text-[11px]"
                >
                  <Plus className="h-3 w-3" />
                  Add Option
                </Button>
              </div>
              <div className="space-y-1.5">
                {(field.options || []).map((opt) => (
                  <div key={opt.uuid} className="flex items-center gap-1.5">
                    <Input
                      value={opt.value}
                      onChange={(e) => handleUpdateOption(opt.uuid, e.target.value)}
                      className="h-7 text-xs"
                    />
                    <button
                      type="button"
                      onClick={() => handleRemoveOption(opt.uuid)}
                      className="text-muted-foreground hover:text-destructive p-1"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          </>
        )}

        {/* Formula & Validation */}
        {(field.type === 'number' || field.type === 'text' || field.type === 'payment') && (
          <>
            <Separator />
            <div className="space-y-1.5">
              <Label className="text-xs flex items-center gap-1">
                <Calculator className="h-3.5 w-3.5 text-primary" />
                Calculated Formula
              </Label>
              <Input
                value={field.preferences?.formula || ''}
                onChange={(e) =>
                  onChange({
                    ...field,
                    preferences: { ...(field.preferences || {}), formula: e.target.value },
                  })
                }
                placeholder="e.g. {{price}} * {{qty}}"
                className="h-8 text-xs font-mono"
              />
            </div>
          </>
        )}

        {/* Conditional Visibility Rules */}
        <Separator />
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label className="text-xs flex items-center gap-1 font-semibold">
              <GitBranch className="h-3.5 w-3.5 text-primary" />
              Conditional Rules
            </Label>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handleAddCondition}
              className="h-6 px-2 text-[11px]"
            >
              <Plus className="h-3 w-3" />
              Add Rule
            </Button>
          </div>

          {(field.conditions || []).map((cond, idx) => (
            <div
              key={idx}
              className="rounded-md border border-border bg-secondary/40 p-2 space-y-1.5"
            >
              <div className="flex items-center justify-between gap-1">
                <span className="text-[11px] font-medium">Show when field:</span>
                <button
                  type="button"
                  onClick={() => handleRemoveCondition(idx)}
                  className="text-muted-foreground hover:text-destructive"
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </div>
              <Select
                value={cond.field_uuid}
                onChange={(e) => handleUpdateCondition(idx, { field_uuid: e.target.value })}
                className="h-7 text-xs"
              >
                <option value="">Select field...</option>
                {allFields
                  .filter((f) => f.uuid !== field.uuid)
                  .map((f) => (
                    <option key={f.uuid} value={f.uuid}>
                      {f.name || f.type}
                    </option>
                  ))}
              </Select>
              <div className="grid grid-cols-2 gap-1.5">
                <Select
                  value={cond.operator}
                  onChange={(e) =>
                    handleUpdateCondition(idx, {
                      operator: e.target.value as FieldCondition['operator'],
                    })
                  }
                  className="h-7 text-xs"
                >
                  <option value="equal">Equals</option>
                  <option value="not_equal">Not Equal</option>
                  <option value="not_empty">Not Empty</option>
                  <option value="empty">Is Empty</option>
                  <option value="contains">Contains</option>
                </Select>
                {cond.operator !== 'not_empty' && cond.operator !== 'empty' && (
                  <Input
                    value={cond.value || ''}
                    onChange={(e) => handleUpdateCondition(idx, { value: e.target.value })}
                    placeholder="Value..."
                    className="h-7 text-xs"
                  />
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
