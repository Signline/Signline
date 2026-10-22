import * as React from 'react';
import { cn } from '@/lib/utils';

export const Alert: React.FC<
  React.HTMLAttributes<HTMLDivElement> & {
    variant?: 'default' | 'destructive' | 'success' | 'info';
  }
> = ({ className, variant = 'default', ...props }) => {
  const variantClasses = {
    default: 'bg-card text-foreground border-border',
    destructive: 'border-destructive/50 text-destructive bg-destructive/5',
    success: 'border-emerald-500/40 text-emerald-900 bg-emerald-50',
    info: 'border-primary/30 text-foreground bg-secondary/70',
  }[variant];

  return (
    <div
      role="alert"
      className={cn('relative w-full rounded-lg border p-4 text-sm', variantClasses, className)}
      {...props}
    />
  );
};

export const AlertTitle: React.FC<React.HTMLAttributes<HTMLHeadingElement>> = ({
  className,
  ...props
}) => <h5 className={cn('mb-1 font-semibold leading-none tracking-tight', className)} {...props} />;

export const AlertDescription: React.FC<React.HTMLAttributes<HTMLDivElement>> = ({
  className,
  ...props
}) => <div className={cn('text-sm [&_p]:leading-relaxed', className)} {...props} />;
