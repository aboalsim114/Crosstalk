import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import App from '../../app/App';
import { useApp } from '../../app/state/store';
import { NAV } from '../../app/state/data';
import { useAuth } from '../login/auth';
import { useLocale } from '../../shared/i18n';
import { CATEGORIES, PAGE_CHAPTER, normalize, search, useGuide } from './guide';
import { CHAPTERS, FAQ } from './content';

const app0 = useApp.getState();
const guide0 = useGuide.getState();

describe('guide search', () => {
  it('ignores case and accents', () => {
    expect(normalize('  Écran  PLEIN-écran ')).toBe('ecran plein ecran');
    const ids = (q: string) => search(q, CHAPTERS, FAQ).map((h) => (h.kind === 'chapter' ? h.chapter.id : `faq:${h.faq.q}`));
    expect(ids('GLOSSAIRE')).toEqual(ids('glossaire'));
    expect(ids('reconnaissance vocale')).toContain('audio');
    expect(ids('reconnaissance vocale')).toEqual(ids('Reconnaissance VOCALE'));
  });

  it('needs every word, ranks title matches first and finds FAQ answers', () => {
    const hits = search('overlay raccourcis', CHAPTERS, FAQ);
    expect(hits[0]).toMatchObject({ kind: 'chapter', chapter: { id: 'overlay' } });
    const mouse = search('mouse 4', CHAPTERS, FAQ);
    expect(mouse.some((h) => h.kind === 'faq')).toBe(true);
    expect(mouse.every((h) => h.kind === 'faq' || normalize(h.excerpt).includes('mouse'))).toBe(true);
  });

  it('finds nothing for unknown words or an empty query', () => {
    expect(search('zzzz introuvable', CHAPTERS, FAQ)).toEqual([]);
    expect(search('  ', CHAPTERS, FAQ)).toEqual([]);
  });
});

describe('guide content', () => {
  const pages = NAV.map(([id]) => id);

  it('covers the twelve themes, each one complete', () => {
    expect(CHAPTERS).toHaveLength(12);
    expect(new Set(CHAPTERS.map((c) => c.id)).size).toBe(12);
    for (const c of CHAPTERS) {
      expect(CATEGORIES.map(([id]) => id)).toContain(c.category);
      expect(['implémenté', 'à configurer', 'expérimental']).toContain(c.status);
      expect(c.purpose.length, c.id).toBeGreaterThan(20);
      for (const part of [c.setup, c.use, c.check, c.limits]) expect(part.length, c.id).toBeGreaterThan(0);
      if (c.status === 'expérimental') expect(c.statusNote, c.id).toBeTruthy();
      if (c.settings) expect(pages, c.id).toContain(c.settings.page);
      for (const n of c.shot?.notes ?? []) {
        expect(n.x).toBeGreaterThanOrEqual(0);
        expect(n.x).toBeLessThanOrEqual(100);
        expect(n.y).toBeGreaterThanOrEqual(0);
        expect(n.y).toBeLessThanOrEqual(100);
      }
    }
  });

  it('links each FAQ answer and each screen to an existing chapter', () => {
    const ids = CHAPTERS.map((c) => c.id);
    expect(FAQ.length).toBeGreaterThan(5);
    for (const f of FAQ) expect(ids, f.q).toContain(f.chapter);
    for (const chapter of Object.values(PAGE_CHAPTER)) expect(ids).toContain(chapter);
  });
});

