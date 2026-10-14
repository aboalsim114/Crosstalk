// Bridge between the React UI and the Rust core (src-tauri).
//
// The UI always runs inside the Tauri desktop app in production. These helpers also
// degrade to harmless fallbacks when the frontend is loaded outside Tauri (unit tests,
// `npm run dev` for quick styling work), so components never have to check themselves.
//
// One file per domain of the Rust core; this barrel keeps `../backend` as the
// single import path for the interface.

export * from './core';
export * from './actions';
export * from './overlay';
export * from './autostart';
export * from './audio';
export * from './speech';
export * from './translation';
export * from './llm';
export * from './downloads';
export * from './chat';
export * from './settings';
export * from './updater';
export * from './history';
export * from './diagnostics';
export * from './gsi';
export * from './reply';
export * from './stream';
export * from './speaker';
export * from './tray';
export * from './recap';
export * from './dictation';
