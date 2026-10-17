import { useEffect, useRef } from 'react';
import { fnt } from '../../shared/ui/atoms';
import { useApp } from '../../app/state/store';
import { NAV, type PageId } from '../../app/state/data';
import { useLocale, useT } from '../../shared/i18n';
import { CATEGORIES, PAGE_CHAPTER, STATUS_HELP, search, useGuide, type Chapter, type FeatureStatus, type Shot } from './guide';
import { CHAPTERS, FAQ } from './content';

/** Opens the guide on a chapter (null: its home). */
export function openGuide(chapter: string | null = null) {
  useGuide.getState().set({ chapter, faq: null, query: '' });
  useApp.getState().setPage('guide');
}

/** The "?" of a page header: the chapter about that screen. */
export function GuideButton({ page }: { page: PageId }) {
  const t = useT();
  const chapter = PAGE_CHAPTER[page];
  if (page === 'guide' || !chapter) return null;
  return (
    <button type="button" className="pill pill-ghost px-3 py-1.5" aria-label={t('guide.helpLabel')} title={t('guide.helpLabel')} onClick={() => openGuide(chapter)} style={{ font: fnt(500, 11.5), color: '#bdb6ab', background: 'transparent' }}>
      {t('guide.help')}
    </button>
  );
}

const STATUS_COLOR: Record<FeatureStatus, string> = { implémenté: '#7fe0a8', 'à configurer': '#7fd4ff', expérimental: '#ffa24a' };

function StatusBadge({ status }: { status: FeatureStatus }) {
  return (
    <span data-testid="guide-status" title={STATUS_HELP[status]} className="rounded-full px-2.5 py-1" style={{ font: fnt(500, 9.5, 'mono'), letterSpacing: '.14em', color: STATUS_COLOR[status], border: `1px solid ${STATUS_COLOR[status]}55` }}>
      {status.toUpperCase()}
    </span>
  );
}

const SECTIONS: [keyof Pick<Chapter, 'prerequisites' | 'setup' | 'use' | 'check' | 'limits'>, string, boolean][] = [
  ['prerequisites', 'Prérequis', false],
  ['setup', 'Activer et configurer', true],
  ['use', 'Utiliser', true],
  ['check', 'Vérifier que ça marche', false],
  ['limits', 'Limites et dépannage', false],
];

function Screenshot({ shot }: { shot: Shot }) {
  return (
    <figure className="flex flex-col gap-3" style={{ margin: 0 }}>
      <div className="relative overflow-hidden rounded-xl" style={{ border: '1px solid rgba(255,255,255,.08)', maxWidth: shot.width }}>
        <img src={shot.src} alt={shot.alt} className="block w-full" loading="lazy" />
        {shot.notes.map((n, i) => (
          <span
            key={i}
            aria-hidden
            className="absolute flex h-6 w-6 items-center justify-center rounded-full"
            style={{ left: `${n.x}%`, top: `${n.y}%`, transform: 'translate(-50%,-50%)', background: '#ffa24a', color: '#140a06', font: fnt(700, 11.5), boxShadow: '0 0 0 3px rgba(0,0,0,.55),0 0 14px rgba(255,162,74,.7)' }}
          >
            {i + 1}
          </span>
        ))}
      </div>
      <figcaption>
        <ol className="flex flex-col gap-1.5" style={{ margin: 0, paddingLeft: 0, listStyle: 'none' }}>
          {shot.notes.map((n, i) => (
            <li key={i} className="flex gap-2.5" style={{ font: fnt(400, 12.5, 'sans', 1.5), color: '#bdb6ab' }}>
              <span className="flex h-5 w-5 flex-none items-center justify-center rounded-full" style={{ background: 'rgba(255,162,74,.15)', color: '#ffa24a', font: fnt(700, 10.5) }}>
                {i + 1}
              </span>
              {n.text}
            </li>
          ))}
        </ol>
      </figcaption>
    </figure>
  );
}

