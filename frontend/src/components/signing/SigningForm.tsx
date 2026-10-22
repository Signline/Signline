import React, { useState, useMemo, useRef } from 'react';
import {
  CheckCircle2,
  Download,
  FileCheck,
  Lock,
  MailCheck,
  PenTool,
  ShieldCheck,
  Upload,
  AlertTriangle,
} from 'lucide-react';
import type { SigningSession, TemplateField } from '@/types';
import { evaluateFormula, formatDate, isFieldVisible } from '@/lib/utils';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';
import { SignaturePadModal } from '@/components/signing/SignaturePadModal';
import { GuidedWizardBar } from '@/components/signing/GuidedWizardBar';
import { DeclineModal } from '@/components/signing/DeclineModal';
import { DelegateModal } from '@/components/signing/DelegateModal';

interface SigningFormProps {
  slug: string;
  session: SigningSession;
  onSessionUpdate: () => Promise<void>;
}

export const SigningForm: React.FC<SigningFormProps> = ({
  slug,
  session,
  onSessionUpdate,
}) => {
  const { toast } = useToast();
  const { submitter, submission, template } = session;

  const [values, setValues] = useState<Record<string, any>>(() => {
    const initial: Record<string, any> = {
      ...(session.all_values || {}),
      ...(submitter?.values || {}),
    };
    const fields = session.fields || template?.fields || [];
    for (const f of fields) {
      if (initial[f.uuid] === undefined && initial[f.name] !== undefined) {
        initial[f.uuid] = initial[f.name];
      }
      if (initial[f.uuid] === undefined && f.default_value) {
        initial[f.uuid] = f.default_value;
      }
      if (f.type === 'datestamp' && !initial[f.uuid]) {
        initial[f.uuid] = new Date().toISOString().slice(0, 10);
      }
      if (f.type === 'timestamp' && !initial[f.uuid]) {
        initial[f.uuid] = new Date().toISOString().replace('T', ' ').slice(0, 19) + ' UTC';
      }
    }
    return initial;
  });

  const [activeSigField, setActiveSigField] = useState<TemplateField | null>(null);
  const [currentFieldIndex, setCurrentFieldIndex] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const [declineOpen, setDeclineOpen] = useState(false);
  const [delegateOpen, setDelegateOpen] = useState(false);

  // Email 2FA state
  const [otpSent, setOtpSent] = useState(false);
  const [otpCode, setOtpCode] = useState('');
  const [verifyingOtp, setVerifyingOtp] = useState(false);

  const fieldRefs = useRef<Record<string, HTMLElement | null>>({});

  const allFields: TemplateField[] = useMemo(
    () => session.fields || template?.fields || [],
    [session.fields, template?.fields]
  );

  // Compute live values including formula fields
  const computedValues = useMemo(() => {
    const next = { ...values };
    for (const f of allFields) {
      if (f.preferences?.formula) {
        const res = evaluateFormula(f.preferences.formula, allFields, next);
        next[f.uuid] = res;
        if (f.name) next[f.name] = res;
      }
    }
    return next;
  }, [values, allFields]);

  // Fields assigned to current submitter and currently visible
  const myVisibleFields = useMemo(() => {
    return allFields.filter((f) => {
      const belongsToMe =
        !f.submitter_uuid ||
        !submitter?.submitter_uuid ||
        f.submitter_uuid === submitter.submitter_uuid ||
        allFields.every((item) => item.submitter_uuid !== submitter.submitter_uuid);
      if (!belongsToMe) return false;
      return isFieldVisible(f, allFields, computedValues);
    });
  }, [allFields, submitter?.submitter_uuid, computedValues]);

  const interactiveFields = useMemo(
    () =>
      myVisibleFields.filter(
        (f) => f.type !== 'heading' && f.type !== 'note' && !f.readonly && !f.preferences?.formula
      ),
    [myVisibleFields]
  );

  const requiredFields = useMemo(
    () => interactiveFields.filter((f) => f.required),
    [interactiveFields]
  );

  const isFieldFilled = (f: TemplateField): boolean => {
    const val = computedValues[f.uuid] ?? (f.name ? computedValues[f.name] : undefined);
    if (f.type === 'checkbox') {
      return val === true || val === 'true';
    }
    if (Array.isArray(val)) {
      return val.length > 0;
    }
    return val !== undefined && val !== null && String(val).trim() !== '';
  };

  const completedRequiredCount = useMemo(
    () => requiredFields.filter((f) => isFieldFilled(f)).length,
    [requiredFields, computedValues]
  );

  const setFieldValue = (field: TemplateField, val: any) => {
    setValues((prev) => {
      const next = { ...prev, [field.uuid]: val };
      if (field.name) {
        next[field.name] = val;
      }
      return next;
    });
  };

  const handleNextField = () => {
    if (interactiveFields.length === 0) return;
    // Find next unfilled required field or cycle
    const unfilledIdx = interactiveFields.findIndex(
      (f, idx) => idx >= currentFieldIndex && f.required && !isFieldFilled(f)
    );
    const nextIdx =
      unfilledIdx !== -1 ? unfilledIdx : (currentFieldIndex + 1) % interactiveFields.length;
    setCurrentFieldIndex(nextIdx);
    const target = interactiveFields[nextIdx];
    if (target && fieldRefs.current[target.uuid]) {
      fieldRefs.current[target.uuid]?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      fieldRefs.current[target.uuid]?.focus();
    }
  };

  const handleCompleteSigning = async () => {
    for (const reqField of requiredFields) {
      if (!isFieldFilled(reqField)) {
        toast({
          title: `Missing required field: ${reqField.title || reqField.name || reqField.type}`,
          description: 'Please fill out all required fields before completing.',
          variant: 'destructive',
        });
        fieldRefs.current[reqField.uuid]?.scrollIntoView({ behavior: 'smooth', block: 'center' });
        return;
      }
    }

    setSubmitting(true);
    try {
      await api.put(`/api/s/${slug}`, {
        values: computedValues,
        completed: true,
        timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
      });
      toast({
        title: 'Document Signed Successfully',
        description: 'Your cryptographic digital signature and audit trail have been recorded.',
        variant: 'success',
      });
      await onSessionUpdate();
    } catch (err: any) {
      toast({
        title: 'Failed to submit signature',
        description: err.message || 'Please check your inputs and try again.',
        variant: 'destructive',
      });
    } finally {
      setSubmitting(false);
    }
  };

  const handleDecline = async (reason: string) => {
    await api.post(`/api/s/${slug}/decline`, { reason });
    toast({
      title: 'Document Declined',
      description: 'The sender has been notified.',
    });
    await onSessionUpdate();
  };

  const handleDelegate = async (email: string, name: string) => {
    await api.post(`/api/s/${slug}/delegate`, { email, name });
    toast({
      title: 'Signing Delegated',
      description: `Signing has been delegated to ${name} (${email}).`,
      variant: 'success',
    });
    await onSessionUpdate();
  };

  const handleSendOtp = async () => {
    try {
      await api.post(`/api/s/${slug}/verify_email`, {});
      setOtpSent(true);
      toast({
        title: 'Verification Code Sent',
        description: `Sent a 6-digit verification code to ${submitter.email}.`,
        variant: 'success',
      });
    } catch (err: any) {
      toast({
        title: 'Failed to send code',
        description: err.message,
        variant: 'destructive',
      });
    }
  };

  const handleVerifyOtp = async (e: React.FormEvent) => {
    e.preventDefault();
    setVerifyingOtp(true);
    try {
      await api.put(`/api/s/${slug}/verify_email`, { code: otpCode.trim() });
      toast({
        title: 'Email Verified',
        description: 'You may now view and sign the document.',
        variant: 'success',
      });
      await onSessionUpdate();
    } catch (err: any) {
      toast({
        title: 'Invalid Verification Code',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setVerifyingOtp(false);
    }
  };

  // 1. Check if Email 2FA verification is required first
  if (session.requires_email_verification && !session.email_verified) {
    return (
      <div className="container max-w-md mx-auto py-16 px-4">
        <Card>
          <CardHeader className="text-center">
            <div className="mx-auto mb-2 h-12 w-12 rounded-full bg-primary/10 text-primary flex items-center justify-center">
              <MailCheck className="h-6 w-6" />
            </div>
            <CardTitle>Email Verification Required</CardTitle>
            <CardDescription>
              To protect this document, please verify your email address (
              <strong>{submitter?.email}</strong>) before viewing and signing.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {!otpSent ? (
              <Button className="w-full" onClick={handleSendOtp}>
                Send 6-Digit Verification Code
              </Button>
            ) : (
              <form onSubmit={handleVerifyOtp} className="space-y-3">
                <div className="space-y-1.5">
                  <Label htmlFor="email-otp">6-Digit Verification Code</Label>
                  <Input
                    id="email-otp"
                    value={otpCode}
                    onChange={(e) => setOtpCode(e.target.value)}
                    placeholder="123456"
                    maxLength={6}
                    required
                  />
                </div>
                <Button type="submit" className="w-full" disabled={verifyingOtp}>
                  {verifyingOtp ? 'Verifying...' : 'Verify & Open Document'}
                </Button>
              </form>
            )}
          </CardContent>
        </Card>
      </div>
    );
  }

  // 2. Check if already declined
  if (submitter?.status === 'declined' || submitter?.declined_at) {
    return (
      <div className="container max-w-xl mx-auto py-14 px-4">
        <Card className="border-destructive/40">
          <CardHeader className="text-center">
            <div className="mx-auto mb-2 h-12 w-12 rounded-full bg-red-100 text-red-700 flex items-center justify-center">
              <AlertTriangle className="h-6 w-6" />
            </div>
            <CardTitle>Signing Declined</CardTitle>
            <CardDescription>
              This document was declined on {formatDate(submitter.declined_at)}.
            </CardDescription>
          </CardHeader>
          {submitter.decline_reason && (
            <CardContent>
              <div className="rounded-lg bg-red-50 border border-red-200 p-3 text-sm text-red-900">
                <strong>Reason:</strong> {submitter.decline_reason}
              </div>
            </CardContent>
          )}
        </Card>
      </div>
    );
  }

  // 3. Check if completed
  if (submitter?.status === 'completed' || submitter?.completed_at) {
    const downloadDocUrl = `/api/submissions/${submission?.id || submitter.submission_id}/download`;
    const downloadAuditUrl = `/api/submissions/${submission?.id || submitter.submission_id}/download?audit=true`;

    return (
      <div className="container max-w-2xl mx-auto py-12 px-4 space-y-6">
        <Card className="border-emerald-500/40 shadow-md">
          <CardHeader className="text-center pb-4">
            <div className="mx-auto mb-3 h-14 w-14 rounded-full bg-emerald-100 text-emerald-700 flex items-center justify-center">
              <CheckCircle2 className="h-8 w-8" />
            </div>
            <Badge variant="success" className="mx-auto mb-2">
              Cryptographically Signed &amp; Sealed
            </Badge>
            <CardTitle className="text-2xl">
              {template?.name || submission?.name || 'Document Completed'}
            </CardTitle>
            <CardDescription>
              Thank you, <strong>{submitter.name || submitter.email}</strong>. Your signature was
              recorded on {formatDate(submitter.completed_at)}.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="flex flex-wrap items-center justify-center gap-3">
              <a href={downloadDocUrl} target="_blank" rel="noopener noreferrer">
                <Button className="gap-2">
                  <Download className="h-4 w-4" />
                  Download Signed PDF
                </Button>
              </a>
              <a href={downloadAuditUrl} target="_blank" rel="noopener noreferrer">
                <Button variant="outline" className="gap-2">
                  <FileCheck className="h-4 w-4" />
                  Download Audit Trail Certificate
                </Button>
              </a>
            </div>

            <div className="rounded-lg border border-border bg-secondary/40 p-4 text-xs space-y-2">
              <div className="font-semibold text-foreground flex items-center gap-1.5">
                <ShieldCheck className="h-4 w-4 text-primary" />
                Digital Signature &amp; Audit Summary
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-muted-foreground">
                <div>
                  Signer: <strong className="text-foreground">{submitter.name || '—'}</strong> (
                  {submitter.email})
                </div>
                <div>
                  Role: <strong className="text-foreground">{submitter.role || 'Signer'}</strong>
                </div>
                <div>
                  Completed At:{' '}
                  <strong className="text-foreground">{formatDate(submitter.completed_at)}</strong>
                </div>
                <div>
                  Signature ID:{' '}
                  <code className="text-foreground font-mono">{submitter.uuid || submitter.id}</code>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }

  // Extract pages from template documents
  const documents = template?.documents || submission?.documents || [];
  const pagesList: Array<{
    docUuid: string;
    docName: string;
    pageIndex: number; // 0-indexed within document
    globalPageIndex: number;
    previewUrl: string;
  }> = [];

  let globalCounter = 0;
  if (documents.length > 0) {
    for (const doc of documents) {
      const pageCount =
        doc.pages?.length ||
        doc.preview_images?.length ||
        doc.metadata?.pdf?.number_of_pages ||
        1;
      for (let p = 0; p < pageCount; p++) {
        const previewUrl =
          doc.pages?.[p]?.preview_url ||
          doc.preview_images?.[p] ||
          `/api/templates/${template?.id}/pages/${p + 1}`;
        pagesList.push({
          docUuid: doc.uuid,
          docName: doc.name,
          pageIndex: p,
          globalPageIndex: globalCounter++,
          previewUrl,
        });
      }
    }
  } else {
    pagesList.push({
      docUuid: 'default',
      docName: template?.name || 'Document',
      pageIndex: 0,
      globalPageIndex: 0,
      previewUrl: '',
    });
  }

  const renderFieldWidget = (field: TemplateField) => {
    const val = computedValues[field.uuid] ?? '';
    const isMyField = myVisibleFields.some((f) => f.uuid === field.uuid);

    if (!isMyField) {
      return (
        <div className="w-full h-full flex items-center px-2 text-xs text-slate-600 bg-slate-100/75 select-none truncate">
          {val ? String(val) : `${field.name || field.type} (Other Signer)`}
        </div>
      );
    }

    if (field.type === 'heading') {
      return (
        <div className="w-full h-full flex items-center px-2 font-bold text-sm text-slate-900">
          {field.title || field.default_value || field.name}
        </div>
      );
    }

    if (field.type === 'note') {
      return (
        <div className="w-full h-full flex items-center px-2 text-xs italic text-slate-700 bg-amber-50/90">
          {field.description || field.default_value || field.title || field.name}
        </div>
      );
    }

    if (field.type === 'signature' || field.type === 'initials') {
      return (
        <button
          type="button"
          ref={(el) => {
            fieldRefs.current[field.uuid] = el;
          }}
          onClick={() => setActiveSigField(field)}
          className={`w-full h-full flex flex-col items-center justify-center rounded border-2 transition-all ${
            val
              ? 'border-emerald-600 bg-white/95'
              : 'border-primary bg-primary/10 hover:bg-primary/20 text-primary'
          }`}
        >
          {val && String(val).startsWith('data:image') ? (
            <img
              src={String(val)}
              alt={field.name}
              className="max-h-full max-w-full object-contain p-0.5"
            />
          ) : val ? (
            <span className="font-signature text-lg text-blue-900 px-2 truncate">
              {String(val)}
            </span>
          ) : (
            <span className="inline-flex items-center gap-1 text-xs font-semibold px-1.5">
              <PenTool className="h-3.5 w-3.5 shrink-0" />
              {field.type === 'initials' ? 'Click to Initial' : 'Click to Sign'}
              {field.required && <span className="text-red-600">*</span>}
            </span>
          )}
        </button>
      );
    }

    if (field.type === 'checkbox') {
      const checked = val === true || val === 'true';
      return (
        <label className="w-full h-full flex items-center justify-center cursor-pointer bg-white/90 hover:bg-primary/10">
          <input
            type="checkbox"
            ref={(el) => {
              fieldRefs.current[field.uuid] = el;
            }}
            checked={checked}
            onChange={(e) => setFieldValue(field, e.target.checked)}
            className="h-4 w-4 accent-primary cursor-pointer"
          />
        </label>
      );
    }

    if (field.type === 'select') {
      const opts = field.options || [];
      return (
        <select
          ref={(el) => {
            fieldRefs.current[field.uuid] = el;
          }}
          value={String(val)}
          onChange={(e) => setFieldValue(field, e.target.value)}
          className="w-full h-full bg-white/95 px-1.5 text-xs border-0 focus:outline-none focus:ring-2 focus:ring-primary"
        >
          <option value="">{field.placeholder || `Select ${field.name}...`}</option>
          {opts.map((o) => (
            <option key={o.uuid} value={o.value}>
              {o.value}
            </option>
          ))}
        </select>
      );
    }

    if (field.type === 'radio') {
      const opts = field.options || [];
      return (
        <div className="w-full h-full flex items-center gap-2 px-2 bg-white/95 overflow-x-auto text-xs">
          {opts.length > 0 ? (
            opts.map((o) => (
              <label key={o.uuid} className="inline-flex items-center gap-1 cursor-pointer">
                <input
                  type="radio"
                  name={field.uuid}
                  checked={val === o.value}
                  onChange={() => setFieldValue(field, o.value)}
                />
                <span>{o.value}</span>
              </label>
            ))
          ) : (
            <label className="inline-flex items-center gap-1 cursor-pointer">
              <input
                type="radio"
                name={field.uuid}
                checked={Boolean(val)}
                onChange={() => setFieldValue(field, 'selected')}
              />
              <span>{field.title || field.name}</span>
            </label>
          )}
        </div>
      );
    }

    if (field.type === 'multiple') {
      const opts = field.options || [];
      const selectedArr: string[] = Array.isArray(val)
        ? val
        : val
        ? String(val)
            .split(',')
            .map((s) => s.trim())
        : [];
      const toggleOpt = (optVal: string) => {
        const next = selectedArr.includes(optVal)
          ? selectedArr.filter((i) => i !== optVal)
          : [...selectedArr, optVal];
        setFieldValue(field, next);
      };
      return (
        <div className="w-full h-full flex flex-wrap items-center gap-2 px-2 bg-white/95 overflow-y-auto text-xs">
          {opts.map((o) => (
            <label key={o.uuid} className="inline-flex items-center gap-1 cursor-pointer">
              <input
                type="checkbox"
                checked={selectedArr.includes(o.value)}
                onChange={() => toggleOpt(o.value)}
              />
              <span>{o.value}</span>
            </label>
          ))}
        </div>
      );
    }

    if (field.type === 'file' || field.type === 'image') {
      return (
        <label className="w-full h-full flex items-center justify-center gap-1 px-2 bg-white/95 hover:bg-primary/10 cursor-pointer text-xs text-primary font-medium truncate">
          <Upload className="h-3.5 w-3.5 shrink-0" />
          <span className="truncate">
            {val ? 'Uploaded ✓' : `Upload ${field.title || field.name || field.type}`}
          </span>
          <input
            type="file"
            accept={field.type === 'image' ? 'image/*' : '*/*'}
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (!file) return;
              const reader = new FileReader();
              reader.onload = () => {
                setFieldValue(field, reader.result);
              };
              reader.readAsDataURL(file);
            }}
          />
        </label>
      );
    }

    const isReadonly = Boolean(
      field.readonly ||
        field.preferences?.formula ||
        field.type === 'datestamp' ||
        field.type === 'timestamp'
    );

    return (
      <input
        ref={(el) => {
          fieldRefs.current[field.uuid] = el;
        }}
        type={
          field.type === 'date'
            ? 'date'
            : field.type === 'number' || field.type === 'payment'
            ? 'number'
            : field.type === 'phone'
            ? 'tel'
            : field.preferences?.mask
            ? 'password'
            : 'text'
        }
        readOnly={isReadonly}
        value={String(val)}
        onChange={(e) => setFieldValue(field, e.target.value)}
        placeholder={
          field.placeholder ||
          field.title ||
          field.name ||
          (field.type === 'cells' ? 'C E L L S' : field.type)
        }
        style={{
          letterSpacing: field.type === 'cells' ? '0.35em' : undefined,
        }}
        className={`w-full h-full px-2 text-xs bg-white/95 text-slate-900 focus:outline-none focus:ring-2 focus:ring-primary ${
          isReadonly ? 'bg-slate-100/90 text-slate-600 cursor-default' : ''
        }`}
      />
    );
  };

  return (
    <div className="flex flex-col min-h-[calc(100vh-7rem)]">
      {/* Top Document Banner */}
      <div className="border-b border-border bg-card px-4 py-3">
        <div className="container mx-auto flex flex-wrap items-center justify-between gap-2">
          <div>
            <h1 className="text-base font-bold text-foreground">
              {template?.name || submission?.name || 'Document Signing Request'}
            </h1>
            <p className="text-xs text-muted-foreground">
              Signing as <strong>{submitter?.name || submitter?.email}</strong> ({submitter?.role || 'Signer'})
            </p>
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Lock className="h-3.5 w-3.5 text-emerald-600" />
            <span>SHA-256 &amp; X.509 PKCS#7 Protected</span>
          </div>
        </div>
      </div>

      {/* Main Document Canvas + Sidebar Summary */}
      <div className="flex-1 container mx-auto py-6 px-4 grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Document Pages */}
        <div className="lg:col-span-8 space-y-6">
          {pagesList.map((page) => {
            const pageFields = allFields.filter((f) => {
              if (!isFieldVisible(f, allFields, computedValues)) return false;
              const area = f.areas?.[0];
              const fieldPage = area ? area.page : 0;
              return fieldPage === page.pageIndex || fieldPage === page.globalPageIndex;
            });

            return (
              <div
                key={`${page.docUuid}-${page.pageIndex}`}
                className="relative mx-auto w-full max-w-[780px] rounded-lg border border-border bg-white shadow-md overflow-hidden"
                style={{ aspectRatio: '8.5 / 11' }}
              >
                {page.previewUrl ? (
                  <img
                    src={page.previewUrl}
                    alt={`${page.docName} - Page ${page.pageIndex + 1}`}
                    className="w-full h-full object-contain select-none pointer-events-none"
                    onError={(e) => {
                      (e.currentTarget as HTMLImageElement).style.display = 'none';
                    }}
                  />
                ) : null}

                {/* Subtle Page Watermark fallback if no raster preview */}
                <div className="pointer-events-none absolute top-3 right-4 text-[10px] text-slate-400">
                  {page.docName} · Page {page.pageIndex + 1}
                </div>

                {/* Field Overlays */}
                {pageFields.map((field) => {
                  const area = field.areas?.[0] || {
                    x: 0.1,
                    y: 0.1,
                    w: 0.25,
                    h: 0.05,
                    page: 0,
                  };
                  const filled = isFieldFilled(field);
                  return (
                    <div
                      key={field.uuid}
                      style={{
                        position: 'absolute',
                        left: `${area.x * 100}%`,
                        top: `${area.y * 100}%`,
                        width: `${area.w * 100}%`,
                        height: `${area.h * 100}%`,
                      }}
                      className={`rounded shadow-sm transition-all ${
                        filled
                          ? 'ring-1 ring-emerald-600/70'
                          : field.required
                          ? 'ring-2 ring-primary'
                          : 'ring-1 ring-slate-400'
                      }`}
                    >
                      {renderFieldWidget(field)}
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>

        {/* Right Form Checklist Sidebar (always accessible even if fields have no coordinates) */}
        <div className="lg:col-span-4 space-y-4">
          <Card className="sticky top-20">
            <CardHeader className="pb-3">
              <CardTitle className="text-base">Fields to Complete</CardTitle>
              <CardDescription>
                Fill out each field below or click directly on the document pages.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3 max-h-[65vh] overflow-y-auto">
              {myVisibleFields.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No fields are required from you. Click <strong>Complete Signing</strong> below to finish.
                </p>
              ) : (
                myVisibleFields.map((field) => {
                  const filled = isFieldFilled(field);
                  return (
                    <div
                      key={field.uuid}
                      className={`rounded-lg border p-3 space-y-1.5 transition-colors ${
                        filled ? 'border-emerald-500/40 bg-emerald-50/30' : 'border-border bg-card'
                      }`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <Label className="text-xs font-semibold">
                          {field.title || field.name || field.type}
                          {field.required && <span className="text-destructive ml-0.5">*</span>}
                        </Label>
                        <Badge variant={filled ? 'success' : 'secondary'} className="text-[10px]">
                          {field.type}
                        </Badge>
                      </div>
                      {field.description && (
                        <p className="text-[11px] text-muted-foreground">{field.description}</p>
                      )}
                      <div className="h-9 rounded border border-input overflow-hidden">
                        {renderFieldWidget(field)}
                      </div>
                    </div>
                  );
                })
              )}
            </CardContent>
          </Card>
        </div>
      </div>

      {/* Guided Bottom Bar */}
      <GuidedWizardBar
        totalRequired={requiredFields.length}
        completedRequired={completedRequiredCount}
        currentFieldIndex={currentFieldIndex}
        totalFields={interactiveFields.length}
        submitting={submitting}
        allowDecline={session.allow_to_decline !== false}
        onNextField={handleNextField}
        onSubmitForm={handleCompleteSigning}
        onOpenDecline={() => setDeclineOpen(true)}
        onOpenDelegate={() => setDelegateOpen(true)}
      />

      {/* Signature / Initials Modal */}
      <SignaturePadModal
        open={Boolean(activeSigField)}
        onOpenChange={(open) => !open && setActiveSigField(null)}
        title={
          activeSigField?.type === 'initials' ? 'Adopt Your Initials' : 'Adopt Your Signature'
        }
        defaultTypedName={submitter?.name || ''}
        allowTyped={session.allow_typed_signature !== false}
        onSave={(dataUrl) => {
          if (activeSigField) {
            setFieldValue(activeSigField, dataUrl);
          }
        }}
      />

      {/* Decline & Delegate Modals */}
      <DeclineModal
        open={declineOpen}
        onOpenChange={setDeclineOpen}
        onConfirmDecline={handleDecline}
      />
      <DelegateModal
        open={delegateOpen}
        onOpenChange={setDelegateOpen}
        onConfirmDelegate={handleDelegate}
      />
    </div>
  );
};
