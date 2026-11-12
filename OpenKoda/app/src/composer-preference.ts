const key = 'openkoda.composer.submit.v1';
export type ComposerShortcut = 'modifier-enter' | 'enter';

export function composerShortcut(): ComposerShortcut {
  try { return localStorage.getItem(key) === 'modifier-enter' ? 'modifier-enter' : 'enter'; }
  catch { return 'enter'; }
}
export function saveComposerShortcut(value: ComposerShortcut) { localStorage.setItem(key,value); }

type ComposerKey = Pick<KeyboardEvent,'key'|'repeat'|'shiftKey'|'altKey'|'ctrlKey'|'metaKey'|'isComposing'|'keyCode'>;

export function matchesComposerShortcut(event: ComposerKey, shortcut: ComposerShortcut) {
  if (event.key !== 'Enter' || event.shiftKey || event.altKey || event.isComposing || event.keyCode === 229) return false;
  return shortcut === 'enter' ? !event.ctrlKey && !event.metaKey : event.ctrlKey || event.metaKey;
}

export function composerSubmitKey(event: ComposerKey, shortcut: ComposerShortcut) {
  return !event.repeat && matchesComposerShortcut(event, shortcut);
}
