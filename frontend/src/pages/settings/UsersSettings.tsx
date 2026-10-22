import React, { useState, useEffect, useCallback } from 'react';
import { Users, UserPlus, Archive, RotateCcw } from 'lucide-react';
import type { User, UserRole } from '@/types';
import { api, unwrapList } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Badge } from '@/components/ui/badge';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table';
import { useToast } from '@/components/ui/toast';

export const UsersSettings: React.FC = () => {
  const { toast } = useToast();
  const [users, setUsers] = useState<User[]>([]);
  const [inviteOpen, setInviteOpen] = useState(false);
  const [firstName, setFirstName] = useState('');
  const [lastName, setLastName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState<UserRole>('member');
  const [saving, setSaving] = useState(false);

  const loadUsers = useCallback(async () => {
    try {
      const res = await api.get('/api/settings/users');
      setUsers(unwrapList<User>(res, 'users'));
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadUsers();
  }, [loadUsers]);

  const handleCreateUser = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await api.post('/api/settings/users', {
        first_name: firstName.trim(),
        last_name: lastName.trim(),
        email: email.trim(),
        password: password || 'Signline123!',
        role,
      });
      toast({
        title: 'Team Member Added',
        description: `Created ${email} with ${role} role.`,
        variant: 'success',
      });
      setInviteOpen(false);
      setFirstName('');
      setLastName('');
      setEmail('');
      setPassword('');
      await loadUsers();
    } catch (err: any) {
      toast({ title: 'Failed to create user', description: err.message, variant: 'destructive' });
    } finally {
      setSaving(false);
    }
  };

  const handleUpdateRole = async (userId: string, newRole: UserRole) => {
    try {
      await api.put(`/api/settings/users/${userId}`, { role: newRole });
      toast({ title: 'Role Updated', variant: 'success' });
      await loadUsers();
    } catch (err: any) {
      toast({ title: 'Update failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleToggleArchive = async (u: User) => {
    try {
      if (u.archived_at) {
        await api.put(`/api/settings/users/${u.id}`, { archived_at: null, restore: true });
        toast({ title: 'User Restored', variant: 'success' });
      } else {
        await api.delete(`/api/settings/users/${u.id}`);
        toast({ title: 'User Archived' });
      }
      await loadUsers();
    } catch (err: any) {
      toast({ title: 'Action failed', description: err.message, variant: 'destructive' });
    }
  };

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <div>
          <CardTitle className="flex items-center gap-2">
            <Users className="h-5 w-5 text-primary" />
            Team Members &amp; Role-Based Access Control
          </CardTitle>
          <CardDescription>
            Manage team members and assign RBAC roles (`admin`, `manager`, `member`, `agent`).
          </CardDescription>
        </div>
        <Button size="sm" onClick={() => setInviteOpen(true)}>
          <UserPlus className="h-4 w-4" />
          Add Team Member
        </Button>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Email</TableHead>
              <TableHead>Role</TableHead>
              <TableHead>2FA Status</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {users.map((u) => (
              <TableRow key={u.id}>
                <TableCell className="font-semibold">
                  {u.first_name} {u.last_name}
                  {u.archived_at && (
                    <Badge variant="secondary" className="ml-2">
                      Archived
                    </Badge>
                  )}
                </TableCell>
                <TableCell>{u.email}</TableCell>
                <TableCell>
                  <Select
                    value={u.role || 'member'}
                    onChange={(e) => handleUpdateRole(u.id, e.target.value as UserRole)}
                    className="h-8 w-32 text-xs"
                  >
                    <option value="admin">Admin</option>
                    <option value="manager">Manager</option>
                    <option value="member">Member</option>
                    <option value="agent">Agent</option>
                  </Select>
                </TableCell>
                <TableCell>
                  <Badge variant={u.otp_required_for_login ? 'success' : 'secondary'}>
                    {u.otp_required_for_login ? '2FA Active' : 'Standard'}
                  </Badge>
                </TableCell>
                <TableCell className="text-right">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleToggleArchive(u)}
                  >
                    {u.archived_at ? (
                      <>
                        <RotateCcw className="h-3.5 w-3.5" />
                        Restore
                      </>
                    ) : (
                      <>
                        <Archive className="h-3.5 w-3.5" />
                        Archive
                      </>
                    )}
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>

        <Dialog open={inviteOpen} onOpenChange={setInviteOpen}>
          <DialogContent onClose={() => setInviteOpen(false)}>
            <form onSubmit={handleCreateUser} className="space-y-4">
              <DialogHeader>
                <DialogTitle>Invite / Create Team Member</DialogTitle>
                <DialogDescription>
                  Create a new user account in this organization and assign their role.
                </DialogDescription>
              </DialogHeader>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label>First Name</Label>
                  <Input
                    value={firstName}
                    onChange={(e) => setFirstName(e.target.value)}
                    required
                  />
                </div>
                <div className="space-y-1.5">
                  <Label>Last Name</Label>
                  <Input
                    value={lastName}
                    onChange={(e) => setLastName(e.target.value)}
                    required
                  />
                </div>
              </div>
              <div className="space-y-1.5">
                <Label>Email Address</Label>
                <Input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label>Role</Label>
                  <Select value={role} onChange={(e) => setRole(e.target.value as UserRole)}>
                    <option value="admin">Admin (Full Control)</option>
                    <option value="manager">Manager (Templates &amp; Users)</option>
                    <option value="member">Member (Own Templates)</option>
                    <option value="agent">Agent (Send Only)</option>
                  </Select>
                </div>
                <div className="space-y-1.5">
                  <Label>Initial Password</Label>
                  <Input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="••••••••"
                    required
                  />
                </div>
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setInviteOpen(false)}>
                  Cancel
                </Button>
                <Button type="submit" disabled={saving}>
                  {saving ? 'Creating...' : 'Create User'}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </CardContent>
    </Card>
  );
};
