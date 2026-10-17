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

const typeUser = (value: string) => fireEvent.change(screen.getByPlaceholderText('vous@exemple.com'), { target: { value } });
const typePw = (value: string) => fireEvent.change(screen.getByLabelText('Mot de passe'), { target: { value } });
const submit = () => fireEvent.click(screen.getByRole('button', { name: 'Se connecter' }));

describe('sign-in', () => {
  it('only knows the built-in admin account', () => {
    expect(checkCredentials('admin', 'admin')).toBe('admin');
    expect(checkCredentials(' Admin ', 'admin')).toBe('admin');
    expect(checkCredentials('admin', 'Admin')).toBeNull();
    expect(checkCredentials('joueur', 'admin')).toBeNull();
    expect(displayName('vous@exemple.com')).toBe('vous');
  });

  it('is shown before the app while signed out', () => {
    render(<App />);
    expect(screen.getByTestId('login')).toBeInTheDocument();
    expect(screen.queryByTestId('intro')).not.toBeInTheDocument();
  });

  it('asks for the missing fields, then refuses a wrong password', () => {
    render(<App />);
    submit();
    expect(screen.getByText('Saisissez votre identifiant ou e-mail.')).toBeInTheDocument();
    expect(screen.getByText('Saisissez votre mot de passe.')).toBeInTheDocument();
    typeUser('admin');
    expect(screen.queryByText('Saisissez votre identifiant ou e-mail.')).not.toBeInTheDocument();
    typePw('mauvais');
    submit();
    expect(screen.getByRole('button', { name: /Connexion…/ })).toBeDisabled();
    act(() => vi.advanceTimersByTime(CHECK_MS));
    expect(screen.getByRole('alert')).toHaveTextContent('Identifiant ou mot de passe incorrect.');
    // The wrong password is cleared, the identifier kept.
    expect(screen.getByLabelText('Mot de passe')).toHaveValue('');
    expect(screen.getByPlaceholderText('vous@exemple.com')).toHaveValue('admin');
    expect(useAuth.getState().user).toBeNull();
  });

  it('opens the app with admin / admin, and remembers it', () => {
    render(<App />);
    typeUser('admin');
    typePw('admin');
    submit();
    act(() => vi.advanceTimersByTime(CHECK_MS));
    expect(screen.getByTestId('login-success')).toHaveTextContent('Bonjour, admin');
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
    fireEvent.click(screen.getByRole('checkbox', { name: /Rester connecté/ }));
    typeUser('admin');
    typePw('admin');
    submit();
    act(() => vi.advanceTimersByTime(CHECK_MS + ENTER_MS));
    expect(useAuth.getState().user).toBe('admin');
    expect(localStorage.getItem('crosstalk.session')).toBeNull();
  });

  it('shows translated sample comms that rotate', () => {
    render(<App />);
    const feed = screen.getByTestId('login-feed');
    expect(feed).toHaveTextContent('Two on short, one is hit');
    act(() => vi.advanceTimersByTime(3200));
    expect(feed).toHaveTextContent('Bombe posée en B, partez sur le retake');
  });

  it('goes back to the form with "Changer de compte"', () => {
    render(<App />);
    typeUser('admin');
    typePw('admin');
    submit();
    act(() => vi.advanceTimersByTime(CHECK_MS));
    fireEvent.click(screen.getByText('Changer de compte'));
    act(() => vi.advanceTimersByTime(ENTER_MS));
    expect(useAuth.getState().user).toBeNull();
    expect(screen.getByRole('button', { name: 'Se connecter' })).toBeInTheDocument();
  });

  it('says that online accounts are not available yet', () => {
    render(<App />);
    fireEvent.click(screen.getByText('Créer un compte'));
    expect(screen.getByText('Les comptes crosstalk.gg arrivent bientôt.')).toBeInTheDocument();
  });
});
