// Sign-in gate in front of the app. No server yet: a single built-in account,
// checked on this PC. "Rester connecté" keeps the session across launches.

import { create } from 'zustand';

/** The only account until real accounts exist (crosstalk.gg). */
export const ADMIN = { user: 'admin', password: 'admin' } as const;

const KEY = 'crosstalk.session';

/** The account name for these credentials, or null when they are wrong. */
export function checkCredentials(user: string, password: string): string | null {
  return user.trim().toLowerCase() === ADMIN.user && password === ADMIN.password ? ADMIN.user : null;
}

/** Name shown on the welcome screen: the part before "@" of an e-mail. */
export function displayName(user: string): string {
  const u = user.trim();
  return u.includes('@') ? u.split('@')[0] : u;
}

// Storage can be missing or refused (private profile): the session then lasts until the app closes.
function stored(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

function store(user: string | null) {
  try {
    if (user) localStorage.setItem(KEY, user);
    else localStorage.removeItem(KEY);
  } catch {
    // Not kept: the user signs in again next time.
  }
}

interface AuthState {
  /** The signed-in account, null while the sign-in page is shown. */
  user: string | null;
  login: (user: string, remember: boolean) => void;
  logout: () => void;
}

export const useAuth = create<AuthState>((set) => ({
  user: stored(),
  login: (user, remember) => {
    store(remember ? user : null);
    set({ user });
  },
  logout: () => {
    store(null);
    set({ user: null });
  },
}));
