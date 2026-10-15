# Rapport : migration vers une traduction 100 % LLM

Branche `feat/llm-only-translation`, partie de `develop` (`3e7eab1`). Pull Request vers `develop`, non fusionnée. Rapport rédigé le 15/10/2026.

> **À lire d'abord.** Rien de cette migration n'a été exécuté sur un véritable PC Windows, ni avec une vraie clé API, ni avec un vrai Ollama ou LM Studio. Aucune clé n'est disponible dans l'environnement de développement, et les serveurs d'Ollama (registre, téléchargements) y sont bloqués. Tous les tests de fournisseurs et de serveurs locaux utilisent un **transport HTTP simulé** qui renvoie des réponses écrites à la main. Ce qui a réellement tourné est détaillé dans [Tests exécutés](#tests-exécutés).

## Architecture avant / après

**Avant** (`develop` à `3e7eab1`) :

```
Voix : WASAPI → Silero VAD → Whisper → HybridEngine ─┬─ LLM en ligne (facultatif, désactivé par défaut)
Chat ─────────────────────────────────────────────────┘    └─ en cas d'échec : Opus-MT local (candle)
Répondre : phrases CS → Opus-MT (fr → en / ru)
Installateur : Whisper + VAD + modèles Opus-MT (≈ 1,9 Go)
```

**Après** :

```
Voix : WASAPI → Silero VAD → Whisper ─┐
Chat ─────────────────────────────────┼→ TranslationService → LlmClient → LLM choisi → Overlay / historique / radar / streamer / résumé
Répondre (écrit, dicté) ──────────────┘   (file, cache, glossaire,    ├─ Cloud : API compatible OpenAI ou Anthropic, clé de l'utilisateur
                                           priorités, délais, purge)   └─ Local : Ollama, LM Studio, adresse manuelle (sans clé)
Installateur : Whisper + VAD seulement (≈ 640 Mo)
```

Un seul trait relie le service au moteur (`translation::engine::Translator`) ; une seule implémentation : `LlmClient`. Pas d'abstraction par fournisseur : tous les services compatibles OpenAI passent par le même code, Anthropic a son format.

### Modules Rust (`src-tauri/src/translation/`)

| Module | Rôle |
| --- | --- |
| `llm/config.rs` | `LlmConfig` (connecté, fournisseur, adresse, modèle), type Cloud / Local, serveur local reconnu (Ollama, LM Studio), contrôle des adresses (`https://`, `http://` seulement en local, pas d'identifiants dans l'URL) |
| `llm/keys.rs` | `KeyStore` : Gestionnaire d'identifiants Windows (`Crosstalk/llm-api-key`), mémoire ailleurs ; nettoyage de la clé |
| `llm/transport.rs` | Trait `Transport` (POST, GET) et `UreqTransport` : sans redirection, erreurs décrites sans le corps de réponse |
| `llm/prompt.rs` | Consigne CS2, format TranslateGemma, requêtes OpenAI / Anthropic, liste des modèles (`GET …/models`), extraction de la réponse |
| `llm/validate.rs` | Rejet des réponses qui ne sont pas une traduction |
| `llm/local.rs` | Détection d'Ollama et de LM Studio et de leurs modèles réels |
| `llm/client.rs` | Appels, délais, pause de quota, blocage après clé refusée, provisoires, statistiques, tests de connexion et de traduction, préchauffage local |
| `service/` | File bornée, cache LRU, glossaire protégé, finales avant provisoires, délai maximal, purge |
| `languages.rs` | Les 8 langues, toutes cibles possibles |
| `legacy.rs` | Inventaire et suppression **sur demande** des anciens dossiers `opus-mt-*` |

Contrats Tauri : `translation_status`, `translation_configure`, `translation_rescan`, `translation_test`, `llm_status`, `llm_configure`, `llm_set_key`, `llm_clear_key`, `llm_test`, `llm_ollama_local`, `llm_open_key_page` sont conservés. Ajoutés : `llm_translate_test`, `llm_models`, `llm_local_servers`, `translation_legacy_models`, `translation_legacy_cleanup`. Retiré : `translation_models` (paires Opus-MT). `models_download_*` ne prennent plus de plan de traduction. Voir `docs/architecture/ipc-contract.txt`.

## Composants Opus-MT supprimés

