import React, { useState, useEffect, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import type { Template } from '@/types';
import { api } from '@/lib/api';
import { useToast } from '@/components/ui/toast';
import { TemplateBuilder } from '@/components/builder/TemplateBuilder';
import { NewSubmissionDialog } from '@/components/submissions/NewSubmissionDialog';

export const TemplateEditPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const { toast } = useToast();
  const [template, setTemplate] = useState<Template | null>(null);
  const [loading, setLoading] = useState(true);
  const [sendOpen, setSendOpen] = useState(false);

  const loadTemplate = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    try {
      const res = await api.get<any>(`/api/templates/${id}`);
      setTemplate(res.template || res.data || res);
    } catch (err: any) {
      toast({
        title: 'Failed to load template',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  }, [id, toast]);

  useEffect(() => {
    loadTemplate();
  }, [loadTemplate]);

  if (loading || !template) {
    return (
      <div className="py-12 text-center text-sm text-muted-foreground">
        Loading WYSIWYG Template Builder...
      </div>
    );
  }

  return (
    <>
      <TemplateBuilder
        template={template}
        onTemplateSaved={(updated) => setTemplate(updated)}
        onOpenSendModal={() => setSendOpen(true)}
      />
      <NewSubmissionDialog
        open={sendOpen}
        onOpenChange={setSendOpen}
        template={template}
        onCreated={() => {}}
      />
    </>
  );
};
