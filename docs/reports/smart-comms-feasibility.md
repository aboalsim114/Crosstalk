# Smart Comms et multilingue : faisabilité et plan

Branche `feat/smart-comms`, depuis `develop` (build 300). Ce document précède toute implémentation. Il s'appuie sur le code du dépôt et sur le modèle de reconnaissance réellement livré.

## 1. Incompatibilités, à lire d'abord

### 1.1 La reconnaissance vocale ne couvre que 6 des 10 langues demandées

Le moteur livré est Parakeet TDT 0.6B v3 (sherpa-onnx, D-14), défini dans `src-tauri/src/speech/models.rs:68`. Deux preuves indépendantes :

- **Fiche officielle** (`nvidia/parakeet-tdt-0.6b-v3`) : 25 langues européennes, soit bg, cs, da, de, el, en, es, et, fi, fr, hr, hu, it, lt, lv, mt, nl, pl, pt, ro, ru, sk, sl, sv, uk.
- **Vocabulaire du modèle** (`tokens.txt`, 8 193 jetons) : uniquement des caractères latins (21 288), cyrilliques (3 398) et grecs (1 364). Aucun caractère arabe, chinois ou japonais, et aucune des lettres turques ğ, ş, ı. Le modèle ne peut donc pas écrire ces langues, quel que soit le réglage.

