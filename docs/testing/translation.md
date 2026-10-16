# Protocole de test — traduction IA (Cloud et Local)

À exécuter sur un PC **Windows 10/11 x64**. La traduction passe uniquement par le LLM connecté dans **Audio & IA › TRADUCTION IA** : un fournisseur en ligne avec **votre** clé, ou Ollama / LM Studio sur le PC. Crosstalk ne télécharge aucun modèle de traduction.

> Les voix, les pseudos et le texte des parties sont des données personnelles : ne collez pas les phrases des joueurs dans le dépôt ni dans la PR, seulement des compteurs et les phrases du corpus ci-dessous. Ne collez **jamais** une clé API.

## 0. Préparation

1. Construire le commit testé : `powershell -ExecutionPolicy Bypass -File scripts\validate.ps1` (ou installer la version de test depuis À propos).
2. Pour le mode Local : installer Ollama (ollama.com) puis `ollama pull qwen2.5:7b` (ou `translategemma`), et/ou LM Studio avec un modèle chargé et le serveur local démarré.
3. Pour le mode Cloud : une clé d'un fournisseur (OpenAI, Anthropic, Mistral, Groq, OpenRouter, Ollama Cloud).

## 1. Premier lancement

| # | Action | Attendu |
| --- | --- | --- |
| 1.1 | Installation neuve | L'installateur ne télécharge que la VAD et Parakeet (≈ 671 Mo), aucun `opus-mt-*`. |
| 1.2 | Après « Vos langues » | Fenêtre « Moteur de traduction » : API Cloud / Ollama sur ce PC / LM Studio sur ce PC / Configurer plus tard. |
| 1.3 | « Configurer plus tard », puis Démarrer en partie | Transcription normale, lignes « Non traduit — aucun moteur de traduction connecté (Audio & IA › Traduction IA) ». L'Accueil indique « aucun moteur connecté ». |
| 1.4 | Mise à jour depuis une version avec Opus-MT | Réglages conservés ; Audio & IA propose « Libérer l'espace » pour les anciens dossiers, rien n'est supprimé sans clic. |

## 2. Cloud

| # | Action | Attendu |
| --- | --- | --- |
| 2.1 | Choisir un fournisseur, coller la clé, Enregistrer | « Clé enregistrée · Gestionnaire d'identifiants Windows ». La clé n'apparaît ni dans `%APPDATA%\Crosstalk\config.json` ni dans le journal (`logs/`). |
| 2.2 | « Lister les modèles » | Liste réelle du fournisseur ; choisir un modèle. |
| 2.3 | « Connecter » sans cocher l'accord | Refusé : « Acceptez d'abord l'envoi du texte des phrases à … ». |
| 2.4 | Cocher l'accord, Connecter, « Tester la connexion » | « ✓ Réponse en … ms » avec la phrase d'exemple traduite. |
| 2.5 | « Tester une traduction » : corpus §5, FR → RU puis RU → FR | Traductions correctes ; notez la latence. |
| 2.6 | Clé fausse | « clé refusée par le fournisseur » et le conseil ; plus aucun appel tant que la clé ne change pas. |
| 2.7 | Couper Internet en partie | Lignes en langue d'origine avec « réseau indisponible » ; l'application ne se fige pas. Rebrancher : les lignes suivantes sont traduites. |
| 2.8 | Changer de fournisseur | Déconnexion automatique, nouvelle clé à coller, reconnecter. |
| 2.9 | Effacer la clé, Déconnecter | « Aucune clé enregistrée » ; plus aucune traduction. |

## 3. Local

| # | Action | Attendu |
| --- | --- | --- |
| 3.1 | Onglet « Local », Ollama démarré avec des modèles | « Détecté · N modèles », liste réelle ; aucun champ de clé, aucun accord demandé. |
| 3.2 | Ollama arrêté | « Non détecté sur ce PC », bouton « Télécharger Ollama ↗ ». Démarrer Ollama, « Actualiser les modèles locaux » : détecté. |
| 3.3 | Ollama démarré sans modèle | « Démarré, aucun modèle disponible » et la commande `ollama pull …`. |
| 3.4 | LM Studio avec un modèle chargé | Idem 3.1 sur le port 1234. |
| 3.5 | Connecter, Tester une traduction (corpus §5) | Traduction correcte ; notez la première latence (chargement) et les suivantes. |
| 3.6 | Arrêter Ollama en pleine partie, puis le relancer | Lignes « réseau indisponible » pendant l'arrêt, traduction reprise toute seule ensuite. |
| 3.7 | PC hors ligne (Wi-Fi coupé) | La traduction locale continue. |
| 3.8 | Gestionnaire des tâches | CPU/GPU d'Ollama ou LM Studio et FPS de CS2 avec / sans traduction. |

## 4. Toutes les fonctions sur le moteur choisi

| # | Action | Attendu |
| --- | --- | --- |
| 4.1 | Voix russe en partie | Provisoire en russe, puis FINAL traduit ; overlay « VOIX • RUSSE → FRANÇAIS ». En mode Cloud, aucun appel pour les provisoires. |
| 4.2 | Chat anglais | Traduit par le même moteur. |
| 4.3 | Mouse 4 : « Mon équipe ne sait pas jouer. » vers le russe | Texte copié en russe (sens : « Моя команда не умеет играть. »). « Éco » reste la phrase CS toute prête. |
| 4.4 | Mouse 5 (dictée) | Même résultat après transcription. |
| 4.5 | Historique, mode streamer, radar, résumé de partie, Diagnostic | Lignes traduites présentes ; Diagnostic « Traduction IA · LLM » avec l'état réel. |
| 4.6 | Ctrl+Alt+F10 pendant une traduction | Aucune traduction n'arrive après l'arrêt. |

## 5. Corpus linguistique

| Français | Russe attendu (sens) | Anglais attendu (sens) |
| --- | --- | --- |
| Mon équipe ne sait pas jouer. | Моя команда не умеет играть. | My team can't play. |
| Deux ennemis arrivent sur B. | Два врага идут на B. | Two enemies are coming to B. |
| Attendez-moi au mid. | Ждите меня на миде. | Wait for me at mid. |
| Il est derrière toi. | Он позади тебя. | He's behind you. |
| Ils ont posé la bombe. | Они поставили бомбу. | They planted the bomb. |

Critères (sens, pas mot à mot) : négation conservée, nombre « 2 » conservé, position (B, mid) conservée, aucune explication ajoutée, une seule ligne.

## 6. Compte rendu

```
Commit : …   Windows : … (build …)   CPU : …   GPU : …   RAM : …
Cloud : fournisseur … modèle … · latence p50 … ms · corpus FR→RU …/5 FR→EN …/5 RU→FR …/5
Local : Ollama / LM Studio … modèle … · 1re latence … ms, suivantes … ms · corpus …/5
1.1 – 4.6 : ✅ / ❌ + remarques (sans texte de joueurs, sans clé)
FPS sans / avec traduction locale : … / …
```
