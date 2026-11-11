import { useEffect, useMemo, useSyncExternalStore } from 'react';

export interface DeviceAppearance {
  interfaceScale: number;
  density: 'comfortable' | 'compact';
  motion: 'system' | 'reduced';
}

export const interfaceScales = [100, 110, 125];
export const appearanceStorageKey = 'openkoda.appearance.v1';
const changeEvent = 'openkoda:appearance';
export const defaultAppearance: DeviceAppearance = {interfaceScale: 100, density: 'comfortable', motion: 'system'};

function parseAppearance(raw: string | null): DeviceAppearance {
  if (!raw) return defaultAppearance;
  try {
    const value = JSON.parse(raw);
    return {
      interfaceScale: interfaceScales.includes(value.interfaceScale) ? value.interfaceScale : defaultAppearance.interfaceScale,
      density: value.density === 'compact' ? 'compact' : 'comfortable',
      motion: value.motion === 'reduced' ? 'reduced' : 'system',
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
  const { interfaceScale, density, motion } = useDeviceAppearance();
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = theme ?? 'light';
    root.dataset.density = density;
    root.dataset.motion = motion;
    root.style.setProperty('--interface-scale', String(interfaceScale / 100));
  }, [theme, interfaceScale, density, motion]);
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