function ChapterView({ chapter }: { chapter: Chapter }) {
  const setPage = useApp((s) => s.setPage);
  const go = useGuide((s) => s.set);
  const heading = useRef<HTMLHeadingElement>(null);
  const index = CHAPTERS.findIndex((c) => c.id === chapter.id);
  const prev = CHAPTERS[index - 1];
  const next = CHAPTERS[index + 1];
  // Keyboard and screen-reader users land on the chapter title, at the top of the page.
  useEffect(() => {
    const box = heading.current?.closest('.overflow-auto');
    if (box) box.scrollTop = 0;
    heading.current?.focus({ preventScroll: true });
  }, [chapter.id]);
  return (
    <article className="flex flex-col gap-5" data-testid="guide-chapter" aria-labelledby={`guide-${chapter.id}`}>
      <div className="flex flex-col gap-2.5">
        <div className="flex flex-wrap items-center gap-3">
          <span className="lbl">{CATEGORIES.find(([id]) => id === chapter.category)?.[1].toUpperCase()}</span>
          <StatusBadge status={chapter.status} />
          <span style={{ font: fnt(400, 11.5, 'sans', 1.4), color: '#8f8a83' }}>{STATUS_HELP[chapter.status]}</span>
        </div>
        <h2 id={`guide-${chapter.id}`} ref={heading} tabIndex={-1} style={{ font: fnt(400, 30, 'serif', 1.1), color: '#ece6dc', outline: 'none' }}>
          {chapter.title}
        </h2>
        <p style={{ font: fnt(400, 14, 'sans', 1.6), color: '#bdb6ab', margin: 0 }}>{chapter.purpose}</p>
        {chapter.statusNote && (
          <p data-testid="guide-status-note" className="rounded-lg px-3 py-2" style={{ margin: 0, font: fnt(400, 12.5, 'sans', 1.5), color: '#ffc68a', background: 'rgba(255,162,74,.07)', border: '1px solid rgba(255,162,74,.2)' }}>
            {chapter.statusNote}
          </p>
        )}
        {chapter.settings && (
          <div className="flex flex-wrap items-center gap-3">
            <button type="button" className="pill pill-ember px-4 py-2" onClick={() => setPage(chapter.settings!.page)} style={{ font: fnt(500, 12.5), background: 'transparent' }}>
              Ouvrir l’écran {NAV.find(([id]) => id === chapter.settings!.page)?.[1]} →
            </button>
            {chapter.settings.card && (
              <span data-testid="guide-card" style={{ font: fnt(400, 12.5, 'sans', 1.5), color: '#8f8a83' }}>
                Le réglage est dans la carte « {chapter.settings.card.toUpperCase()} » de cet écran.
              </span>
            )}
          </div>
        )}
      </div>
      {chapter.shot && <Screenshot shot={chapter.shot} />}
      {SECTIONS.map(([key, title, ordered]) => {
        const items = chapter[key];
        if (!items.length) return null;
        const List = ordered ? 'ol' : 'ul';
        return (
          <section key={key} className="flex flex-col gap-2">
            <h3 style={{ font: fnt(600, 13.5), color: '#ece6dc', margin: 0 }}>{title}</h3>
            <List className="flex flex-col gap-1.5" style={{ margin: 0, paddingLeft: 20, listStyle: ordered ? 'decimal' : 'disc', font: fnt(400, 13, 'sans', 1.6), color: '#bdb6ab' }}>
              {items.map((it, i) => (
                <li key={i}>{it}</li>
              ))}
            </List>
          </section>
        );
      })}
      <nav aria-label="Chapitres voisins" className="flex justify-between gap-3 pt-2" style={{ borderTop: '1px solid rgba(255,255,255,.06)' }}>
        {prev ? (
          <button type="button" className="pill pill-ghost px-3.5 py-2" onClick={() => go({ chapter: prev.id })} style={{ font: fnt(400, 12.5), color: '#bdb6ab', background: 'transparent' }}>
            ← {prev.title}
          </button>
        ) : (
          <span />
        )}
        {next && (
          <button type="button" className="pill pill-ghost px-3.5 py-2" onClick={() => go({ chapter: next.id })} style={{ font: fnt(400, 12.5), color: '#bdb6ab', background: 'transparent' }}>
            {next.title} →
          </button>
        )}
      </nav>
    </article>
  );
}

