import React from 'react';
import { Link, NavLink, Outlet, useNavigate } from 'react-router-dom';
import {
  FileText,
  Send,
  ShieldCheck,
  Settings,
  LogOut,
  User as UserIcon,
  ChevronDown,
  FlaskConical,
} from 'lucide-react';
import { BrandLogo, UpstreamAttribution } from '@/lib/branding';
import { useAuth } from '@/lib/auth';
import { Badge } from '@/components/ui/badge';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu';

export const AppLayout: React.FC = () => {
  const { user, account, logout } = useAuth();
  const navigate = useNavigate();

  const handleLogout = async () => {
    await logout();
    navigate('/sign_in');
  };

  return (
    <div className="min-h-screen flex flex-col bg-background text-foreground">
      {/* Top Navbar */}
      <header className="sticky top-0 z-40 border-b border-border bg-card/95 backdrop-blur">
        <div className="container mx-auto flex h-16 items-center justify-between px-4 sm:px-6">
          <div className="flex items-center gap-6">
            <Link to="/" className="flex items-center gap-2">
              <BrandLogo className="h-8 w-8" />
            </Link>

            {account?.testing && (
              <Badge variant="warning" className="gap-1">
                <FlaskConical className="h-3.5 w-3.5" />
                Sandbox Mode
              </Badge>
            )}

            <nav className="hidden md:flex items-center gap-1">
              <NavLink
                to="/"
                end
                className={({ isActive }) =>
                  `inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                    isActive
                      ? 'bg-secondary text-primary font-semibold'
                      : 'text-muted-foreground hover:bg-secondary/60 hover:text-foreground'
                  }`
                }
              >
                <FileText className="h-4 w-4" />
                Templates
              </NavLink>
              <NavLink
                to="/submissions"
                className={({ isActive }) =>
                  `inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                    isActive
                      ? 'bg-secondary text-primary font-semibold'
                      : 'text-muted-foreground hover:bg-secondary/60 hover:text-foreground'
                  }`
                }
              >
                <Send className="h-4 w-4" />
                Submissions
              </NavLink>
              <NavLink
                to="/verify"
                className={({ isActive }) =>
                  `inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                    isActive
                      ? 'bg-secondary text-primary font-semibold'
                      : 'text-muted-foreground hover:bg-secondary/60 hover:text-foreground'
                  }`
                }
              >
                <ShieldCheck className="h-4 w-4" />
                Verify PDF
              </NavLink>
              <NavLink
                to="/settings/profile"
                className={({ isActive }) =>
                  `inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors ${
                    isActive || window.location.pathname.startsWith('/settings')
                      ? 'bg-secondary text-primary font-semibold'
                      : 'text-muted-foreground hover:bg-secondary/60 hover:text-foreground'
                  }`
                }
              >
                <Settings className="h-4 w-4" />
                Settings
              </NavLink>
            </nav>
          </div>

          <div className="flex items-center gap-3">
            {user && (
              <DropdownMenu>
                <DropdownMenuTrigger>
                  <div className="inline-flex items-center gap-2 rounded-lg border border-border bg-secondary/40 px-3 py-1.5 text-sm hover:bg-secondary transition-colors">
                    <div className="h-6 w-6 rounded-full bg-primary text-primary-foreground flex items-center justify-center text-xs font-semibold">
                      {(user.first_name || user.email || 'U').charAt(0).toUpperCase()}
                    </div>
                    <div className="hidden sm:block text-left">
                      <div className="text-xs font-semibold leading-tight">
                        {user.first_name
                          ? `${user.first_name} ${user.last_name || ''}`.trim()
                          : user.email}
                      </div>
                      <div className="text-[10px] text-muted-foreground capitalize">
                        {account?.name || 'Organization'} · {user.role}
                      </div>
                    </div>
                    <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
                  </div>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-56">
                  <div className="px-2.5 py-2 text-xs text-muted-foreground border-b border-border mb-1">
                    Signed in as <strong className="text-foreground block truncate">{user.email}</strong>
                  </div>
                  <DropdownMenuItem onClick={() => navigate('/settings/profile')}>
                    <UserIcon className="h-4 w-4" />
                    Profile & Security
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => navigate('/settings/personalization')}>
                    <Settings className="h-4 w-4" />
                    Personalization & Branding
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem destructive onClick={handleLogout}>
                    <LogOut className="h-4 w-4" />
                    Sign Out
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            )}
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 container mx-auto px-4 sm:px-6 py-6">
        <Outlet />
      </main>

      {/* Mandatory Upstream Attribution Footer */}
      <footer className="border-t border-border bg-card/70 py-4 px-4">
        <div className="container mx-auto flex flex-col sm:flex-row items-center justify-between gap-2">
          <UpstreamAttribution />
          <div className="flex items-center gap-4 text-xs text-muted-foreground">
            <Link to="/verify" className="hover:text-foreground hover:underline">
              Verify Signed PDF
            </Link>
            <span>v0.1.0b</span>
          </div>
        </div>
      </footer>
    </div>
  );
};
