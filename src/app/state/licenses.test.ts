import { describe, expect, it } from 'vitest';
import { LICENSES } from './data';
import cargo from '../../../src-tauri/Cargo.toml?raw';

describe('third-party licences shown in À propos', () => {
  const names = LICENSES.map(([n]) => n.toLowerCase()).join(' | ');

  it('name the libraries the app is really built from', () => {
    for (const dep of ['tauri', 'whisper-rs', 'candle', 'windows', 'ureq']) {
      expect(cargo).toContain(dep);
      expect(names).toContain(dep);
    }
    // Listed once although never a dependency.
    expect(names).not.toContain('rubato');
  });

  it('include the downloaded models, with the CC-BY ones apart', () => {
    expect(names).toContain('whisper');
    expect(names).toContain('silero');
    expect(LICENSES.filter(([n]) => n.startsWith('Opus-MT')).map(([, l]) => l)).toEqual(['Apache-2.0', 'CC-BY-4.0']);
  });
});
