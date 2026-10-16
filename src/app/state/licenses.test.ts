import { describe, expect, it } from 'vitest';
import { LICENSES } from './data';
import cargo from '../../../src-tauri/Cargo.toml?raw';

describe('third-party licences shown in À propos', () => {
  const names = LICENSES.map(([n]) => n.toLowerCase()).join(' | ');

  it('name the libraries the app is really built from', () => {
    for (const dep of ['tauri', 'sherpa-onnx', 'windows', 'ureq']) {
      expect(cargo).toContain(dep);
      expect(names).toContain(dep);
    }
    // Listed once although never a dependency; candle left with Opus-MT.
    expect(names).not.toContain('rubato');
    expect(names).not.toContain('candle');
    expect(cargo).not.toMatch(/^candle/m);
    // Whisper is gone, with its runtime.
    expect(cargo).not.toMatch(/whisper/i);
    expect(names).not.toContain('whisper');
  });

  it('include the downloaded speech models, and no translation model any more', () => {
    expect(names).toContain('parakeet');
    expect(names).toContain('silero');
    expect(names).not.toContain('opus-mt');
    expect(names).not.toContain('helsinki');
  });
});
