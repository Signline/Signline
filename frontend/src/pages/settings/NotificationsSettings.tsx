import React, { useState, useEffect } from 'react';
import { BellRing, Save, Play } from 'lucide-react';
import { REMINDER_DURATION_OPTIONS } from '@/lib/utils';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const NotificationsSettings: React.FC = () => {
  const { toast } = useToast();

  const [firstDuration, setFirstDuration] = useState('twenty_four_hours');
  const [secondDuration, setSecondDuration] = useState('three_days');
  const [thirdDuration, setThirdDuration] = useState('seven_days');
  const [saving, setSaving] = useState(false);
  const [runningNow, setRunningNow] = useState(false);

  useEffect(() => {
    api
      .get<any>('/api/settings/notifications')
      .then((res) => {
        const cfg = res.reminders || res.notifications || res.data || res;
        if (cfg.first_duration !== undefined) setFirstDuration(cfg.first_duration || '');
        if (cfg.second_duration !== undefined) setSecondDuration(cfg.second_duration || '');
        if (cfg.third_duration !== undefined) setThirdDuration(cfg.third_duration || '');
      })
      .catch(() => {});
  }, []);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await api.put('/api/settings/notifications', {
        first_duration: firstDuration,
        second_duration: secondDuration,
        third_duration: thirdDuration,
      });
      toast({
        title: 'Automated Reminder Schedule Saved',
        description: 'Pending signers will automatically receive up to 3 sequential reminder emails.',
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Save failed', description: err.message, variant: 'destructive' });
    } finally {
      setSaving(false);
    }
  };

  const handleRunReminderCheckNow = async () => {
    setRunningNow(true);
    try {
      const res = await api.post<any>('/api/settings/notifications', {
        run_now: true,
        first_duration: firstDuration,
        second_duration: secondDuration,
        third_duration: thirdDuration,
      });
      toast({
        title: 'Reminder Worker Executed',
        description: `Checked pending submitters (dispatched ${res.sent_count ?? 0} reminder emails).`,
        variant: 'success',
      });
    } catch (err: any) {
      toast({ title: 'Reminder run failed', description: err.message, variant: 'destructive' });
    } finally {
      setRunningNow(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <BellRing className="h-5 w-5 text-primary" />
          Automated Signature Request Reminders
        </CardTitle>
        <CardDescription>
          Configure up to three automatic follow-up reminders sent to recipients who have not yet completed their documents. Each stage measures elapsed time since the previous email.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSave} className="space-y-5">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="rounded-lg border border-border bg-secondary/30 p-4 space-y-2">
              <Label htmlFor="rem-first" className="font-semibold">
                1st Reminder Delay
              </Label>
              <p className="text-[11px] text-muted-foreground">
                Time after initial signature invitation (`sent_at`)
              </p>
              <Select
                id="rem-first"
                value={firstDuration}
                onChange={(e) => setFirstDuration(e.target.value)}
              >
                {REMINDER_DURATION_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
            </div>

            <div className="rounded-lg border border-border bg-secondary/30 p-4 space-y-2">
              <Label htmlFor="rem-second" className="font-semibold">
                2nd Reminder Delay
              </Label>
              <p className="text-[11px] text-muted-foreground">
                Time after 1st reminder email was sent
              </p>
              <Select
                id="rem-second"
                value={secondDuration}
                onChange={(e) => setSecondDuration(e.target.value)}
              >
                {REMINDER_DURATION_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
            </div>

            <div className="rounded-lg border border-border bg-secondary/30 p-4 space-y-2">
              <Label htmlFor="rem-third" className="font-semibold">
                3rd Reminder Delay
              </Label>
              <p className="text-[11px] text-muted-foreground">
                Time after 2nd reminder email was sent
              </p>
              <Select
                id="rem-third"
                value={thirdDuration}
                onChange={(e) => setThirdDuration(e.target.value)}
              >
                {REMINDER_DURATION_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
            </div>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-3 pt-2 border-t border-border">
            <Button type="submit" disabled={saving}>
              <Save className="h-4 w-4" />
              {saving ? 'Saving...' : 'Save Reminder Schedule'}
            </Button>

            <Button
              type="button"
              variant="secondary"
              onClick={handleRunReminderCheckNow}
              disabled={runningNow}
            >
              <Play className="h-4 w-4" />
              {runningNow ? 'Checking Pending Submitters...' : 'Run Reminder Check Now'}
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
};
