import { useSyncExternalStore } from 'react';
import type { Attachment } from './project-chat';

// Drafts may contain source code or images. They live only in this WebView's memory.
export interface ChatDraft {
  text: string;
  attachment?: Attachment;
  skill?: import('./commercial').Skill;
  proposing: boolean;
}

const empty: ChatDraft = { text: '', proposing: false };
const drafts = new Map<string, ChatDraft>();
// A new conversation's in-memory key becomes its permanent assessment id on admission.
const savedIds = new Map<string, string>();
const listeners = new Set<() => void>();

function subscribe(notify: () => void) {
  listeners.add(notify);
  return () => { listeners.delete(notify); };
}

export function readDraft(id: string) {
  return drafts.get(savedIds.get(id) ?? id) ?? empty;
}

export function useChatDraft(id: string) {
  return useSyncExternalStore(subscribe, () => readDraft(id));
}

export function updateDraft(id: string, patch: Partial<ChatDraft>) {
  id = savedIds.get(id) ?? id;
  const next = { ...readDraft(id), ...patch };
  if (next.text || next.attachment || next.proposing || next.skill) drafts.set(id, next);
  else drafts.delete(id);
  listeners.forEach(notify => notify());
}

export function finishDraft(id: string, sent: ChatDraft) {
  const current = readDraft(id);
  // Another conversation can be open, or the next message already in the composer.
  if (current === sent) updateDraft(id, { ...empty, attachment: undefined, skill:undefined });
  else if (sent.attachment && current.attachment === sent.attachment) updateDraft(id, { attachment: undefined });
}

export function bindChatDraft(draftId: string, assessmentId: string) {
  const draft = drafts.get(draftId);
  if (draft) {
    drafts.set(assessmentId, draft);
    drafts.delete(draftId);
  }
  savedIds.set(draftId, assessmentId);
}

export function pruneChatDrafts(liveAssessmentIds: ReadonlySet<string>) {
  let changed = false;
  const live = new Set(liveAssessmentIds);
  for (const [draftId, id] of savedIds) {
    if (live.has(draftId)) live.add(id);
    else if (!live.has(id)) savedIds.delete(draftId);
  }
  for (const id of drafts.keys()) {
    if (!live.has(id)) changed = drafts.delete(id) || changed;
  }
  if (changed) listeners.forEach(notify => notify());
}

export function conversationTitle(line: string) {
  const words = line.trim().split(/\s+/u);
  const title = words.slice(0, 9).join(' ');
  const letters = Array.from(title);
  return letters.length > 68 || words.length > 9 ? letters.slice(0, 67).join('').trimEnd() + '…' : title;
}
