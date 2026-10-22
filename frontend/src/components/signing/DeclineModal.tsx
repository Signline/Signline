import React, { useState } from 'react';
import { Ban } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';

interface DeclineModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirmDecline: (reason: string) => Promise<void>;
}

export const DeclineModal: React.FC<DeclineModalProps> = ({
  open,
  onOpenChange,
  onConfirmDecline,
}) => {
  const [reason, setReason] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!reason.trim()) return;
    setLoading(true);
    try {
      await onConfirmDecline(reason.trim());
      onOpenChange(false);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent onClose={() => onOpenChange(false)}>
        <form onSubmit={handleSubmit} className="space-y-4">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-destructive">
              <Ban className="h-5 w-5" />
              Decline to Sign Document
            </DialogTitle>
            <DialogDescription>
              Declining this document will cancel the signing process and notify the sender with your reason.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-1.5">
            <Label htmlFor="decline-reason">Reason for Declining (Required)</Label>
            <Textarea
              id="decline-reason"
              required
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="Please state why you are declining to sign this document..."
              rows={3}
            />
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" variant="destructive" disabled={loading || !reason.trim()}>
              {loading ? 'Declining...' : 'Confirm Decline'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
