// Diagnostic journal and report (src-tauri/src/logs.rs).

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './core';

export interface LogLine {
  ts: number;
  level: 'INFO' | 'WARN' | 'ERROR';
  source: string;
  message: string;
}
export interface DiagReportRow {
  name: string;
  detail: string;
  state: string;
}

/** Pipeline pulse of the running capture (src-tauri/src/pulse.rs): counters only. */
export interface PulseView {
  capturing: boolean;
  summary: string | null;
  alerts: string[];
}

export const diagnostics = {
  recent: (limit: number): Promise<LogLine[]> => (isTauri ? invoke<LogLine[]>('logs_recent', { limit }) : Promise.resolve([])),
  write: (level: LogLine['level'], source: string, message: string): Promise<void> => (isTauri ? invoke<void>('logs_write', { level, source, message }) : Promise.resolve()),
  openFolder: (): Promise<string> => invoke<string>('logs_open_folder'),
  report: (rows: DiagReportRow[]): Promise<string> => invoke<string>('diag_report', { rows }),
  pulse: (): Promise<PulseView | null> => (isTauri ? invoke<PulseView>('diag_pulse') : Promise.resolve(null)),
  reportIssue: (exported: boolean): Promise<void> => invoke<void>('diag_report_issue', { exported }),
};
