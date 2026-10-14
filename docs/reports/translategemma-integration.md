# Intégration de TranslateGemma 4B : rapport technique

Branche `feat/translategemma-local`, à partir de `develop` (7bf2dc0). Octobre 2026.

## Résumé honnête

| Point | État |
|---|---|
| Moteur llama.cpp intégré (processus `crosstalk-llm`, sans Ollama ni Python) | **Fait, testé** sous Linux et sous Windows émulé (Wine), avec des modèles Gemma 3 miniatures à poids aléatoires |
| Téléchargement par l'installateur et l'application (progression, reprise, SHA-256, espace disque, réparation) | **Fait, testé** par les tests automatiques (serveur HTTP local, faux disque plein, fichier corrompu) |
| Carte Audio & IA, choix du moteur, repli Opus-MT | **Fait, testé** (Vitest, tests Rust) |
| Benchmark Opus-MT / TranslateGemma et corpus CS2 (48 annonces inventées) | **Fait**, outil prêt ; **non exécuté avec le vrai modèle** |
| Traduction réelle russe → français et anglais → français avec TranslateGemma | **Non vérifiée** : le vrai modèle n'a pas pu être téléchargé dans l'environnement de développement (voir Blocages) |
| Vitesse CPU / GPU, RAM, VRAM, FPS de CS2 | **Non mesurés** sur du vrai matériel |
| Installateur Windows | Construit par compilation croisée MinGW ; **pas encore lancé sur un vrai PC** |

TranslateGemma n'est donc **pas déclaré meilleur qu'Opus-MT** : le mode automatique l'utilise s'il est installé et assez rapide, et Opus-MT reste installé comme repli. Le protocole de validation sur Windows est dans [docs/testing/translategemma.md](../testing/translategemma.md).

## Modèle

