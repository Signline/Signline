import React, { useState, useEffect, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import type { SigningSession } from '@/types';
import { api } from '@/lib/api';
import { SigningForm } from '@/components/signing/SigningForm';

export const PublicSignPage: React.FC = () => {
  const { slug } = useParams<{ slug: string }>();
  const [session, setSession] = useState<SigningSession | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState<boolean>(true);

  const loadSession = useCallback(async () => {
    if (!slug) return;
    setLoading(true);
    try {
      const res = await api.get<any>(`/api/s/${slug}`);
      const data: SigningSession = res.session || res.data || res;
      setSession(data);
      setError(null);
    } catch (err: any) {
      setError(err.message || 'Signing link not found or expired.');
    } finally {
      setLoading(false);
    }
  }, [slug]);

  useEffect(() => {
    loadSession();
  }, [loadSession]);

  if (loading) {
    return (
      <div className="py-16 text-center text-sm text-muted-foreground">
        Loading secure signing session...
      </div>
    );
  }

  if (error || !session || !slug) {
    return (
      <div className="container max-w-md mx-auto py-16 px-4 text-center space-y-2">
        <h2 className="text-lg font-bold text-destructive">Unable to Open Signing Session</h2>
        <p className="text-xs text-muted-foreground">{error}</p>
      </div>
    );
  }

  return <SigningForm slug={slug} session={session} onSessionUpdate={loadSession} />;
};
