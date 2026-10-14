import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import type { DownloadStatus, LlmStatus } from '../backend';
import { LlmCard } from '../pages/LlmCard';
import { snapshot, restore } from './persist';
import { useSetup } from './setup';
import { useApp } from './store';
import { llmView, translationConfigFrom, useTranslation } from './translation';

const llm = (over: Partial<LlmStatus> = {}): LlmStatus => ({
  id: 'translategemma-4b-q4km',
  name: 'TranslateGemma 4B',
  quant: 'Q4_K_M',
  baseModel: 'google/translategemma-4b-it',
  repo: 'mradermacher/translategemma-4b-it-GGUF',
  file: 'translategemma-4b-it.Q4_K_M.gguf',
  license: 'Gemma Terms of Use',
  licenseUrl: 'https://ai.google.dev/gemma/terms',
  sizeMb: 2489,
  ramMb: 3300,
  install: { state: 'verified', sha256: 'ab'.repeat(32) },
  helper: true,
  state: 'ready',
  choice: 'auto',
  backendChoice: 'auto',
  backend: 'cpu',
  device: null,
  gpuSkipped: null,
  threads: 4,
  loadMs: 4200,
  lastMs: 900,
  p50Ms: 850,
  p95Ms: 1400,
  translations: 12,
  rssMb: 3100,
  error: null,
  ...over,
});

const dl = (over: Partial<DownloadStatus> = {}): DownloadStatus => ({
  state: 'downloading',
  current: 'Traduction (TranslateGemma 4B Q4_K_M)',
  doneBytes: 1_245_000_000,
  totalBytes: 2_490_000_000,
  missingFiles: 1,
  missingBytes: 2_490_000_000,
  error: null,
  bytesPerSec: 12_400_000,
  ...over,
});

describe('TranslateGemma card', () => {
  it('shows the measured state of a loaded model', () => {
    const v = llmView(llm(), null);
    expect(v.state).toBe('PRÊT');
    expect(v.backend).toBe('Processeur · 4 threads');
    expect(v.timing).toBe('p50 850 ms · p95 1400 ms · 12 phrases');
    expect(v.detail).toContain('4,2 s');
    expect(v.detail).toContain('3100 Mo');
    expect(llmView(llm({ backend: 'vulkan', device: 'RTX 3060' }), null).backend).toBe('Carte graphique (vulkan) · RTX 3060');
  });

  it('offers the download when the model is missing, and says Opus-MT stands in', () => {
    const v = llmView(llm({ state: 'missing', install: { state: 'missing' }, backend: null }), null);
    expect(v.state).toBe('NON INSTALLÉ');
    expect(v.canDownload).toBe(true);
    expect(v.detail).toContain('2,49 Go');
    expect(v.detail).toContain('Opus-MT');
    expect(llmView(llm({ state: 'missing', install: { state: 'partial', bytes: 10 } }), null).state).toBe('INCOMPLET');
    expect(llmView(llm({ state: 'missing', helper: false }), null).state).toBe('MOTEUR ABSENT');
  });

  it('follows the real download: MB, percent, speed, then the SHA-256 check', () => {
    const v = llmView(llm({ state: 'missing', install: { state: 'partial', bytes: 1 } }), dl());
    expect(v.state).toBe('TÉLÉCHARGEMENT');
    expect(v.progress).toEqual({ pct: 50, text: '1245 / 2490 Mo · 50 % · 12,4 Mo/s' });
    expect(llmView(llm({ state: 'missing' }), dl({ verifying: true })).state).toBe('VÉRIFICATION');
    // Another model being downloaded does not show here.
    expect(llmView(llm(), dl({ current: 'Transcription (Whisper small)' })).progress).toBeNull();
  });

  it('explains fallbacks: unverified, slow, error, Opus-MT chosen', () => {
    expect(llmView(llm({ state: 'unverified', install: { state: 'unverified' } }), null)).toMatchObject({ state: 'À VÉRIFIER', canVerify: true });
    expect(llmView(llm({ state: 'slow' }), null).detail).toContain('Opus-MT prend le relais');
    expect(llmView(llm({ state: 'error', error: 'mémoire insuffisante' }), null).detail).toContain('mémoire insuffisante');
    expect(llmView(llm({ state: 'off', choice: 'opus' }), null).detail).toContain('Opus-MT est choisi');
    expect(llmView(null, null).detail).toContain('Windows');
  });

  describe('rendered', () => {
    beforeEach(() => {
      useTranslation.setState({ status: { config: { enabled: true, glossary: [] }, pairs: [], modelsDir: 'C:\\m', queued: 0, llm: llm() } });
      useSetup.setState({ status: null });
    });

    it('shows engine, backend and timings', () => {
      render(<LlmCard />);
      expect(screen.getByTestId('llm-state').textContent).toBe('PRÊT');
      expect(screen.getByTestId('llm-backend').textContent).toBe('Processeur · 4 threads');
      expect(screen.getByTestId('llm-timing').textContent).toContain('p50 850 ms');
      expect(screen.getByText('Vérifier / réparer')).toBeTruthy();
      expect(screen.getByText(/Gemma Terms of Use/)).toBeTruthy();
    });

    it('shows the progress bar while the model downloads', () => {
      useTranslation.setState({ status: { config: { enabled: true, glossary: [] }, pairs: [], modelsDir: 'C:\\m', queued: 0, llm: llm({ state: 'missing', install: { state: 'partial', bytes: 1 } }) } });
      useSetup.setState({ status: dl() });
      render(<LlmCard />);
      expect((screen.getByTestId('llm-progress') as HTMLElement).style.width).toBe('50%');
    });
  });
});

describe('engine settings', () => {
  it('reach the Rust core and are kept between launches', () => {
    const s = { ...useApp.getState(), trEngine: 'opus' as const, llmBackend: 'cpu' as const };
    expect(translationConfigFrom(s)).toMatchObject({ engine: 'opus', llmBackend: 'cpu', llmThreads: 0 });
    const saved = snapshot(s, null);
    const back = restore(saved.config, useApp.getState());
    expect(back.app.trEngine).toBe('opus');
    expect(back.app.llmBackend).toBe('cpu');
    expect(restore({ version: 1, settings: { trEngine: 'cloud' } }, useApp.getState()).app.trEngine).toBeUndefined();
  });

  it('default to automatic', () => {
    expect(useApp.getState().trEngine).toBe('auto');
    expect(useApp.getState().llmBackend).toBe('auto');
  });
});
