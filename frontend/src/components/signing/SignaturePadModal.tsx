import React, { useRef, useState, useEffect } from 'react';
import { Eraser, PenTool, Type, Upload, QrCode, Check } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';

interface SignaturePadModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title?: string;
  defaultTypedName?: string;
  allowTyped?: boolean;
  onSave: (dataUrl: string) => void;
}

const PEN_COLORS = [
  { label: 'Navy Blue', value: '#1e3a8a' },
  { label: 'Black', value: '#111827' },
  { label: 'Signline Blue', value: '#2161AD' },
];

export const SignaturePadModal: React.FC<SignaturePadModalProps> = ({
  open,
  onOpenChange,
  title = 'Adopt Your Signature',
  defaultTypedName = '',
  allowTyped = true,
  onSave,
}) => {
  const [mode, setMode] = useState<string>('draw');
  const [penColor, setPenColor] = useState<string>('#1e3a8a');
  const [typedText, setTypedText] = useState<string>(defaultTypedName);
  const [fontStyle, setFontStyle] = useState<string>('italic 44px "Brush Script MT", "Dancing Script", cursive');
  const [uploadedDataUrl, setUploadedDataUrl] = useState<string>('');
  const [hasDrawn, setHasDrawn] = useState(false);

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const isDrawingRef = useRef(false);
  const lastPointRef = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (defaultTypedName && !typedText) {
      setTypedText(defaultTypedName);
    }
  }, [defaultTypedName, typedText]);

  useEffect(() => {
    if (open && mode === 'draw') {
      setTimeout(() => initCanvas(), 50);
    }
  }, [open, mode]);

  const initCanvas = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.lineWidth = 3;
    ctx.strokeStyle = penColor;
    setHasDrawn(false);
  };

  const getCoordinates = (
    e: React.MouseEvent<HTMLCanvasElement> | React.TouchEvent<HTMLCanvasElement>
  ) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;

    if ('touches' in e) {
      const touch = e.touches[0] || e.changedTouches[0];
      return {
        x: (touch.clientX - rect.left) * scaleX,
        y: (touch.clientY - rect.top) * scaleY,
      };
    }
    return {
      x: (e.clientX - rect.left) * scaleX,
      y: (e.clientY - rect.top) * scaleY,
    };
  };

  const startDrawing = (
    e: React.MouseEvent<HTMLCanvasElement> | React.TouchEvent<HTMLCanvasElement>
  ) => {
    e.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const pt = getCoordinates(e);
    isDrawingRef.current = true;
    lastPointRef.current = pt;
    ctx.strokeStyle = penColor;
    ctx.lineWidth = 3.2;
    ctx.beginPath();
    ctx.moveTo(pt.x, pt.y);
    setHasDrawn(true);
  };

  const draw = (
    e: React.MouseEvent<HTMLCanvasElement> | React.TouchEvent<HTMLCanvasElement>
  ) => {
    if (!isDrawingRef.current) return;
    e.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx || !lastPointRef.current) return;

    const pt = getCoordinates(e);
    const midX = (lastPointRef.current.x + pt.x) / 2;
    const midY = (lastPointRef.current.y + pt.y) / 2;

    ctx.quadraticCurveTo(lastPointRef.current.x, lastPointRef.current.y, midX, midY);
    ctx.stroke();
    lastPointRef.current = pt;
  };

  const endDrawing = () => {
    isDrawingRef.current = false;
    lastPointRef.current = null;
  };

  const handleUploadImage = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        setUploadedDataUrl(reader.result);
      }
    };
    reader.readAsDataURL(file);
  };

  const renderTypedToDataUrl = (): string => {
    const offscreen = document.createElement('canvas');
    offscreen.width = 520;
    offscreen.height = 160;
    const ctx = offscreen.getContext('2d');
    if (!ctx) return '';
    ctx.clearRect(0, 0, offscreen.width, offscreen.height);
    ctx.fillStyle = penColor;
    ctx.font = fontStyle;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(typedText.trim() || 'Signed', offscreen.width / 2, offscreen.height / 2);
    return offscreen.toDataURL('image/png');
  };

  const handleConfirm = () => {
    if (mode === 'draw') {
      const canvas = canvasRef.current;
      if (!canvas) return;
      onSave(canvas.toDataURL('image/png'));
      onOpenChange(false);
    } else if (mode === 'type') {
      if (!typedText.trim()) return;
      onSave(renderTypedToDataUrl());
      onOpenChange(false);
    } else if (mode === 'upload') {
      if (!uploadedDataUrl) return;
      onSave(uploadedDataUrl);
      onOpenChange(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl" onClose={() => onOpenChange(false)}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            Draw your signature using your mouse or touchscreen, type it, or upload an image.
          </DialogDescription>
        </DialogHeader>

        <div className="flex items-center justify-between gap-2 py-1">
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">Ink Color:</span>
            {PEN_COLORS.map((c) => (
              <button
                key={c.value}
                type="button"
                onClick={() => setPenColor(c.value)}
                className={`h-6 w-6 rounded-full border-2 transition-transform ${
                  penColor === c.value ? 'scale-110 border-primary ring-2 ring-primary/30' : 'border-transparent'
                }`}
                style={{ backgroundColor: c.value }}
                title={c.label}
              />
            ))}
          </div>
          {mode === 'draw' && (
            <Button variant="ghost" size="sm" onClick={initCanvas}>
              <Eraser className="h-3.5 w-3.5" />
              Clear Pad
            </Button>
          )}
        </div>

        <Tabs value={mode} onValueChange={setMode}>
          <TabsList className="w-full grid grid-cols-4">
            <TabsTrigger value="draw" className="gap-1.5">
              <PenTool className="h-3.5 w-3.5" />
              Draw
            </TabsTrigger>
            {allowTyped && (
              <TabsTrigger value="type" className="gap-1.5">
                <Type className="h-3.5 w-3.5" />
                Type
              </TabsTrigger>
            )}
            <TabsTrigger value="upload" className="gap-1.5">
              <Upload className="h-3.5 w-3.5" />
              Upload
            </TabsTrigger>
            <TabsTrigger value="mobile" className="gap-1.5">
              <QrCode className="h-3.5 w-3.5" />
              Mobile QR
            </TabsTrigger>
          </TabsList>

          <TabsContent value="draw">
            <div className="relative rounded-lg border-2 border-dashed border-border bg-white overflow-hidden">
              <canvas
                ref={canvasRef}
                width={520}
                height={180}
                onMouseDown={startDrawing}
                onMouseMove={draw}
                onMouseUp={endDrawing}
                onMouseLeave={endDrawing}
                onTouchStart={startDrawing}
                onTouchMove={draw}
                onTouchEnd={endDrawing}
                className="w-full h-44 cursor-crosshair touch-none"
              />
              <div className="pointer-events-none absolute bottom-6 left-8 right-8 border-b border-slate-300 flex items-end justify-between text-[10px] text-slate-400 pb-0.5">
                <span>Sign Here ×</span>
                <span>Electronic Signature</span>
              </div>
            </div>
          </TabsContent>

          {allowTyped && (
            <TabsContent value="type" className="space-y-3">
              <div>
                <Label htmlFor="typed-sig-input">Type Your Full Name / Initials</Label>
                <Input
                  id="typed-sig-input"
                  value={typedText}
                  onChange={(e) => setTypedText(e.target.value)}
                  placeholder="e.g. Jane Doe"
                  className="mt-1"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                {[
                  {
                    label: 'Classic Script',
                    font: 'italic 44px "Brush Script MT", "Dancing Script", cursive',
                  },
                  {
                    label: 'Formal Cursive',
                    font: 'italic 40px Georgia, "Times New Roman", serif',
                  },
                ].map((f) => (
                  <button
                    key={f.label}
                    type="button"
                    onClick={() => setFontStyle(f.font)}
                    className={`rounded-lg border p-3 bg-white text-center transition-all ${
                      fontStyle === f.font ? 'border-primary ring-2 ring-primary/20' : 'border-border'
                    }`}
                  >
                    <div
                      style={{ font: f.font, color: penColor }}
                      className="truncate py-1"
                    >
                      {typedText || 'Jane Doe'}
                    </div>
                    <div className="text-[11px] text-muted-foreground mt-1">{f.label}</div>
                  </button>
                ))}
              </div>
            </TabsContent>
          )}

          <TabsContent value="upload" className="space-y-3">
            <div className="rounded-lg border-2 border-dashed border-border bg-secondary/30 p-6 text-center">
              <Input
                type="file"
                accept="image/png,image/jpeg,image/webp,image/svg+xml"
                onChange={handleUploadImage}
                className="max-w-xs mx-auto"
              />
              {uploadedDataUrl && (
                <div className="mt-4 bg-white p-3 rounded border border-border inline-block">
                  <img
                    src={uploadedDataUrl}
                    alt="Uploaded signature"
                    className="max-h-28 object-contain"
                  />
                </div>
              )}
            </div>
          </TabsContent>

          <TabsContent value="mobile" className="space-y-3">
            <div className="rounded-lg border border-border bg-secondary/30 p-4 text-center space-y-2">
              <QrCode className="h-16 w-16 mx-auto text-primary" />
              <div className="text-sm font-medium">Sign on your mobile device</div>
              <p className="text-xs text-muted-foreground max-w-sm mx-auto">
                Open this signing link on your smartphone browser to draw with your finger or stylus:
              </p>
              <code className="block rounded bg-card px-3 py-1.5 text-xs border border-border break-all">
                {window.location.href}
              </code>
            </div>
          </TabsContent>
        </Tabs>

        <p className="text-[11px] text-muted-foreground leading-relaxed">
          By clicking <strong>Adopt &amp; Sign</strong>, I agree that this electronic signature or initials
          representation is the legal equivalent of my handwritten signature on this document.
        </p>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          {mode !== 'mobile' && (
            <Button
              onClick={handleConfirm}
              disabled={
                (mode === 'draw' && !hasDrawn) ||
                (mode === 'type' && !typedText.trim()) ||
                (mode === 'upload' && !uploadedDataUrl)
              }
            >
              <Check className="h-4 w-4" />
              Adopt &amp; Sign
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
