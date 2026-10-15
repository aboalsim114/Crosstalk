# Rapport : traduction en ligne par un LLM (clé fournie par l'utilisateur)

Branche `feat/llm-translation`, partie de `develop` (`7edd9a7`), puis mise à jour avec `develop` (`37d9d71`, PR #85). Rapport rédigé le 14/10/2026.

## Point de départ

La mission décrivait un code déjà écrit mais jamais compilé :
- `llm.rs` ;
- `Engine::last_engine` ;
- `Translation.engine` ;
- `engines_changed` ;
- les commandes `llm_*` ;
- la fonctionnalité `Win32_Security_Credentials`.

**Rien de cela n'existait.** La branche `feat/llm-translation` était introuvable sur GitHub comme dans le conteneur, et aucun de ces éléments n'était présent dans `develop`. Le mainteneur a choisi de tout écrire ici. Le code de cette branche a donc été **écrit de zéro**, en suivant la conception décrite, et n'est pas une reprise de code existant.

## Ce qui a été fait

### Cœur Rust

**`src-tauri/src/translation/llm.rs`**

- **`LlmConfig`** (fournisseur, adresse de base, modèle, interrupteur) :
  - désactivé par défaut ;
  - adresse nettoyée (espaces, `/` final) ;
  - adresse vide : celle du fournisseur ;
  - `http://` refusé sauf pour un serveur local (localhost, boucle locale, réseau privé), qui peut fonctionner sans clé.
- **`LlmClient`** :
  - requête compatible OpenAI : `POST {base}/chat/completions` avec `Authorization: Bearer` ;
  - requête Anthropic : `POST {base}/messages` avec `x-api-key` et `anthropic-version: 2023-06-01`. Pas de `temperature`, refusée par les modèles Claude récents. `max_tokens: 1024`, pour laisser la place à la réflexion activée par défaut. `stop_reason: "refusal"` compte comme un échec ;
  - délai de **3,5 s**, consigne système fixe. Seul le texte de la ligne est envoyé ;
  - compteurs : appels, échecs, dernière latence, dernière erreur.
- **Erreurs** :
  - messages tirés **uniquement du code HTTP**. Le corps des réponses d'erreur est ignoré, car certains fournisseurs y recopient une partie de la clé ;
  - masquage de la clé par sécurité au cas où elle apparaîtrait dans un message.
- **Contrôle de plausibilité** :
  - la réponse est refusée si elle est vide, trop longue ou trop courte, contient des explications sur plusieurs lignes, ou si elle refuse ou commente ;
  - les guillemets et les préfixes « Traduction : » sont retirés.
- **`KeyStore`** :
  - `CredentialStore` (Windows) : identifiant générique `Crosstalk/llm-api-key`, via `CredWriteW`, `CredReadW`, `CredFree` et `CredDeleteW` (windows 0.62) ;
  - `MemoryKeyStore` hors Windows ;
  - `clean_key` rejette une clé vide, contenant des espaces ou trop longue.
- **`HybridLoader` / `HybridEngine`** :
  - le LLM d'abord, Opus-MT en secours sur toute erreur ;
  - les lignes en direct (`translate_fast`) restent locales ;
  - une route sans modèle Opus-MT n'existe que tant que le LLM est actif.
- **Transport HTTP abstrait (`Transport`)** : les tests n'atteignent jamais le réseau.

**Autres fichiers du cœur**

- `Engine::last_engine`, sur Marian et `Chain` : `"opus-mt"`.
- `Translation.engine` : `"llm"`, `"opus-mt"` ou `"lexicon"` (annonces Counter-Strike intégrées). Cette dernière valeur est un ajout à la conception, pour que le test de traduction dise d'où vient la phrase. Le cache conserve le moteur.
- `TranslationService::engines_changed` : vide le cache et reconstruit toutes les routes.
- Commandes `llm_status`, `llm_configure`, `llm_set_key`, `llm_clear_key` (`translation/mod.rs`), enregistrées et gérées dans `lib.rs`. Le journal note seulement « clé enregistrée » ou « clé effacée », jamais la clé.
- `Cargo.toml` : fonctionnalité `Win32_Security_Credentials`. Aucune nouvelle dépendance : `ureq` et `serde_json` étaient déjà là ; `ureq` est utilisé sans sa fonctionnalité `json`.

### Interface

- `src/shared/ipc/llm.ts`, exporté par `index.ts` : `llm.status`, `configure`, `setKey`, `clearKey`. Le statut indique `hasKey`, jamais la clé.
- État `llm` dans le store, désactivé par défaut :
  - conservé dans `config.json` (fournisseur, adresse, modèle), jamais la clé ;
  - les valeurs invalides sont ignorées au chargement.
- Pont natif : la configuration suit l'interface ; les compteurs sont rafraîchis toutes les 3 s quand la fonction est activée.
- Carte **« Traduction en ligne · LLM »** sous la carte de traduction locale, dans Audio & IA. Elle contient :
  - un interrupteur ;
  - le fournisseur ;
  - l'adresse, préremplie selon le fournisseur et conservée si l'utilisateur en a tapé une autre ;
  - le modèle, en saisie libre ;
  - la clé : champ mot de passe, vidé après l'envoi, boutons Enregistrer et Effacer ;
  - l'état : actif, problème ou désactivé, appels, échecs, dernière latence, dernière erreur ;
  - l'avertissement de confidentialité.
- Le test de traduction affiche le moteur : « LLM en ligne », « Opus-MT local » ou « lexique Counter-Strike ».
- La carte locale précisait « rien n'est envoyé en ligne » ; elle ajoute désormais « sauf si vous activez la traduction en ligne ci-dessous ».

### Documentation

- `docs/translation-llm.md` couvre :
  - le fonctionnement et le secours Opus-MT ;
  - la confidentialité et la clé ;
  - le choix du fournisseur ;
  - des exemples pour OpenAI, OpenRouter, Anthropic et Ollama ;
  - les requêtes exactes envoyées.
- README : ligne « Traduction en ligne (facultative) », et statut « Implémenté · à valider ».
- Licences : aucune mise à jour nécessaire, faute de nouvelle dépendance.
- `docs/architecture/ipc-contract.txt` : les 4 commandes.

## Fichiers modifiés

| Fichier | Lignes ajoutées ou modifiées |
| --- | --- |
| `src-tauri/src/translation/llm.rs` (nouveau) | 744 |
| `src-tauri/src/translation/llm/tests.rs` (nouveau) | 412 |
| `src-tauri/src/translation/mod.rs` | 66 |
| `src-tauri/src/translation/service/tests.rs` | 49 |
| `src-tauri/src/translation/service/worker.rs` | 16 |
| `src-tauri/src/translation/service/mod.rs` | 13 |
| `src-tauri/src/translation/engine.rs` | 11 |
| `src-tauri/src/lib.rs` | 10 |
| `src-tauri/src/translation/service/cache.rs` | 7 |
| `src-tauri/src/translation/service/types.rs` | 5 |
| `src-tauri/Cargo.toml` | 1 |
| `src/features/translation/LlmCard.tsx` (nouveau) | 122 |
| `src/features/translation/llm.test.tsx` (nouveau) | 119 |
| `src/features/translation/llm.ts` (nouveau) | 90 |
| `src/shared/ipc/llm.ts` (nouveau) | 45 |
| `src/app/native.ts` | 22 |
| `src/features/audio/AudioPage.test.tsx` | 21 |
| `src/app/state/persist.ts` | 15 |
| `src/app/state/persist.test.ts` | 10 |
| `src/app/state/store.ts` | 9 |
| `src/features/audio/AudioPage.tsx` | 9 |
| `src/shared/ipc/translation.ts` | 5 |
| `src/shared/ipc/index.ts` | 1 |
| `docs/translation-llm.md` (nouveau) | 125 |
| `docs/architecture/ipc-contract.txt` | 4 |
| `README.md` | 2 |

## Tests exécutés et résultats réels

Tous exécutés le 14/10/2026 dans un conteneur Linux x64, sur le résultat de la fusion de `develop` (`37d9d71`) dans la branche.

| Contrôle | Commande | Résultat |
| --- | --- | --- |
| Format Rust | `cargo fmt --all -- --check` | OK |
| Lint Rust (Linux) | `cargo clippy --all-targets -- -D warnings` | OK, 0 avertissement |
| Tests Rust (Linux) | `cargo test --all-targets` | **259 réussis**, 0 échec (249 sur `develop`) |
| Lint Rust (Windows) | `cargo clippy --target x86_64-pc-windows-gnu --no-default-features --lib --tests -- -D warnings` | OK. Compile `CredentialStore` avec windows 0.62 |
| Tests Rust (Windows, sous Wine) | exécutable de test `x86_64-pc-windows-gnu` lancé avec Wine, filtre `translation` | **56 réussis**, 0 échec, dont `credential_manager_round_trip` |
| Types | `npx tsc --noEmit` | OK |
| Tests interface | `npx vitest run` | **183 réussis**, 29 fichiers (173 sur `develop`) |
| Build interface | `npx vite build` | OK |
| Architecture et contrat IPC | `npm run check:architecture` | OK, 4 commandes ajoutées au contrat |
| Anti-triche | `npm run check:anticheat` | OK |
| Clé dans le code | recherche de `sk-…` dans le diff | seulement les jetons de test `sk-test-placeholder*` et `sk-test-second` |

Détail des nouveaux tests Rust :
- **10 tests dans `translation::llm`**, dont le test du Gestionnaire d'identifiants, compilé seulement sous Windows. Ils couvrent :
  - la configuration et les adresses locales ;
  - la forme exacte des requêtes OpenAI et Anthropic : le texte seul, pas de `temperature` côté Anthropic ;
  - les réponses avec bloc de réflexion et les refus ;
  - les erreurs 401, 429, délai, réseau et JSON illisible, sans que la clé fuie dans un message ou dans le statut ;
  - la plausibilité ;
  - l'activation selon l'interrupteur, le modèle et la clé ;
  - le moteur hybride et son secours ;
  - la route sans modèle local.
- **1 test de service** : moteur rapporté, conservé par le cache, cache vidé par `engines_changed`, `"lexicon"` pour les annonces intégrées.

Tous ces tests utilisent un transport simulé et des réponses simulées. **Aucune** n'est une réponse réelle d'un fournisseur.

Détail des nouveaux tests Vitest :
- **6 tests de la carte** :
  - désactivée par défaut ;
  - adresse préremplie ;
  - clé envoyée une fois, champ vidé, clé absente de la page et de l'état ;
  - erreur de clé affichée ;
  - effacement ;
  - compteurs et dernière erreur.
- **3 tests des fonctions utilitaires.**
- **1 test de persistance** : réglages conservés sans clé, valeurs invalides ignorées.
- **1 test du résultat de traduction** : moteur affiché.

## Ce qui n'a pas été testé

- **Aucun appel réel à une API de LLM.** Sans clé, aucun appel n'est possible. Rien n'a été envoyé à OpenAI, OpenRouter, Mistral, Groq, Anthropic, Ollama ou LM Studio. La forme des requêtes suit la documentation des API, mais aucune réponse réelle n'a été reçue.
  - Format Anthropic : vérifié avec la référence d'API fournie par l'outil `claude-api`. C'est elle qui a fait retirer `temperature`.
  - Format OpenAI : tiré de la documentation publique connue, non vérifié en direct.
- **Windows réel.** Rien n'a été lancé sur un vrai Windows 10 ou 11 : ni l'application, ni `scripts\validate.ps1`, ni la compilation MSVC, ni les installateurs.
- **Gestionnaire d'identifiants réel.** Le test d'aller-retour a tourné sous **Wine**, qui réimplémente `advapi32`, et non dans le vrai Gestionnaire d'identifiants Windows. Le comportement réel reste à confirmer, notamment la persistance après un redémarrage et la visibilité dans le Panneau de configuration.
- **Délai de 3,5 s en conditions réelles** : vérifié seulement par le code (`ureq` : `timeout_connect` + `timeout`). La détection « délai dépassé » repose sur le message de l'erreur `ureq` ; une coupure réseau peut aussi apparaître comme « réseau indisponible ». Dans les deux cas, Opus-MT prend le relais.
- **Latence, coût et qualité des traductions** des différents fournisseurs.
- **En partie dans CS2** : rien n'a été essayé.

## Limites connues

- Le délai de 3,5 s s'ajoute au temps de transcription. Si le fournisseur est lent, chaque ligne attend 3,5 s avant le secours Opus-MT. Il n'y a pas encore de coupe-circuit qui désactiverait temporairement le LLM après plusieurs échecs d'affilée.
- Modèles de raisonnement en mode compatible OpenAI (séries « o », GPT-5) :
  - certains refusent `temperature` ou `max_tokens`, et chaque ligne passe alors par Opus-MT ;
  - c'est documenté : il faut choisir un petit modèle rapide.
- Sur les modèles Claude récents, la réflexion est activée par défaut et peut augmenter la latence. Aucun paramètre ne la règle, car les modèles n'acceptent pas tous les mêmes réglages.
- Le contrôle de plausibilité est heuristique. Une traduction fausse mais bien formée passe.
- Le texte d'un message de chat peut contenir des instructions (« ignore previous instructions… »). La consigne système les désigne comme du texte à traduire, et la plausibilité rejette les refus et commentaires. Ce n'est pas une garantie absolue, mais l'effet est limité : on ne peut obtenir qu'un texte affiché dans l'overlay.
- `ureq` lit la réponse en entier : il n'y a pas de streaming. C'est sans importance pour des phrases courtes.
- **Hors Windows**, la clé reste en mémoire et doit être ressaisie à chaque lancement. C'est sans effet en pratique, l'application ne visant que Windows.
- Les réglages (sans la clé) sont envoyés au cœur Rust 400 ms après la dernière frappe. Pendant la saisie d'une adresse, des configurations intermédiaires invalides sont donc signalées comme « problème ».
- La réponse dans la langue des coéquipiers (Ctrl+Alt+F6 / F7) n'utilise pas le LLM : elle reste sur Opus-MT et le lexique.

## Test manuel sous Windows, avec sa propre clé

Prérequis : une version de cette branche construite sous Windows (`scripts\validate.ps1`), et une clé chez au moins un fournisseur. Ollama sert pour le cas sans clé.

1. **Désactivée par défaut**
   - Lancer Crosstalk et ouvrir Audio & IA.
   - Attendu : la carte « Traduction en ligne · LLM » affiche « Désactivé · Opus-MT local seulement ».
   - Lancer un test de traduction : il affiche « Opus-MT local » (ou « lexique Counter-Strike » pour une annonce connue).
2. **Clé dans le Gestionnaire d'identifiants**
   - Choisir « Compatible OpenAI », garder `https://api.openai.com/v1`, modèle `gpt-4o-mini`, coller la clé, puis Enregistrer.
   - Attendu : le champ se vide et la carte affiche « Clé enregistrée · Gestionnaire d'identifiants Windows ».
   - Ouvrir Panneau de configuration › Gestionnaire d'identifiants › Informations d'identification Windows. Attendu : une entrée générique `Crosstalk/llm-api-key` existe.
3. **Activation**
   - Activer l'interrupteur. Attendu : « Actif · Opus-MT en secours ».
   - Test de traduction en RU : « Двое идут на B, двое на B ».
   - Attendu : une traduction suivie de « · LLM en ligne », et le compteur à « 1 appel · 0 échec · dernière latence … ms ».
4. **Aucune fuite de la clé**
   - Ouvrir `%APPDATA%\Crosstalk\config.json` : il contient `llm` (fournisseur, adresse, modèle) mais **pas** la clé.
   - Ouvrir le dossier du journal (Diagnostic) et chercher la clé : elle ne doit apparaître nulle part. Seules les lignes « clé API du LLM enregistrée » et « effacée » doivent être présentes.
5. **Clé refusée**
   - Enregistrer une fausse clé (`sk-faux`) et lancer un test.
   - Attendu : la traduction vient de « Opus-MT local », et la carte affiche « 1 échec » et « Dernière erreur : clé refusée par le fournisseur ».
   - Remettre la vraie clé.
6. **Pas de réseau**
   - Couper le Wi-Fi ou débrancher le câble, puis lancer un test.
   - Attendu : « Opus-MT local » en moins de 4 s environ, et une erreur « délai dépassé » ou « réseau indisponible ».
7. **Anthropic**
   - Choisir « Anthropic » : l'adresse devient `https://api.anthropic.com/v1`.
   - Modèle `claude-haiku-5-5` (ou `claude-haiku-4-5`), avec une clé Anthropic.
   - Attendu : « · LLM en ligne ». Noter la latence (réflexion activée par défaut sur `claude-haiku-5-5`).
8. **OpenRouter**
   - Adresse `https://openrouter.ai/api/v1`, modèle `openai/gpt-4o-mini`, clé OpenRouter.
   - Attendu : « · LLM en ligne ».
9. **Ollama, sans clé**
   - Effacer la clé, puis lancer `ollama pull qwen2.5:7b` et `ollama serve`.
   - Adresse `http://localhost:11434/v1`, modèle `qwen2.5:7b`.
   - Attendu : « Actif » sans clé, et « · LLM en ligne ».
   - Essayer aussi `http://exemple.com/v1`. Attendu : « Problème : adresse http:// acceptée seulement en local ».
10. **En partie (CS2 en fenêtré sans bordures)**
    - Avec le LLM actif, vérifier que les lignes finales de l'overlay sont traduites et que les lignes en direct restent fluides (elles restent locales).
    - Observer le compteur d'appels et la latence pendant quelques rounds, et noter tout ralentissement des FPS (surtout avec Ollama).
11. **Effacer**
    - Cliquer sur Effacer. Attendu : « Aucune clé enregistrée » et l'entrée disparaît du Gestionnaire d'identifiants.
    - Désactiver l'interrupteur et relancer l'application. Attendu : la fonction reste désactivée, et le fournisseur, l'adresse et le modèle sont conservés.

Reporter dans la PR les résultats, les latences observées par fournisseur et toute erreur affichée, sans jamais copier la clé.

## Mise en route simplifiée (feat/llm-simple-setup)

Demande : un joueur qui ne connaît pas les clés API doit pouvoir activer la traduction en ligne, et Ollama doit être utilisable avec une clé (Ollama Cloud).

### Ce qui a changé

- **Services prêts à l'emploi** : OpenAI, OpenRouter, Mistral AI, Groq, Anthropic, Ollama Cloud, Ollama sur ce PC, LM Studio sur ce PC, Autre. Choisir un service remplit l'adresse, le format d'API et un modèle proposé. L'adresse, le modèle et le format passent dans « Réglages avancés ».
- **Parcours en 3 étapes** : choisir le service ; « Obtenir ma clé … ↗ » ouvre la page de création de clé du service ; coller la clé.
  - Les pages viennent d'une liste fixe côté Rust (`KEY_PAGES`, commande `llm_open_key_page`) : l'interface ne transmet qu'un identifiant de service.
- **Ollama Cloud** : `https://ollama.com/v1`, compatible OpenAI, clé Bearer créée sur ollama.com/settings/keys. Aucun code de transport nouveau : c'est le client compatible OpenAI existant.
- **Ollama sur ce PC** : la commande `llm_ollama_local` lit `http://localhost:11434/api/tags` (connexion 0,8 s, total 1,5 s).
  - Si Ollama répond avec des modèles, la liste s'affiche et le premier modèle est choisi.
  - Sinon, l'interface propose « Télécharger Ollama ↗ », la commande `ollama pull qwen2.5:7b`, et « Vérifier à nouveau ».
- **« Tester la connexion »** (`llm_test`) :
  - les réglages affichés sont d'abord envoyés au cœur ;
  - la phrase d'exemple « Двое идут на B, двое на B » est ensuite traduite vers la langue cible ;
  - le résultat affiche la traduction et le temps, ou l'erreur avec un conseil en clair ;
  - le test fonctionne même interrupteur coupé, et la clé est retirée de tout message.
- Les trois nouvelles commandes sont `async`, donc hors du fil principal (leçon du blocage au démarrage corrigé en 0.4.1).
- Réglages : un champ `service` est ajouté. Pour les réglages enregistrés avant, il est déduit de l'adresse ; une adresse inconnue donne « Autre ». La clé n'est toujours stockée que dans le Gestionnaire d'identifiants.
- Le modèle Anthropic proposé est `claude-haiku-5-5`.

### Ce qui n'a pas été testé

- **Toujours aucun appel réel** : ni à Ollama Cloud, ni aux autres services, ni à un Ollama local réel.
  - Les tests utilisent un transport simulé et des réponses `/api/tags` écrites à la main, sur le format de la documentation d'Ollama.
  - Le nom de modèle `gemma4:31b` vient de l'exemple de la documentation Ollama Cloud ; la liste réelle peut différer.
- L'ouverture des pages dans le navigateur (`explorer <url>`) n'a pas été lancée sous un vrai Windows.

### Test manuel complémentaire sous Windows

1. **Ollama Cloud**
   - Choisir « Ollama Cloud », puis « Obtenir ma clé Ollama Cloud ↗ ». Attendu : ollama.com/settings/keys s'ouvre dans le navigateur.
   - Créer et coller la clé, puis « Tester la connexion ». Attendu : « ✓ Connexion réussie » et une traduction.
2. **Ollama sur ce PC**
   - Sans Ollama : choisir le service. Attendu : « Ollama n'est pas détecté » et le bouton de téléchargement.
   - Installer Ollama, puis `ollama pull qwen2.5:7b`, puis « Vérifier à nouveau ». Attendu : « Ollama détecté · 1 modèle installé », le modèle est choisi, aucun champ de clé.
3. **Mauvaise clé** : coller `sk-faux` et tester. Attendu : « ✗ clé refusée par le fournisseur » et un conseil pour recréer la clé.
