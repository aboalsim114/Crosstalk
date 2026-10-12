// Pure view logic of the installer (phases and their texts), shared by the UI
// and its tests. Bounds and labels follow the design; the sub-messages say
// what the Rust installer really does in each phase.

/** Phase bounds in percent: PRÉPARATION 0–12, EXTRACTION 12–62, CONFIGURATION 62–88, FINALISATION 88–100. */
export const PH = [0, 12, 62, 88, 100];

export const PHASES: [string, string, string][] = [
  ['PRÉPARATION', 'Préparation de Crosstalk…', 'Vérification de l’espace disponible'],
  ['EXTRACTION', 'Extraction des fichiers…', 'Copie de l’application et téléchargement des modèles de voix et de traduction'],
  ['CONFIGURATION', 'Configuration de l’application…', 'Enregistrement de Crosstalk dans Windows'],
  ['FINALISATION', 'Finalisation de l’installation…', 'Création des raccourcis'],
];

/** Index of the phase shown for a progress value. */
export function phaseOf(prog: number, done = false): number {
  if (done || prog >= 100) return 3;
  const i = PH.findIndex((v, k) => prog >= v && prog < PH[k + 1]);
  return Math.max(0, i);
}

/** Sub-message: the installer's own detail (current file, MB) when it gives one. */
export function phaseSub(ph: number, detail: string, desktop: boolean): string {
  if (detail) return detail;
  return PHASES[ph][2] + (ph === 3 && !desktop ? ' (menu Démarrer uniquement)' : '');
}
