---
name: linear-ticket
description: Traiter uniquement le ticket Linear indiqué par l'utilisateur
argument-hint: "SAB-XX [--dry-run]"
disable-model-invocation: true
user-invocable: true
allowed-tools: mcp__Linear__get_issue, mcp__Linear__list_comments, mcp__Linear__get_document, mcp__Linear__list_issues, Bash(git status:*), Bash(git log:*), Bash(git branch --list:*), Bash(git fetch:*), Bash(uname:*), Bash(gh api "repos/aboalsim114/Crosstalk/pulls?state=open":*)
---

# /linear-ticket — un ticket Linear, et seulement lui

Arguments reçus : `$ARGUMENTS`

Ce skill est lancé **uniquement par l'utilisateur**. La commande `/linear-ticket SAB-XX` est l'ordre explicite de traiter **ce ticket-là et rien d'autre**. Guide complet : `docs/development/linear-workflow.md`. Les consignes de `CLAUDE.md` et `CONTRIBUTING.md` restent prioritaires.

## 0. Lire les arguments (avant toute autre action)

- Le premier mot doit être un identifiant `SAB-<nombre>` (majuscules ou minuscules ; le normaliser en `SAB-<nombre>`).
- Le seul autre mot accepté est `--dry-run`.
- Identifiant absent, invalide, plusieurs identifiants ou mot inconnu : **refuser** avec le message « Indiquez un seul ticket : `/linear-ticket SAB-XX [--dry-run]` » et **s'arrêter**. Ne jamais choisir un autre ticket, ne jamais « deviner ».
- `--dry-run` présent → **mode simulation** (section 2). Sinon → **mode normal** (section 3).

## 1. Analyse (commune aux deux modes, lecture seule)

1. **Ticket** : `get_issue` (avec relations) et `list_comments`. Ticket introuvable → refus, arrêt.
2. **Gouvernance** : lire `CLAUDE.md`, `CONTRIBUTING.md` et le document Linear « Crosstalk — Protocole agents ChatGPT / Linear / Claude Code » ; les décisions les plus récentes du ticket (description et commentaires) priment sur les anciennes.
3. **Statut** : Done ou Canceled → refus (déjà terminé), arrêt. Backlog ou Todo ne bloquent pas : la commande de l'utilisateur est l'autorisation. Ne jamais démarrer un ticket au seul motif qu'il est Todo.
4. **Dépendances** : une relation « bloqué par » vers un ticket non terminé → arrêt avec rapport, sauf si l'utilisateur confirme explicitement de continuer.
5. **Doublons** : chercher dans le projet Crosstalk (`list_issues` par mots du titre), dans les PR GitHub ouvertes (`gh api "repos/aboalsim114/Crosstalk/pulls?state=open" --jq '.[] | "#\(.number) \(.head.ref) \(.title)"'`) et dans les branches (`git branch --list -a '*SAB-XX*'`, sans tube) une PR ou une branche qui porte déjà `SAB-XX` ou le même sujet. Trouvé → arrêt avec rapport.
   Outil Linear indisponible dans la session → le dire, ne rien deviner, s'arrêter (le ticket n'est pas lu).
6. **Périmètre** : retenir seulement ce que le ticket approuve (par exemple « V1 uniquement ») ; noter ce qui est hors périmètre.
7. **Faisabilité dans cet environnement** :
   - indices d'un besoin Windows / CS2 : étiquette « Test Windows / CS2 », mots « vraie partie », « en partie », « CS2 lancé », « validate.ps1 », « MSVC », « installateur », « WebView2 », « FPS » ;
   - environnement réel : `uname -s`, présence de `powershell`/`pwsh` ;
   - besoin Windows/CS2 et environnement incompatible → **BLOQUÉ** : rédiger les instructions de test pour un humain (à partir de `docs/testing/` et `CONTRIBUTING.md` « Validation locale ») et s'arrêter. Ne jamais présenter un test Linux comme une validation Windows MSVC ou CS2 ;
   - doute → poser la question à l'utilisateur.
8. **Plan** : branche (`feat/`, `fix/` ou `chore/SAB-XX-slug`, depuis `develop`), fichiers probables, tests prévus, nouveauté joueurs (`release-notes.json`) oui/non, texte des commentaires Linear prévus.

## 2. Mode simulation (`--dry-run`) — aucun effet de bord

Interdit : commentaire ou statut Linear, création de ticket, branche, fichier, commit, push, PR, commande qui modifie quoi que ce soit (ni dans Linear, ni dans GitHub, ni dans le dépôt).

Afficher puis **s'arrêter** :

```
SAB-XX — simulation
Ticket : <titre> (<statut>, <priorité>)
Prêt : oui / non — <raison>
Dépendances : <aucune | liste>
Doublons : <aucun | liste>
Exécutable ici : oui / non (BLOQUÉ — <raison>) / à confirmer
Plan : <branche, fichiers, tests, release-notes>
Commentaires Linear qui seraient publiés : <textes>
Autorisations nécessaires en mode normal : <liste>
Aucune modification effectuée.
```

## 3. Mode normal

1. Faire l'analyse (section 1). Arrêt ou BLOQUÉ → publier dans Linear un commentaire « BLOQUÉ — <cause> / <action attendue> » (avec, si utile, les instructions de test humain), puis s'arrêter.
2. Afficher le plan et demander **une** confirmation à l'utilisateur avant la première écriture.
3. Commentaire de départ dans Linear (branche prévue, périmètre), puis passage du ticket à **In Progress**.
4. Vérifier `git status` propre ; `git fetch origin develop` ; créer la branche depuis `origin/develop`.
5. Implémenter **uniquement** le périmètre approuvé, en suivant `docs/development/conventions.md`.
6. Lancer les tests réellement possibles et noter les commandes et leurs résultats exacts, par exemple :
   - `npm run check:anticheat`, `npm run check:architecture`, `npm run check:release-notes`, `npm run typecheck`, `npm test`, `npm run build` ;
   - Rust si concerné (voir `CLAUDE.md`, `CONTRIBUTING.md`) ;
   - sur Windows : `scripts\validate.ps1`.
   Un test non lancé n'est jamais déclaré réussi.
7. Changement visible par les joueurs → entrée en tête de `release-notes.json` (règles de `CLAUDE.md`).
8. Commits clairs, `git push -u origin <branche>`, PR **vers `develop`** remplie selon `.github/pull_request_template.md`. **Ne pas fusionner.**
9. Rapport final dans Linear, puis lien de la PR sur le ticket :

```
État : PR OUVERTE (ou BLOQUÉ)
Branche :
PR :
Travail réalisé :
Tests exécutés (commandes + résultat) :
Tests Windows/CS2 non réalisés :
Risques / retour arrière :
Prochaine action :
```

10. **S'arrêter** et attendre un nouvel ordre. Ne pas enchaîner sur un autre ticket.

## Règles permanentes

- Une écriture Linear ou une commande refusée par l'utilisateur n'est **jamais** retentée automatiquement : la noter dans le rapport et laisser le statut réel.
- Jamais : push sur `main`, modification de `stable/build-288`, fusion de PR, release, GitHub Actions, suppression de ressources, création de ticket, surveillance de Linear en arrière-plan.
- Done : jamais décidé par ce skill. Seulement sur demande de l'utilisateur, quand les critères sont vérifiés et le changement intégré.
- Après une fusion demandée par l'utilisateur (hors de ce skill), `CLAUDE.md` impose de publier la version de test.
- Anti-triche : aucune lecture mémoire, injection ou hook de `cs2.exe`.
