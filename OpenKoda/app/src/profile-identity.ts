import { useMemo, useSyncExternalStore } from 'react';

export const avatarColors = { azul: 'Azul', verde: 'Verde', ameixa: 'Ameixa', cobre: 'Cobre' };
export type Identity = { name: string; color: keyof typeof avatarColors };
const defaultIdentity: Identity = {name: '', color: 'azul'};
const key = 'openkoda.profile-display.v1';
const changeEvent = 'openkoda:profile-display';

function read() {
  try { return localStorage.getItem(key); } catch { return null; }
}

function decode(raw: string | null): Identity {
  try {
    const value = JSON.parse(raw ?? 'null');
    if (typeof value?.name === 'string' && Object.hasOwn(avatarColors, value.color)) {
      return {name: value.name.trim().slice(0, 60), color: value.color};
    }
  } catch { /* An unavailable or malformed device profile uses the defaults. */ }
  return defaultIdentity;
}

function subscribe(callback: () => void) {
  const onStorage = (event: StorageEvent) => { if (!event.key || event.key === key) callback(); };
  window.addEventListener('storage', onStorage);
  window.addEventListener(changeEvent, callback);
  return () => {
    window.removeEventListener('storage', onStorage);
    window.removeEventListener(changeEvent, callback);
  };
}

export function useIdentity() {
  const raw = useSyncExternalStore(subscribe, read);
  return useMemo(() => decode(raw), [raw]);
}

export function saveIdentity(changes: Partial<Identity>) {
  const next = {...decode(read()), ...changes};
  next.name = next.name.trim().slice(0, 60);
  localStorage.setItem(key, JSON.stringify(next));
  window.dispatchEvent(new Event(changeEvent));
}
