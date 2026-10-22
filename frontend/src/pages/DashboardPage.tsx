import React, { useState, useEffect, useCallback } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  FileText,
  Folder,
  FolderPlus,
  Plus,
  Upload,
  Code2,
  Search,
  Send,
  Copy,
  Archive,
  RotateCcw,
  Trash2,
  MoreVertical,
  Edit3,
  ExternalLink,
} from 'lucide-react';
import type { Template, TemplateFolder } from '@/types';
import { formatShortDate } from '@/lib/utils';
import { api, unwrapList } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu';
import { useToast } from '@/components/ui/toast';
import { HtmlTemplateModal } from '@/components/builder/HtmlTemplateModal';
import { NewSubmissionDialog } from '@/components/submissions/NewSubmissionDialog';

export const DashboardPage: React.FC = () => {
  const navigate = useNavigate();
  const { toast } = useToast();

  const [templates, setTemplates] = useState<Template[]>([]);
  const [folders, setFolders] = useState<TemplateFolder[]>([]);
  const [selectedFolderId, setSelectedFolderId] = useState<string>('');
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [viewFilter, setViewFilter] = useState<'active' | 'archived'>('active');
  const [loading, setLoading] = useState<boolean>(true);

  // Modals
  const [uploadModalOpen, setUploadModalOpen] = useState(false);
  const [uploadName, setUploadName] = useState('');
  const [uploadFile, setUploadFile] = useState<File | null>(null);
  const [uploading, setUploading] = useState(false);

  const [htmlModalOpen, setHtmlModalOpen] = useState(false);
  const [folderModalOpen, setFolderModalOpen] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');

  const [sendTemplate, setSendTemplate] = useState<Template | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const params = new URLSearchParams();
      if (searchQuery.trim()) params.set('q', searchQuery.trim());
      if (selectedFolderId) params.set('folder_id', selectedFolderId);
      if (viewFilter === 'archived') params.set('archived', 'true');

      const [tplRes, fldRes] = await Promise.all([
        api.get(`/api/templates?${params.toString()}`),
        api.get('/api/folders').catch(() => []),
      ]);
      setTemplates(unwrapList<Template>(tplRes, 'templates'));
      setFolders(unwrapList<TemplateFolder>(fldRes, 'folders'));
    } catch (err: any) {
      toast({
        title: 'Failed to load templates',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  }, [searchQuery, selectedFolderId, viewFilter, toast]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleUploadTemplate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!uploadFile && !uploadName.trim()) return;
    setUploading(true);
    try {
      if (uploadFile) {
        const formData = new FormData();
        formData.append('file', uploadFile);
        if (uploadName.trim()) formData.append('name', uploadName.trim());
        if (selectedFolderId) formData.append('folder_id', selectedFolderId);

        const res = await api.post<any>('/api/templates/upload', formData);
        const created = res.template || res.data || res;
        toast({
          title: 'PDF Template Uploaded',
          description: 'Rendered document pages and extracted form fields.',
          variant: 'success',
        });
        setUploadModalOpen(false);
        setUploadFile(null);
        setUploadName('');
        if (created?.id) {
          navigate(`/templates/${created.id}/edit`);
        } else {
          await loadData();
        }
      } else {
        const res = await api.post<any>('/api/templates', {
          name: uploadName.trim() || 'Untitled Template',
          folder_id: selectedFolderId || undefined,
        });
        const created = res.template || res.data || res;
        setUploadModalOpen(false);
        setUploadName('');
        if (created?.id) {
          navigate(`/templates/${created.id}/edit`);
        } else {
          await loadData();
        }
      }
    } catch (err: any) {
      toast({
        title: 'Failed to create template',
        description: err.message,
        variant: 'destructive',
      });
    } finally {
      setUploading(false);
    }
  };

  const handleCreateFromHtml = async (name: string, html: string) => {
    const res = await api.post<any>('/api/templates/html', {
      name,
      html,
      folder_id: selectedFolderId || undefined,
    });
    const created = res.template || res.data || res;
    toast({
      title: 'Dynamic HTML Template Created',
      variant: 'success',
    });
    if (created?.id) {
      navigate(`/templates/${created.id}/edit`);
    } else {
      await loadData();
    }
  };

  const handleCreateFolder = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newFolderName.trim()) return;
    try {
      await api.post('/api/folders', {
        name: newFolderName.trim(),
        parent_folder_id: selectedFolderId || undefined,
      });
      setNewFolderName('');
      setFolderModalOpen(false);
      toast({ title: 'Folder Created', variant: 'success' });
      await loadData();
    } catch (err: any) {
      toast({ title: 'Failed to create folder', description: err.message, variant: 'destructive' });
    }
  };

  const handleCloneTemplate = async (id: string) => {
    try {
      await api.post(`/api/templates/${id}/clone`, {});
      toast({ title: 'Template Cloned', variant: 'success' });
      await loadData();
    } catch (err: any) {
      toast({ title: 'Clone failed', description: err.message, variant: 'destructive' });
    }
  };

  const handleArchiveOrRestore = async (tpl: Template) => {
    try {
      if (tpl.archived_at) {
        await api.post(`/api/templates/${tpl.id}/restore`, {});
        toast({ title: 'Template Restored', variant: 'success' });
      } else {
        await api.delete(`/api/templates/${tpl.id}`);
        toast({ title: 'Template Archived' });
      }
      await loadData();
    } catch (err: any) {
      toast({ title: 'Action failed', description: err.message, variant: 'destructive' });
    }
  };

  const filteredTemplates = templates.filter((t) => {
    if (viewFilter === 'active' && t.archived_at) return false;
    if (viewFilter === 'archived' && !t.archived_at) return false;
    if (selectedFolderId && t.folder_id && t.folder_id !== selectedFolderId) return false;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      return t.name?.toLowerCase().includes(q) || t.slug?.toLowerCase().includes(q);
    }
    return true;
  });

  return (
    <div className="space-y-6">
      {/* Top Action Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Document Templates</h1>
          <p className="text-sm text-muted-foreground">
            Create reusable PDF and dynamic HTML templates, configure signer roles, and send signature requests.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" onClick={() => setFolderModalOpen(true)}>
            <FolderPlus className="h-4 w-4" />
            New Folder
          </Button>
          <Button variant="secondary" onClick={() => setHtmlModalOpen(true)}>
            <Code2 className="h-4 w-4" />
            Create from HTML
          </Button>
          <Button onClick={() => setUploadModalOpen(true)}>
            <Upload className="h-4 w-4" />
            Upload PDF / New Template
          </Button>
        </div>
      </div>

      {/* Folders Bar & Search Filter */}
      <div className="flex flex-col md:flex-row items-stretch md:items-center justify-between gap-3 rounded-xl border border-border bg-card p-3 shadow-sm">
        <div className="flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            onClick={() => setSelectedFolderId('')}
            className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              selectedFolderId === ''
                ? 'bg-primary text-primary-foreground'
                : 'bg-secondary/60 text-foreground hover:bg-secondary'
            }`}
          >
            <Folder className="h-3.5 w-3.5" />
            All Folders
          </button>
          {folders.map((folder) => (
            <button
              key={folder.id}
              type="button"
              onClick={() => setSelectedFolderId(folder.id)}
              className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                selectedFolderId === folder.id
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary/60 text-foreground hover:bg-secondary'
              }`}
            >
              <Folder className="h-3.5 w-3.5" />
              <span>{folder.name}</span>
            </button>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <div className="relative flex-1 sm:w-64">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search templates..."
              className="pl-8 h-9"
            />
          </div>

          <div className="inline-flex rounded-lg border border-border bg-secondary p-0.5 text-xs">
            <button
              type="button"
              onClick={() => setViewFilter('active')}
              className={`rounded-md px-2.5 py-1 font-medium ${
                viewFilter === 'active' ? 'bg-card text-foreground shadow-sm' : 'text-muted-foreground'
              }`}
            >
              Active
            </button>
            <button
              type="button"
              onClick={() => setViewFilter('archived')}
              className={`rounded-md px-2.5 py-1 font-medium ${
                viewFilter === 'archived'
                  ? 'bg-card text-foreground shadow-sm'
                  : 'text-muted-foreground'
              }`}
            >
              Archived
            </button>
          </div>
        </div>
      </div>

      {/* Templates Grid */}
      {loading ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {[1, 2, 3].map((n) => (
            <div
              key={n}
              className="h-44 rounded-xl border border-border bg-card animate-pulse p-5"
            />
          ))}
        </div>
      ) : filteredTemplates.length === 0 ? (
        <Card className="border-dashed">
          <CardContent className="flex flex-col items-center justify-center py-14 text-center space-y-3">
            <div className="h-12 w-12 rounded-full bg-primary/10 text-primary flex items-center justify-center">
              <FileText className="h-6 w-6" />
            </div>
            <div className="space-y-1">
              <h3 className="font-semibold text-base">No templates found</h3>
              <p className="text-xs text-muted-foreground max-w-sm">
                Upload a PDF document or create a dynamic HTML template to start collecting legally binding digital signatures.
              </p>
            </div>
            <div className="flex items-center gap-2 pt-2">
              <Button onClick={() => setUploadModalOpen(true)}>
                <Plus className="h-4 w-4" />
                Upload PDF Template
              </Button>
              <Button variant="outline" onClick={() => setHtmlModalOpen(true)}>
                <Code2 className="h-4 w-4" />
                Create from HTML
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredTemplates.map((tpl) => {
            const rolesCount = tpl.submitters?.length || 1;
            const fieldsCount = tpl.fields?.length || 0;
            return (
              <Card
                key={tpl.id}
                className="group hover:border-primary/60 transition-all flex flex-col justify-between"
              >
                <CardContent className="p-5 space-y-4">
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex items-start gap-3 min-w-0">
                      <div className="h-10 w-10 rounded-lg bg-secondary text-primary flex items-center justify-center shrink-0 mt-0.5">
                        <FileText className="h-5 w-5" />
                      </div>
                      <div className="min-w-0">
                        <Link
                          to={`/templates/${tpl.id}`}
                          className="font-semibold text-base text-foreground hover:text-primary truncate block"
                        >
                          {tpl.name}
                        </Link>
                        <div className="text-xs text-muted-foreground flex items-center gap-2 mt-0.5">
                          <span>{formatShortDate(tpl.created_at)}</span>
                          <span>·</span>
                          <span>{rolesCount} role(s)</span>
                          <span>·</span>
                          <span>{fieldsCount} field(s)</span>
                        </div>
                      </div>
                    </div>

                    <DropdownMenu>
                      <DropdownMenuTrigger>
                        <Button variant="ghost" size="icon" className="h-8 w-8">
                          <MoreVertical className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => navigate(`/templates/${tpl.id}`)}>
                          <ExternalLink className="h-4 w-4" />
                          View Details
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => navigate(`/templates/${tpl.id}/edit`)}>
                          <Edit3 className="h-4 w-4" />
                          Edit in Builder
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => handleCloneTemplate(tpl.id)}>
                          <Copy className="h-4 w-4" />
                          Clone Template
                        </DropdownMenuItem>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem
                          destructive={!tpl.archived_at}
                          onClick={() => handleArchiveOrRestore(tpl)}
                        >
                          {tpl.archived_at ? (
                            <>
                              <RotateCcw className="h-4 w-4" />
                              Restore Template
                            </>
                          ) : (
                            <>
                              <Archive className="h-4 w-4" />
                              Archive Template
                            </>
                          )}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>

                  {/* Role Badges */}
                  <div className="flex flex-wrap items-center gap-1.5">
                    {(tpl.submitters || [{ uuid: '1', name: 'First Party' }]).map((s) => (
                      <Badge key={s.uuid} variant="secondary" className="text-[11px]">
                        {s.name}
                      </Badge>
                    ))}
                    {tpl.shared_link && (
                      <Badge variant="info" className="text-[11px]">
                        Shared Link Active
                      </Badge>
                    )}
                  </div>

                  {/* Bottom Actions */}
                  <div className="pt-2 border-t border-border flex items-center justify-between gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => navigate(`/templates/${tpl.id}/edit`)}
                      className="flex-1"
                    >
                      <Edit3 className="h-3.5 w-3.5" />
                      Builder
                    </Button>
                    <Button
                      size="sm"
                      onClick={() => setSendTemplate(tpl)}
                      className="flex-1"
                    >
                      <Send className="h-3.5 w-3.5" />
                      Send to Sign
                    </Button>
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      {/* Upload PDF / Create Template Modal */}
      <Dialog open={uploadModalOpen} onOpenChange={setUploadModalOpen}>
        <DialogContent onClose={() => setUploadModalOpen(false)}>
          <form onSubmit={handleUploadTemplate} className="space-y-4">
            <DialogHeader>
              <DialogTitle>Upload PDF or Create New Template</DialogTitle>
              <DialogDescription>
                Upload a PDF or image file. Signline will automatically render page previews and detect any embedded form fields or text tags.
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-1.5">
              <Label htmlFor="tpl-upload-name">Template Name</Label>
              <Input
                id="tpl-upload-name"
                value={uploadName}
                onChange={(e) => setUploadName(e.target.value)}
                placeholder="e.g. Non-Disclosure Agreement (NDA)"
                required={!uploadFile}
              />
            </div>

            <div className="space-y-1.5">
              <Label htmlFor="tpl-upload-file">PDF or Image Document</Label>
              <Input
                id="tpl-upload-file"
                type="file"
                accept=".pdf,image/png,image/jpeg,image/webp"
                onChange={(e) => {
                  const f = e.target.files?.[0] || null;
                  setUploadFile(f);
                  if (f && !uploadName) {
                    setUploadName(f.name.replace(/\.[^.]+$/, ''));
                  }
                }}
              />
            </div>

            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setUploadModalOpen(false)}>
                Cancel
              </Button>
              <Button type="submit" disabled={uploading}>
                {uploading ? 'Uploading & Processing...' : 'Create & Open Builder'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* Create Folder Modal */}
      <Dialog open={folderModalOpen} onOpenChange={setFolderModalOpen}>
        <DialogContent onClose={() => setFolderModalOpen(false)}>
          <form onSubmit={handleCreateFolder} className="space-y-4">
            <DialogHeader>
              <DialogTitle>Create Template Folder</DialogTitle>
              <DialogDescription>
                Organize your document templates into folders by department or workflow.
              </DialogDescription>
            </DialogHeader>
            <div className="space-y-1.5">
              <Label htmlFor="folder-name-input">Folder Name</Label>
              <Input
                id="folder-name-input"
                value={newFolderName}
                onChange={(e) => setNewFolderName(e.target.value)}
                placeholder="e.g. HR & Onboarding"
                required
              />
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setFolderModalOpen(false)}>
                Cancel
              </Button>
              <Button type="submit">Create Folder</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* Create from HTML Modal */}
      <HtmlTemplateModal
        open={htmlModalOpen}
        onOpenChange={setHtmlModalOpen}
        onSave={handleCreateFromHtml}
      />

      {/* Send to Sign Modal */}
      <NewSubmissionDialog
        open={Boolean(sendTemplate)}
        onOpenChange={(open) => !open && setSendTemplate(null)}
        template={sendTemplate}
        onCreated={() => loadData()}
      />
    </div>
  );
};
