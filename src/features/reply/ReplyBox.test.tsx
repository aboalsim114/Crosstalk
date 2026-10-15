import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { recentLangs } from '../languages/langs';

const translate = vi.fn();
const voiceStatus = vi.fn(() => Promise.resolve(null as unknown));
const say = vi.fn();
vi.mock('../../shared/ipc', async (orig) => {
  const real = await orig<typeof import('../../shared/ipc')>();
  return {
    ...real,
    reply: {
      ...real.reply,
      phrases: () => Promise.resolve([{ id: 'eco', fr: 'Éco', quick: true }]),
      phraseText: (id: string, target: string) => Promise.resolve(id === 'eco' ? (target === 'ru' ? 'Эко раунд' : 'Eco round') : null),
      translate: (text: string, target: string) => translate(text, target),
    },
    voice: { ...real.voice, status: () => voiceStatus(), say: (text: string, lang: string) => say(text, lang) },
  };
});

const { ReplyBox, PASTE_HINT, teammatesLanguage } = await import('./ReplyBox');

describe('reply in the teammates’ language', () => {
  it('picks the language heard most in the recent real lines', () => {
    const feed = [
      { src: 'ru', tid: 1 },
      { src: 'ru', tid: 2 },
      { src: 'en', tid: 3 },
      { src: 'en' }, // demo line, ignored
      { src: 'en' },
    ];
    expect(recentLangs(feed)).toEqual(['ru', 'ru', 'en']);
    expect(teammatesLanguage(recentLangs(feed))).toBe('ru');
    expect(teammatesLanguage([])).toBe('en');
    // A Russian user replying to English and German teammates.
    expect(teammatesLanguage(['de', 'de', 'en'], ['en', 'de'])).toBe('de');
    expect(teammatesLanguage([], ['de'])).toBe('de');
  });

  it('guesses from the latest lines, not the oldest ones (the feed is newest first)', () => {
    // 20 English lines just heard, then 15 older Russian ones.
    const feed = [...Array.from({ length: 20 }, (_, i) => ({ src: 'en', tid: 100 - i })), ...Array.from({ length: 15 }, (_, i) => ({ src: 'ru', tid: 50 - i }))];
    expect(recentLangs(feed)).toHaveLength(20);
    expect(recentLangs(feed).every((l) => l === 'en')).toBe(true);
    expect(teammatesLanguage(recentLangs(feed))).toBe('en');
  });

  it('translates on Enter, copies the result and says how to paste it', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    translate.mockResolvedValue({ text: 'Они идут через верх', target: 'ru', via: 'llm', ms: 640 });
    const copied = vi.fn();
    render(<ReplyBox target="ru" onTarget={() => {}} onCopied={copied} />);
    const input = screen.getByLabelText('Message (Français)');
    fireEvent.change(input, { target: { value: 'ils arrivent par le haut' } });
    await act(async () => {
      fireEvent.keyDown(input, { key: 'Enter' });
    });
    expect(translate).toHaveBeenCalledWith('ils arrivent par le haut', 'ru');
    expect(writeText).toHaveBeenCalledWith('Они идут через верх');
    expect(screen.getByTestId('reply-out').textContent).toBe('Они идут через верх');
    expect(screen.getByText(`Traduit par le LLM en 0,6 s · ${PASTE_HINT}`)).toBeTruthy();
    expect(copied).toHaveBeenCalled();
  });

  it('quick phrases are copied without the model; errors are shown', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    translate.mockReset();
    translate.mockRejectedValue('modèle français → RU absent (pytorch_model.bin) : téléchargez-le');
    render(<ReplyBox target="ru" onTarget={() => {}} />);
    await act(async () => {});
    await act(async () => {
      fireEvent.click(await screen.findByText('Éco', undefined, { timeout: 2000 }));
    });
    expect(writeText).toHaveBeenCalledWith('Эко раунд');
    const input = screen.getByLabelText('Message (Français)');
    fireEvent.change(input, { target: { value: 'on se regroupe' } });
    await act(async () => {
      fireEvent.keyDown(input, { key: 'Enter' });
    });
    expect(screen.getByText(/téléchargez-le/)).toBeTruthy();
  });

  it('says the translation in the game when the AI voice is ready', async () => {
    voiceStatus.mockResolvedValueOnce({ options: { enabled: true }, routing: true });
    translate.mockResolvedValueOnce({ text: 'Двое на B', via: 'phrasebook', ms: 1 });
    say.mockResolvedValueOnce(1500);
    Object.assign(navigator, { clipboard: { writeText: () => Promise.resolve() } });
    render(<ReplyBox target="ru" onTarget={() => {}} />);
    const box = await screen.findByLabelText(/Message/);
    fireEvent.change(box, { target: { value: 'deux en B' } });
    await act(async () => fireEvent.keyDown(box, { key: 'Enter' }));
    await act(async () => fireEvent.click(await screen.findByText('🔊 Dire en jeu')));
    expect(say).toHaveBeenCalledWith('Двое на B', 'ru');
    expect(await screen.findByText('Dit en jeu par la voix IA (1,5 s)')).toBeInTheDocument();
  });
});
