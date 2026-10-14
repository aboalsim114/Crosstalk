import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { recentLangs } from '../languages/langs';

const translate = vi.fn();
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

  it('translates on Enter, copies the result and says how to paste it', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    translate.mockResolvedValue({ text: 'Они идут через верх', target: 'ru', via: 'model', ms: 640 });
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
    expect(screen.getByText(`Traduit sur ce PC en 0,6 s · ${PASTE_HINT}`)).toBeTruthy();
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
});
