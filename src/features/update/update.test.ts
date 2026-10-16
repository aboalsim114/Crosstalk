import { describe, expect, it } from 'vitest';
import type { UpdateStatus } from '../../shared/ipc';
import { UNAVAILABLE, formatDate, friendlyError, installedNews, newsSince, upcomingNews, updateSummary, updateView, versionLabel } from './update';

const news = [
  { build: 95, date: '2026-10-16', title: 'La voix IA dit vos réponses écrites', items: ['Entrée fait parler la voix IA.'] },
  { build: 94, date: '2026-10-15', title: 'Sous-titres plus fluides', items: ['Modèle plus léger quand le PC est chargé.'] },
  { build: 90, date: '2026-10-14', title: 'Traduction par l’IA de votre choix', items: ['Votre clé, ou Ollama.'] },
];
const latest = { build: 95, version: '0.2.0', commit: 'abc1234', date: '2026-10-16 01:35 UTC', notes: ['feat: x'], news, setup: { url: 'https://raw.githubusercontent.com/x', sha256: 'a'.repeat(64), bytes: 15_000_000 } };
const st = (over: Partial<UpdateStatus>): UpdateStatus => ({ state: 'idle', build: 92, version: '0.2.0', commit: 'f2be466', latest: null, doneBytes: 0, totalBytes: 0, error: null, ...over });

/** Nothing a player should not see: commit, build hash, checksum, repository, pull request words. */
const DEV = /abc1234|f2be466|SHA|github|feat|fix|build \d|#\d/i;

describe('in-app update, for players', () => {
  it('offers the new version with what it brings, until dismissed', () => {
    const v = updateView(st({ state: 'available', latest }), null, false);
    expect(v).toMatchObject({ banner: true, action: 'download', title: 'Nouvelle version disponible' });
    expect(v.detail).toBe('La voix IA dit vos réponses écrites · Sous-titres plus fluides');
    expect(`${v.title} ${v.detail}`).not.toMatch(DEV);
    expect(updateView(st({ state: 'available', latest }), 95, false).banner).toBe(false);
    expect(updateView(st({ state: 'uptodate', latest: { ...latest, build: 92 } }), null, false).banner).toBe(false);
    // An older feed without news: the date, never the developers' notes.
    const old = updateView(st({ state: 'available', latest: { ...latest, news: undefined } }), null, false);
    expect(old.detail).toBe('Publiée le 16 octobre 2026');
  });

  it('sorts the news: coming with the update, in the installed version, since the last one seen', () => {
    const s = st({ state: 'available', latest });
    expect(upcomingNews(s).map((n) => n.build)).toEqual([95, 94]);
    expect(installedNews(s).map((n) => n.build)).toEqual([90]);
    const after = st({ state: 'uptodate', build: 95, latest });
    expect(upcomingNews(after)).toEqual([]);
    expect(newsSince(after, 92).map((n) => n.build)).toEqual([95, 94]);
    expect(newsSince(after, 95)).toEqual([]);
  });

  it('shows the download, then waits for the end of the session to install', () => {
    const d = updateView(st({ state: 'downloading', latest, doneBytes: 7_500_000, totalBytes: 15_000_000 }), null, false);
    expect(d.percent).toBe(50);
    expect(d.detail).toBe('8 Mo / 15 Mo · vérifiée avant l’installation');
    const busy = updateView(st({ state: 'ready', latest }), null, true);
    expect(busy).toMatchObject({ action: 'install', blocked: true });
    expect(busy.detail).toMatch(/Arrêtez la session/);
    expect(updateView(st({ state: 'ready', latest }), null, false).blocked).toBe(false);
  });

  it('says errors in plain words, without technical details', () => {
    const v = updateView(st({ state: 'error', latest, error: 'empreinte SHA-256 invalide : installateur refusé' }), null, false);
    expect(v).toMatchObject({ banner: true, action: 'retry', title: 'Mise à jour impossible' });
    expect(v.detail).toBe('Le fichier téléchargé était incomplet : il a été supprimé. Réessayez.');
    expect(updateSummary(st({ state: 'error', error: 'recherche impossible : le serveur a répondu 404' }))).toBe(UNAVAILABLE);
    expect(updateSummary(st({ state: 'error', error: 'recherche impossible : connexion impossible (Dns)' }))).toBe(UNAVAILABLE);
    expect(friendlyError('téléchargement interrompu : timeout')).toMatch(/connexion/);
    expect(friendlyError('écriture impossible : disque plein')).toMatch(/espace disque/);
    expect(friendlyError('annulé')).toBe('Téléchargement annulé.');
    expect(friendlyError('quelque chose d’imprévu')).toBe('Mise à jour impossible pour le moment. Réessayez plus tard.');
    for (const e of ['empreinte SHA-256 invalide', 'recherche impossible : le serveur a répondu 404', 'lancement de l\'installateur impossible : os error 5'])
      expect(friendlyError(e)).not.toMatch(DEV);
    expect(updateSummary(st({ state: 'available', latest }))).toBe('Une nouvelle version est disponible (16 octobre 2026).');
    expect(updateSummary(st({ state: 'uptodate' }))).toBe('Crosstalk est à jour.');
    expect(updateSummary(null)).toMatch(/application Windows/);
  });

  it('formats dates and versions for people', () => {
    expect(formatDate('2026-10-16 01:35 UTC')).toBe('16 octobre 2026');
    expect(formatDate('2026-01-02')).toBe('2 janvier 2026');
    expect(formatDate('demain')).toBe('demain');
    expect(versionLabel('0.4.1', 276)).toBe('0.4.1.276');
    expect(versionLabel('0.4.1', undefined)).toBe('0.4.1');
  });
});
