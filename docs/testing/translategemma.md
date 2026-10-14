# Protocole de test : TranslateGemma 4B

À exécuter sur un PC **Windows 10/11 x64**. Rien de ce protocole n'a encore été exécuté sur Windows ni pendant une partie de CS2 (voir [le rapport](../reports/translategemma-integration.md)).

> Les voix, les pseudos et les messages des joueurs sont des données personnelles. Ne collez dans la PR que des compteurs, des temps et les phrases du corpus inventé (`translation-corpus.tsv`).

## 0. Installation

| # | Action | Attendu |
| --- | --- | --- |
| 0.1 | Lancer `Crosstalk-setup.exe` (nouveau PC ou nouvel utilisateur) | L'étape EXTRACTION affiche « Traduction (TranslateGemma 4B Q4_K_M) · X / 2490 Mo · N % · V Mo/s », puis « vérification de l'intégrité (SHA-256) ». En fin d'installation : « Chargement de test de TranslateGemma 4B », puis « TranslateGemma 4B prêt (processeur) » ou « (carte graphique, vulkan) ». |
| 0.2 | Couper le Wi-Fi pendant le téléchargement, réessayer | Message « Téléchargement des modèles interrompu… ». Au nouvel essai, le téléchargement **reprend** (les Mo déjà reçus ne sont pas retéléchargés). |
| 0.3 | Relancer l'installateur (réinstallation) | Aucun gros téléchargement : le modèle est déjà vérifié (manifeste `crosstalk-manifest.json`). |
| 0.4 | Supprimer `crosstalk-manifest.json` puis réinstaller | « vérification de l'intégrité (SHA-256) » pendant quelques secondes, **sans** retéléchargement. |
| 0.5 | Disque presque plein (moins de 3 Go libres) | Refus avant de télécharger : « Espace insuffisant… ». |
| 0.6 | Notez le SHA-256 affiché par Audio & IA › Vérifier / réparer | Il doit être identique sur deux PC. |

## 1. Banc hors partie (CS2 fermé)

Dans le dossier portable :

```powershell
.\crosstalk-translate-bench.exe --markdown banc-auto.md
.\crosstalk-translate-bench.exe --backend cpu --markdown banc-cpu.md
.\crosstalk-translate-bench.exe --backend gpu --markdown banc-gpu.md   # si une carte graphique est présente
```

Pour chaque fichier, relisez les 48 lignes et notez pour chaque moteur : **juste / compréhensible / faux / inventé** (hallucination). Les colonnes « termes » et « ⚠ » sont des contrôles automatiques, elles ne remplacent pas la lecture.

Critères proposés pour faire de TranslateGemma le moteur par défaut :
- au moins autant de lignes **justes** qu'Opus-MT, en russe **et** en anglais ;
- **aucune** ligne inventée (information ou position absente de la phrase) ;
- p50 ≤ 1,5 s sur la carte graphique, ou ≤ 3,5 s sur le processeur (au-delà, le mode automatique repasse sur Opus-MT).

## 2. Dans l'application

| # | Action | Attendu |
| --- | --- | --- |
| 2.1 | Audio & IA, carte « Moteur de traduction · TranslateGemma 4B » | État **INSTALLÉ**, puis **PRÊT** après le premier démarrage de la traduction ; exécution (processeur / carte graphique), temps de chargement, mémoire. |
| 2.2 | Test « Traduire » sur la phrase proposée | La traduction, son temps et « · TranslateGemma ». « · lexique CS » pour une annonce connue (« rush B »). |
| 2.3 | Moteur utilisé : Opus-MT | `crosstalk-llm.exe` disparaît du Gestionnaire des tâches dans la seconde ; la mémoire est rendue. |
| 2.4 | Désactiver « Traduire les voix en français » | Même chose : aucun processus `crosstalk-llm.exe`. |
| 2.5 | Vérifier / réparer | « Intact · SHA-256 … » (lecture complète, quelques secondes). |
| 2.6 | Modifier un octet du fichier `.gguf` puis Vérifier / réparer | « modèle corrompu : supprimé, téléchargez-le à nouveau » ; la carte propose **Télécharger** ; la traduction passe sur Opus-MT en attendant. |
| 2.7 | Mode avion, puis traduire | Fonctionne : aucune connexion n'est utilisée pendant la traduction. |
| 2.8 | Réponse « Répondre dans leur langue » en texte libre | Traduite en anglais / russe par TranslateGemma. |

## 3. Pendant une partie (le plus important)

Avec CS2 en partie (match contre des bots ou en ligne) :

| # | Mesure | Comment |
| --- | --- | --- |
| 3.1 | FPS de CS2 | `cl_showfps 1` ou l'overlay du pilote : moyenne sur 2 minutes **sans** Crosstalk, avec Crosstalk + Opus-MT, avec Crosstalk + TranslateGemma (processeur), puis (carte graphique). |
| 3.2 | Latence d'une annonce | Carte de traduction : p50 / p95 après 20 phrases. |
| 3.3 | Mémoire | Gestionnaire des tâches : `crosstalk-llm.exe` (Mo), mémoire GPU dédiée. |
| 3.4 | Annonces en retard | Une traduction arrivée plus de 5 s après la phrase doit être affichée comme non traduite, jamais après la suivante. |
| 3.5 | Whisper ou traduction ? | Enregistrez (avec leur accord) ou lisez vous-même quelques phrases du corpus dans le chat vocal, récupérez le texte de Whisper (`crosstalk-asr-bench`) dans un fichier `id<TAB>texte`, puis `crosstalk-translate-bench --transcripts whisper.tsv` : la colonne « origine » dit si l'erreur vient de la transcription ou de la traduction. |

## 4. Compte rendu

```
Commit : …      Windows : …      CPU : …      RAM : …      GPU : … (… Go)
SHA-256 du .gguf : …
Installation : téléchargement … min · reprise testée oui/non · réinstallation sans retéléchargement oui/non
Banc CPU : TranslateGemma p50 … / p95 … ms · Opus-MT p50 … / p95 … ms
Banc GPU : TranslateGemma p50 … / p95 … ms · VRAM … Mo
Lecture des 48 lignes (justes / compréhensibles / fausses / inventées) :
  RU  Opus-MT … / … / … / …    TranslateGemma … / … / … / …
  EN  Opus-MT … / … / … / …    TranslateGemma … / … / … / …
FPS CS2 : sans … · Opus-MT … · TG CPU … · TG GPU …
```