function FaqView() {
  const { faq: target, set: go } = useGuide();
  const opened = useRef<HTMLElement>(null);
  // A search result: its answer is opened, scrolled to and focused. Only the
  // page's own scroll area moves (scrollIntoView would also shift the window frame).
  useEffect(() => {
    const el = opened.current;
    const box = el?.closest('.overflow-auto');
    if (el && box) box.scrollTop += el.getBoundingClientRect().top - box.getBoundingClientRect().top - 16;
    el?.focus({ preventScroll: true });
  }, [target]);
  return (
    <section className="flex flex-col gap-4" data-testid="guide-faq" aria-labelledby="guide-faq-title">
      <h2 id="guide-faq-title" style={{ font: fnt(400, 30, 'serif', 1.1), color: '#ece6dc' }}>
        Questions fréquentes
      </h2>
      {FAQ.map((f) => (
        <details key={f.q} className="glass px-4 py-3" open={f.q === target || undefined}>
          <summary ref={f.q === target ? opened : undefined} style={{ font: fnt(500, 13.5), color: '#ece6dc', cursor: 'pointer' }}>
            {f.q}
          </summary>
          <p style={{ font: fnt(400, 13, 'sans', 1.6), color: '#bdb6ab', margin: '8px 0 6px' }}>{f.a}</p>
          <button type="button" onClick={() => go({ chapter: f.chapter })} style={{ font: fnt(500, 12), color: '#ffa24a', background: 'none', border: 'none', padding: 0, cursor: 'pointer' }}>
            Lire le chapitre « {CHAPTERS.find((c) => c.id === f.chapter)?.title} » →
          </button>
        </details>
      ))}
    </section>
  );
}

