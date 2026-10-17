# Linear → Claude Code → GitHub : le workflow `/linear-ticket`

Ce guide complète [`CLAUDE.md`](../../CLAUDE.md) et [`CONTRIBUTING.md`](../../CONTRIBUTING.md) sans les remplacer. Il reprend le document Linear « Crosstalk — Protocole agents ChatGPT / Linear / Claude Code ».

## Qui fait quoi

| Acteur | Rôle |
| --- | --- |
| **Propriétaire du projet** | Décide des priorités, écrit ou valide les tickets avec ChatGPT, lance Claude Code sur un ticket précis, relit et fusionne les PR. |
| **ChatGPT** | Product Manager / architecte : prépare les tickets (contexte, périmètre, critères d'acceptation, tests), relit les rapports dans Linear. |
| **Linear** | Source de vérité des tâches, priorités et états. |
| **Claude Code** | Ingénieur : traite **un** ticket désigné par l'utilisateur, ouvre une PR vers `develop`, rend compte dans Linear, puis s'arrête. |
| **GitHub** | Branches, commits, PR. Pas de GitHub Actions. |

Claude Code ne crée pas de tickets, ne choisit pas une tâche seul et ne surveille pas Linear. Il ne s'exécute que quand une session est ouverte et que l'utilisateur tape la commande.

## Utilisation

Dans une session Claude Code ouverte sur le dépôt :

```
/linear-ticket SAB-20 --dry-run    # analyse en lecture seule, aucun effet de bord
/linear-ticket SAB-20              # traite SAB-20, et seulement lui
```

- Le skill est dans [`.claude/skills/linear-ticket/SKILL.md`](../../.claude/skills/linear-ticket/SKILL.md). Il est chargé au démarrage d'une session (poste local ou session cloud) depuis le dépôt cloné : il faut donc qu'il soit présent sur la branche ouverte, en pratique sur `develop` une fois fusionné.
- `disable-model-invocation: true` : Claude ne peut pas le lancer de lui-même ; seule la commande tapée par l'utilisateur le déclenche.
- Identifiant absent ou invalide (`/linear-ticket`, `/linear-ticket 20`, deux tickets…) : refus, aucun autre ticket n'est choisi.

### `--dry-run` (simulation)

Lit le ticket, ses commentaires, ses relations, les règles du dépôt et les PR ouvertes. Affiche :
- si le ticket est prêt, ses dépendances et les doublons ;
- s'il est exécutable dans l'environnement actuel ;
- le plan (branche, fichiers, tests) et les commentaires Linear qui seraient publiés.

Rien n'est écrit : ni Linear, ni GitHub, ni le dépôt.

### Mode normal

1. Même analyse. Un arrêt (ticket terminé, dépendance ouverte, doublon, environnement incompatible) donne un commentaire Linear « BLOQUÉ — cause / action attendue », puis le skill s'arrête.
2. Plan affiché, **une** confirmation demandée à l'utilisateur.
3. Commentaire de départ et passage à **In Progress** dans Linear.
4. Branche `feat/`, `fix/` ou `chore/SAB-XX-slug` depuis `develop` à jour.
5. Implémentation du seul périmètre approuvé, tests réellement exécutés, `release-notes.json` si le changement est visible pour les joueurs.
6. PR vers `develop`, **jamais fusionnée** par le skill.
7. Rapport final dans Linear (format ci-dessous) et lien de la PR, puis arrêt.

Format du rapport, repris du protocole :

```
État : EN COURS / BLOQUÉ / PR OUVERTE / VALIDÉ
Branche :
PR :
Travail réalisé :
Tests exécutés (commandes + résultat) :
Tests Windows/CS2 non réalisés :
Risques / retour arrière :
Prochaine action :
```

## États Linear

| État | Sens | Qui le met |
| --- | --- | --- |
| Backlog | Idée non prête | Propriétaire / ChatGPT |
| Todo | Validé et priorisé | Propriétaire / ChatGPT |
| In Progress | Travail réellement commencé | Le skill, au démarrage |
| Done | Critères vérifiés **et** changement intégré | Propriétaire, ou Claude Code sur sa demande — jamais le skill seul |

Il n'existe pas d'état « Bloqué » : le statut reste le statut réel et un commentaire « BLOQUÉ — … » explique pourquoi. Un ticket qui exige un test en vraie partie n'est jamais terminé à la suite d'un test Linux ou d'une lecture du code.

## Autorisations

Les lectures (ticket, commentaires, protocole, état Git) sont pré-autorisées dans le skill (`allowed-tools`). Tout le reste demande l'accord de l'utilisateur au moment de l'action :
- commentaire et changement d'état Linear, lien de PR ;
- commit, `git push`, création de PR.

Une autorisation refusée n'est jamais retentée automatiquement : le refus figure dans le rapport.

Toujours exclus du skill : fusion, release, publication de la version de test, push sur `main`, modification de `stable/build-288`, GitHub Actions, suppression, création de ticket.

> Les noms d'outils de `allowed-tools` (`mcp__Linear__…`) correspondent au connecteur Linear nommé « Linear ». Si le connecteur porte un autre nom sur un poste, les lectures demandent simplement une confirmation : rien n'est autorisé en plus.

## Environnement : poste Windows ou session cloud

| Possible | Poste Windows | Session cloud (Linux) |
| --- | --- | --- |
| Tests interface (`npm test`, typecheck, build), contrôles `check:*` | ✅ | ✅ |
| Tests et clippy Rust | ✅ (MSVC) | ✅ Linux, clippy cible Windows GNU |
| `scripts\validate.ps1`, MSVC, installateurs | ✅ | ❌ |
| Essais en vraie partie CS2 | ✅ (humain) | ❌ |

Dans une session cloud, un ticket qui demande Windows ou CS2 (SAB-5, SAB-6, SAB-7, SAB-8 par exemple) est marqué BLOQUÉ, avec les instructions de test pour un humain (`docs/testing/`). Un résultat Linux n'est jamais présenté comme une validation Windows ou CS2.

## Exemple de cycle

```
/linear-ticket SAB-8 --dry-run
→ « Exécutable ici : non (BLOQUÉ — validate.ps1 exige Windows) », instructions PowerShell, aucune modification.

/linear-ticket SAB-19
→ plan, confirmation, commentaire de départ, In Progress,
  branche chore/SAB-19-linear-workflow, contrôles npm, PR vers develop,
  rapport dans Linear, arrêt.
```
