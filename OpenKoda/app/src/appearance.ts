import { useEffect, useMemo, useSyncExternalStore } from 'react';

export interface DeviceAppearance {
  interfaceScale: number;
  density: 'comfortable' | 'compact';
  motion: 'system' | 'reduced';
  uiFont: string;
  uiFontSize: number;
  codeFont: string;
  codeFontSize: number;
  palette: 'neutral' | 'slate' | 'sand';
  wallpaper: 'none' | 'mist' | 'dunes' | 'custom';
  wallpaperImage: string;
  wallpaperOpacity: number;
  wallpaperMotion: 'static' | 'dynamic';
  listStyle: 'preview' | 'line';
  sidebarStyle: 'auto' | 'icons';
}

export const interfaceScales = [100, 110, 125];
export const appearanceStorageKey = 'openkoda.appearance.v1';
const changeEvent = 'openkoda:appearance';
export const defaultAppearance: DeviceAppearance = {
  interfaceScale: 100, density: 'comfortable', motion: 'system', uiFont: '', uiFontSize: 16,
  codeFont: '', codeFontSize: 12, palette: 'neutral', wallpaper: 'none', wallpaperImage: '',
  wallpaperOpacity: 12, wallpaperMotion: 'static', listStyle: 'preview', sidebarStyle: 'auto',
};
export const appearanceKeys = Object.keys(defaultAppearance) as (keyof DeviceAppearance)[];
export function appearanceFrom(draft: DeviceAppearance): DeviceAppearance {
  return Object.fromEntries(appearanceKeys.map(key => [key, draft[key]])) as unknown as DeviceAppearance;
}
export function validFontFamily(name: string) {
  return /^[\p{L}\p{N} _.\-]{0,70}$/u.test(name);
}
function bounded(value: unknown, min: number, max: number, fallback: number) {
  return typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max ? value : fallback;
}

export function parseAppearance(raw: string | null): DeviceAppearance {
  if (!raw) return defaultAppearance;
  try {
    const value = JSON.parse(raw);
    const image = typeof value.wallpaperImage === 'string' && value.wallpaperImage.length <= 470_000 &&
      /^data:image\/webp;base64,[A-Za-z0-9+/]+=*$/.test(value.wallpaperImage) ? value.wallpaperImage : '';
    return {
      interfaceScale: interfaceScales.includes(value.interfaceScale) ? value.interfaceScale : defaultAppearance.interfaceScale,
      density: value.density === 'compact' ? 'compact' : 'comfortable',
      motion: value.motion === 'reduced' ? 'reduced' : 'system',
      uiFont: typeof value.uiFont === 'string' && validFontFamily(value.uiFont) ? value.uiFont.trim() : '',
      uiFontSize: bounded(value.uiFontSize, 12, 24, 16),
      codeFont: typeof value.codeFont === 'string' && validFontFamily(value.codeFont) ? value.codeFont.trim() : '',
      codeFontSize: bounded(value.codeFontSize, 10, 24, 12),
      palette: value.palette === 'slate' || value.palette === 'sand' ? value.palette : 'neutral',
      wallpaper: value.wallpaper === 'mist' || value.wallpaper === 'dunes' || (value.wallpaper === 'custom' && image) ? value.wallpaper : 'none',
      wallpaperImage: image,
      wallpaperOpacity: bounded(value.wallpaperOpacity, 0, 20, 12),
      wallpaperMotion: value.wallpaperMotion === 'dynamic' ? 'dynamic' : 'static',
      listStyle: value.listStyle === 'line' ? 'line' : 'preview',
      sidebarStyle: value.sidebarStyle === 'icons' ? 'icons' : 'auto',
    };
  } catch {
    return defaultAppearance;
  }
}

function storedAppearance() {
  try { return localStorage.getItem(appearanceStorageKey); }
  catch { return null; }
}

function subscribe(callback: () => void) {
  const onStorage = (event: StorageEvent) => {
    if (!event.key || event.key === appearanceStorageKey) callback();
  };
  window.addEventListener('storage', onStorage);
  window.addEventListener(changeEvent, callback);
  return () => {
    window.removeEventListener('storage', onStorage);
    window.removeEventListener(changeEvent, callback);
  };
}

export function useDeviceAppearance() {
  const raw = useSyncExternalStore(subscribe, storedAppearance);
  return useMemo(() => parseAppearance(raw), [raw]);
}

export function useApplyAppearance(theme?: string) {
  const appearance = useDeviceAppearance();
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = theme ?? 'light';
    root.dataset.density = appearance.density;
    root.dataset.motion = appearance.motion;
    root.dataset.palette = appearance.palette;
    root.dataset.wallpaper = appearance.wallpaper;
    root.dataset.wallpaperMotion = appearance.wallpaperMotion;
    root.dataset.listStyle = appearance.listStyle;
    root.dataset.sidebarStyle = appearance.sidebarStyle;
    root.style.setProperty('--interface-scale', String(appearance.interfaceScale / 100));
    root.style.fontSize = appearance.uiFontSize + 'px';
    root.style.fontFamily = appearance.uiFont ? '"' + appearance.uiFont + '", system-ui, sans-serif' : '';
    root.style.setProperty('--code-font', appearance.codeFont ? '"' + appearance.codeFont + '", monospace' : 'Consolas, "Cascadia Code", monospace');
    root.style.setProperty('--code-font-size', appearance.codeFontSize + 'px');
    root.style.setProperty('--wallpaper-strength', appearance.wallpaperOpacity + '%');
    if (appearance.wallpaper === 'custom') root.style.setProperty('--workspace-backdrop', 'url("' + appearance.wallpaperImage + '")');
    else root.style.removeProperty('--workspace-backdrop');
  }, [theme, appearance]);
}

/** The SQLite theme remains compatible with older releases. Device-only choices
 * are committed to this WebView; a rejected native save restores the prior value. */
export async function saveAppearance(next: DeviceAppearance, saveTheme?: () => Promise<unknown>) {
  const raw = JSON.stringify(next);
  let previous: string | null;
  try {
    previous = localStorage.getItem(appearanceStorageKey);
    localStorage.setItem(appearanceStorageKey, raw);
  } catch {
    throw new Error('O dispositivo não permitiu salvar as preferências de aparência.');
  }
  try {
    await saveTheme?.();
  } catch (error) {
    try {
      if (localStorage.getItem(appearanceStorageKey) === raw) {
        if (previous === null) localStorage.removeItem(appearanceStorageKey);
        else localStorage.setItem(appearanceStorageKey, previous);
      }
    } catch {
      throw new Error('O tema não foi salvo e o dispositivo impediu a reposição das preferências visuais. Confira as opções antes de tentar novamente.');
    }
    throw error;
  } finally {
    window.dispatchEvent(new Event(changeEvent));
  }
}
