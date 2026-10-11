import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import App from './App';
import { FALLBACK_INFO, getAppInfo, isTauri } from './backend';

describe('App shell', () => {
  it('renders the product name and tagline', () => {
    render(<App />);
    expect(screen.getByText('CROSSTALK')).toBeInTheDocument();
    expect(screen.getByText('en français.')).toBeInTheDocument();
  });
});

describe('backend bridge', () => {
  it('falls back to static info outside Tauri', async () => {
    expect(isTauri).toBe(false);
    await expect(getAppInfo()).resolves.toEqual(FALLBACK_INFO);
  });
});
