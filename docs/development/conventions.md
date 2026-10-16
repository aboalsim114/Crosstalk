# Conventions

## Règles non négociables

- **CS2** : aucune injection dans CS2, aucune lecture mémoire du jeu, aucun hook interne, aucun contournement anti-triche, aucun changement intrusif de la capture. `npm run check:anticheat` refuse les API d'accès à un autre processus.
- **Vie privée** :
  - aucun audio écrit sur disque par défaut ;
  - rien n'est envoyé à un service en ligne ;
  - les voix et les pseudos de joueurs n'entrent jamais dans le dépôt : les exemples utilisent des noms inventés.
- **Honnêteté** : un test non exécuté n'est jamais déclaré réussi, et des données simulées ne sont jamais présentées comme réelles.

## Rust (`src-tauri`, `src-installer`)

- **Organisation** : un module par fonctionnalité. Un fichier dépasse rarement 500 lignes de production ; au-delà, on le découpe en sous-modules (`speech/service/`, `translation/service/`).
- **Couches** ([D-02](../architecture/decisions.md)) :
  - le domaine (règles pures) et l'application (services, threads) ne dépendent pas de Tauri ;
  - les adaptateurs `Tauri*Events` et les commandes vivent dans `mod.rs` ;
  - la colle entre services vit dans `wiring.rs`.
- **Temps réel** :
  - files bornées avec une politique d'abandon écrite en commentaire ;
  - purge par numéro de génération ;
  - threads de calcul en priorité basse (`crate::cpu::lower_current_thread_priority`), sauf Whisper : ses fils de travail attendent le fil appelant à chaque étape, qui doit rester en priorité normale ;
  - aucune allocation nouvelle sur le thread de capture ;
  - jamais de calcul long dans une commande Tauri synchrone (`#[tauri::command(async)]`).
- **Erreurs** :
  - `Result<_, String>` aux frontières IPC (message en français, montré à l'utilisateur) ;
  - `thiserror` dans les modules (`SpeechError`, `AudioError`) ;
  - jamais de `unwrap()` sur une E/S, une analyse ou une donnée externe.
- **`unwrap` et `expect`** : `lock().unwrap()` est accepté ([D-06](../architecture/decisions.md) : la release est en `panic = "abort"`). Un `expect` porte un message qui dit pourquoi il ne peut pas échouer.
- **Fichiers partagés avec l'installateur** (`downloads.rs`, `paths.rs`, `speech/models.rs`, `translation/models.rs`) : ils n'importent que d'autres fichiers partagés. `npm run check:architecture` le vérifie.
- **Commentaires** :
  - en anglais dans le code ;
  - textes visibles en français ou passés par l'i18n côté interface ;
  - chaque module commence par un `//!` qui dit son rôle.
- **Mise en forme** : `cargo fmt`, et `cargo clippy --all-targets -- -D warnings` propre dans les deux crates.

## Interface (`src`)

- **Organisation par fonctionnalité** :
  - `src/features/<nom>/` regroupe composants, store et règles ;
  - `src/shared/` contient ce qui sert partout (`ipc`, `i18n`, `ui`) ;
  - `src/app/` la fenêtre principale ;
  - `src/windows/` les fenêtres secondaires.
- **Couche IPC** : seul `src/shared/ipc` appelle `invoke` et `listen`. Elle n'importe rien de l'interface. Les composants importent `../../shared/ipc`.
- **État** :
  - Zustand ; un store par source de vérité ;
  - l'état d'un service Rust a un seul miroir (`useSpeech`, `useTranslation`…), mis à jour par son pont dans `app/native.ts` ;
  - pas de copie d'un état dans un autre store.
- **Abonnements** : toute fonction `start…Bridge`, `listen` ou `subscribe` rend un désabonnement, et l'appelant l'appelle au démontage. Les minuteries d'un pont sont annulées à son arrêt (`debouncedSync`).
- **Types** : pas de `any`. Un `as` se justifie en commentaire.
- **Textes** : toujours par `useT()` / `t()` (9 langues, [docs/i18n.md](../i18n.md)).

## Git

- **Branches** : `feat/*`, `fix/*`, `chore/*`, `refactor/*` partent de `develop`. On ne modifie jamais `main` directement.
- **Commits** : [Conventional Commits](https://www.conventionalcommits.org/fr/), message en français, atomiques (un commit compile et passe ses tests).
- **Pull Requests** :
  - vers `develop`, avec le rapport de validation locale ;
  - rien ne fusionne automatiquement ;
  - une refactorisation ne contient aucun changement fonctionnel ; un défaut trouvé en chemin est signalé et corrigé dans une PR `fix/*`.
