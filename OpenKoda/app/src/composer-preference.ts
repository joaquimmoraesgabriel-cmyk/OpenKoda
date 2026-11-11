const key = 'openkoda.composer.submit.v1';
export type ComposerShortcut = 'modifier-enter' | 'enter';

export function composerShortcut(): ComposerShortcut {
  try { return localStorage.getItem(key) === 'enter' ? 'enter' : 'modifier-enter'; }
  catch { return 'modifier-enter'; }
}
export function saveComposerShortcut(value: ComposerShortcut) { localStorage.setItem(key,value); }

export function composerSubmitKey(event: Pick<KeyboardEvent,'key'|'repeat'|'shiftKey'|'altKey'|'ctrlKey'|'metaKey'|'isComposing'|'keyCode'>, shortcut: ComposerShortcut) {
  if (event.key !== 'Enter' || event.repeat || event.shiftKey || event.altKey || event.isComposing || event.keyCode === 229) return false;
  return shortcut === 'enter' ? !event.ctrlKey && !event.metaKey : event.ctrlKey || event.metaKey;
}
