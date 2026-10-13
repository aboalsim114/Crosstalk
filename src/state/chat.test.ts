import { beforeEach, describe, expect, it } from 'vitest';
import type { ChatMessage, ChatStatus } from '../backend';
import { feedRow, subtitle } from '../pages/derive';
import { chatDiag } from '../pages/DiagPage';
import { overlayBlocks } from '../pages/OverlayPage';
import { chatConfigFrom, chatEntry, cs2SetupSummary } from './chat';
import { useApp, withTranslation } from './store';

const msg = (over: Partial<ChatMessage> = {}): ChatMessage => ({ id: 2 ** 40, channel: 'CT', sender: 'Иван', message: 'Двое на B', location: null, dead: false, lang: 'ru', ...over });
const status = (over: Partial<ChatStatus> = {}, setup: Partial<ChatStatus['setup']> = {}): ChatStatus => ({
  state: 'following',
  setup: {
    steamDir: 'C:\\Program Files (x86)\\Steam',
    libraries: ['C:\\Program Files (x86)\\Steam', 'D:\\SteamLibrary'],
    cs2Dir: 'D:\\SteamLibrary\\steamapps\\common\\Counter-Strike Global Offensive',
    consoleLog: 'D:\\SteamLibrary\\steamapps\\common\\Counter-Strike Global Offensive\\game\\csgo\\console.log',
    consoleLogExists: true,
    condebug: 'present',
    launchOptions: '-condebug',
    manual: false,
    ...setup,
  },
  config: { manualPath: null, ownName: null, ignoreOwn: true },
  rulesVersion: 'cs2-2026.10',
  rulesFile: null,
  rulesWarning: null,
  offset: 1200,
  messages: 3,
  ownIgnored: 0,
  duplicates: 0,
  unrecognized: 0,
  ...over,
});

describe('real chat lines', () => {
  it('wait for their translation, with channel and sender, never shown as French before', () => {
    const e = chatEntry(msg());
    expect(e.status).toBe('transcrit');
    const row = feedRow(e);
    expect(row.tag).toBe('CHAT · ÉQUIPE CT');
    expect(row.langs).toBe('RU · ORIGINAL');
    expect(row.lead).toBe('Иван — ');
    expect(row.note).toMatch(/traduction en cours/);
    expect(subtitle(e, true).tag).toBe('CHAT · ÉQUIPE CT · RUSSE · ORIGINAL');
    expect(overlayBlocks([e], 3)[0].line).toBe('Иван : « Двое на B »');

    const done = withTranslation(e, { id: e.tid!, lang: 'ru', original: 'Двое на B', translated: 'Deux sur B', error: null, ms: 300, queueMs: 10, cached: false, glossary: true });
    expect(done.status).toBe('final');
    expect(feedRow(done).shown).toBe('Deux sur B');
    expect(feedRow(done).orig).toBe('Двое на B');
  });

  it('keeps French as is and says why other lines stay untranslated', () => {
    expect(chatEntry(msg({ lang: 'fr', message: 'bien joué' })).status).toBe('original');
    const unknown = chatEntry(msg({ lang: null, message: 'gg' }));
    expect(unknown.src).toBe('xx');
    expect(feedRow(unknown).note).toBe('Non traduit — langue non reconnue');
    expect(feedRow(chatEntry(msg({ lang: 'zh', message: '你好' }))).note).toBe('Non traduit — langue non gérée (zh)');
    expect(chatEntry(msg({ dead: true })).sender).toBe('Иван (mort)');
  });
});

describe('store', () => {
  const app0 = useApp.getState();
  beforeEach(() => useApp.setState(app0, true));

  it('adds chat lines except in voices-only mode', () => {
    useApp.getState().addChat(chatEntry(msg()));
    expect(useApp.getState().feed).toHaveLength(1);
    useApp.setState({ mode: 'voice', feed: [] });
    useApp.getState().addChat(chatEntry(msg()));
    expect(useApp.getState().feed).toHaveLength(0);
  });

  it('sends the chat settings to Rust', () => {
    expect(chatConfigFrom({ chatPath: '  ', ownName: ' Me ', tg: { ...app0.tg, ignoreOwn: false } })).toEqual({ manualPath: null, ownName: 'Me', ignoreOwn: false });
  });
});

describe('CS2 setup and diagnostic', () => {
  it('guides the player to add -condebug or the CS2 folder', () => {
    expect(cs2SetupSummary(status()).value).toBe('Chat suivi');
    const absent = cs2SetupSummary(status({}, { condebug: 'absent' }));
    expect(absent.value).toBe('-condebug absent');
    expect(absent.action).toMatch(/Options de lancement/);
    const none = cs2SetupSummary(status({ state: 'notfound' }, { consoleLog: null, cs2Dir: null }));
    expect(none.value).toBe('CS2 introuvable');
    expect(none.action).toMatch(/dossier/);
    expect(cs2SetupSummary(status({ state: 'waiting' }, { consoleLogExists: false })).value).toBe('En attente de console.log');
  });

  it('shows the real path, option and rules in Diagnostic', () => {
    const [log, opt, rules] = chatDiag(status());
    expect(log[1]).toContain('D:\\SteamLibrary');
    expect(log[2]).toBe('OK');
    expect(opt[2]).toBe('OK');
    expect(rules[1]).toBe('Règles cs2-2026.10 · intégrées');
    expect(chatDiag(status({}, { condebug: 'absent' }))[1][2]).toBe('ÉCHEC');
    expect(chatDiag(status({}, { consoleLog: null }))[0][2]).toBe('ÉCHEC');
  });
});
