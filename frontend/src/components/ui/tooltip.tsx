import * as React from 'react';
import { cn } from '@/lib/utils';

export const Tooltip: React.FC<{
  content: string;
  children: React.ReactNode;
  className?: string;
}> = ({ content, children, className }) => {
  return (
    <div className={cn('group relative inline-flex', className)}>
      {children}
      <div className="pointer-events-none absolute bottom-full left-1/2 z-50 mb-1.5 -translate-x-1/2 whitespace-nowrap rounded bg-foreground px-2 py-1 text-xs text-card opacity-0 shadow transition-opacity group-hover:opacity-100">
        {content}
      </div>
    </div>
  );
};
