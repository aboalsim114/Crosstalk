// French: the source language of the interface. Every other locale must
// provide every key (checked by TypeScript and by i18n.test.ts).
// Placeholders: {name}. Plurals: an object with Intl.PluralRules categories
// (one, few, many, other); `other` is required.

export const fr = {
  'brand.tagline': 'POUR COUNTER-STRIKE 2',
  'brand.tooltip': 'Crosstalk — traduction CS2',

  'nav.dash': 'Accueil',
  'nav.overlay': 'Overlay',
  'nav.audio': 'Audio & IA',
  'nav.lang': 'Langues & glossaire',
  'nav.hist': 'Historique',
  'nav.diag': 'Diagnostic',
  'nav.about': 'À propos',

  'window.minimize': 'Réduire',
  'window.maximize': 'Agrandir',
  'window.close': 'Fermer',

  'header.chapter': 'CHAPITRE {n} / {total}',
  'session.idle': 'INACTIF',
  'session.initializing': 'INITIALISATION',
  'session.listening': 'EN ÉCOUTE',
  'session.degraded': 'DÉGRADÉ',
  'session.paused': 'EN PAUSE',
  'session.stopping': 'ARRÊT…',
  'session.stopped': 'ARRÊTÉ',
  'action.pause': 'Pause',
  'action.resume': 'Reprendre',
  'action.emergency': 'Arrêt d’urgence',
  'action.start': 'Démarrer',
  'action.stop': 'Arrêter',
  'action.overlay': 'Overlay',

  'tray.show': 'Afficher Crosstalk',
  'tray.session': 'Démarrer / arrêter la session',
  'tray.reply': 'Répondre dans leur langue',
  'tray.dictate': 'Dicter une réponse',
  'tray.quit': 'Quitter Crosstalk',

  'uiLang.title': 'LANGUE DE L’APPLICATION',
  'uiLang.label': 'Langue de l’interface',
  'uiLang.auto': 'Automatique ({name})',
  'uiLang.note': 'Change la langue des menus et des écrans. Les sous-titres restent traduits en français pour l’instant.',
  'uiLang.partial': 'Traduction en cours : certains écrans restent en français.',
} as const;

export type MessageKey = keyof typeof fr;
export type Plural = { one?: string; two?: string; few?: string; many?: string; zero?: string; other: string };
export type Messages = Record<MessageKey, string | Plural>;
