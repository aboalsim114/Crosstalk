import { create } from 'zustand';
import { isTauri } from '../backend';
import { translation, type TranslationGlossaryEntry, type TranslationStatus } from '../backend/translation';
import type { GlossEntry } from './data';

export interface TranslationState {
  native: boolean;
  status: TranslationStatus | null;
}

export const useTranslation = create<TranslationState>()(() => ({ native: isTauri, status: null }));

/** Protected terms keep their spelling; imposed renderings are restored in Rust. */
export function translationGlossary(glossary: GlossEntry[]): TranslationGlossaryEntry[] {
  return glossary.map(([source, mode, rendering]) => ({ source, target: mode === 'Traduire' ? rendering : null }));
}

export async function refreshTranslation(): Promise<void> {
  const status = await translation.status();
  useTranslation.setState({ status });
}