- `src-tauri/src/translation/opus.rs`, `marian.rs` (réseau Marian sur candle), `network.rs`, `pth.rs` (lecture des poids PyTorch), `spm.rs` (tokenizer SentencePiece), `models.rs` (catalogue des 37 paires et routage par l'anglais) ;
- l'ancien `llm.rs` monolithique et `HybridEngine` (LLM devant, Opus-MT derrière), remplacés par le module `llm/` ;
- `src-tauri/examples/translate.rs` (`crosstalk-translate.exe`), `src-tauri/tests/fixtures/marian-tiny/`, `scripts/make-marian-fixture.py` ;
- le plan de téléchargement des paires (`TranslationPlan`) dans `downloads.rs`, l'installateur et l'interface ;
- la détection de la langue de Windows dans l'installateur (`user_locale`), qui servait à choisir les paires ;
- le bloc `RAYON_NUM_THREADS` de `lib.rs` (threads de candle) ;
- côté interface : paires, « via l'anglais », téléchargements de modèles de traduction, bandeau « Télécharger » de Répondre, textes « Opus-MT en secours », licences Opus-MT et candle ;
- dans `scripts/download-models.ps1`, `validate.ps1` et `cross-build-windows-gnu.sh` : l'option `-Translation` et l'exemple `translate`.

### Dépendances supprimées

`candle-core`, `candle-nn`, `candle-transformers` (0.11) et leurs dépendances transitives : environ 800 lignes de moins dans `Cargo.lock`. Aucune dépendance ajoutée : les requêtes LLM utilisent `ureq` et `serde_json`, déjà présents.

## Nouveau fonctionnement Cloud / Local

Interface : **Audio & IA › TRADUCTION IA** (remplace « Traduction en ligne facultative »). Détails pour l'utilisateur : [translation-llm.md](../translation-llm.md).

- **Cloud — Fournisseur API** : OpenAI, OpenRouter, Mistral, Groq, Anthropic, Ollama Cloud, ou toute adresse compatible OpenAI. Clé enregistrée / effacée, liste des modèles du fournisseur ou saisie libre, **accord d'envoi obligatoire avant la connexion**, test de connexion, test de traduction avec latence, connecter / déconnecter.
- **Local — LLM installé sur ce PC** : Ollama et LM Studio détectés, modèles réels listés, adresse manuelle avancée ; jamais de clé ni d'accord demandés ; rien n'est installé ni téléchargé par Crosstalk.
- **Panneau d'état** : fournisseur, modèle, type, serveur local, état, langues, traductions / appels / échecs, dernière latence, dernière erreur, pause de quota. L'usage facturé n'est pas affiché : aucune API commune ne le fournit.
- **Premier lancement** : après « Vos langues », fenêtre « Moteur de traduction » : API Cloud, Ollama local, LM Studio local, Configurer plus tard. Montrée une seule fois (`engineSetupDone`), jamais si un moteur est déjà connecté.
- **Sans moteur** : l'application reste utilisable ; les lignes restent dans leur langue avec « aucun moteur de traduction connecté (Audio & IA › Traduction IA) » (statut TRANSCRIT, pas une erreur) ; l'Accueil, le Diagnostic et Répondre l'indiquent.
- **Réglages existants** : conservés. Un fournisseur cloud déjà activé avant la migration garde son accord (il avait accepté l'avertissement de l'époque) ; sinon l'accord est demandé.

### Compatibilité Ollama

- Détection : `GET http://localhost:11434/api/tags` (1,5 s) ; traduction par l'API compatible OpenAI `http://localhost:11434/v1/chat/completions`, sans clé.
- Modèles : ceux qu'Ollama renvoie ; si le modèle choisi n'existe plus, le premier installé est proposé ; aucun modèle inventé.
- Délai de requête 15 s (premier chargement du modèle) ; préchauffage après la connexion.
- TranslateGemma reconnu par son nom (`translategemma:4b`…) : un seul message utilisateur au format officiel, sans message système.
- Ollama Cloud (`https://ollama.com/v1`, avec clé) est traité comme un fournisseur cloud.

### Compatibilité LM Studio

- Détection et modèles : `GET http://localhost:1234/v1/models` ; les modèles d'embedding sont écartés.
- Traduction par `http://localhost:1234/v1/chat/completions`, sans clé.

## « Réponds dans leur langue »

`ReplyService` reçoit le même `LlmClient` que le service de traduction :

1. les phrases CS toutes prêtes (« Éco », « Drop-moi une arme »…), écrites à la main, restent instantanées (français → anglais ou russe, `via: "phrasebook"`) ;
2. tout autre texte part au LLM connecté (`via: "llm"`), dans toutes les langues des coéquipiers, polonais et turc compris (impossible avec Opus-MT) ;
3. sans moteur, le texte libre est refusé avec la raison ; la fenêtre et la carte l'indiquent, les phrases toutes prêtes fonctionnent toujours.

La dictée (Ctrl+Alt+F6) suit le même chemin après Whisper. Exemple vérifié par test (moteur simulé) : « Mon équipe ne sait pas jouer. » → russe passe bien par le LLM avec la paire fr → ru.

## Gestion des erreurs et temps réel

| Situation | Comportement |
| --- | --- |
| Aucun moteur connecté | Ligne en langue d'origine, statut TRANSCRIT, raison affichée |
| Clé absente, refusée (401/403) ou crédit épuisé (402) | Aucun nouvel appel jusqu'au changement de clé ou de réglages ; message et conseil |
| Quota / limite de débit (429) | Pause de 20 s sans appel, affichée |
| Réseau absent, serveur local arrêté | « réseau indisponible » ; reprise automatique à la ligne suivante |
| Délai dépassé | Requête : 6 s cloud, 15 s local ; ligne : 8 s au total, puis « délai dépassé : la ligne reste dans sa langue » |
| Réponse commentée, vide, multiligne, refus | Rejetée ; la ligne reste dans sa langue |
| Redirection HTTP | Refusée (« vérifiez l'adresse ») : la clé ne suit pas une redirection |
| Provisoires | Jamais envoyés à un fournisseur cloud ; en local, au plus un toutes les 1,5 s |
| File pleine | 8 lignes au plus ; finales avant provisoires ; un provisoire remplace le précédent |
| Arrêt / changement de réglages | Purge : les lignes en attente ne partent pas |

Aucun thread sensible n'attend le réseau : le service a son thread, les commandes lentes sont `async`. Le streaming n'est pas utilisé (phrases de quelques jetons). Aucun secours automatique ; un LLM secondaire n'est pas implémenté (il ne le serait que configuré et autorisé par l'utilisateur).

**Correctif trouvé en route** (antérieur à la migration) : la validation des réponses découpait le texte sur un octet au milieu d'un caractère quand la réponse commençait par une lettre accentuée (« Ils ont posé… ») : plantage du thread de traduction. Corrigé (`get(..)`), test de régression ajouté.

## Fournisseurs réellement testés

**Aucun.** Pas de clé API disponible, pas d'accès réseau à Ollama. OpenAI, Anthropic, Mistral, Groq, OpenRouter, Ollama Cloud, Ollama local et LM Studio sont couverts uniquement par des tests à transport simulé, qui vérifient les requêtes envoyées (adresse, en-têtes, corps, délai) et le traitement des réponses et des erreurs. Aucune donnée simulée n'est présentée comme réelle.

## Tests exécutés

Tous exécutés dans le conteneur de développement Linux, sauf mention.

| Vérification | Résultat |
| --- | --- |
| `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (Linux) | OK / OK (aucun avertissement) |
| `cargo test --all-targets` (Linux) | **274 réussis**, 0 échec |
| Installateur : fmt, clippy, `cargo test` | OK / OK / **21 réussis** |
| Clippy croisé `x86_64-pc-windows-gnu` (`--no-default-features --lib --tests`) | OK (aucun avertissement) |
| Tests Rust Windows sous Wine (module `translation`, Gestionnaire d'identifiants) | `translation` : **54 réussis** (dont `credential_manager_round_trip`, vrai `CredWriteW` / `CredReadW` / `CredDeleteW` de Wine), `reply` : **11 réussis**. Suite complète : 278 réussis, 2 échecs dans `chat` (`tail::starts_at_the_end…`, `follows_new_messages…`) : **mêmes échecs sur `develop`** sous ce Wine (gestion des fichiers par Wine), module non modifié par cette branche |
| `tsc --noEmit`, `vite build` (application et installateur) | OK |
| Vitest | **234 réussis** (34 fichiers) |
| `check:architecture`, `check:anticheat` | OK / OK |

Ce que couvrent les tests (tous avec moteur ou serveur simulé) :

- **Cloud** : requête OpenAI (consigne CS2, une seule fois la phrase, pas d'audio ni de pseudo), Anthropic (pas de `temperature`, refus), chaque fournisseur compatible OpenAI, clé enregistrée / absente / refusée (blocage), crédit épuisé, quota (pause), délai, réseau, redirection, changement de fournisseur (cache vidé, blocage levé), liste des modèles avec la clé, clé jamais présente dans les erreurs, statuts, journaux ni réglages.
- **Local** : Ollama et LM Studio détectés avec leurs modèles, absents, démarrés sans modèle, modèle choisi, serveur arrêté pendant la traduction puis relancé (reprise sans reconfiguration), hors ligne (aucune clé requise), TranslateGemma.
- **Corpus FR / RU / EN** (5 phrases de la mission, dans les deux sens, pour OpenAI, Anthropic, Ollama et TranslateGemma) : vérification **sémantique** (négation, nombre, position, sens gardés ; une seule ligne), pas mot à mot. Les réponses sont écrites à la main : le test vérifie le chemin de Crosstalk, **pas la qualité d'un modèle**. Un test vérifie que la vérification détecte une négation, un nombre ou une position perdus.
- **Service** : finales avant provisoires, purge, file pleine, cache, glossaire, délai, lexique, moteur absent.
- **Répondre** : phrases toutes prêtes, LLM, moteur absent, langues de réponse.
- **Interface** (Vitest) : carte TRADUCTION IA (Cloud, Local, accord, clé, modèles, tests, état), premier lancement (4 choix, « plus tard » laisse l'application utilisable), carte de traduction, nettoyage Opus-MT sur demande, Répondre sans téléchargement, migration des réglages, licences.
- **Non-régression** : toute la suite Rust et Vitest (transcription, chat, overlay, historique, radar, réglages, installateur, modèles Whisper, voix IA, mode streamer, résumé).

**Sur un véritable PC Windows : rien.** Le protocole à suivre est dans [testing/translation.md](../testing/translation.md).

## Performances mesurées

- **Téléchargement à l'installation** : environ 1,9 Go → environ 640 Mo (Whisper base et small, VAD). Calculé à partir des tailles exactes des fichiers ; aucune installation réelle mesurée.
- **Mémoire de traduction** : plus aucun modèle Opus-MT chargé dans `crosstalk.exe` (environ 600 Mo de moins attendus d'après l'ancien protocole) ; non mesuré.
- **Latence de traduction réelle (cloud ou local), CPU / GPU, FPS** : **non mesurés** (pas de fournisseur ni de serveur local joignable). À relever avec le protocole.

## Problèmes restants

- Aucun essai réel : fournisseurs, Ollama, LM Studio, Gestionnaire d'identifiants sur un vrai Windows, partie de CS2.
- La qualité dépend du modèle choisi par l'utilisateur ; un petit modèle local peut se tromper sur les langues rares.
- La carte TRADUCTION IA et la carte de traduction sont en français seulement (comme le reste d'Audio & IA, voir `docs/i18n.md`) ; la fenêtre du premier lancement est traduite dans les 9 langues.
- Une seule clé est gardée : changer de fournisseur cloud demande de coller la clé du nouveau.
- Pas de LLM de secours configurable.
- PR ouvertes en conflit probable : **#60** (`feat/translategemma-local`, TranslateGemma par llama.cpp avec Opus-MT en secours) contredit cette migration et touche les mêmes fichiers ; **#57** (`feat/audio-pipeline-benchmark`) n'a pas été examinée en détail, elle porte sur l'audio. Elles n'ont pas été modifiées.
- Les documents historiques (`docs/quality/`, `docs/architecture/audit.md`, rapports précédents, cahier des charges) décrivent encore Opus-MT tel qu'il était à leur date ; ils n'ont pas été réécrits.

## Instructions d'utilisation

1. Installer ou mettre à jour Crosstalk (aucun modèle de traduction téléchargé).
2. Au premier lancement, choisir les langues, puis le moteur :
   - **API Cloud** : Audio & IA › TRADUCTION IA › Cloud ; choisir le fournisseur, « Obtenir ma clé », coller, Enregistrer ; choisir un modèle ; cocher l'accord ; Tester ; Connecter.
   - **Ollama** : installer Ollama, `ollama pull qwen2.5:7b` (ou `translategemma`) ; Local › Ollama ; choisir le modèle ; Connecter.
   - **LM Studio** : charger un modèle, démarrer le serveur local ; Local › LM Studio ; choisir le modèle ; Connecter.
   - **Configurer plus tard** : la transcription marche, la traduction attend un moteur.
3. Anciens modèles : Audio & IA › Traduction des parties › « Libérer l'espace » (facultatif).

## Commits et PR

| Commit | Contenu |
| --- | --- |
| COMMITS_TABLE |

Pull Request : PR_LINK (vers `develop`, non fusionnée, `main` non modifiée, pas de GitHub Actions).
