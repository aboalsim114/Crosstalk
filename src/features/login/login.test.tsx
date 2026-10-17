import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import App from '../../app/App';
import { useApp } from '../../app/state/store';
import { checkCredentials, displayName, useAuth } from './auth';
import { CHECK_MS, ENTER_MS } from './LoginPage';

const initial = useApp.getState();

beforeEach(() => {
  vi.useFakeTimers();
  useApp.setState(initial, true);
  useAuth.getState().logout();
});
afterEach(() => {
  vi.useRealTimers();
});

const type = (label: string, value: string) => fireEvent.change(screen.getByPlaceholderText(label), { target: { value } });
const submit = () => fireEvent.click(screen.getByRole('button', { name: /SE CONNECTER/ }));

describe('sign-in', () => {
  it('only knows the built-in admin account', () => {
    expect(checkCredentials('admin', 'admin')).toBe('admin');
    expect(checkCredentials(' Admin ', 'admin')).toBe('admin');
    expect(checkCredentials('admin', 'Admin')).toBeNull();
    expect(checkCredentials('joueur', 'admin')).toBeNull();
    expect(displayName('vous@exemple.com')).toBe('VOUS');
  });

  it('is shown before the app while signed out', () => {
    render(<App />);
    expect(screen.getByTestId('login')).toBeInTheDocument();
    expect(screen.queryByTestId('intro')).not.toBeInTheDocument();
  });

  it('asks for the missing fields, then refuses a wrong password', () => {
    render(<App />);
    submit();
    expect(screen.getByRole('alert')).toHaveTextContent('Saisissez votre identifiant ou e-mail.');
    type('pseudo ou vous@exemple.com', 'admin');
    submit();
    expect(screen.getByRole('alert')).toHaveTextContent('Saisissez votre mot de passe.');
    type('••••••••', 'mauvais');
    submit();
    expect(screen.getByRole('button', { name: /VÉRIFICATION/ })).toBeInTheDocument();
    act(() => vi.advanceTimersByTime(CHECK_MS));
    expect(screen.getByRole('alert')).toHaveTextContent('Identifiant ou mot de passe incorrect.');
    expect(useAuth.getState().user).toBeNull();
  });

  it('opens the app with admin / admin, and remembers it', () => {
    render(<App />);
    type('pseudo ou vous@exemple.com', 'admin');
    type('••••••••', 'admin');
    submit();
    act(() => vi.advanceTimersByTime(CHECK_MS));
    expect(screen.getByTestId('login-success')).toHaveTextContent('BIENVENUE,ADMIN');
    act(() => vi.advanceTimersByTime(ENTER_MS));
    expect(useAuth.getState().user).toBe('admin');
    expect(screen.getByTestId('intro')).toBeInTheDocument();
    expect(localStorage.getItem('crosstalk.session')).toBe('admin');
    // Signing out from the app brings the page back and forgets the session.
    fireEvent.click(screen.getByText('Entrer'));
    act(() => vi.advanceTimersByTime(2500));
    fireEvent.click(screen.getByText('Se déconnecter'));
    expect(screen.getByTestId('login')).toBeInTheDocument();
    expect(localStorage.getItem('crosstalk.session')).toBeNull();
  });

  it('without "Rester connecté" the session is not kept', () => {
    render(<App />);
    fireEvent.click(screen.getByRole('switch', { name: /Rester connecté/ }));
    type('pseudo ou vous@exemple.com', 'admin');
    type('••••••••', 'admin');
    submit();
    act(() => vi.advanceTimersByTime(CHECK_MS + ENTER_MS));
    expect(useAuth.getState().user).toBe('admin');
    expect(localStorage.getItem('crosstalk.session')).toBeNull();
  });

  it('says that online accounts are not available yet', () => {
    render(<App />);
    fireEvent.click(screen.getByText(/S'inscrire sur crosstalk.gg/));
    expect(screen.getByText('Les comptes crosstalk.gg arrivent bientôt.')).toBeInTheDocument();
  });
});
