import React, { useState } from 'react';
import { Plus, Trash2, Users } from 'lucide-react';
import type { FieldType, SubmitterRole } from '@/types';
import { ALL_FIELD_TYPES, generateUuid, getSubmitterColor } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

interface FieldPaletteProps {
  submitters: SubmitterRole[];
  selectedSubmitterUuid: string;
  onSelectSubmitter: (uuid: string) => void;
  onUpdateSubmitters: (submitters: SubmitterRole[]) => void;
  onAddField: (type: FieldType) => void;
}

const DEFAULT_PARTY_NAMES = [
  'First Party',
  'Second Party',
  'Third Party',
  'Fourth Party',
  'Fifth Party',
  'Sixth Party',
];

export const FieldPalette: React.FC<FieldPaletteProps> = ({
  submitters,
  selectedSubmitterUuid,
  onSelectSubmitter,
  onUpdateSubmitters,
  onAddField,
}) => {
  const [editingUuid, setEditingUuid] = useState<string | null>(null);

  const handleAddParty = () => {
    const nextIdx = submitters.length;
    const newRole: SubmitterRole = {
      uuid: generateUuid(),
      name: DEFAULT_PARTY_NAMES[nextIdx] || `Party ${nextIdx + 1}`,
      color_index: nextIdx,
    };
    const updated = [...submitters, newRole];
    onUpdateSubmitters(updated);
    onSelectSubmitter(newRole.uuid);
  };

  const handleRemoveParty = (uuid: string) => {
    if (submitters.length <= 1) return;
    const updated = submitters.filter((s) => s.uuid !== uuid);
    onUpdateSubmitters(updated);
    if (selectedSubmitterUuid === uuid && updated[0]) {
      onSelectSubmitter(updated[0].uuid);
    }
  };

  const handleRenameParty = (uuid: string, name: string) => {
    onUpdateSubmitters(
      submitters.map((s) => (s.uuid === uuid ? { ...s, name } : s))
    );
  };

  const categories = ['Signature', 'Input', 'Selection', 'Advanced', 'Layout'] as const;

  return (
    <div className="space-y-5">
      {/* Submitter Roles Section */}
      <div className="space-y-2.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            <Users className="h-3.5 w-3.5" />
            <span>Signer Roles</span>
          </div>
          <Button variant="ghost" size="sm" onClick={handleAddParty} className="h-7 px-2 text-xs">
            <Plus className="h-3.5 w-3.5" />
            Add Role
          </Button>
        </div>

        <div className="space-y-1.5">
          {submitters.map((sub, idx) => {
            const color = getSubmitterColor(idx);
            const isSelected = sub.uuid === selectedSubmitterUuid;
            return (
              <div
                key={sub.uuid}
                onClick={() => onSelectSubmitter(sub.uuid)}
                className={`flex items-center justify-between gap-2 rounded-lg border px-2.5 py-1.5 cursor-pointer transition-all ${
                  isSelected
                    ? 'border-primary bg-secondary shadow-sm'
                    : 'border-border bg-card hover:bg-secondary/40'
                }`}
              >
                <div className="flex items-center gap-2 flex-1 min-w-0">
                  <span
                    className="h-3 w-3 rounded-full shrink-0"
                    style={{ backgroundColor: color.dot }}
                  />
                  {editingUuid === sub.uuid ? (
                    <Input
                      value={sub.name}
                      autoFocus
                      onBlur={() => setEditingUuid(null)}
                      onKeyDown={(e) => e.key === 'Enter' && setEditingUuid(null)}
                      onChange={(e) => handleRenameParty(sub.uuid, e.target.value)}
                      className="h-6 text-xs px-1.5"
                    />
                  ) : (
                    <span
                      onDoubleClick={() => setEditingUuid(sub.uuid)}
                      className="text-xs font-medium truncate"
                      title="Double-click to rename role"
                    >
                      {sub.name}
                    </span>
                  )}
                </div>

                {submitters.length > 1 && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRemoveParty(sub.uuid);
                    }}
                    className="text-muted-foreground hover:text-destructive p-0.5"
                    title="Remove role"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* 20 Field Types Palette */}
      <div className="space-y-4">
        {categories.map((cat) => {
          const items = ALL_FIELD_TYPES.filter((f) => f.category === cat);
          return (
            <div key={cat} className="space-y-1.5">
              <div className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
                {cat} Fields
              </div>
              <div className="grid grid-cols-2 gap-1.5">
                {items.map((item) => (
                  <button
                    key={item.type}
                    type="button"
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData('application/signline-field-type', item.type);
                    }}
                    onClick={() => onAddField(item.type)}
                    className="flex items-center justify-start gap-2 rounded-md border border-border bg-card px-2.5 py-2 text-xs font-medium text-foreground shadow-sm hover:border-primary hover:bg-secondary/60 transition-all text-left"
                  >
                    <span className="h-2 w-2 rounded-full bg-primary shrink-0" />
                    <span className="truncate">{item.label}</span>
                  </button>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
