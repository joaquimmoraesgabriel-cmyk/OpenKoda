import { afterEach, expect, it, vi } from 'vitest';
import { composerShortcut, composerSubmitKey, matchesComposerShortcut, saveComposerShortcut } from './composer-preference';

afterEach(() => vi.unstubAllGlobals());

it('usa Enter no primeiro acesso e mantém o atalho que a pessoa escolheu', () => {
  const store = new Map<string, string>();
  vi.stubGlobal('localStorage', { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value) });
  expect(composerShortcut()).toBe('enter');
  saveComposerShortcut('modifier-enter');
  expect(composerShortcut()).toBe('modifier-enter');
  saveComposerShortcut('enter');
  expect(composerShortcut()).toBe('enter');
});

it('usa Enter quando o armazenamento está indisponível', () => {
  vi.stubGlobal('localStorage', { getItem() { throw new Error('indisponível'); } });
  expect(composerShortcut()).toBe('enter');
});

it('consome a repetição do atalho sem solicitar outro envio', () => {
  const key = { key: 'Enter', repeat: true, shiftKey: false, altKey: false, metaKey: false, ctrlKey: false, isComposing: false, keyCode: 13 };
  expect(matchesComposerShortcut(key, 'enter')).toBe(true);
  expect(composerSubmitKey(key, 'enter')).toBe(false);
  expect(matchesComposerShortcut({...key, shiftKey: true}, 'enter')).toBe(false);
  expect(matchesComposerShortcut({...key, isComposing: true}, 'enter')).toBe(false);
  expect(matchesComposerShortcut(key, 'modifier-enter')).toBe(false);
  expect(matchesComposerShortcut({...key, ctrlKey: true}, 'modifier-enter')).toBe(true);
});
