import React, { createContext, useContext, useEffect, useState, useCallback } from 'react';
import type { BrandingConfig } from '@/types';
import { contrastingForegroundHsl, hexToHslString } from '@/lib/utils';

const DEFAULT_BRANDING: BrandingConfig = {
  product_name: 'Signline',
  primary_color: '#2161AD',
  accent_color: '#253E8A',
  logo_url: '/branding/logo',
  favicon_url: '/branding/favicon',
  upstream_name: 'Signline',
  upstream_url: 'https://github.com/signline/signline',
};

interface BrandingContextValue {
  branding: BrandingConfig;
  refreshBranding: () => Promise<void>;
  applyColorPreview: (primaryHex: string, accentHex: string) => void;
}

const BrandingContext = createContext<BrandingContextValue>({
  branding: DEFAULT_BRANDING,
  refreshBranding: async () => {},
  applyColorPreview: () => {},
});

export function applyBrandingCssVariables(primaryHex: string, accentHex: string) {
  const root = document.documentElement;
  if (primaryHex && /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(primaryHex)) {
    const primaryHsl = hexToHslString(primaryHex);
    const primaryFg = contrastingForegroundHsl(primaryHex);
    root.style.setProperty('--primary', primaryHsl);
    root.style.setProperty('--ring', primaryHsl);
    root.style.setProperty('--primary-foreground', primaryFg);

    const themeColorMeta = document.getElementById('app-theme-color');
    if (themeColorMeta) {
      themeColorMeta.setAttribute('content', primaryHex);
    }
  }

  if (accentHex && /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(accentHex)) {
    const accentHsl = hexToHslString(accentHex);
    const accentFg = contrastingForegroundHsl(accentHex);
    root.style.setProperty('--accent', accentHsl);
    root.style.setProperty('--accent-foreground', accentFg);
  }
}

export const BrandingProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [branding, setBranding] = useState<BrandingConfig>(DEFAULT_BRANDING);

  const refreshBranding = useCallback(async () => {
    try {
      const res = await fetch('/api/branding', { credentials: 'include' });
      if (res.ok) {
        const json = await res.json();
        const data: Partial<BrandingConfig> = json.branding || json.data || json;
        const merged: BrandingConfig = {
          ...DEFAULT_BRANDING,
          ...data,
          product_name: data.product_name || 'Signline',
          primary_color: data.primary_color || '#2161AD',
          accent_color: data.accent_color || '#253E8A',
          logo_url: data.logo_url || '/branding/logo',
          favicon_url: data.favicon_url || '/branding/favicon',
          upstream_name: 'Signline',
          upstream_url: 'https://github.com/signline/signline',
        };
        setBranding(merged);
        applyBrandingCssVariables(merged.primary_color, merged.accent_color);
        document.title = `${merged.product_name} — Open Source Document Signing`;

        const faviconEl = document.getElementById('app-favicon') as HTMLLinkElement | null;
        if (faviconEl && merged.favicon_url) {
          faviconEl.href = merged.favicon_url;
        }
      }
    } catch {
      // Fallback to default Signline branding
      applyBrandingCssVariables(DEFAULT_BRANDING.primary_color, DEFAULT_BRANDING.accent_color);
    }
  }, []);

  useEffect(() => {
    refreshBranding();
  }, [refreshBranding]);

  const applyColorPreview = useCallback((primaryHex: string, accentHex: string) => {
    applyBrandingCssVariables(primaryHex, accentHex);
  }, []);

  return (
    <BrandingContext.Provider value={{ branding, refreshBranding, applyColorPreview }}>
      {children}
    </BrandingContext.Provider>
  );
};

export function useBranding() {
  return useContext(BrandingContext);
}

/**
 * Mandatory AGPLv3 Section 7(b) Upstream Attribution Component.
 * Cannot be removed or hidden by white-label personalization settings.
 */
export const UpstreamAttribution: React.FC<{ className?: string }> = ({ className = '' }) => {
  const { branding } = useBranding();
  const productName = branding.product_name || 'Signline';

  return (
    <div
      data-testid="upstream-attribution"
      className={`text-xs text-muted-foreground flex flex-wrap items-center justify-center gap-x-3 gap-y-1 ${className}`}
    >
      <span>
        Powered by <strong className="font-semibold text-foreground">{productName}</strong>, based on{' '}
        <a
          href="https://github.com/signline/signline"
          target="_blank"
          rel="noopener noreferrer"
          className="font-medium text-primary underline underline-offset-2 hover: opacity-90"
        >
          Signline
        </a>
      </span>
      {(branding.support_url || branding.support_email || branding.terms_url || branding.privacy_url) && (
        <span className="inline-flex items-center gap-2 text-muted-foreground/80">
          <span>·</span>
          {branding.support_url && (
            <a
              href={branding.support_url}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:underline"
            >
              Support
            </a>
          )}
          {!branding.support_url && branding.support_email && (
            <a href={`mailto:${branding.support_email}`} className="hover:underline">
              {branding.support_email}
            </a>
          )}
          {branding.terms_url && (
            <a
              href={branding.terms_url}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:underline"
            >
              Terms
            </a>
          )}
          {branding.privacy_url && (
            <a
              href={branding.privacy_url}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:underline"
            >
              Privacy
            </a>
          )}
        </span>
      )}
    </div>
  );
};

export const BrandLogo: React.FC<{ className?: string; showName?: boolean }> = ({
  className = 'h-8 w-8',
  showName = true,
}) => {
  const { branding } = useBranding();
  const [imgError, setImgError] = useState(false);

  return (
    <div className="inline-flex items-center gap-2.5 select-none">
      {!imgError && branding.logo_url ? (
        <img
          src={branding.logo_url}
          alt={branding.product_name}
          className={`${className} object-contain`}
          onError={() => setImgError(true)}
        />
      ) : (
        <div
          className={`${className} rounded-lg bg-primary text-primary-foreground flex items-center justify-center font-bold text-lg shadow-sm`}
        >
          {(branding.product_name || 'S').charAt(0).toUpperCase()}
        </div>
      )}
      {showName && (
        <span className="font-bold text-lg tracking-tight text-foreground">
          {branding.product_name || 'Signline'}
        </span>
      )}
    </div>
  );
};
