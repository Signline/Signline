import React from 'react';
import { ArrowRight, CheckCircle2, Ban, UserPlus } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface GuidedWizardBarProps {
  totalRequired: number;
  completedRequired: number;
  currentFieldIndex: number;
  totalFields: number;
  submitting: boolean;
  allowDecline?: boolean;
  onNextField: () => void;
  onSubmitForm: () => void;
  onOpenDecline: () => void;
  onOpenDelegate: () => void;
}

export const GuidedWizardBar: React.FC<GuidedWizardBarProps> = ({
  totalRequired,
  completedRequired,
  currentFieldIndex,
  totalFields,
  submitting,
  allowDecline = true,
  onNextField,
  onSubmitForm,
  onOpenDecline,
  onOpenDelegate,
}) => {
  const allRequiredDone = completedRequired >= totalRequired;
  const progressPercent =
    totalRequired > 0 ? Math.min(100, Math.round((completedRequired / totalRequired) * 100)) : 100;

  return (
    <div className="sticky bottom-0 z-30 border-t border-border bg-card/95 backdrop-blur px-4 py-3 shadow-lg">
      <div className="container mx-auto flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-4">
          <div className="text-sm">
            <span className="font-semibold text-foreground">
              {completedRequired} of {totalRequired}
            </span>{' '}
            <span className="text-muted-foreground">required fields completed</span>
          </div>
          <div className="hidden sm:block w-36 h-2 rounded-full bg-secondary overflow-hidden">
            <div
              className="h-full bg-primary transition-all duration-300"
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={onOpenDelegate}>
            <UserPlus className="h-3.5 w-3.5" />
            Delegate
          </Button>
          {allowDecline && (
            <Button variant="outline" size="sm" onClick={onOpenDecline} className="text-destructive">
              <Ban className="h-3.5 w-3.5" />
              Decline
            </Button>
          )}
          {!allRequiredDone && totalFields > 0 && (
            <Button variant="secondary" onClick={onNextField}>
              Next Field ({Math.min(currentFieldIndex + 1, totalFields)}/{totalFields})
              <ArrowRight className="h-4 w-4" />
            </Button>
          )}
          <Button
            onClick={onSubmitForm}
            disabled={submitting || !allRequiredDone}
            className="min-w-[160px]"
          >
            <CheckCircle2 className="h-4 w-4" />
            {submitting ? 'Completing & Signing...' : 'Complete Signing'}
          </Button>
        </div>
      </div>
    </div>
  );
};
