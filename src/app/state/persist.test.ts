import { beforeEach, describe, expect, it } from 'vitest';
import { useAudio } from '../../features/audio/audio';
import { GLOSSARY } from './data';
import { loadSettings, restore, restoreGlossary, snapshot, startSettingsSync, SETTINGS_VERSION } from './persist';
import { useApp } from './store';

const app0 = useApp.getState();
const audio0 = useAudio.getState();

describe('settings kept between launches', () => {
  beforeEach(() => {
    useApp.setState(app0, true);
    useAudio.setState(audio0, true);
    window.localStorage.clear();
  });

  it('keeps "Focus voix"', () => {
    expect(restore(snapshot({ ...app0, voiceFocus: true }, null).config, app0).app.voiceFocus).toBe(true);
    expect(restore({ version: 1, settings: { voiceFocus: 'yes' } }, app0).app.voiceFocus).toBeUndefined();
  });

  it('keeps the "Qui parle ?" options, with the area bounded to the window', () => {
    const snap = snapshot({ ...app0, speaker: { enabled: true, region: { x: 0.1, y: 0.6, w: 0.2, h: 0.25 } } }, null);
    expect(restore(snap.config, app0).app.speaker).toEqual({ enabled: true, region: { x: 0.1, y: 0.6, w: 0.2, h: 0.25 } });
    const bad = restore({ version: 1, settings: { speaker: { enabled: 'yes', region: { x: 4, y: 0.7, w: null } } } }, app0).app.speaker;
    expect(bad).toEqual({ enabled: false, region: { ...app0.speaker.region, y: 0.7 } });
    expect(app0.speaker.enabled).toBe(false);
  });

  it('keeps the translation engine settings, not connected by default, without any key', () => {
    expect(app0.llm.enabled).toBe(false);
    const llm = { enabled: true, service: 'anthropic' as const, provider: 'anthropic' as const, baseUrl: 'https://api.anthropic.com/v1', model: 'claude-haiku-4-5', cloudConsent: true };
    const snap = snapshot({ ...app0, llm }, null);
    expect(restore(JSON.parse(JSON.stringify(snap.config)), app0).app.llm).toEqual(llm);
    // Settings saved before the service list: the service is found from the address.
    const old = (baseUrl: string) => restore({ version: 1, settings: { llm: { enabled: true, provider: 'openai', baseUrl, model: 'm' } } }, app0).app.llm?.service;
    expect(old('https://ollama.com/v1')).toBe('ollama-cloud');
    expect(old('http://localhost:11434/v1/')).toBe('ollama');
    expect(old('https://llm.example.org/v1')).toBe('custom');
    expect(restore({ version: 1, settings: { llm: { ...llm, service: 'skynet' } } }, app0).app.llm?.service).toBe('anthropic');
    // No API key in the file (the AI voice's push-to-talk key name aside).
    expect(JSON.stringify(snap).replace(/"pttKey":"[^"]*"/, '')).not.toMatch(/key/i);
    const bad = restore({ version: 1, settings: { llm: { enabled: 'yes', provider: 'gemini', baseUrl: 42, model: 'x'.repeat(600) } } }, app0).app.llm;
    expect(bad).toEqual(app0.llm);
  });

  it('keeps the consent of a cloud provider connected before it existed, never for one never switched on', () => {
    const old = (llm: Record<string, unknown>) => restore({ version: 1, settings: { llm } }, app0).app.llm?.cloudConsent;
    expect(old({ enabled: true, provider: 'openai', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini' })).toBe(true);
    expect(old({ enabled: false, provider: 'openai', baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini' })).toBe(false);
    expect(old({ enabled: true, provider: 'openai', baseUrl: 'http://localhost:11434/v1', model: 'qwen2.5:7b' })).toBe(false);
    expect(old({ enabled: true, provider: 'openai', baseUrl: 'https://api.openai.com/v1', model: 'm', cloudConsent: false })).toBe(false);
  });

  it('asks for the engine once, and keeps the answer', () => {
    expect(app0.engineSetupDone).toBe(false);
    expect(restore(snapshot({ ...app0, engineSetupDone: true }, null).config, app0).app.engineSetupDone).toBe(true);
  });

  it('round-trips the settings, the device and the glossary', () => {
    const s = { ...app0, model: 'parakeet-tdt-0.6b-v3' as const, ownName: 'Иван', fmt: 'minimal' as const, opacity: 40, tg: { ...app0.tg, ignoreOwn: false } };
    const snap = snapshot(s, 'dev-1');
    expect(snap.config.version).toBe(SETTINGS_VERSION);
    const back = restore(JSON.parse(JSON.stringify(snap.config)), app0);
    expect(back.app).toMatchObject({ model: 'parakeet-tdt-0.6b-v3', ownName: 'Иван', fmt: 'minimal', opacity: 40 });
    expect(back.app.tg?.ignoreOwn).toBe(false);
    expect(back.deviceId).toBe('dev-1');
    expect(restoreGlossary(snap.glossary)).toEqual(GLOSSARY);
  });

  it('ignores invalid or unknown values and files from a newer version', () => {
    const back = restore({ version: 1, settings: { model: 'huge', opacity: 400, fmt: 'bilingue', tg: { ignoreOwn: 'yes', translate: false, bogus: true }, evil: 1 } }, app0);
    expect(back.app).toEqual({ fmt: 'bilingue', tg: { ...app0.tg, translate: false } });
    expect(restore({ version: 99, settings: { model: 'small' } }, app0).app).toEqual({});
    expect(restore('garbage', app0).app).toEqual({});
  });

  it('migrates version 0 files and the old "CT / T" glossary entry', () => {
    expect(restore({ model: 'parakeet-tdt-0.6b-v3' }, app0).app).toEqual({ model: 'parakeet-tdt-0.6b-v3' });
    expect(restoreGlossary({ version: 1, terms: [['CT / T', 'Conserver', 'CT / T'], ['eco', 'Traduire', 'éco'], ['', 'Conserver', ''], ['x', 'Bad', 'y']] })).toEqual([
      ['CT', 'Conserver', 'CT'],
      ['T', 'Conserver', 'T'],
      ['eco', 'Traduire', 'éco'],
    ]);
  });

  it('saves changes and loads them back at the next launch (browser preview)', async () => {
    const stop = startSettingsSync(0);
    useApp.setState({ ownName: 'Me', vad: 70, gloss: [['rush B', 'Conserver', 'rush B']] });
    await new Promise((r) => setTimeout(r, 20));
    stop();
    useApp.setState(app0, true);
    expect(useApp.getState().vad).toBe(55);
    await loadSettings();
    expect(useApp.getState().vad).toBe(70);
    expect(useApp.getState().ownName).toBe('Me');
    expect(useApp.getState().gloss).toEqual([['rush B', 'Conserver', 'rush B']]);
  });

  it('drops the old Whisper settings: the new recognition model applies', () => {
    for (const model of ['tiny', 'base', 'small', 'medium', 'large-v3-turbo']) {
      const back = restore({ version: 1, settings: { model, accel: 'gpu', opacity: 40 } }, useApp.getState()).app;
      expect(back).toEqual({ opacity: 40 });
    }
    expect(app0.model).toBe('parakeet-tdt-0.6b-v3');
  });
});
