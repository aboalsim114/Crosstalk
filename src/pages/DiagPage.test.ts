import { describe, expect, it } from 'vitest';
import { journalRows } from './DiagPage';

describe('diagnostic journal', () => {
  it('shows the real journal lines with their level and source', () => {
    const ts = new Date(2026, 9, 13, 14, 2, 11).getTime();
    expect(journalRows([{ ts, level: 'WARN', source: 'audio', message: 'capture impossible [processLoopback] : refusé' }])).toEqual([
      ['14:02:11', 'WARN', 'audio: capture impossible [processLoopback] : refusé'],
    ]);
  });
});
