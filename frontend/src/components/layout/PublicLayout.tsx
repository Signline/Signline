import React from 'react';
import { Link, Outlet } from 'react-router-dom';
import { ShieldCheck } from 'lucide-react';
import { BrandLogo, UpstreamAttribution } from '@/lib/branding';

export const PublicLayout: React.FC<{ children?: React.ReactNode }> = ({ children }) => {
  return (
    <div className="min-h-screen flex flex-col bg-background text-foreground">
      <header className="sticky top-0 z-30 border-b border-border bg-card/95 backdrop-blur">
        <div className="container mx-auto flex h-14 items-center justify-between px-4">
          <Link to="/" className="flex items-center gap-2">
            <BrandLogo className="h-7 w-7" />
          </Link>
          <div className="flex items-center gap-3 text-xs text-muted-foreground">
            <Link
              to="/verify"
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-secondary/50 px-2.5 py-1 font-medium text-foreground hover:bg-secondary"
            >
              <ShieldCheck className="h-3.5 w-3.5 text-primary" />
              Verify Signed PDF
            </Link>
          </div>
        </div>
      </header>

      <main className="flex-1">{children || <Outlet />}</main>

      <footer className="border-t border-border bg-card py-3.5 px-4">
        <div className="container mx-auto flex flex-col sm:flex-row items-center justify-between gap-2">
          <UpstreamAttribution />
        </div>
      </footer>
    </div>
  );
};
