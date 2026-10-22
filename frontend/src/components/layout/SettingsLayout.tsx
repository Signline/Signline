import React from 'react';
import { NavLink, Outlet } from 'react-router-dom';
import {
  User,
  Building2,
  Palette,
  Mail,
  BellRing,
  FileKey2,
  Users,
  Webhook,
  HardDrive,
  KeyRound,
  Cpu,
} from 'lucide-react';

const SETTINGS_NAV = [
  { to: '/settings/profile', label: 'Profile & 2FA', icon: User },
  { to: '/settings/account', label: 'Account & Signing', icon: Building2 },
  { to: '/settings/personalization', label: 'Personalization', icon: Palette },
  { to: '/settings/email', label: 'Email & Gmail OAuth', icon: Mail },
  { to: '/settings/notifications', label: 'Reminders & Alerts', icon: BellRing },
  { to: '/settings/esign', label: 'E-Sign Certificates', icon: FileKey2 },
  { to: '/settings/users', label: 'Team & Roles', icon: Users },
  { to: '/settings/webhooks', label: 'Webhooks', icon: Webhook },
  { to: '/settings/storage', label: 'File Storage', icon: HardDrive },
  { to: '/settings/api', label: 'REST API & Sandbox', icon: KeyRound },
  { to: '/settings/mcp', label: 'MCP Server', icon: Cpu },
];

export const SettingsLayout: React.FC = () => {
  return (
    <div className="flex flex-col md:flex-row gap-6">
      <aside className="w-full md:w-64 shrink-0">
        <div className="rounded-xl border border-border bg-card p-2 shadow-sm">
          <div className="px-3 py-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Settings & Administration
          </div>
          <nav className="flex flex-col gap-0.5">
            {SETTINGS_NAV.map((item) => {
              const Icon = item.icon;
              return (
                <NavLink
                  key={item.to}
                  to={item.to}
                  className={({ isActive }) =>
                    `flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                      isActive
                        ? 'bg-primary text-primary-foreground shadow-sm'
                        : 'text-foreground hover:bg-secondary'
                    }`
                  }
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  <span>{item.label}</span>
                </NavLink>
              );
            })}
          </nav>
        </div>
      </aside>

      <div className="flex-1 min-w-0">
        <Outlet />
      </div>
    </div>
  );
};
