import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './index';

export type TranslationPlan = 'free' | 'pro';
export type TranslationSource = 'ru' | 'en' | 'fr';
export interface TranslationGlossaryEntry { source: string; target: string | null }
export interface TranslationConfig {
  enabled: boolean;
  plan: TranslationPlan;
  glossary: TranslationGlossaryEntry[];
}
export interface TranslationStatus {
  enabled: boolean;
  plan: TranslationPlan;
  keyConfigured: boolean;
  supported: boolean;
}
export interface TranslationResult {
  original: string;
  translated: string | null;
  source: TranslationSource;
  target: 'fr';
  status: 'translated' | 'original';
  reason: string | null;
  cached: boolean;
  latencyMs: number;
}

export const UNAVAILABLE_TRANSLATION: TranslationStatus = {
  enabled: false, plan: 'free', keyConfigured: false, supported: false,
};
const unavailable = () => Promise.reject(new Error('Traduction disponible dans l’application Windows.'));

/** Credentials travel only to the native credential store, never back to the UI. */
export const translation = {
  status: (): Promise<TranslationStatus> => isTauri ? invoke('translation_status') : Promise.resolve(UNAVAILABLE_TRANSLATION),
  configure: (config: TranslationConfig): Promise<TranslationStatus> => isTauri ? invoke('translation_configure', { config }) : unavailable(),
  saveKey: (plan: TranslationPlan, key: string): Promise<TranslationStatus> => isTauri ? invoke('translation_set_key', { plan, key }) : unavailable(),
  deleteKey: (plan: TranslationPlan): Promise<TranslationStatus> => isTauri ? invoke('translation_delete_key', { plan }) : unavailable(),
  test: (text: string, source: TranslationSource): Promise<TranslationResult> => isTauri ? invoke('translation_test', { text, source }) : unavailable(),
};
