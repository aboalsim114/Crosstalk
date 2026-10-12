# Cahier des charges — Crosstalk

| | |
| --- | --- |
| **Version du document** | 2.0, octobre 2026 |
| **Statut** | Référence en vigueur. Remplace la v1.0 (`design/maquette/Cahier des charges.dc.html`, conservée pour l'historique) |
| **Plateforme** | Windows 10 (22H2) et Windows 11, x64 |
| **Stack** | Tauri v2 : cœur Rust, interface React + TypeScript rendue par WebView2 |
| **Architecture détaillée** | [architecture.md](architecture.md) |

Crosstalk est une application de bureau Windows qui traduit en français, en temps réel, le chat écrit et le chat vocal de Counter-Strike 2. Les traductions s'affichent dans un overlay transparent au-dessus du jeu.

## Historique des versions

| Version | Changements |
| --- | --- |
| 1.0 | Version initiale (stack egui/wgpu, locuteur hors périmètre, latence voix p50 ≤ 1,5 s). |
| **2.0** | **Stack officielle Tauri v2 + Rust + React/TypeScript.** Identification du locuteur ajoutée en fonctionnalité **expérimentale**, par HUD + OCR. Latence voix harmonisée à **p50 ≤ 2 s et p95 ≤ 4 s sur GPU, p50 ≤ 3,5 s sur CPU**. Nettoyage audio repriorisé : traitements légers en P0, traitements coûteux à évaluer. Widget Xbox Game Bar reporté en **P2**. **L'état de session est géré par Rust.** Interdiction de lecture mémoire réaffirmée, et explicitement écartée pour l'identification du locuteur. Planning actualisé : interface cinématique déjà livrée. |

---

## 1. Contexte et objectifs

### 1.1 Contexte
En matchmaking européen sur CS2, les joueurs francophones se retrouvent souvent avec des coéquipiers russophones ou anglophones. Les annonces vocales (« callouts ») et les messages du chat sont alors incompréhensibles, ce qui nuit à la coordination de l'équipe.

### 1.2 Objectifs
- Traduire en français les messages du chat écrit en moins d'une seconde.
- Transcrire puis traduire le chat vocal avec un délai médian de 2 secondes au plus sur GPU. C'est un objectif initial, à améliorer à partir des mesures réelles.
- Afficher, quand c'est possible de façon fiable, le pseudo du joueur qui parle. Sinon, afficher « Joueur inconnu ». Fonctionnalité expérimentale (§4.8).
- Afficher les traductions sans gêner la partie : overlay transparent, qui laisse passer les clics et ne prend jamais le focus clavier.
- Ne jamais interagir avec le processus du jeu d'une manière susceptible de déclencher l'anti-triche VAC (§3).
- Garder la voix en local par défaut : aucune transmission d'audio sans accord explicite.

### 1.3 Indicateurs de réussite

| Indicateur | Cible |
| --- | --- |
| Latence chat (lecture → affichage) | p50 ≤ 0,6 s · p95 ≤ 1,2 s |
| Latence voix (**fin de parole → affichage final**) | **p50 ≤ 2 s · p95 ≤ 4 s (GPU) ; p50 ≤ 3,5 s (CPU)** |
| Taux d'erreur de transcription (WER) sur le corpus de test CS2 | ≤ 25 % en russe, ≤ 18 % en anglais |
| Identification du locuteur (expérimentale) | Aucun pseudo erroné affiché : précision ≥ 98 % sur les pseudos affichés. Le taux de « Joueur inconnu » est mesuré, sans cible pour la 1.0. |
| Perte de FPS dans le jeu | ≤ 3 % (mesurée sur une configuration de référence) |
| Plantages par heure de session | 0 sur la campagne de test de 20 h |

Ces cibles sont des **objectifs**, pas des performances démontrées. Elles seront révisées après les mesures du lot L0 (§9).

### 1.4 Référence du marché : Unitra AI
[Unitra AI](https://unitra.ai/games/cs2/) propose déjà un traducteur vocal pour CS2. Ses choix valident l'approche et servent de point de comparaison, sans reprendre son identité ni ses contenus.

| Leur choix | Ce que l'on retient |
| --- | --- |
| Capture audio au niveau du système, sans intégration au jeu | Même principe externe. Option « tout le son du PC » (F-21), capture ciblée sur `cs2.exe` par défaut. |
| Reconnaissance vocale sur l'appareil, audio non envoyé sans opt-in | Confirme le choix de Whisper en local. Crosstalk va plus loin : tout reste local, transcription et traduction (F-24, F-30). |
| Overlay en fenêtré sans bordures, widget Game Bar en plein écran exclusif | MVP en fenêtré sans bordures. Widget Game Bar en P2 (F-47). |
| Modèles adaptés au vocabulaire de CS | Glossaire étendu aux callouts de cartes et normalisation des annonces (F-36). |
| Latence annoncée d'environ 1 s | Cible initiale p50 ≤ 2 s, à resserrer après mesures. |
| Traduction dans les deux sens | Traduction de ses propres annonces en option P2 (F-50). |
| 35 langues, offre gratuite limitée | Centré sur ru/en → fr pour la 1.0. Différence : gratuit, open source, 100 % local possible, sans compte. |

## 2. Utilisateurs et cas d'usage

**Persona principal :** joueur francophone de CS2, de niveau intermédiaire à confirmé, qui joue en matchmaking ou sur Premier avec des inconnus. À l'aise avec un PC gaming, mais pas forcément technicien.

| ID | Cas d'usage |
| --- | --- |
| UC-01 | Premier lancement : l'application détecte CS2, vérifie l'option `-condebug`, télécharge un modèle Whisper et lance un test audio. |
| UC-02 | Pendant la partie, un coéquipier écrit en russe : la traduction apparaît dans l'overlay en moins d'une seconde, avec son pseudo. |
| UC-03 | Un coéquipier fait une annonce vocale en russe. Une transcription provisoire peut s'afficher, puis la traduction finale la remplace. Le pseudo s'ajoute ensuite s'il est identifié de façon fiable, sinon « Joueur inconnu ». |
| UC-04 | Le joueur masque l'overlay (Ctrl+Alt+F8) ou coupe toutes les captures d'urgence (Ctrl+Alt+F10) sans quitter le jeu. |
| UC-05 | Le joueur ajoute un terme au glossaire (par ex. « boost ») pour qu'il reste tel quel dans la traduction. |
| UC-06 | Après une mise à jour de CS2, le chat ou l'indicateur vocal n'est plus reconnu : l'écran Diagnostic l'indique et propose de signaler le problème. |

## 3. Contraintes et principes non négociables

L'application ne doit **jamais** :
- injecter de DLL ni de code dans `cs2.exe` ;
- **lire ou écrire dans la mémoire du jeu**, y compris en lecture seule et y compris pour identifier le locuteur. Cette option a été étudiée puis **écartée définitivement** : risque de ban VAC permanent pour les utilisateurs, maintenance à chaque mise à jour du jeu, contradiction avec la promesse du produit ;
- intercepter, déchiffrer ou modifier le trafic réseau du jeu ;
- s'accrocher au rendu du jeu (hooks DirectX/Vulkan) ;
- envoyer des messages, des commandes ou des entrées clavier dans le jeu à la place du joueur ;
- contourner VAC, le mode approuvé (trusted mode) ou toute autre protection.

**Seules interactions autorisées avec le jeu :**
1. la lecture du fichier `console.log` écrit par le jeu lui-même (option `-condebug`) ;
2. la capture audio via les API publiques de Windows (WASAPI) ;
3. la capture d'image de la fenêtre du jeu via l'API publique Windows Graphics Capture, uniquement pour l'identification expérimentale du locuteur (§4.8), sans enregistrement des images ;
4. l'affichage d'une fenêtre séparée au premier plan ;
5. les interfaces officielles de Valve (Game State Integration), en lecture.

Aucune garantie « VAC safe » ne sera revendiquée publiquement. Les interactions sont documentées dans l'écran À propos.

**Conformité :** RGPD. La voix et le pseudo de tiers sont des données personnelles : traitement local par défaut, aucune conservation de l'audio ni des images, historique texte désactivé par défaut. Respect des conditions d'utilisation de Steam et de Valve.

## 4. Exigences fonctionnelles

Priorités :
- **P0** : indispensable au MVP ;
- **P1** : version 1.0 ;
- **P2** : souhaitable ;
- **Exp.** : expérimental. Évalué sur le corpus ou en conditions réelles avant adoption, désactivé par défaut tant qu'il n'est pas validé.

Statut : ✅ réalisé dans le dépôt (interface et données simulées comprises) · ◐ partiellement réalisé · — à faire.

### 4.1 Détection du jeu

| ID | Exigence | Prio. | Statut |
| --- | --- | --- | --- |
| F-01 | Localiser les bibliothèques Steam (registre + `libraryfolders.vdf`) et le dossier d'installation de CS2. | P0 | ✅ (à valider sous Windows) |
| F-02 | Détecter le lancement et la fermeture de `cs2.exe` (énumération des processus toutes les 2 s) et connaître son PID. | P0 | ✅ |
| F-03 | Vérifier la présence de `-condebug` dans les options de lancement (`localconfig.vdf`) et guider l'utilisateur pour l'ajouter. Aucune modification automatique. | P0 | ✅ (à valider sous Windows) |
| F-04 | Chemin de CS2 saisissable manuellement si la détection échoue. | P1 | ✅ |

### 4.2 Chat écrit

| ID | Exigence | Prio. | Statut |
| --- | --- | --- | --- |
| F-10 | Suivre `console.log` en continu (« tail ») à partir de la fin du fichier, en gérant la troncature et la recréation du fichier. | P0 | ✅ |
| F-11 | Extraire le canal (Tous, CT, T), l'expéditeur et le message. Parseur versionné, défini par des expressions régulières dans un fichier de règles mis à jour sans recompiler. | P0 | ✅ (format à confirmer sur une vraie partie) |
| F-12 | Ignorer les lignes système, les messages vides et les doublons (fenêtre de 2 s). | P0 | ✅ |
| F-13 | Option : ignorer ses propres messages (pseudo Steam configurable). | P1 | ✅ |
| F-14 | Alimenter la liste des pseudos connus de la session avec les expéditeurs du chat (utilisée par §4.8). | P1 | — |

### 4.3 Chat vocal

| ID | Exigence | Prio. | Statut |
| --- | --- | --- | --- |
| F-20 | Capturer uniquement l'audio de `cs2.exe` par process loopback WASAPI (`ActivateAudioInterfaceAsync`, documenté à partir du build 20348). Son fonctionnement sous Windows 10 22H2 (19045) est à vérifier en L0. | P0 | — |
| F-21 | Mode « tout le son du PC » (loopback du périphérique de sortie) : repli quand le process loopback est indisponible, et mode volontaire pour traduire aussi Discord ou TeamSpeak. Avertissement explicite : capte aussi le navigateur et les autres applications. | P0 | — |
| F-22 | Ne jamais capturer le microphone de l'utilisateur sans activation explicite (voir F-50). | P0 | — |
| F-23 | Rééchantillonner en 16 kHz mono. Détection de voix (VAD Silero via ONNX Runtime) avec seuil réglable, pré-roll 250 ms, post-roll 400 ms, segment maximal de 5 s (8 s initialement : trop long avec le bruit du jeu, mesuré en partie le 12/10/2026). | P0 | — |
| F-24 | Transcription locale avec whisper.cpp (crate `whisper-rs`), modèles tiny à large-v3-turbo, accélération CUDA ou Vulkan si disponible, sinon CPU. Whisper est le choix de départ, confirmé ou remplacé après le banc d'essai (A-11). | P0 | — |
| F-25 | Transcription provisoire pendant la parole (fenêtre glissante), remplacée par la version finale. Adoptée seulement si son coût GPU/CPU reste compatible avec la cible FPS. | P1 | ✅ (transcription et traduction provisoires ; coût CPU à mesurer en partie) |
| F-26 | Détection de langue restreinte aux langues sources préférées (ru, en par défaut). | P0 | — |
| F-27 | Abandonner les segments périmés (plus de 6 s d'attente) pour éviter l'accumulation de retard. | P0 | — |
| F-28 | ~~Transcription cloud en option~~ : abandonnée (décision du 12/10/2026, aucun service payant ni clé API pour l'utilisateur). | — | abandonnée |
| F-29 | Chaîne de nettoyage audio avant la VAD (détails §4.3 bis) : traitements légers en P0, traitements coûteux en Exp. | P0 | — |

### 4.3 bis Séparation de la voix et des bruits de jeu

La voix des coéquipiers arrive mélangée aux tirs, explosions et pas. C'est le principal risque qualité du projet. Les traitements légers sont adoptés d'emblée. Les traitements coûteux ne sont adoptés que s'ils améliorent le WER sur le corpus sans dépasser le budget de latence et de FPS.

```
audio 48 kHz stéréo (capture WASAPI)
  ─► [A1] extraction mid/side        Exp.
  ─► [A2] passe-bande 200–4 000 Hz   P0
  ─► [A3] DeepFilterNet 3            Exp.  (repli RNNoise [A4], Exp.)
  ─► rééchantillonnage 16 kHz mono
  ─► [A5] VAD Silero + validation des segments   P0
  ─► [A6] Whisper + prompt glossaire CS          P0
  ─► [A7] filtre anti-hallucinations             P0
```

Chaque étape est désactivable et mesurable dans l'écran Diagnostic.

| ID | Exigence | Prio. |
| --- | --- | --- |
| A-01 | **Mid/side.** Hypothèse : le chat vocal de CS2 n'est pas spatialisé, contrairement aux sons de jeu. Le canal side sert de référence de bruit pour une soustraction adaptative (NLMS) sur le canal mid. Désactivé si la source est mono. | Exp. |
| A-02 | **Passe-bande** 200–4 000 Hz (biquads Butterworth d'ordre 4). | P0 |
| A-03 | **Réduction de bruit neuronale** DeepFilterNet 3 (crate `deep_filter`). Adoptée seulement si le gain de WER est mesuré et que le budget (≤ 10 ms par trame de 10 ms sur un cœur) est tenu. | Exp. |
| A-04 | Repli **RNNoise** (crate `nnnoiseless`) sur les machines trop faibles pour DeepFilterNet. | Exp. |
| A-05 | **Validation des segments** (garde anti-transitoires) : un pic d'énergie bref (< 150 ms, montée > 20 dB) ne peut pas ouvrir un segment de parole à lui seul. Les segments trop courts ou sans parole confirmée par la VAD sont rejetés avant Whisper. | P0 |
| A-06 | **Prompt glossaire** : les termes CS et les callouts de la carte en cours (carte fournie par Game State Integration) sont passés à Whisper comme texte initial. | P0 |
| A-07 | **Filtre anti-hallucinations** : rejet des sorties à faible confiance (`avg_logprob` < −1,0 ou `no_speech_prob` > 0,6), des répétitions et des phrases typiques d'hallucination (« Merci d'avoir regardé », « Sous-titres réalisés par… »). | P0 |
| A-08 | **Capture Discord dédiée** : process loopback sur `discord.exe` en parallèle de `cs2.exe`. | P1 |
| A-09 | **Assistant de réglage du jeu** : recommander de baisser le volume principal du jeu et de monter celui du chat vocal, avec un test avant/après. | P1 |
| A-10 | **Whisper affiné pour CS2** (version 2) : fine-tuning LoRA sur un corpus d'annonces ru/en mélangées à des sons de CS2, export GGML. | P2 |
| A-11 | **Banc d'essai et mesure continue** : chaque version est évaluée hors ligne sur le corpus de test, avec et sans chaque étape, pour mesurer le WER, la latence ajoutée, les faux déclenchements et le coût CPU/GPU. | P0 |

**Budget de latence indicatif (GPU)**

| Étape | Cible |
| --- | --- |
| A2 + rééchantillonnage | ≤ 10 ms |
| Post-roll (fin de parole détectée) | 400 ms |
| A6 Whisper small, segment de 3 s | ≤ 800 ms |
| Traduction | ≤ 500 ms |
| Événement Tauri + affichage overlay | ≤ 100 ms |
| **Total** | **≈ 1,8 s (cible p50 ≤ 2 s)** |

**Corpus**
- **Voix** : annonces enregistrées avec l'accord des joueurs (objectif 20 h, ru et en), plus des corpus libres (Common Voice ru/en).
- **Bruits** : sons de jeu enregistrés en partie privée, sans voix. Les fichiers audio du jeu, propriété de Valve, ne sont jamais redistribués.
- **Test** : 2 h de parties réelles annotées à la main, jamais utilisées pour l'entraînement.

### 4.4 Traduction

| ID | Exigence | Prio. | Statut |
| --- | --- | --- | --- |
| F-30 | **Traduction locale uniquement**, sur le CPU du joueur : Helsinki-NLP Opus-MT ru→fr et en→fr (Apache-2.0, utilisable dans un produit vendu), exécuté par candle (Rust). **Aucune clé API, aucun abonnement, aucun envoi en ligne** (décision du 12/10/2026 : l'utilisateur paie l'application et rien d'autre). NLLB est exclu (licence non commerciale). | P0 | ✅ |
| F-31 | Détection de langue restreinte aux langues sources préférées pour les messages courts. | P0 | ◐ (ru, en, fr ; sinon original affiché) |
| F-32 | Ne pas traduire les messages déjà en français : l'original est affiché et aucun appel n'est fait. | P0 | ✅ |
| F-33 | Glossaire CS : termes conservés (smoke, flash, eco, rush B…) remplacés par des repères avant traduction puis restaurés (insensible à la casse, mots entiers), et termes traduits avec un rendu imposé. Si le modèle perd un repère, la phrase est retraduite sans protection. Modifiable par l'utilisateur. | P0 | ✅ (fiabilité à mesurer sur les vrais modèles) |
| F-34 | Une ligne qui attend plus de 5 s, ou une file pleine, est affichée en original avec la mention « moteur indisponible ». | P0 | ✅ |
| F-35 | Cache LRU des traductions (1 000 entrées) pour les messages répétés (« gg », « nt »…). | P1 | ✅ |
| F-36 | Normalisation des annonces : callouts de cartes, économie et utilitaires traduits comme des annonces courtes. **Aucune information absente de l'original ne doit être ajoutée.** Glossaire par carte. | P1 | ◐ (lexique des annonces en/ru → fr ; glossaire par carte à faire) |

### 4.5 Overlay en jeu

| ID | Exigence | Prio. | Statut |
| --- | --- | --- | --- |
| F-40 | Fenêtre séparée, transparente, toujours au premier plan, sans bordure, absente de la barre des tâches. | P0 | ✅ |
| F-41 | Clics transmis au jeu et aucun vol de focus clavier, en permanence. | P0 | ✅ à valider sous Windows |
| F-42 | Fonctionne en mode « fenêtré sans bordures ». Le plein écran exclusif n'est pas garanti au MVP. | P0 | ✅ à valider sous Windows |
| F-43 | Trois formats : Minimal, Bilingue, Accessible. | P1 | ✅ |
| F-44 | Réglages : coin d'ancrage, opacité du fond (0–100 %), taille du texte (14–28 px), durée d'affichage (4–8 s), 2 ou 3 blocs visibles. | P0 | ✅ (non persistés) |
| F-45 | Mode de repositionnement temporaire (clics capturés), position libre enregistrée par écran. | P1 | ◐ |
| F-46 | Prise en charge du multi-écran et des mises à l'échelle DPI de 100 à 200 %. | P1 | ◐ |
| F-47 | Widget Xbox Game Bar (UWP) qui affiche les mêmes sous-titres en plein écran exclusif. | **P2** | — |
| F-48 | Afficher le pseudo devant la traduction vocale dès qu'il est résolu (§4.8), sans retarder l'affichage de la traduction. | P1 | — |
| F-50 | Traduction de ses propres annonces (micro) vers la langue des coéquipiers, affichée en texte. Opt-in, désactivé par défaut. | P2 | — |

### 4.6 Fenêtre principale

Sept écrans, conformes à la maquette *v2 Cinematic*. **Réalisés en React avec des données simulées, à brancher sur les événements Rust.**

| Écran | Contenu |
| --- | --- |
| Accueil | État de la session, dernier sous-titre, progression du pipeline vocal, statut CS2, capture, moteurs et latence, flux « Script » filtrable (Texte, Voix, Mixte). |
| Overlay | Aperçu à l'échelle et réglages F-43 à F-45. |
| Audio & IA | Source de capture, test audio de 10 s, seuil VAD, choix et téléchargement des modèles Whisper, CPU/GPU, modèles de traduction locaux et test de traduction. Option d'identification expérimentale du locuteur. |
| Langues & glossaire | Langue cible, langues sources préférées, option « ignorer le français », édition du glossaire. |
| Historique | Désactivé par défaut ; recherche, export TXT/JSON, effacement. |
| Diagnostic | Tests intégrés (journal, -condebug, parseur, capture, modèle, traduction, raccourcis, indicateur vocal), journal expurgé, export d'un rapport. |
| À propos | Ce que l'application ne fait jamais, licences tierces, version, lancement avec Windows. |

### 4.7 Session et raccourcis

- **L'état de session est géré par le cœur Rust**, seule source de vérité. L'interface et l'overlay en sont des vues : ils envoient des commandes et reçoivent des événements typés. Le traitement continue quand la fenêtre principale est masquée.
- Machine à états : `IDLE → INITIALIZING → LISTENING ⇄ PAUSED → STOPPING → STOPPED`, avec un état `DEGRADED` en cas de capture de repli ou de moteur indisponible.

| Raccourci global | Action | Statut |
| --- | --- | --- |
| Ctrl+Alt+F8 | Afficher / masquer l'overlay | ✅ |
| Ctrl+Alt+F9 | Pause / reprise de la session | ✅ |
| Ctrl+Alt+F10 | Arrêt d'urgence : toutes les captures sont arrêtées et les tampons audio purgés en moins de 200 ms | ✅ (simulé) |

Raccourcis via `RegisterHotKey` (pas de hook clavier bas niveau), avec détection des conflits ✅. Reconfiguration des raccourcis en P1.

Également réalisés : instance unique ✅, icône dans la zone de notification avec fermeture vers la zone de notification ✅, lancement au démarrage de Windows en option ✅.

### 4.8 Identification du locuteur (expérimentale)

**Objectif :** afficher le pseudo exact du joueur qui parle quand son identification est fiable. Sinon, afficher « Joueur inconnu ».

**Sources officielles examinées (octobre 2026) :**
- **Game State Integration** : aucun champ lié à la voix. La liste des joueurs n'est exposée qu'en spectateur ; en jeu, seulement le joueur local.
- **`console.log`** : aucune commande documentée n'y écrit qui est en train de parler. Les expéditeurs du **chat écrit** y figurent, eux, de façon fiable. Les variables de console liées à la voix restent à inventorier en L0.
- **Lecture mémoire** : écartée (§3).

La solution retenue est donc l'**analyse du HUD par capture d'image et OCR**.

| ID | Exigence | Prio. |
| --- | --- | --- |
| S-01 | Module indépendant de la transcription et de la traduction : **il ne bloque ni ne retarde jamais** l'affichage d'une traduction. | Exp. |
| S-02 | Capture d'une petite région de la fenêtre CS2 via Windows Graphics Capture, environ 10 fois par seconde. Aucune image n'est écrite sur disque ni transmise. | Exp. |
| S-03 | Détection de l'indicateur vocal par comparaison de motif. OCR (Windows.Media.Ocr) déclenché uniquement à son apparition. | Exp. |
| S-04 | Rapprochement du texte lu par l'OCR avec la liste des pseudos connus de la session (chat, lectures précédentes). Un pseudo absent de la liste n'est jamais inventé. | Exp. |
| S-05 | Corrélation temporelle sur une horloge commune audio/vidéo (QPC) : chevauchement entre l'intervalle d'activité de l'indicateur et le segment vocal. | Exp. |
| S-06 | Score de confiance. En dessous du seuil, ou si plusieurs indicateurs se chevauchent : « Joueur inconnu ». | Exp. |
| S-07 | Attribution tardive : la traduction est affichée d'abord, puis le pseudo est ajouté par un événement `speaker.resolved`. | Exp. |
| S-08 | Fonction désactivée par défaut et activable dans Audio & IA, avec information sur la capture d'image du jeu. | Exp. |
| S-09 | Discord : identification via l'API locale de Discord si la permission voix est obtenue. | P2 |
| S-10 | Regroupement des voix par session (empreinte vocale en mémoire uniquement, jamais enregistrée, opt-in). Donnée biométrique au sens du RGPD. | P2 |

**Préalable à toute implémentation (lot L4) :** valider sur des captures réelles, à plusieurs résolutions et échelles, que le HUD de CS2 affiche un indicateur vocal **associé à un pseudo lisible**. S'il n'affiche qu'un avatar, la faisabilité sera réévaluée. Limites connues :
- bordure jaune de capture sous Windows 10 ;
- aucune image quand le jeu est minimisé ;
- pack de langue Windows nécessaire pour l'OCR des pseudos en cyrillique ;
- pseudos stylisés illisibles.

## 5. Exigences non fonctionnelles

| Domaine | Exigence |
| --- | --- |
| Performance | Latences de la section 1.3. Inférence sur un thread dédié de priorité inférieure au jeu. Overlay redessiné uniquement en cas de changement. Scène 3D de l'interface suspendue quand la fenêtre principale est masquée, et désactivable (mode performance). |
| Concurrence | Modules isolés reliés par des files bornées. En cas de saturation, les éléments les plus anciens sont abandonnés plutôt que d'accumuler du retard. |
| Ressources | RAM ≤ 300 Mo hors modèle ; CPU au repos ≤ 1 % ; VRAM ≤ 1,2 Go avec whisper-small. |
| Configuration minimale | CPU 6 cœurs (2018+), 16 Go de RAM. Recommandée : GPU NVIDIA 6 Go+ (CUDA) ou AMD/Intel (Vulkan). |
| Fiabilité | Une panne de capture, de transcription, de traduction ou d'identification ne fait jamais planter l'application. Reconnexion automatique quand CS2 redémarre. |
| Sécurité | Aucune clé API ni secret : transcription et traduction sont locales. Les seuls téléchargements (modèles) passent en HTTPS avec contrôle de taille et d'empreinte. Exécutable et installateurs signés (Authenticode). CSP stricte sur le WebView. |
| Confidentialité | Aucun audio ni image écrit sur disque ou envoyé sans opt-in. Historique désactivé par défaut et purgé après 7 jours. Pseudos expurgés des journaux. Aucune télémétrie par défaut. |
| Accessibilité | Contraste 4,5:1 minimum, navigation complète au clavier, format d'overlay Accessible, respect du réglage Windows « réduire les animations ». |
| Localisation | Interface en français ; chaînes externalisées pour ajouter d'autres langues. |
| Maintenabilité | `clippy` sans avertissement, `rustfmt`, TypeScript strict, couverture de tests ≥ 70 % hors interface, validation locale Windows (`scripts/validate.ps1`) avec rapport sur chaque PR. |

## 6. Architecture technique

Détail complet : [architecture.md](architecture.md).

### 6.1 Pipeline

```
console.log ─► chat-ingest ─► parseur ───────────────────────┐
                                                             ├─► translation ─► session (Rust) ─► événements Tauri ─► UI + overlay
cs2.exe audio ─► capture ─► nettoyage ─► VAD ─► segments ─► asr ┘                ▲
                                                                                  │ speaker.resolved (tardif)
fenêtre CS2 ─► capture HUD ─► indicateur ─► OCR ─► corrélation (horloge QPC) ─────┘
```

### 6.2 Pile technique

| Besoin | Choix |
| --- | --- |
| Application de bureau | **Tauri v2** (WebView2), installateurs NSIS et MSI |
| Interface | **React 19 + TypeScript**, Tailwind CSS, Motion, Three.js |
| Cœur natif | **Rust** (édition 2021, stable), `windows-rs` |
| Asynchrone | `tokio` (réseau, orchestration), threads dédiés (capture, inférence), `reqwest` (rustls) |
| Audio | `rubato` (rééchantillonnage), `ort` (VAD Silero ONNX), `biquad`. Expérimental : `deep_filter`, `nnnoiseless` |
| Reconnaissance vocale | `whisper-rs` (whisper.cpp, CUDA / Vulkan), à confirmer par le banc d'essai |
| Locuteur (exp.) | Windows Graphics Capture, Windows.Media.Ocr via `windows-rs` |
| Données | `serde` + JSON (config), `rusqlite` (historique) |
| Journaux | `tracing`, rotation quotidienne, expurgation des clés et pseudos |
| Distribution | Bundler Tauri (NSIS par utilisateur + MSI), WebView2 embarqué, mises à jour signées (`tauri-plugin-updater`), release sur tag avec installateurs construits en local |

### 6.3 Données et fichiers

| Chemin | Contenu |
| --- | --- |
| `%APPDATA%\Crosstalk\config.json` | Réglages, versionnés avec migration automatique |
| `%APPDATA%\Crosstalk\glossary.json` | Glossaire utilisateur |
| `%LOCALAPPDATA%\Crosstalk\models\` | Modèles, avec vérification SHA-256 après téléchargement |
| `%LOCALAPPDATA%\Crosstalk\history.db` | Historique texte, si activé |
| `%LOCALAPPDATA%\Crosstalk\logs\` | Journaux expurgés, conservés 7 jours |

## 7. Direction artistique

Référence : la maquette *v2 Cinematic*, **implémentée**. Ambiance cinéma :
- fond noir chaud et grain de film ;
- titres en Instrument Serif, texte en Geist, données en JetBrains Mono ;
- orange `#ffa24a` pour la voix, bleu glacier `#7fd4ff` pour le chat, vert `#7fe0a8` pour les états sains.

Le casque 3D réactif (Three.js) et le verre dépoli sont livrés. Ils doivent rester désactivables et ne jamais consommer de ressources quand la fenêtre est masquée ou que le mode performance est actif.

## 8. Tests et recette

- **Tests unitaires** : parseur de chat (≥ 40 lignes réelles par version de CS2), glossaire, machine à états de session, cache, corrélation du locuteur.
- **Tests d'intégration** : pipeline vocal alimenté par des fichiers WAV, moteurs de traduction simulés (mock HTTP), événements Tauri.
- **Banc d'essai hors ligne** (A-11) : WER, latence, faux déclenchements, CPU/GPU, par moteur et par étape.
- **Corpus vocal CS2** : au moins 2 h d'annonces (ru/en) avec transcriptions de référence.
- **Corpus HUD** : captures de l'indicateur vocal à plusieurs résolutions et échelles, avec les pseudos attendus.
- **Mesure des FPS** : PresentMon, avec et sans Crosstalk, en CPU et en GPU.
- **Tests manuels** : 20 h de parties réelles sur 3 configurations (GPU NVIDIA, GPU AMD, CPU seul), sous Windows 10 et 11.
- **Validation locale** : pas de GitHub Actions (décision du 12/10/2026). Sur chaque PR vers `develop` et `main`, `scripts/validate.ps1` sur un PC Windows : garde anti-triche, typecheck, tests, clippy, rustfmt, build MSVC, installateurs, avec rapport joint à la PR.

**Critères d'acceptation de la version 1.0**
- Toutes les exigences P0 et P1 sont implémentées et testées.
- Les cibles de latence et de WER de la section 1.3 sont atteintes sur le corpus.
- Aucune interaction avec le jeu en dehors de celles autorisées en section 3 (revue de code dédiée).
- L'arrêt d'urgence coupe toutes les captures en moins de 200 ms.
- L'installation, la désinstallation et la mise à jour fonctionnent sans droits administrateur.
- Si l'identification du locuteur est livrée, elle n'affiche aucun pseudo erroné sur le corpus HUD.

## 9. Planning et lots

**Déjà réalisé :**
- socle Tauri v2, build Windows et installateurs (validation locale, sans CI hébergée) ;
- les sept écrans, l'intro et la scène 3D ;
- zone de notification, raccourcis globaux, fenêtre d'overlay, démarrage avec Windows, instance unique ;
- workflow Git `develop` / `main`.

| Lot | Contenu | Durée (1 dév.) |
| --- | --- | --- |
| **L0 · Preuve de concept voix** | Chaîne réelle CS2 → WASAPI → VAD → Whisper → traduction → overlay. Validation du process loopback (19045 et Windows 11), premier corpus, banc d'essai, premières mesures de latence et de FPS. | 2 à 3 semaines |
| L1 · Session Rust et chat | Machine à états en Rust, événements typés, détection CS2, `-condebug`, chat-ingest, glossaire, cache, persistance des réglages | 2 semaines |
| L2 · Voix 1.0 | Nettoyage P0, VAD, segmentation, filtre anti-hallucinations, choix du modèle, résultats provisoires si viables, évaluation des traitements Exp. | 4 semaines |
| L3 · Interface branchée | Écrans reliés aux données réelles, diagnostic réel, historique SQLite, téléchargement des modèles | 2 semaines |
| L4 · Locuteur expérimental | Préalable (corpus HUD), puis capture, indicateur, OCR, corrélation, « Joueur inconnu » | 3 semaines, après validation du préalable |
| L5 · Industrialisation | Signature, mises à jour, tests 20 h, bêta fermée | 3 semaines |
| P2 | Widget Xbox Game Bar, traduction de ses annonces, Discord, regroupement des voix | — |
| V2 · Modèle CS2 | Corpus bruité, fine-tuning Whisper (A-10), évaluation | 6 à 8 semaines |

**Total indicatif :** version 1.0 sans locuteur à la fin de L3 + L5, environ 13 à 14 semaines. Avec le locuteur expérimental, environ 16 à 17 semaines.

## 10. Risques

| Risque | Niveau | Parade |
| --- | --- | --- |
| Transcription médiocre (voix compressée, bruit de jeu, argot) | Élevé | Nettoyage P0, corpus réel dès L0, prompt glossaire, banc d'essai, Whisper affiné en V2 |
| Process loopback indisponible sous Windows 10 22H2 | Moyen | Repli loopback du périphérique avec avertissement (F-21), vérifié en L0 |
| HUD sans pseudo lisible ou modifié par une mise à jour | Élevé (pour §4.8) | Préalable de faisabilité, « Joueur inconnu » par défaut, détection dans Diagnostic, fonctionnalité expérimentale |
| Pseudo erroné affiché | Moyen | Seuil de confiance, rapprochement avec les pseudos connus, jamais d'invention |
| Changement du format du chat après une mise à jour CS2 | Moyen | Règles du parseur téléchargeables sans nouvelle version, détection dans Diagnostic |
| Overlay invisible en plein écran exclusif | Moyen | Recommander le fenêtré sans bordures. Widget Game Bar en P2 |
| Faux positif VAC ou des antivirus | Faible à moyen | Aucune interaction interdite (§3), exécutable signé, soumission aux éditeurs d'antivirus |
| Impact sur les FPS (GPU partagé avec le jeu) | Moyen | Mesure CPU vs GPU, priorité basse, modèle adapté au matériel, mode performance |
| Concurrent déjà installé (Unitra AI) | Moyen | Se différencier : gratuit, open source, local, sans compte, spécialisé CS2 → français |
| Charge CPU de la traduction locale | Moyen | Un seul thread, priorité basse, couche de sortie quantifiée (q8_0) limitée au vocabulaire français, cache LRU |

## 11. Hors périmètre

- macOS, Linux et autres jeux que CS2 (l'architecture doit cependant permettre d'ajouter d'autres jeux).
- Synthèse vocale de ses annonces vers les coéquipiers (voice-to-voice).
- Toute lecture de la mémoire du jeu, y compris pour identifier le locuteur.
- Comptes utilisateurs, synchronisation cloud, monétisation.

## 12. Glossaire

| Terme | Définition |
| --- | --- |
| VAC | Valve Anti-Cheat, l'anti-triche de Steam. |
| `-condebug` | Option de lancement de CS2 qui écrit la console du jeu dans `console.log`. |
| Game State Integration | Interface officielle de CS2 qui envoie l'état de la partie en JSON à un serveur HTTP local. |
| WASAPI | API audio de Windows ; le « loopback » permet d'enregistrer ce qui est joué. |
| Windows Graphics Capture | API Windows de capture d'image d'une fenêtre (utilisée notamment par OBS). |
| OCR | Reconnaissance de texte dans une image. |
| QPC | `QueryPerformanceCounter`, horloge haute précision de Windows, commune à l'audio et à la capture d'image. |
| VAD | Voice Activity Detection : repère les moments où quelqu'un parle. |
| Whisper | Modèle open source de reconnaissance vocale (OpenAI, licence MIT). |
| DeepFilterNet | Modèle open source de réduction de bruit en temps réel, écrit en Rust. |
| Mid / side | Décomposition stéréo : le centre (mid) contient ce qui est identique à gauche et à droite, les côtés (side) ce qui diffère. |
| WER | Word Error Rate : pourcentage de mots mal transcrits. |
| p50 / p95 | Valeur sous laquelle se trouvent 50 % / 95 % des mesures. |
