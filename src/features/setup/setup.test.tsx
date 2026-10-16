import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { DownloadStatus } from '../../shared/ipc';
import { SetupBanner } from './SetupBanner';
import { mb, setupNeeds, setupView, useSetup } from './setup';

const st = (over: Partial<DownloadStatus> = {}): DownloadStatus => ({
  state: 'downloading', current: 'Transcription (Whisper base)', doneBytes: 300_000_000, totalBytes: 780_000_000, missingFiles: 9, missingBytes: 780_000_000, error: null, ...over,
});

describe('first-run setup', () => {
  it('shows the download progress with what is being fetched', () => {
    const v = setupView(st());
    expect(v).toMatchObject({ visible: true, title: 'Préparation de Crosstalk · 38 %', percent: 38, canCancel: true, canRetry: false });
    expect(v.detail).toBe('Transcription (Whisper base) · 300 Mo / 780 Mo');
  });

  it('offers to resume after an error or a cancel, and hides once complete', () => {
    expect(setupView(st({ state: 'error', error: 'connexion impossible (Dns)' }))).toMatchObject({ visible: true, tone: 'error', canRetry: true, detail: 'connexion impossible (Dns)' });
    expect(setupView(st({ state: 'cancelled' })).canRetry).toBe(true);
    expect(setupView(st({ state: 'done', missingFiles: 0 })).visible).toBe(false);
    expect(setupView(st({ state: 'idle', missingFiles: 0 })).visible).toBe(false);
    expect(setupView(st({ state: 'idle', missingFiles: 2, missingBytes: 149_000_000 })).detail).toBe('2 fichiers · 149 Mo');
    expect(setupView(null).visible).toBe(false);
  });

  it('needs the recognition model only: no translation model is ever downloaded', () => {
    expect(setupNeeds({ model: 'parakeet-tdt-0.6b-v3' })).toEqual({ model: 'parakeet-tdt-0.6b-v3' });
    expect(mb(147_951_465)).toBe('148 Mo');
  });

  it('renders the banner with its progress bar', () => {
    useSetup.setState({ status: st() });
    render(<SetupBanner />);
    expect(screen.getByTestId('setup-banner')).toHaveTextContent('Préparation de Crosstalk · 38 %');
    expect(screen.getByTestId('setup-progress')).toHaveStyle({ width: '38%' });
    expect(screen.getByText('Annuler')).toBeInTheDocument();
    useSetup.setState({ status: null });
  });
});
