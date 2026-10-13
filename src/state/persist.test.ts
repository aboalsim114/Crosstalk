import { beforeEach, describe, expect, it } from 'vitest';
import { useAudio } from './audio';
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

  it('round-trips the settings, the device and the glossary', () => {
    const s = { ...app0, model: 'small' as const, ownName: 'Иван', fmt: 'minimal' as const, opacity: 40, tg: { ...app0.tg, ignoreOwn: false } };
    const snap = snapshot(s, 'dev-1');
    expect(snap.config.version).toBe(SETTINGS_VERSION);
    const back = restore(JSON.parse(JSON.stringify(snap.config)), app0);
    expect(back.app).toMatchObject({ model: 'small', ownName: 'Иван', fmt: 'minimal', opacity: 40 });
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
    expect(restore({ model: 'small' }, app0).app).toEqual({ model: 'small' });
    expect(restoreGlossary({ version: 1, terms: [['CT / T', 'Conserver', 'CT / T'], ['eco', 'Traduire', 'éco'], ['', 'Conserver', ''], ['x', 'Bad', 'y']] })).toEqual([
      ['CT', 'Conserver', 'CT'],
      ['T', 'Conserver', 'T'],
      ['eco', 'Traduire', 'éco'],
    ]);
  });

  it('saves changes and loads them back at the next launch (browser preview)', async () => {
    const stop = startSettingsSync(0);
    useApp.setState({ model: 'small', ownName: 'Me', gloss: [['rush B', 'Conserver', 'rush B']] });
    await new Promise((r) => setTimeout(r, 20));
    stop();
    useApp.setState(app0, true);
    expect(useApp.getState().model).toBe('base');
    await loadSettings();
    expect(useApp.getState().model).toBe('small');
    expect(useApp.getState().ownName).toBe('Me');
    expect(useApp.getState().gloss).toEqual([['rush B', 'Conserver', 'rush B']]);
  });

  it('turns the old simulated "cuda" choice into the real GPU setting', () => {
    expect(restore({ version: 1, settings: { accel: 'cuda' } }, useApp.getState()).app.accel).toBe('gpu');
    expect(restore({ version: 1, settings: { accel: 'cpu' } }, useApp.getState()).app.accel).toBe('cpu');
    expect(restore({ version: 1, settings: { accel: 'tpu' } }, useApp.getState()).app.accel).toBeUndefined();
  });
});