describe('guide page', () => {
  beforeEach(() => {
    useApp.setState({ ...app0, entered: true, showIntro: false, page: 'overlay' }, true);
    useGuide.setState(guide0, true);
    useAuth.setState({ user: 'admin' });
  });
  afterEach(() => {
    useApp.getState().emergency();
    useLocale.setState({ locale: 'fr' });
  });

  it('opens from the menu on its home, with what each status means, and closes back', () => {
    useApp.getState().setPage('lang');
    render(<App />);
    fireEvent.click(screen.getAllByText('Guide')[0]);
    expect(screen.getByTestId('guide-home')).toBeInTheDocument();
    expect(screen.getByText('CHAPITRE 07 / 08')).toBeInTheDocument();
    expect(screen.getByTestId('guide-legend')).toHaveTextContent('pas une garantie de résultat en partie');
    expect(screen.queryByTestId('guide-fr-only')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('← Fermer le guide'));
    expect(useApp.getState().page).toBe('lang');
  });

  it('"? Aide" opens the chapter of the screen, then goes back to it', () => {
    render(<App />);
    fireEvent.click(screen.getByLabelText('Aide sur cet écran (Guide)'));
    const chapter = screen.getByTestId('guide-chapter');
    expect(within(chapter).getByRole('heading', { level: 2 })).toHaveTextContent('Overlay et raccourcis');
    expect(within(chapter).getByTestId('guide-status')).toHaveTextContent(/implémenté/i);
    fireEvent.click(screen.getByText('← Fermer le guide'));
    expect(useApp.getState().page).toBe('overlay');
  });

  it('"Ouvrir l’écran" goes to the screen and names the card to look for', () => {
    useGuide.setState({ chapter: 'chat' });
    useApp.setState({ page: 'guide' });
    render(<App />);
    expect(screen.getByTestId('guide-card')).toHaveTextContent('« CHAT ÉCRIT »');
    fireEvent.click(screen.getByText('Ouvrir l’écran Langues & glossaire →'));
    expect(useApp.getState().page).toBe('lang');
  });

  it('flags experimental features', () => {
    useGuide.setState({ chapter: 'advanced' });
    useApp.setState({ page: 'guide' });
    render(<App />);
    expect(screen.getByTestId('guide-status')).toHaveTextContent(/expérimental/i);
    expect(screen.getByTestId('guide-status-note')).toBeInTheDocument();
  });

  it('searches and opens a result, and shows the FAQ', () => {
    useApp.setState({ page: 'guide' });
    render(<App />);
    const box = screen.getByLabelText('Rechercher dans le guide');
    fireEvent.change(box, { target: { value: 'zzzz' } });
    expect(screen.getByTestId('guide-results')).toHaveTextContent('Aucun résultat');
    fireEvent.change(box, { target: { value: 'glossaire' } });
    fireEvent.click(within(screen.getByTestId('guide-results')).getAllByRole('button')[0]);
    expect(screen.getByTestId('guide-chapter')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Questions fréquentes'));
    expect(screen.getByTestId('guide-faq').querySelectorAll('details')).toHaveLength(FAQ.length);
  });

  it('a FAQ search result opens that answer, and only it', () => {
    useApp.setState({ page: 'guide' });
    render(<App />);
    fireEvent.change(screen.getByLabelText('Rechercher dans le guide'), { target: { value: 'bannir' } });
    const results = screen.getByTestId('guide-results');
    fireEvent.click(within(results).getByText('FAQ — Crosstalk peut-il me faire bannir ?'));
    const faq = screen.getByTestId('guide-faq');
    const open = faq.querySelectorAll('details[open]');
    expect(open).toHaveLength(1);
    expect(open[0].querySelector('summary')).toHaveTextContent('Crosstalk peut-il me faire bannir ?');
    expect(open[0].querySelector('summary')).toHaveFocus();
    // The answer is inside the opened <details>, so it is shown.
    expect(open[0]).toContainElement(within(faq).getByText(/ne pas toucher au processus de CS2/));
  });

  it('in another language: navigation is translated and the guide says it is French only', () => {
    useApp.setState({ uiLang: 'en' });
    render(<App />);
    expect(useLocale.getState().locale).toBe('en');
    fireEvent.click(screen.getByLabelText('Help for this screen (Guide)'));
    expect(screen.getByTestId('guide-fr-only')).toHaveTextContent('only available in French');
    expect(screen.getByTestId('guide-chapter').closest('[lang]')).toHaveAttribute('lang', 'fr');
    fireEvent.click(screen.getByText('← Close the guide'));
    expect(useApp.getState().page).toBe('overlay');
  });
});
