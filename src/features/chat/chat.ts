// Written chat of CS2, read by the Rust core from console.log (src-tauri/src/chat).

import { t } from '../../shared/i18n';
import { create } from 'zustand';
import { isTauri, type ChatConfig, type ChatMessage, type ChatStatus, type Cs2Setup } from '../../shared/ipc';
import type { Lang } from '../../app/state/data';
import type { AppState, FeedEntry } from '../../app/state/store';
import { useTargetLang } from '../../shared/i18n';
import { isPlayerLang } from '../languages/langs';

export interface ChatStore {
  native: boolean;
  status: ChatStatus | null;
}

export const useChat = create<ChatStore>()(() => ({
  native: isTauri,
  status: null,
}));

export function chatConfigFrom(s: Pick<AppState, 'chatPath' | 'ownName' | 'tg'>): ChatConfig {
  return {
    manualPath: s.chatPath.trim() || null,
    ownName: s.ownName.trim() || null,
    ignoreOwn: s.tg.ignoreOwn,
  };
}

/** Why a chat line is shown untranslated, when it is. */
export function chatUntranslated(lang: string | null): string | null {
  if (isPlayerLang(lang)) return null;
  return lang ? `langue non gérée (${lang})` : 'langue non reconnue';
}

/** A real chat message as a Script / overlay line: waits for its translation. */
export function chatEntry(m: ChatMessage, now = new Date(), target = useTargetLang.getState().target): FeedEntry {
  const src: Lang = isPlayerLang(m.lang) ? m.lang : 'xx';
  const note = chatUntranslated(m.lang);
  const base = {
    kind: 'chat' as const,
    ch: m.channel,
    sender: m.dead ? `${m.sender} (mort)` : m.sender,
    src,
    orig: m.message,
    tr: null,
    id: m.id + Math.random() / 10,
    tid: m.id,
    time: now.toLocaleTimeString('fr-FR'),
    shown: null,
    lat: null,
    err: false,
  };
  // Already in the user's language: shown as is.
  if (src === target) return { ...base, status: 'original', lat: 0 };
  return { ...base, status: 'transcrit', ...(note ? { note } : {}) };
}

export interface Summary {
  value: string;
  detail: string;
  color: string;
}

/** The console.log card: where CS2 is, whether -condebug is set, what to do. */
export function cs2SetupSummary(st: ChatStatus | null): Summary & { action: string | null } {
  if (!st)
    return {
      value: '…',
      detail: t('chat.searching'),
      color: '#8f8a83',
      action: null,
    };
  const s: Cs2Setup = st.setup;
  if (!s.consoleLog)
    return {
      value: t('chat.notFound'),
      detail: s.steamDir ? t('chat.steamFound', { count: s.libraries.length }) : t('chat.noSteam'),
      color: '#ff8a7a',
      action: t('chat.pickFolder'),
    };
  if (s.condebug === 'absent')
    return {
      value: t('chat.noCondebug'),
      detail: s.consoleLog,
      color: '#ffa24a',
      action: t('chat.addCondebug'),
    };
  if (st.state === 'waiting' || !s.consoleLogExists)
    return {
      value: t('chat.waitingLog'),
      detail: s.consoleLog,
      color: '#ffa24a',
      action: s.condebug === 'unknown' ? t('chat.launchCondebug') : t('chat.launch'),
    };
  return {
    value: st.state === 'following' ? t('chat.following') : s.condebug === 'present' ? t('chat.ready') : t('chat.found'),
    detail: s.consoleLog,
    color: '#7fe0a8',
    action: s.condebug === 'unknown' ? t('chat.optionsUnreadable') : null,
  };
}

export function chatCounts(st: ChatStatus | null): string {
  if (!st) return '';
  const parts = [`${st.messages} message${st.messages > 1 ? 's' : ''}`];
  if (st.ownIgnored) parts.push(`${st.ownIgnored} des vôtres ignoré${st.ownIgnored > 1 ? 's' : ''}`);
  if (st.unrecognized) parts.push(`${st.unrecognized} non reconnue${st.unrecognized > 1 ? 's' : ''} (voir Diagnostic)`);
  if (st.duplicates) parts.push(`${st.duplicates} doublon${st.duplicates > 1 ? 's' : ''}`);
  return parts.join(' · ');
}