function Home() {
  const go = useGuide((s) => s.set);
  return (
    <div className="flex flex-col gap-5" data-testid="guide-home">
      <div className="flex flex-col gap-2">
        <h2 style={{ font: fnt(400, 30, 'serif', 1.1), color: '#ece6dc' }}>Centre d’aide</h2>
        <p style={{ font: fnt(400, 13.5, 'sans', 1.6), color: '#bdb6ab', margin: 0 }}>
          Tout Crosstalk, chapitre par chapitre : à quoi sert chaque fonction, comment l’activer, comment vérifier qu’elle marche et que faire si ce n’est pas le cas. Le guide fonctionne sans Internet.
        </p>
        <ul data-testid="guide-legend" className="flex flex-col gap-1" style={{ margin: 0, paddingLeft: 18, listStyle: 'disc', font: fnt(400, 12, 'sans', 1.5), color: '#8f8a83' }}>
          {Object.values(STATUS_HELP).map((h) => (
            <li key={h}>{h}</li>
          ))}
        </ul>
      </div>
      {CATEGORIES.map(([cat, name]) => (
        <section key={cat} className="flex flex-col gap-2.5" aria-label={name}>
          <h3 className="lbl">{name.toUpperCase()}</h3>
          <div className="grid grid-cols-2 gap-2.5">
            {CHAPTERS.filter((c) => c.category === cat).map((c) => (
              <button key={c.id} type="button" className="glass flex flex-col items-start gap-1.5 p-4 text-left" onClick={() => go({ chapter: c.id })}>
                <span className="flex w-full items-center justify-between gap-2">
                  <span style={{ font: fnt(500, 13.5), color: '#ece6dc' }}>{c.title}</span>
                  {c.status !== 'implémenté' && <StatusBadge status={c.status} />}
                </span>
                <span style={{ font: fnt(400, 12, 'sans', 1.5), color: '#8f8a83' }}>{c.summary}</span>
              </button>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function Results({ query }: { query: string }) {
  const go = useGuide((s) => s.set);
  const hits = search(query, CHAPTERS, FAQ);
  return (
    <section className="flex flex-col gap-3" data-testid="guide-results" aria-live="polite">
      <h2 style={{ font: fnt(400, 24, 'serif', 1.1), color: '#ece6dc' }}>{hits.length ? `${hits.length} résultat${hits.length > 1 ? 's' : ''} pour « ${query} »` : `Aucun résultat pour « ${query} »`}</h2>
      {!hits.length && <p style={{ font: fnt(400, 13, 'sans', 1.6), color: '#8f8a83', margin: 0 }}>Essayez un autre mot : « micro », « overlay », « chat », « Mouse 4 »…</p>}
      {hits.map((h) => (
        <button
          key={h.kind === 'chapter' ? h.chapter.id : h.faq.q}
          type="button"
          className="glass flex flex-col items-start gap-1 p-4 text-left"
          onClick={() => go(h.kind === 'chapter' ? { chapter: h.chapter.id, faq: null, query: '' } : { chapter: 'faq', faq: h.faq.q, query: '' })}
        >
          <span style={{ font: fnt(500, 13.5), color: '#ece6dc' }}>{h.kind === 'chapter' ? h.chapter.title : `FAQ — ${h.faq.q}`}</span>
          <span style={{ font: fnt(400, 12, 'sans', 1.5), color: '#8f8a83' }}>{h.excerpt}</span>
        </button>
      ))}
    </section>
  );
}

/** Guide › the in-app manual: table of contents, search, chapters, FAQ. */
export function GuidePage() {
  const { chapter, query, set } = useGuide();
  const t = useT();
  const french = useLocale((s) => s.locale) === 'fr';
  const { setPage, prevPage } = useApp();
  const open = CHAPTERS.find((c) => c.id === chapter);
  const tocItem = (id: string | null, label: string, current: boolean) => (
    <button
      key={id ?? 'home'}
      type="button"
      aria-current={current ? 'page' : undefined}
      onClick={() => set({ chapter: id, faq: null, query: '' })}
      className="w-full rounded-lg px-3 py-1.5 text-left"
      style={{ font: fnt(400, 12.5), color: current ? '#ece6dc' : '#8f8a83', background: current ? 'rgba(255,255,255,.05)' : 'transparent', border: 'none', cursor: 'pointer' }}
    >
      {label}
    </button>
  );
  return (
    <div className="grid min-h-0 gap-5" style={{ gridTemplateColumns: '250px minmax(0,1fr)' }} data-testid="guide">
      <nav aria-label="Sommaire du guide" className="glass flex flex-col gap-3 self-start p-4">
        <input
          type="search"
          className="text-input"
          aria-label={t('guide.search')}
          placeholder={t('guide.searchHint')}
          value={query}
          onChange={(e) => set({ query: e.target.value })}
        />
        {tocItem(null, 'Accueil du guide', !query && !chapter)}
        {CATEGORIES.map(([cat, name]) => (
          <div key={cat} className="flex flex-col gap-0.5">
            <span className="lbl px-3 pt-1.5 pb-1">{name.toUpperCase()}</span>
            {CHAPTERS.filter((c) => c.category === cat).map((c) => tocItem(c.id, c.title, !query && chapter === c.id))}
            {cat === 'help' && tocItem('faq', 'Questions fréquentes', !query && chapter === 'faq')}
          </div>
        ))}
        <button type="button" className="pill pill-ghost mt-1 px-3 py-2" onClick={() => setPage(prevPage)} style={{ font: fnt(500, 12), color: '#bdb6ab', background: 'transparent' }}>
          {t('guide.close')}
        </button>
      </nav>
      <div className="min-w-0 pb-6">
        {!french && (
          <p data-testid="guide-fr-only" className="mb-4 rounded-lg px-3 py-2" style={{ font: fnt(400, 12.5, 'sans', 1.5), color: '#7fd4ff', background: 'rgba(127,212,255,.06)', border: '1px solid rgba(127,212,255,.2)' }}>
            {t('guide.frOnly')}
          </p>
        )}
        {/* The manual itself is French in every interface language (V1): screen readers read it as French. */}
        <div lang="fr">{query.trim() ? <Results query={query.trim()} /> : open ? <ChapterView chapter={open} /> : chapter === 'faq' ? <FaqView /> : <Home />}</div>
      </div>
    </div>
  );
}