| | |
|---|---|
| Modèle | **TranslateGemma 4B Instruct** (`google/translategemma-4b-it`), Google, janvier 2026, basé sur Gemma 3 (55 langues) |
| Fichier | `translategemma-4b-it.Q4_K_M.gguf`, **2 489 909 760 octets** (taille lue sur Hugging Face) |
| Source des poids | [mradermacher/translategemma-4b-it-GGUF](https://huggingface.co/mradermacher/translategemma-4b-it-GGUF), révision `main` : conversion statique llama.cpp du dépôt officiel. 263 000 téléchargements, dépôt non restreint. |
| Pourquoi pas le dépôt de Google | `google/translategemma-4b-it` est restreint : il faut un compte Hugging Face qui a accepté la licence. Il ne contient pas de GGUF. |
| Empreinte | **SHA-256 annoncé par Hugging Face** (identifiant d'objet Git LFS, en-tête `X-Linked-Etag` de l'URL `resolve`). Il est vérifié sur les octets reçus, puis enregistré dans `crosstalk-manifest.json`. La valeur n'a **pas pu être lue** depuis l'environnement de développement (Hugging Face bloqué). Une fois relevée sur un vrai PC (Audio & IA › Vérifier / réparer), il suffit de la figer dans `LlmSpec::sha256` (`src-tauri/src/translation/llm/catalog.rs`) : l'empreinte ne dépendra alors plus du serveur. Figer aussi la révision Git du dépôt (`revision`) à ce moment. |
| Quantification | Q4_K_M : compromis habituel taille / qualité pour un modèle 4B. Q5_K_M (2,83 Go), Q6_K (3,19 Go) et Q8_0 (4,13 Go) existent dans le même dépôt : changer d'entrée dans le catalogue suffit. |
| Projecteur vision (`mmproj`) | Non téléchargé : seule la traduction de texte sert. |

### Licence (Gemma Terms of Use)

Le modèle est distribué sous les [Gemma Terms of Use](https://ai.google.dev/gemma/terms), pas sous une licence libre classique. La page n'a pas pu être relue depuis l'environnement de développement (réseau bloqué). Ce résumé s'appuie sur les conditions connues ; il reste **à vérifier** sur la version en vigueur avant une distribution publique.

**Ce que la licence permet**
- Utiliser, reproduire, modifier et distribuer le modèle, y compris dans un produit commercial.
- Aucune redevance.

**Ce qu'elle impose à qui distribue le modèle ou un travail dérivé**
- Transmettre les restrictions d'usage (politique d'utilisation interdite) dans les conditions qui encadrent son utilisation.
- Fournir une copie des conditions Gemma.
- Signaler les fichiers modifiés.
- Joindre la mention « Gemma is provided under and subject to the Gemma Terms of Use found at ai.google.dev/gemma/terms ».

**Ce que fait Crosstalk**
- Crosstalk **ne redistribue pas les poids** : l'installateur ne contient pas le modèle. C'est le PC de l'utilisateur qui le télécharge depuis Hugging Face, où il est publié sous la même licence.
- La carte Audio & IA et `docs/translategemma.md` affichent la licence, son lien, la politique d'utilisation interdite et la provenance.
- Le modèle n'est pas modifié (aucun réentraînement, aucune fusion).

**À faire avant une diffusion publique large**
- Afficher les conditions Gemma et la politique d'utilisation interdite dans l'installateur. Une case « J'accepte » serait la mesure la plus prudente.
- Faire relire le point par une personne compétente.

Aucun blocage juridique identifié pour l'usage prévu (traduction de communications de jeu). La politique interdite vise des usages sans rapport.

## Format de prompt

Le format est celui du gabarit officiel `chat_template.jinja`. Il a été lu dans `onnx-community/translategemma-text-4b-it-ONNX` : 16 982 octets, même taille que le fichier du dépôt officiel, et lui-même restreint. Exemple ru → fr :

```
<bos><start_of_turn>user
You are a professional Russian (ru) to French (fr) translator. Your goal is to accurately convey the meaning and nuances of the original Russian text while adhering to French grammar, vocabulary, and cultural sensitivities.
Produce only the French translation, without any additional explanations or commentary. Please translate the following Russian text into French:


<texte><end_of_turn>
<start_of_turn>model
```

- `prompt.rs` reproduit exactement ce rendu. Un test le compare au texte attendu.
- Les jetons de contrôle (`<start_of_turn>`, `<end_of_turn>`…) sont retirés du texte transcrit : une phrase ne peut pas fermer le tour de parole et parler à la place du modèle.
- La génération s'arrête sur `<end_of_turn>` (jeton de fin de génération du GGUF).
- `clean_output` retire seulement ce qu'une traduction ne contient jamais : guillemets ajoutés, étiquette « Translation: », marqueur de tour.
- Aucune consigne générique n'a été ajoutée. L'effet d'éventuelles consignes CS2 (lexique, positions) n'a pas été mesuré, donc elles n'ont pas été introduites.

## Architecture

```
WASAPI → Silero VAD → Whisper ─┐
Chat CS2 (console.log) ────────┤
                               ▼
              TranslationService (file bornée à 8, cache LRU 1000, purge)
                 1. déjà en français → tel quel
                 2. lexique CS (callouts.rs) → instantané
                 3. HybridLoader / Hybrid (par langue)
                      ├─ TranslateGemma : LlmRuntime ─ stdin/stdout ─ crosstalk-llm.exe (llama.cpp, GGUF)
                      └─ Opus-MT (candle) : repli, et traductions provisoires sur processeur
                 glossaire (marqueurs KZn) autour du moteur
                               ▼
                       Overlay / fil / historique
```

**Pourquoi un processus séparé (`src-llm`, binaire `crosstalk-llm`)**

- whisper.cpp (dans l'application) et llama.cpp embarquent chacun leur propre copie de ggml, à des versions différentes. Liées statiquement dans le même exécutable, elles entrent en conflit de symboles.
- Un plantage ou un manque de mémoire du modèle n'emporte pas Crosstalk : le service passe sur Opus-MT.
- La priorité du processus est basse (`BELOW_NORMAL_PRIORITY_CLASS`).

**Sécurité du processus**

- Pas de serveur HTTP ni de port réseau : tout passe par l'entrée et la sortie standard, un objet JSON par ligne.
- Le moteur est lancé sans console (`CREATE_NO_WINDOW`). Il s'arrête de lui-même quand son entrée se ferme : arrêt du moteur, fermeture ou plantage de Crosstalk. Aucun processus orphelin.
- Il n'accède à aucun autre processus : `check:anticheat` couvre maintenant `src-llm`, et les sources de llama.cpp/ggml ne contiennent aucune des API interdites (vérifié par recherche).
- Le texte à traduire ne contrôle pas le moteur : ce sont des données, pas des commandes, et les jetons de contrôle sont retirés.

**Changer de modèle plus tard** : le moteur ne connaît pas le modèle. Il reçoit une invite complète et renvoie le texte généré. Un autre modèle GGUF demande :
1. une entrée `LlmSpec` dans `catalog.rs` ;
2. un `PromptFormat` dans `prompt.rs`.

**Inférence**
- `llama-cpp-2` 0.1.159, qui embarque llama.cpp 0.4.1-dev ; l'architecture `gemma3` est prise en charge.
- Décodage glouton déterministe.
- Contexte de 1024 jetons (une invite fait environ 100 jetons).
- Plafond de jetons générés : 24 à 160 selon la longueur de la phrase.
- Le cache clé/valeur de l'instruction commune est réutilisé d'une phrase à l'autre : seuls le texte et la réponse sont recalculés. Sous Wine, la seconde requête a réutilisé 95 jetons sur 96.

## Ressources, CS2 et repli

| Mécanisme | Réglage |
|---|---|
| Chargement | Une fois, au démarrage de la traduction ; gardé pendant la session ; libéré si la traduction est coupée ou si Opus-MT est choisi |
| Threads | Moitié des cœurs logiques, entre 2 et 6 |
| Priorité | Basse pour tout le processus |
| Carte graphique (mode *Auto*) | Utilisée seulement si la mémoire vidéo libre au chargement est au moins égale à la taille du modèle + 1,5 Go (place pour CS2 et Whisper). Sinon processeur, et la carte affiche pourquoi. |
| File | 8 phrases au plus ; une phrase finale qui attend plus de 5 s est rendue non traduite ; une traduction provisoire de plus de 2 s est abandonnée |
| Lenteur | Si la médiane des 6 dernières phrases dépasse 3,5 s, le mode *Auto* repasse sur Opus-MT (état « trop lent ») |
| Pannes | Une phrase de plus de 12 s redémarre le moteur ; 3 échecs de démarrage → Opus-MT pour la session (bouton « Réessayer ») |
| Traductions provisoires | Opus-MT (glouton) sur processeur ; TranslateGemma seulement sur carte graphique |
| Répondre (fr → en / ru) | TranslateGemma s'il est disponible, sinon Opus-MT ; le recueil de phrases passe avant |

Partage de la mémoire vidéo entre Whisper, TranslateGemma et CS2 :
- La seule protection est le seuil de mémoire libre au chargement.
- Il n'y a pas de surveillance continue de la mémoire vidéo.
- L'effet sur les FPS n'est **pas mesuré**.

## Téléchargement et installation

Le code de téléchargement est le même dans l'application et dans l'installateur (`src-tauri/src/downloads.rs`).

**Vérification**
- Taille exacte, puis SHA-256 (épinglé ou annoncé par le serveur), puis manifeste.
- Un fichier `.part` n'est jamais renommé en modèle tant que la taille et l'empreinte ne correspondent pas.
- Un modèle de la bonne taille sans manifeste (copie manuelle, réinstallation) est revérifié, **jamais retéléchargé** s'il est valide.

**Robustesse**
- Reprise par requête HTTP `Range` après une coupure.
- Refus avant de commencer si l'espace disque libre est inférieur à ce qui reste à télécharger + 256 Mo.
- Vitesse affichée (moyenne glissante).

**Installateur**
- Il affiche le nom du fichier, les Mo, le pourcentage, la vitesse et la phase de vérification.
- En fin d'installation, il lance `crosstalk-llm.exe --check` sans fenêtre : le modèle doit vraiment se charger. En cas d'échec, l'installation réussit avec un avertissement et Opus-MT est utilisé.

**Après l'installation**
- Audio & IA › Moteur de traduction : *Télécharger / Reprendre*, *Vérifier / réparer* (relit tout le fichier ; un fichier corrompu est supprimé), *Réessayer*.

**Distribution hors ligne** : copier le dossier du modèle et son manifeste (voir `docs/translategemma.md`). Un paquet hors ligne pourra fournir ces deux fichiers sans changement de code.

## Benchmark

**Outil** : `crosstalk-translate-bench` (`src-tauri/examples/translate_bench.rs`).

**Corpus** : `src-tauri/tests/data/translation-corpus.tsv`, 48 annonces **inventées**, 24 russes et 24 anglaises. Catégories : courtes, longues, nombres, positions, armes, argot, ambiguës. Chaque annonce a une référence française et la liste des termes à conserver.

**Ce que mesure l'outil, par moteur**
- temps de chargement ;
- latence p50 et p95 ;
- mémoire ;
- charge CPU (temps CPU du moteur rapporté à la durée) ;
- mémoire vidéo prise par le chargement ;
- termes conservés ;
- sorties suspectes : vide, non traduite, commentaire, beaucoup trop longue.

Le sens se juge à la lecture du tableau ligne par ligne.

**Transcription ou traduction ?** Avec `--transcripts`, chaque ligne est traduite deux fois : depuis le texte exact, puis depuis le texte de Whisper. Si le texte exact est bien traduit mais pas celui de Whisper, l'erreur vient de la transcription. Si les deux sont mal traduits, elle vient de la traduction.

10 des 48 lignes sont déjà connues du lexique CS. Dans l'application, elles n'atteignent jamais un modèle.

**Résultats avec le vrai modèle : aucun pour l'instant.** Le seul passage effectué utilise le modèle miniature à poids aléatoires. Il valide la chaîne (chargement, requêtes, tableau, attribution Whisper / traduction), pas la qualité ni la vitesse : ses sorties n'ont aucun sens par construction.

## Validation effectuée (environnement de développement Linux, sans GPU)

| Contrôle | Résultat |
|---|---|
| `src-llm` : `cargo test --release` (protocole, choix GPU, mémoire, temps CPU ; binaire réel sur modèles Gemma 3 miniatures) | 6 tests unitaires et 5 tests d'intégration réussis |
| `src-tauri` : `cargo test --lib` avec `CROSSTALK_LLM_EXE` (vrai moteur) | 256 réussis, 0 échec |
| `src-installer` : `cargo test` (dont le vrai `--check`) | 31 réussis |
| `cargo clippy --all-targets`, `cargo fmt` | aucun avertissement |
| `npx tsc --noEmit`, `npx vitest run` | 0 erreur ; 26 fichiers, 161 tests réussis |
| `npm run check:anticheat` (inclut `src-llm`) | OK |
| Compilation croisée `crosstalk-llm.exe` (MinGW, Vulkan) | 39 Mo ; dépendances : DLL système et `libstdc++-6.dll`, déjà livrée ; `vulkan-1.dll` chargée à l'exécution (le moteur démarre sans pilote Vulkan) |
| `crosstalk-llm.exe` sous Wine (sans Vulkan) | Repli processeur avec raison affichée, génération, réutilisation du préfixe (95/96 jetons), mémoire lue par l'API Windows, `--check` OK |

**Non effectué** :
- vraie traduction avec TranslateGemma ;
- installation sur un vrai Windows ;
- carte graphique Vulkan réelle ;
- mesures de FPS dans CS2 ;
- pipeline complet avec Whisper sur des voix.

## Blocages rencontrés

1. **Réseau** : `huggingface.co`, `ollama.com`, `github.com` (hors `gh`) et `ai.google.dev` sont refusés par le proxy de l'environnement de développement.
   - Ni le modèle, ni sa vraie empreinte SHA-256, ni la page de licence n'ont pu être récupérés.
   - L'API Hugging Face (outil MCP) a seulement permis de lire les tailles, les fiches et le gabarit de prompt.
   - Télécharger le modèle depuis une autre source non vérifiée a été écarté.
2. **Compilation croisée de llama.cpp (Vulkan)** : trois ajustements, intégrés à `scripts/cross-build-windows-gnu.sh`.
   - En-têtes SPIR-V (`spirv-headers`, lien dans le dossier du shim).
   - Archives MinGW sans préfixe `lib`, puis relance du script de build.
   - Fonction `vkGetDeviceProcAddr` ajoutée au shim Vulkan.
   - `advapi32` est lié à la fin.
3. **Version MSVC officielle** (`scripts/validate.ps1`) : elle construit et teste `crosstalk-llm`, mais les paquets NSIS/MSI de Tauri **n'embarquent pas encore** `crosstalk-llm.exe`. L'installateur distribué (`Crosstalk-setup.exe`, issu de la compilation croisée) l'embarque.

## Limites restantes et suite

- **Mesures sur un vrai PC, priorité absolue** : banc processeur et carte graphique, lecture des 48 lignes, FPS CS2.
- Ensuite seulement, décider si TranslateGemma reste le moteur automatique ou s'il devient optionnel.
- Figer le SHA-256 et la révision Git du dépôt de poids dans le catalogue.
- RAM : environ 3,3 Go de plus quand TranslateGemma est chargé. Sur un PC de 8 Go avec CS2, le mode Opus-MT est sans doute préférable. À mesurer.
- Pas d'annulation d'une génération en cours (le moteur est redémarré au-delà de 12 s). Une phrase périmée en cours de génération va jusqu'au bout, puis est jetée si la capture a été purgée.
- La surveillance de la mémoire vidéo se limite au chargement.
- Traduction de l'interface (i18n) de la nouvelle carte : à faire avec le reste de la page Audio & IA.