| Langue | Voix (Parakeet) | Chat écrit | Langue cible (LLM) |
|---|---|---|---|
| Anglais | oui | oui | oui |
| Français | oui | oui | oui |
| Russe | oui | oui | oui |
| Espagnol | oui | oui | oui |
| Allemand | oui | oui | oui |
| Portugais | **oui** (pas encore proposé dans l'app) | à ajouter (détection) | à ajouter |
| Turc | **non** (alphabet incomplet) | oui | oui |
| Arabe | **non** | à ajouter (détection : alphabet arabe) | à ajouter (affichage de droite à gauche) |
| Chinois | **non** | à ajouter (détection : idéogrammes) | à ajouter (police CJK) |
| Japonais | **non** | à ajouter (détection : kana) | à ajouter (police CJK) |

L'ukrainien et le polonais, déjà présents, restent : leur retrait serait une régression.

**Conséquence** :
- **Langue cible** : les 10 langues peuvent être proposées, car la traduction passe par le LLM (Ollama Cloud ou autre), qui écrit dans toutes.
- **Voix** : Crosstalk ne doit pas promettre de reconnaître l'arabe, le chinois, le japonais ou le turc parlés.
- **Chat écrit** : ces quatre langues peuvent être détectées et traduites.

Les reconnaître à la voix demanderait un autre moteur, par exemple un modèle multilingue en plus de Parakeet. Cela tombe sous la contrainte « pas de remplacement du moteur sans preuve ni accord explicite ». Ce n'est pas fait ici ; c'est une piste séparée (§ 7).

### 1.2 « Pas de choix manuel des langues des joueurs » change des comportements existants

Aujourd'hui, l'utilisateur coche les langues de ses coéquipiers (`src`). Cette liste sert à quatre endroits :

1. **Reconnaissance** : `identify` choisit la langue d'une transcription parmi les langues cochées (`speech/engine.rs:57`). Sans liste, un texte latin sans mot reconnaissable est étiqueté anglais, avec une confiance de 0,5.
2. **Traduction** : une langue non cochée n'est pas traduite (« langue non choisie », `translation/service/worker.rs:102`).
3. **Répondre** : les boutons de langue sont les langues cochées (`translation/languages.rs:20`).
4. **Premier lancement** : la fenêtre de choix des langues (`LanguagesCard.tsx`).

Le passage à la détection automatique est faisable. Il demande de remplacer ces quatre usages : liste « langues reconnues » fixe pour la voix, plus de filtre côté traduction, boutons Répondre tirés des langues entendues récemment (`recentLangs`, qui existe déjà).

### 1.3 Ce qui ne peut pas être vérifié depuis l'environnement de développement

Pas de Windows, pas de CS2, pas de micro réel. Les **tests audio réels dans CS2** et les **mesures de latence avant/après en partie** doivent être faits sur le PC de référence. Ce plan fournit l'instrumentation et le protocole pour que ces mesures soient lisibles dans le journal (§ 6).

## 2. Objectif 1 : détection automatique de la langue parlée

### État actuel

- Parakeet écrit dans la langue entendue.
- La langue est ensuite déduite du texte par `chat::lang::detect` (`chat/lang.rs:141`), restreinte aux langues cochées.
- Le score `lang_prob` (0,9 / 0,5 / 0,2) est calculé mais n'est utilisé nulle part.

### Plan

- **Langues reconnues à la voix** : liste fixe en, fr, ru, es, de, pt, uk, pl (`Lang` étendu à `pt`). La liste ne dépend plus d'un choix de l'utilisateur.
- **`detect`** :
  - ajouter le portugais (mots fréquents, ã/õ) ;
  - fiabiliser l'arabe, le chinois et le japonais : présence de kana → japonais, idéogrammes seuls → chinois, au lieu du dernier caractère vu ;
  - compter les formes de présentation arabes.
- **Confiance** : `identify` renvoie une confiance réelle. La règle sûre :
  - en dessous de 0,7, la langue est « incertaine » et la ligne est traduite, jamais masquée ;
  - le texte latin sans indice reste « incertain » au lieu d'être déclaré anglais.
- **Interface** :
  - la carte « Mes langues » ne garde que la langue cible ;
  - la liste des langues des coéquipiers devient informative : reconnues à la voix, ou chat seulement ;
  - les anciens réglages `src` sont ignorés à la lecture.

### Tests

- Tests unitaires de `detect` et `identify` sur des phrases des 10 langues, y compris textes courts, mélange kanji+kana et confusion portugais/espagnol.
- Test qu'aucune langue non reconnue à la voix n'est annoncée comme telle.

## 3. Objectif 2 : traduction multilingue

### État actuel

- Cible unique, choisie parmi 8 langues (`TARGET_LANGS`).
- Le nom anglais de chaque langue figure dans le prompt (`llm/prompt.rs:12`).

### Plan

- **Langues cibles** : en, fr, ar, ru, zh, es, de, pt, tr, ja, plus uk et pl déjà présentes.
  - Ajout des noms dans le prompt, `NATIVE_NAME` et les libellés des 9 langues d'interface.
  - Persistance compatible avec l'ancien format.
- **Affichage** :
  - les blocs de texte traduit portent `dir="auto"`, pour que l'arabe s'affiche de droite à gauche ;
  - polices de secours CJK (Microsoft YaHei, Yu Gothic, présentes sous Windows) dans la pile CSS.
- **Phrases rapides et callouts intégrés** :
  - aujourd'hui seulement fr ↔ en/ru ;
  - hors du français, la traduction passe par le LLM, comme aujourd'hui.
- **Ollama Cloud** : le fournisseur et le client ne changent pas. Seul le nom de langue dans le prompt est ajouté.

### Tests

- Prompt correct pour chaque cible.
- Persistance (ancienne cible conservée).
- Rendu `dir="auto"` dans le Script et l'overlay.

## 4. Objectif 3 : masquer la transcription dans sa propre langue

### État actuel

- Une ligne déjà dans la langue de l'utilisateur est affichée telle quelle, avec la mention « déjà dans votre langue » (`worker.rs:79`, `store.ts:446`).
- Dans l'**overlay**, elle apparaît en double, sous un en-tête « → langue » (`OverlayPage.tsx:27`). C'est un défaut.
- Le réglage `tg.skipFr` existe dans le store mais ne sert qu'à la démo, sans aucune option dans l'interface.

### Plan

- **Option « Masquer les phrases déjà dans ma langue »**, activée par défaut et désactivable (Overlay et Script).
- **Une ligne est masquée seulement si** :
  - la langue détectée est la langue cible ;
  - la confiance est ≥ 0,7 ;
  - ce n'est pas une ligne provisoire.
- **Une langue incertaine n'est jamais masquée** : la ligne part en traduction, et le LLM la rend telle quelle si elle est déjà dans la langue cible.
- **Défaut de l'overlay** : l'affichage en double est corrigé quand l'option est désactivée.
- **Historique et récapitulatif** gardent la ligne : seul l'affichage en direct est filtré.

### Tests

- Masquée si sûre.
- Affichée si incertaine.
- Affichée si l'option est désactivée.
- Overlay sans doublon.

## 5. Objectif 4 : Smart Comms (termes CS2)

### État actuel

- Le prompt demande déjà de garder les nombres, positions, armes et termes de jeu.
- Le glossaire protège les termes par des codes (KZ1…).
- Les callouts de la carte jouée (GSI `keep`) sont ajoutés au glossaire.
- Un lexique instantané (`callouts.rs`) traduit les annonces en/ru exactes vers le français.
- La validation de sortie (`llm/validate.rs`) **ne vérifie pas** que les nombres et les lieux sont conservés.

### Plan

- **Garde-fou anti-hallucination**, après chaque traduction LLM :
  - les nombres (chiffres et nombres écrits de 1 à 5 en en/fr/ru/es/de/pt) doivent avoir la même valeur dans la source et la traduction ;
  - les callouts de carte et termes protégés présents dans la source doivent être présents dans la traduction ;
  - un callout ou un nombre absent de la source ne doit pas apparaître.
  - En cas d'écart : un nouvel essai avec la consigne renforcée. Si l'écart persiste, la ligne est affichée en original avec la mention « traduction écartée : nombre ou position modifié ». Rien d'inventé n'est affiché.
- **Directions** : gauche/droite, haut/bas, devant/derrière, « behind you »… sont ajoutées au prompt comme information tactique à préserver, et au détecteur pour le garde-fou. Elles sont traduites, pas conservées en anglais.
- **Lexique** : les callouts de base ne sont plus réservés aux cibles en français. Pour les autres cibles, ils passent par le LLM sous la protection du glossaire, sans table figée multilingue qui risquerait d'être fausse.

### Tests

- Corpus d'annonces : « two B long », « 1 left on A, low », « they're coming from behind » ; traductions simulées correctes et fautives.
- Le garde-fou accepte les bonnes et écarte celles qui changent un nombre, une position ou une direction.

## 6. Objectif 5 : latence intermittente de la reconnaissance

### Ce que le code montre déjà

- **Plancher fixe avant tout calcul** : environ 608 ms. Ce sont 416 ms de silence pour clore la phrase (`segmenter.rs`, `hang_frames` 13), plus 192 ms de pas VAD (`VAD_HOP_FRAMES` 6).
- **Phrase longue** : coupée à 5 s. Le texte final arrive donc jusqu'à environ 5,6 s + calcul après le début de la parole. Les lignes provisoires commencent à 1,5 s puis suivent au plus toutes les secondes.
- **Un seul fil de reconnaissance** :
  - file de 3 segments finaux, plus un emplacement provisoire servi seulement quand aucun final n'attend (`queue.rs`) ;
  - un passage provisoire déjà en cours retarde le final suivant de toute sa durée ;
  - un segment de plus de 6 s d'attente est abandonné (`STALE_AFTER`).
- **Charge processeur** : avec 4 processus chargés à côté (simulation de CS2), le temps de calcul de Parakeet passe de 480/690 ms à **961/2 170 ms** (p50/p95, banc du 16 octobre).
  - C'est le candidat le plus probable pour des retards intermittents : quand CS2 charge le processeur, un segment de 2 s peut demander plus de 2 s de calcul, et la file s'allonge.
- **Ollama Cloud** ne peut pas expliquer un retard **avant** l'apparition de la transcription : le texte reconnu est affiché avant d'être envoyé au LLM. La traduction a sa propre mesure (`queueMs`, `ms`).

### Ce qui n'est pas mesuré aujourd'hui

- `latency_ms` mesure de la fin de la voix au texte prêt, sans découpage par étape.
- L'attente en file n'apparaît que dans un mode diagnostic.
- La réception par l'interface n'est jamais mesurée.

### Plan

1. **Instrumentation par étape**, pour chaque phrase :
   - fin de voix → clôture du segment (silence + pas VAD) ;
   - clôture → début du calcul (file) ;
   - durée du calcul ;
   - émission → réception par l'interface.
   Chaque étape a son p50/p95 dans les statistiques. Le résumé du journal toutes les 30 s donne ces quatre p50/p95, ainsi que le nombre de segments abandonnés et le RTF.
2. **Mesure de référence (« avant »)** sur le corpus du banc (`scripts/asr-bench`) avec `crosstalk-asr-bench`, à vide et avec charge, publiée dans le rapport.
3. **Corrections ciblées, seulement après mesure** :
   - priorité au segment final sur le provisoire (interrompre ou ne pas lancer un provisoire si un final est imminent) ;
   - nombre de fils réglé selon la charge mesurée ;
   - abandon plus tôt d'un provisoire périmé.
   Chaque changement est mesuré de la même façon (« après »).
4. **Mesure en partie réelle** : protocole dans `docs/testing/speech-recognition.md`, à faire sur le PC de référence avec CS2 lancé. Le journal du jour suffit pour lire le découpage.

### Tests

- Calcul des p50/p95 par étape.
- Ordonnancement final/provisoire.
- Format du résumé du journal.

## 7. Hors périmètre de cette PR (accord explicite requis)

- **Reconnaissance vocale de l'arabe, du chinois, du japonais et du turc** : il faudrait un second moteur ou un autre modèle (par exemple un modèle multilingue en complément pour ces langues). C'est à évaluer par un banc comme celui de D-14, avec la question de la charge processeur en partie. Rien n'est remplacé ici.

## 8. Ordre de travail proposé

1. Instrumentation de latence et mesure « avant » (objectif 5, étapes 1-2).
2. Détection automatique, langues reconnues fixes, `pt` (objectif 1).
3. 10 langues cibles, affichage droite-à-gauche et CJK (objectif 2).
4. Option de masquage, en tenant compte de la confiance (objectif 3).
5. Garde-fou Smart Comms et directions (objectif 4).
6. Corrections de latence mesurées et mesure « après » (objectif 5, étape 3).
7. Tests, documentation, nouveautés joueurs, PR vers `develop` sans fusion.

## Contraintes respectées

- Ollama Cloud : inchangé.
- Pipeline audio : inchangé.
- Moteur ASR : inchangé.
- Branche dédiée, pas de GitHub Actions, pas de fusion automatique.
- Aucune interaction nouvelle avec CS2 : pas de lecture mémoire ni d'injection.
