import { describe, expect, it } from 'vitest';
// @ts-expect-error plain JavaScript module of the publish script
import { assign, validate } from '../../../scripts/release-notes.mjs';
import file from '../../../release-notes.json';
const entry = { id: 'voix-ia', date: '2026-10-16', title: 'La voix IA dit vos réponses', items: ['Entrée fait parler la voix IA.'] };

describe('release notes for the players', () => {
  it('the published notes are valid and written for players', () => {
    expect(validate(file)).toEqual([]);
  });

  it('refuses developer words, duplicates and bad fields', () => {
    expect(validate([{ ...entry, title: 'feat(voice): voix écrite (réf. #103)' }]).join()).toMatch(/vocabulaire de développeur/);
    expect(validate([{ ...entry, items: ['Voir le commit sur GitHub.'] }]).join()).toMatch(/vocabulaire de développeur/);
    expect(validate([entry, entry]).join()).toMatch(/en double/);
    // Whole words only: « présente », « préfixe », « commitment » are fine.
    expect(validate([{ ...entry, items: ['Une fenêtre présente les nouveautés, avec un préfixe.'] }])).toEqual([]);
    expect(validate([{ ...entry, items: ['Voir la PR 12.'] }]).join()).toMatch(/vocabulaire de développeur/);
    expect(validate([{ ...entry, date: '16/10/2026', items: [] }])).toHaveLength(2);
  });

  it('keeps the build that first shipped each entry, newest first', () => {
    const later = { ...entry, id: 'nouvelle-carte', title: 'Nouvelle carte' };
    const out = assign([later, entry], 280, [{ id: 'voix-ia', build: 276 }]);
    expect(out.news.map((n: { build: number }) => n.build)).toEqual([280, 276]);
    expect(out.notes).toEqual(later.items);
    expect(assign([{ ...entry, build: 250 }], 280, []).news[0].build).toBe(250);
    expect(assign([], 280, 'oops').notes).toEqual([]);
  });
});
