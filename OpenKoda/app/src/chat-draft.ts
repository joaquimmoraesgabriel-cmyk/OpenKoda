import { useSyncExternalStore } from 'react';
import type { Attachment } from './project-chat';

// Drafts may contain source code or images. They live only in this WebView's memory.
export interface ChatDraft {
  text: string;
  attachment?: Attachment;
  proposing: boolean;
}

const empty: ChatDraft = { text: '', proposing: false };
const drafts = new Map<string, ChatDraft>();
const listeners = new Set<() => void>();

function subscribe(notify: () => void) {
  listeners.add(notify);
  return () => { listeners.delete(notify); };
}

export function readDraft(id: string) {
  return drafts.get(id) ?? empty;
}

export function useChatDraft(id: string) {
  return useSyncExternalStore(subscribe, () => readDraft(id));
}

export function updateDraft(id: string, patch: Partial<ChatDraft>) {
  const next = { ...readDraft(id), ...patch };
  if (next.text || next.attachment || next.proposing) drafts.set(id, next);
  else drafts.delete(id);
  listeners.forEach(notify => notify());
}

export function finishDraft(id: string, sent: ChatDraft) {
  const current = readDraft(id);
  // Another conversation can be open, or the next message already in the composer.
  if (current === sent) updateDraft(id, { ...empty, attachment: undefined });
  else if (sent.attachment && current.attachment === sent.attachment) updateDraft(id, { attachment: undefined });
}

export function pruneChatDrafts(liveAssessmentIds: ReadonlySet<string>) {
  let changed = false;
  for (const id of drafts.keys()) {
    if (!liveAssessmentIds.has(id)) changed = drafts.delete(id) || changed;
  }
  if (changed) listeners.forEach(notify => notify());
}
