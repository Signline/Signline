import * as React from 'react';
import { cn } from '@/lib/utils';

interface DropdownContextValue {
  open: boolean;
  setOpen: (val: boolean) => void;
}

const DropdownContext = React.createContext<DropdownContextValue>({
  open: false,
  setOpen: () => {},
});

export const DropdownMenu: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [open, setOpen] = React.useState(false);
  const ref = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <DropdownContext.Provider value={{ open, setOpen }}>
      <div ref={ref} className="relative inline-block text-left">
        {children}
      </div>
    </DropdownContext.Provider>
  );
};

export const DropdownMenuTrigger: React.FC<{
  children: React.ReactNode;
  className?: string;
}> = ({ children, className }) => {
  const { open, setOpen } = React.useContext(DropdownContext);
  return (
    <div onClick={() => setOpen(!open)} className={cn('cursor-pointer', className)}>
      {children}
    </div>
  );
};

export const DropdownMenuContent: React.FC<{
  children: React.ReactNode;
  align?: 'start' | 'end';
  className?: string;
}> = ({ children, align = 'end', className }) => {
  const { open } = React.useContext(DropdownContext);
  if (!open) return null;

  return (
    <div
      className={cn(
        'absolute z-50 mt-1.5 min-w-[11rem] overflow-hidden rounded-lg border border-border bg-card p-1 text-card-foreground shadow-lg',
        align === 'end' ? 'right-0' : 'left-0',
        className
      )}
    >
      {children}
    </div>
  );
};

export const DropdownMenuItem: React.FC<
  React.ButtonHTMLAttributes<HTMLButtonElement> & { destructive?: boolean }
> = ({ className, destructive, onClick, children, ...props }) => {
  const { setOpen } = React.useContext(DropdownContext);
  return (
    <button
      type="button"
      onClick={(e) => {
        setOpen(false);
        onClick?.(e);
      }}
      className={cn(
        'relative flex w-full cursor-pointer select-none items-center gap-2 rounded-md px-2.5 py-1.5 text-sm outline-none transition-colors hover:bg-secondary',
        destructive ? 'text-destructive hover:bg-destructive/10' : 'text-foreground',
        className
      )}
      {...props}
    >
      {children}
    </button>
  );
};

export const DropdownMenuSeparator: React.FC = () => (
  <div className="-mx-1 my-1 h-px bg-border" />
);
