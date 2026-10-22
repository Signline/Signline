import React, { useState, useEffect } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { FileSignature, ArrowRight } from 'lucide-react';
import type { Template } from '@/types';
import { api } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { useToast } from '@/components/ui/toast';

export const PublicStartFormPage: React.FC = () => {
  const { slug } = useParams<{ slug: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();

  const [template, setTemplate] = useState<Template | null>(null);
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [phone, setPhone] = useState('');
  const [loading, setLoading] = useState(true);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (!slug) return;
    api
      .get<any>(`/api/d/${slug}`)
      .then((res) => {
        setTemplate(res.template || res.data || res);
      })
      .catch((err) => {
        toast({
          title: 'Shared link unavailable',
          description: err.message,
          variant: 'destructive',
        });
      })
      .finally(() => setLoading(false));
  }, [slug, toast]);

  const handleStart = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!slug) return;
    setStarting(true);
    try {
      const res = await api.post<any>(`/api/d/${slug}`, {
        name: name.trim(),
        email: email.trim(),
        phone: phone.trim() || undefined,
      });
      const submitterSlug =
        res.submitter_slug || res.slug || res.submitter?.slug || res.data?.slug;
      if (submitterSlug) {
        navigate(`/s/${submitterSlug}`);
      } else {
        toast({
          title: 'Session Created',
          description: 'Please check your email for the signing link.',
          variant: 'success',
        });
      }
    } catch (err: any) {
      toast({
        title: 'Failed to start signing',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setStarting(false);
    }
  };

  if (loading) {
    return (
      <div className="py-16 text-center text-sm text-muted-foreground">
        Loading shared document template...
      </div>
    );
  }

  return (
    <div className="container max-w-md mx-auto py-14 px-4">
      <Card className="shadow-lg">
        <CardHeader className="text-center">
          <div className="mx-auto mb-2 h-12 w-12 rounded-full bg-primary/10 text-primary flex items-center justify-center">
            <FileSignature className="h-6 w-6" />
          </div>
          <CardTitle>{template?.name || 'Start Document Signing'}</CardTitle>
          <CardDescription>
            Enter your name and email address below to open and sign your copy of this document.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleStart} className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="shared-name">Your Full Name</Label>
              <Input
                id="shared-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Jane Doe"
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="shared-email">Your Email Address</Label>
              <Input
                id="shared-email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="jane@example.com"
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="shared-phone">Phone Number (Optional)</Label>
              <Input
                id="shared-phone"
                type="tel"
                value={phone}
                onChange={(e) => setPhone(e.target.value)}
                placeholder="+1 555 0100"
              />
            </div>
            <Button type="submit" className="w-full" disabled={starting}>
              {starting ? 'Preparing Document...' : 'Continue to Document'}
              <ArrowRight className="h-4 w-4" />
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
};
