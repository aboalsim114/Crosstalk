import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { IDLE_CAPTURE } from '../backend';
import { feedRow, latencyAdvice, subtitle } from '../pages/derive';
import { Dashboard } from '../pages/Dashboard';
import { overlayBlocks } from '../pages/OverlayPage';
import { streamSummary } from '../pages/StreamCard';
import { captureSummary, cs2Summary } from '../state/audio';
import { translationSummary } from '../state/translation';
import type { FeedEntry } from '../state/store';
import { useApp } from '../state/store';
import { useLocale } from './index';
import { overlayPayload } from '../native';
import { OverlayWindow } from '../overlay/OverlayWindow';

const line = (p: Partial<FeedEntry>): FeedEntry => ({ kind: 'voice', src: 'ru', orig: 'Двое на Б', prov: '', tr: null, id: 1, tid: 1, time: '', status: 'final', shown: 'Deux sur B.', lat: 1800, err: false, ...p });
const app0 = useApp.getState();

describe('Home and Overlay in other languages', () => {
  beforeEach(() => useApp.setState(app0, true));
  afterEach(() => useLocale.setState({ locale: 'fr' }));

  it('labels lines in the interface language, subtitles still French with local quotes', () => {
    useLocale.setState({ locale: 'en' });
    const sub = subtitle(line({}), true);
    expect(sub.tag).toBe('VOICE · RUSSIAN → FRENCH');
    expect(sub.text).toBe('“Deux sur B.”');
    expect(feedRow(line({ kind: 'chat', ch: 'CT', status: 'transcrit', shown: null, src: 'en' })).tag).toBe('CHAT · CT TEAM');
    expect(feedRow(line({ status: 'original', shown: null })).note).toBe('Already in your language — not translated');
    expect(latencyAdvice(5000, 'small')).toMatch(/^Too slow for live use with whisper-small/);
    useLocale.setState({ locale: 'de' });
    expect(subtitle(undefined, false).text).toBe('Stille, vorerst.');
    expect(subtitle(line({}), true).text).toBe('„Deux sur B.“');
  });

  it('builds the overlay blocks in Russian', () => {
    useLocale.setState({ locale: 'ru' });
    const [voice, chat] = overlayBlocks([], 3);
    expect(voice.head).toBe('ГОЛОС • РУССКИЙ → ФРАНЦУЗСКИЙ');
    expect(chat.head).toBe('ЧАТ • КОМАНДА CT • АНГЛИЙСКИЙ → ФРАНЦУЗСКИЙ');
    expect(chat.line).toBe('Player123 : «Je vais poser une smoke au mid.»');
  });

  it('translates status texts, with plurals', () => {
    useLocale.setState({ locale: 'de' });
    expect(captureSummary(IDLE_CAPTURE).value).toBe('Inaktiv');
    expect(cs2Summary({ running: true, pid: 42 } as never).detail).toBe('cs2.exe · PID 42');
    expect(cs2Summary({ running: false, pid: null } as never).detail).toBe('Starte Counter-Strike 2');
    expect(translationSummary(null).label).toBe('Übersetzung');
    useLocale.setState({ locale: 'ru' });
    const st = (sent: number) => streamSummary({ listening: true, clients: 1, sent, error: null } as never, true).detail;
    expect([st(1), st(3), st(5)]).toEqual(['С запуска отправлен 1 субтитр.', 'С запуска отправлено 3 субтитра.', 'С запуска отправлено 5 субтитров.']);
  });

  it('sends the language to the in-game overlay window, which uses it', async () => {
    const payload = overlayPayload({ ...useApp.getState(), uiLang: 'ru', repos: true, fmt: 'bilingue' });
    expect(payload.locale).toBe('ru');
    render(<OverlayWindow initial={payload} />);
    expect(await screen.findByText('ГОЛОС • РУССКИЙ → ФРАНЦУЗСКИЙ')).toBeTruthy();
    expect(useLocale.getState().locale).toBe('ru');
  });

  it('renders the Home page in English', () => {
    useLocale.setState({ locale: 'en' });
    render(<Dashboard />);
    expect(screen.getByText('Nothing on screen.')).toBeTruthy();
    expect(screen.getByText('Silence, for now.')).toBeTruthy();
    expect(screen.getByText('VOICE LATENCY')).toBeTruthy();
    expect(screen.getByText('No voice sent. No audio kept. Mic off.')).toBeTruthy();
  });
});
