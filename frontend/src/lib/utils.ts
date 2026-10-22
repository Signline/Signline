import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import type { FieldCondition, FieldType, TemplateField } from '@/types';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDate(dateStr?: string | null): string {
  if (!dateStr) return '—';
  const d = new Date(dateStr);
  if (isNaN(d.getTime())) return dateStr;
  return d.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function formatShortDate(dateStr?: string | null): string {
  if (!dateStr) return '—';
  const d = new Date(dateStr);
  if (isNaN(d.getTime())) return dateStr;
  return d.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

export function formatBytes(bytes?: number): string {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

export function generateUuid(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

/**
 * Convert Hex color (#RRGGBB or #RGB) to Tailwind/ShadCN space-separated HSL string: "213 68% 40%"
 */
export function hexToHslString(hex: string): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return '213 68% 40%';
  const r = rgb[0] / 255;
  const g = rgb[1] / 255;
  const b = rgb[2] / 255;

  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  const l = (max + min) / 2;

  let h = 0;
  let s = 0;

  if (delta !== 0) {
    s = delta / (1 - Math.abs(2 * l - 1));
    if (max === r) {
      h = 60 * (((g - b) / delta) % 6);
    } else if (max === g) {
      h = 60 * ((b - r) / delta + 2);
    } else {
      h = 60 * ((r - g) / delta + 4);
    }
  }
  if (h < 0) h += 360;

  return `${Math.round(h)} ${Math.round(s * 100)}% ${Math.round(l * 100)}%`;
}

export function hexToRgb(hex: string): [number, number, number] | null {
  const clean = hex.replace(/^#/, '').trim();
  if (!/^[0-9a-fA-F]{3}$|^[0-9a-fA-F]{6}$/.test(clean)) return null;
  const full =
    clean.length === 3
      ? clean
          .split('')
          .map((c) => c + c)
          .join('')
      : clean;
  const num = parseInt(full, 16);
  return [(num >> 16) & 255, (num >> 8) & 255, num & 255];
}

/**
 * Compute contrasting foreground HSL ("215 28% 17%" dark vs "0 0% 100%" white)
 * matching Branding.content_color luminance threshold.
 */
export function contrastingForegroundHsl(hex: string): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return '0 0% 100%';
  const luminance = (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 255;
  return luminance >= 0.62 ? '215 28% 17%' : '0 0% 100%';
}

/**
 * Evaluate conditional visibility rules for a field given current field values.
 */
export function isFieldVisible(
  field: TemplateField,
  allFields: TemplateField[],
  values: Record<string, unknown>
): boolean {
  if (!field.conditions || field.conditions.length === 0) {
    return true;
  }

  for (const cond of field.conditions) {
    if (!cond.field_uuid) continue;
    const targetField = allFields.find((f) => f.uuid === cond.field_uuid);
    const rawVal =
      values[cond.field_uuid] ??
      (targetField ? values[targetField.name] : undefined);
    const strVal =
      rawVal === undefined || rawVal === null ? '' : String(rawVal).trim();
    const expected = (cond.value ?? '').trim();

    let matches = false;
    switch (cond.operator) {
      case 'equal':
        matches = strVal.toLowerCase() === expected.toLowerCase();
        break;
      case 'not_equal':
        matches = strVal.toLowerCase() !== expected.toLowerCase();
        break;
      case 'not_empty':
        matches = strVal.length > 0 && strVal !== 'false';
        break;
      case 'empty':
        matches = strVal.length === 0 || strVal === 'false';
        break;
      case 'contains':
        matches = strVal.toLowerCase().includes(expected.toLowerCase());
        break;
      default:
        matches = true;
    }

    if (cond.action === 'hide') {
      if (matches) return false;
    } else {
      if (!matches) return false;
    }
  }

  return true;
}

/**
 * Evaluate mathematical formulas for calculated fields.
 * Supports field name or field uuid interpolation: e.g. `price * quantity` or `{{price}} + {{tax}}`
 */
export function evaluateFormula(
  formula: string,
  allFields: TemplateField[],
  values: Record<string, unknown>
): string | number {
  if (!formula || !formula.trim()) return '';
  try {
    let expr = formula.trim();

    // Replace {{field_name_or_uuid}} tokens first
    expr = expr.replace(/\{\{\s*([^}]+?)\s*\}\}/g, (_, token) => {
      const f = allFields.find(
        (item) =>
          item.uuid === token.trim() ||
          item.name.toLowerCase() === token.trim().toLowerCase()
      );
      const val = f ? (values[f.uuid] ?? values[f.name]) : values[token.trim()];
      const num = parseFloat(String(val ?? '0'));
      return isNaN(num) ? '0' : String(num);
    });

    // Also replace bare field names sorted longest-first
    const sortedFields = [...allFields].sort(
      (a, b) => (b.name?.length || 0) - (a.name?.length || 0)
    );
    for (const f of sortedFields) {
      if (!f.name) continue;
      const escaped = f.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const regex = new RegExp(`\\b${escaped}\\b`, 'gi');
      const rawVal = values[f.uuid] ?? values[f.name];
      const num = parseFloat(String(rawVal ?? '0'));
      expr = expr.replace(regex, isNaN(num) ? '0' : String(num));
    }

    // Only allow safe arithmetic characters
    if (!/^[0-9+\-*/().\s]+$/.test(expr)) {
      return '';
    }

    // Safe evaluation of arithmetic expression
    const fn = new Function(`return (${expr});`);
    const result = fn();
    if (typeof result === 'number' && isFinite(result)) {
      return Math.round(result * 100) / 100;
    }
    return '';
  } catch {
    return '';
  }
}

export const SUBMITTER_COLORS = [
  {
    name: 'First Party',
    bg: 'bg-blue-500/15',
    border: 'border-blue-600',
    text: 'text-blue-800',
    badge: 'bg-blue-600 text-white',
    dot: '#2161AD',
  },
  {
    name: 'Second Party',
    bg: 'bg-emerald-500/15',
    border: 'border-emerald-600',
    text: 'text-emerald-800',
    badge: 'bg-emerald-600 text-white',
    dot: '#059669',
  },
  {
    name: 'Third Party',
    bg: 'bg-amber-500/15',
    border: 'border-amber-600',
    text: 'text-amber-800',
    badge: 'bg-amber-600 text-white',
    dot: '#D97706',
  },
  {
    name: 'Fourth Party',
    bg: 'bg-purple-500/15',
    border: 'border-purple-600',
    text: 'text-purple-800',
    badge: 'bg-purple-600 text-white',
    dot: '#7C3AED',
  },
  {
    name: 'Fifth Party',
    bg: 'bg-rose-500/15',
    border: 'border-rose-600',
    text: 'text-rose-800',
    badge: 'bg-rose-600 text-white',
    dot: '#E11D48',
  },
  {
    name: 'Sixth Party',
    bg: 'bg-cyan-500/15',
    border: 'border-cyan-600',
    text: 'text-cyan-800',
    badge: 'bg-cyan-600 text-white',
    dot: '#0891B2',
  },
];

export function getSubmitterColor(index: number) {
  return SUBMITTER_COLORS[index % SUBMITTER_COLORS.length];
}

export const ALL_FIELD_TYPES: Array<{
  type: FieldType;
  label: string;
  category: 'Signature' | 'Input' | 'Selection' | 'Advanced' | 'Layout';
  defaultW: number;
  defaultH: number;
}> = [
  { type: 'signature', label: 'Signature', category: 'Signature', defaultW: 0.24, defaultH: 0.055 },
  { type: 'initials', label: 'Initials', category: 'Signature', defaultW: 0.12, defaultH: 0.05 },
  { type: 'datestamp', label: 'Date Stamp', category: 'Signature', defaultW: 0.16, defaultH: 0.04 },
  { type: 'timestamp', label: 'Time Stamp', category: 'Signature', defaultW: 0.18, defaultH: 0.04 },
  { type: 'text', label: 'Text Input', category: 'Input', defaultW: 0.22, defaultH: 0.036 },
  { type: 'date', label: 'Date Picker', category: 'Input', defaultW: 0.16, defaultH: 0.036 },
  { type: 'number', label: 'Number', category: 'Input', defaultW: 0.15, defaultH: 0.036 },
  { type: 'phone', label: 'Phone', category: 'Input', defaultW: 0.18, defaultH: 0.036 },
  { type: 'cells', label: 'Cells (Grid)', category: 'Input', defaultW: 0.25, defaultH: 0.038 },
  { type: 'checkbox', label: 'Checkbox', category: 'Selection', defaultW: 0.035, defaultH: 0.03 },
  { type: 'radio', label: 'Radio Group', category: 'Selection', defaultW: 0.18, defaultH: 0.04 },
  { type: 'select', label: 'Dropdown Select', category: 'Selection', defaultW: 0.2, defaultH: 0.038 },
  { type: 'multiple', label: 'Multi-Select', category: 'Selection', defaultW: 0.2, defaultH: 0.045 },
  { type: 'file', label: 'File Upload', category: 'Advanced', defaultW: 0.2, defaultH: 0.045 },
  { type: 'image', label: 'Image / Photo', category: 'Advanced', defaultW: 0.2, defaultH: 0.08 },
  { type: 'payment', label: 'Payment', category: 'Advanced', defaultW: 0.22, defaultH: 0.05 },
  { type: 'verification', label: 'ID Verification', category: 'Advanced', defaultW: 0.22, defaultH: 0.05 },
  { type: 'kba', label: 'KBA Quiz', category: 'Advanced', defaultW: 0.22, defaultH: 0.05 },
  { type: 'heading', label: 'Heading', category: 'Layout', defaultW: 0.35, defaultH: 0.04 },
  { type: 'note', label: 'Instruction Note', category: 'Layout', defaultW: 0.3, defaultH: 0.05 },
];

export const REMINDER_DURATION_OPTIONS = [
  { value: '', label: 'Disabled (None)' },
  { value: 'one_hour', label: '1 hour' },
  { value: 'two_hours', label: '2 hours' },
  { value: 'four_hours', label: '4 hours' },
  { value: 'eight_hours', label: '8 hours' },
  { value: 'twelve_hours', label: '12 hours' },
  { value: 'twenty_four_hours', label: '24 hours' },
  { value: 'two_days', label: '2 days' },
  { value: 'three_days', label: '3 days' },
  { value: 'four_days', label: '4 days' },
  { value: 'five_days', label: '5 days' },
  { value: 'six_days', label: '6 days' },
  { value: 'seven_days', label: '7 days (1 week)' },
  { value: 'ten_days', label: '10 days' },
  { value: 'fourteen_days', label: '14 days (2 weeks)' },
  { value: 'twenty_one_days', label: '21 days (3 weeks)' },
  { value: 'thirty_days', label: '30 days (1 month)' },
];

export const WEBHOOK_EVENTS_LIST = [
  'form.viewed',
  'form.started',
  'form.completed',
  'form.declined',
  'form.delegated',
  'submission.created',
  'submission.completed',
  'submission.expired',
  'submission.archived',
  'template.created',
  'template.updated',
];
