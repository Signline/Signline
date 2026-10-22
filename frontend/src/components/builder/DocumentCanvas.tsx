import React, { useRef, useState } from 'react';
import { ZoomIn, ZoomOut, Move } from 'lucide-react';
import type { FieldType, SubmitterRole, TemplateDocument, TemplateField } from '@/types';
import { getSubmitterColor } from '@/lib/utils';
import { Button } from '@/components/ui/button';

interface DocumentCanvasProps {
  templateId: string;
  documents: TemplateDocument[];
  fields: TemplateField[];
  submitters: SubmitterRole[];
  selectedFieldUuid: string | null;
  activePage: number;
  onSelectPage: (page: number) => void;
  onSelectField: (uuid: string | null) => void;
  onUpdateField: (updated: TemplateField) => void;
  onDropNewField: (type: FieldType, page: number, x: number, y: number) => void;
}

export const DocumentCanvas: React.FC<DocumentCanvasProps> = ({
  templateId,
  documents,
  fields,
  submitters,
  selectedFieldUuid,
  activePage,
  onSelectPage,
  onSelectField,
  onUpdateField,
  onDropNewField,
}) => {
  const [zoom, setZoom] = useState<number>(100);
  const pageRefs = useRef<Record<number, HTMLDivElement | null>>({});

  // Build flat list of pages
  const pages: Array<{
    pageNumber: number; // 0-indexed
    docName: string;
    previewUrl: string;
  }> = [];

  let counter = 0;
  if (documents && documents.length > 0) {
    for (const doc of documents) {
      const count =
        doc.pages?.length ||
        doc.preview_images?.length ||
        doc.metadata?.pdf?.number_of_pages ||
        1;
      for (let i = 0; i < count; i++) {
        const previewUrl =
          doc.pages?.[i]?.preview_url ||
          doc.preview_images?.[i] ||
          `/api/templates/${templateId}/pages/${counter + 1}`;
        pages.push({
          pageNumber: counter,
          docName: doc.name || 'Document.pdf',
          previewUrl,
        });
        counter++;
      }
    }
  } else {
    pages.push({
      pageNumber: 0,
      docName: 'Page 1',
      previewUrl: `/api/templates/${templateId}/pages/1`,
    });
  }

  const startDragField = (
    e: React.MouseEvent,
    field: TemplateField,
    pageNum: number,
    mode: 'move' | 'resize'
  ) => {
    e.stopPropagation();
    e.preventDefault();
    onSelectField(field.uuid);

    const container = pageRefs.current[pageNum];
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const area = field.areas?.[0] || { x: 0.1, y: 0.1, w: 0.2, h: 0.04, page: pageNum };

    const startX = e.clientX;
    const startY = e.clientY;
    const origX = area.x;
    const origY = area.y;
    const origW = area.w;
    const origH = area.h;

    const handleMouseMove = (moveEvt: MouseEvent) => {
      const dx = (moveEvt.clientX - startX) / rect.width;
      const dy = (moveEvt.clientY - startY) / rect.height;

      if (mode === 'move') {
        const nx = Math.max(0, Math.min(1 - origW, origX + dx));
        const ny = Math.max(0, Math.min(1 - origH, origY + dy));
        onUpdateField({
          ...field,
          areas: [{ ...area, x: Number(nx.toFixed(4)), y: Number(ny.toFixed(4)), page: pageNum }],
        });
      } else {
        const nw = Math.max(0.03, Math.min(1 - origX, origW + dx));
        const nh = Math.max(0.025, Math.min(1 - origY, origH + dy));
        onUpdateField({
          ...field,
          areas: [{ ...area, w: Number(nw.toFixed(4)), h: Number(nh.toFixed(4)), page: pageNum }],
        });
      }
    };

    const handleMouseUp = () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  };

  const handleDropOnPage = (e: React.DragEvent<HTMLDivElement>, pageNum: number) => {
    e.preventDefault();
    const fieldType = e.dataTransfer.getData('application/signline-field-type') as FieldType;
    if (!fieldType) return;

    const container = pageRefs.current[pageNum];
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const x = Math.max(0.02, Math.min(0.75, (e.clientX - rect.left) / rect.width - 0.08));
    const y = Math.max(0.02, Math.min(0.92, (e.clientY - rect.top) / rect.height - 0.02));
    onDropNewField(fieldType, pageNum, Number(x.toFixed(4)), Number(y.toFixed(4)));
  };

  return (
    <div className="flex flex-col h-full">
      {/* Zoom & Page Controls Toolbar */}
      <div className="flex items-center justify-between border-b border-border bg-card px-4 py-2">
        <div className="flex items-center gap-2 text-xs">
          <span className="font-semibold text-foreground">Pages:</span>
          {pages.map((p) => (
            <button
              key={p.pageNumber}
              type="button"
              onClick={() => {
                onSelectPage(p.pageNumber);
                pageRefs.current[p.pageNumber]?.scrollIntoView({ behavior: 'smooth' });
              }}
              className={`h-7 px-2.5 rounded font-medium transition-colors ${
                activePage === p.pageNumber
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground hover:bg-muted'
              }`}
            >
              {p.pageNumber + 1}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="h-7 px-2"
            onClick={() => setZoom((z) => Math.max(50, z - 15))}
          >
            <ZoomOut className="h-3.5 w-3.5" />
          </Button>
          <span className="text-xs font-medium w-12 text-center">{zoom}%</span>
          <Button
            variant="outline"
            size="sm"
            className="h-7 px-2"
            onClick={() => setZoom((z) => Math.min(200, z + 15))}
          >
            <ZoomIn className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      {/* Scrollable Pages Area */}
      <div
        className="flex-1 overflow-auto bg-secondary/50 p-6 space-y-8"
        onClick={() => onSelectField(null)}
      >
        {pages.map((p) => {
          const pageFields = fields.filter((f) => {
            const area = f.areas?.[0];
            return (area?.page ?? 0) === p.pageNumber;
          });

          const widthPx = Math.round(740 * (zoom / 100));

          return (
            <div
              key={p.pageNumber}
              ref={(el) => {
                pageRefs.current[p.pageNumber] = el;
              }}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => handleDropOnPage(e, p.pageNumber)}
              onClick={(e) => {
                e.stopPropagation();
                onSelectPage(p.pageNumber);
                onSelectField(null);
              }}
              style={{
                width: `${widthPx}px`,
                aspectRatio: '8.5 / 11',
              }}
              className={`relative mx-auto rounded-lg bg-white shadow-lg border-2 transition-colors select-none overflow-hidden ${
                activePage === p.pageNumber ? 'border-primary/60' : 'border-border'
              }`}
            >
              {/* Raster PDF Page Image */}
              {p.previewUrl && (
                <img
                  src={p.previewUrl}
                  alt={`Page ${p.pageNumber + 1}`}
                  className="w-full h-full object-contain pointer-events-none"
                  onError={(e) => {
                    (e.currentTarget as HTMLImageElement).style.display = 'none';
                  }}
                />
              )}

              {/* Subtle grid background when no image is loaded */}
              <div className="pointer-events-none absolute top-2 right-3 text-[11px] font-medium text-slate-400">
                {p.docName} — Page {p.pageNumber + 1}
              </div>

              {/* Field Bounding Boxes */}
              {pageFields.map((field) => {
                const area = field.areas?.[0] || {
                  x: 0.1,
                  y: 0.1,
                  w: 0.22,
                  h: 0.045,
                  page: p.pageNumber,
                };
                const subIdx = Math.max(
                  0,
                  submitters.findIndex((s) => s.uuid === field.submitter_uuid)
                );
                const color = getSubmitterColor(subIdx);
                const isSelected = field.uuid === selectedFieldUuid;

                return (
                  <div
                    key={field.uuid}
                    onMouseDown={(e) => startDragField(e, field, p.pageNumber, 'move')}
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelectField(field.uuid);
                    }}
                    style={{
                      position: 'absolute',
                      left: `${area.x * 100}%`,
                      top: `${area.y * 100}%`,
                      width: `${area.w * 100}%`,
                      height: `${area.h * 100}%`,
                    }}
                    className={`group flex items-center justify-between rounded border-2 px-1.5 cursor-move transition-shadow ${
                      color.bg
                    } ${color.border} ${color.text} ${
                      isSelected ? 'ring-2 ring-primary shadow-md z-20' : 'z-10 hover:shadow'
                    }`}
                  >
                    <div className="flex items-center gap-1 min-w-0 truncate text-[11px] font-semibold">
                      <Move className="h-3 w-3 shrink-0 opacity-60" />
                      <span className="truncate">
                        {field.title || field.name || field.type}
                        {field.required ? ' *' : ''}
                      </span>
                    </div>

                    {/* Bottom-right resize handle */}
                    <div
                      onMouseDown={(e) => startDragField(e, field, p.pageNumber, 'resize')}
                      className="absolute bottom-0 right-0 h-3 w-3 cursor-se-resize bg-primary rounded-tl opacity-80 group-hover:opacity-100"
                      title="Drag to resize"
                    />
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
};
