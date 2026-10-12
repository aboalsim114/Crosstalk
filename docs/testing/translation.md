# Protocole de test — traduction locale (feat/local-translation)

À exécuter sur un PC **Windows 10/11 x64**. La traduction tourne sur le processeur avec les modèles Helsinki-NLP **Opus-MT** ru→fr et en→fr (licence Apache-2.0). **Aucune clé API, aucun abonnement, rien n'est envoyé en ligne.**

> Les voix, les pseudos et le texte des parties sont des données personnelles : ne collez pas les phrases des joueurs dans le dépôt ni dans la PR, seulement des compteurs.

## 0. Préparation

1. Construire le commit testé : `powershell -ExecutionPolicy Bypass -File scripts\validate.ps1` (ou les commandes de CONTRIBUTING.md) et `cargo build --release --locked --example translate --target x86_64-pc-windows-msvc` dans `src-tauri`.
2. Installer les modèles (environ 620 Mo en plus de Whisper) :
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -Translation
   ```
   Le script vérifie la taille exacte de chaque fichier et affiche son SHA-256 : notez ceux des deux `pytorch_model.bin`.

## 1. Outil en ligne de commande (sans CS2)

`translate.exe` est dans `src-tauri\target\x86_64-pc-windows-msvc\release\examples\`.

| # | Commande | Résultat attendu |
| --- | --- | --- |
| 1.1 | `.\translate.exe --lang ru "Двое идут на B, у них AWP."` | Une phrase française correcte (« Deux vont sur B, ils ont un AWP » ou proche). Le temps de chargement et de traduction s'affiche. |
| 1.2 | `.\translate.exe --lang en "Two players on B, they have an AWP."` | Idem en anglais. |
| 1.3 | `.\translate.exe --lang ru --repeat 20 "Бомба на A, нужен флеш."` | Temps p50 / max. **Objectif : p50 ≤ 500 ms**, CS2 fermé. |
| 1.4 | Même commande, CS2 lancé en partie | Notez le p50 sous charge. |
| 1.5 | `.\translate.exe --lang ru --exact "Двое идут на B, у них AWP."` | Mode de référence (couche de sortie complète en float32) : même sens que 1.1, plus lent. Notez les deux temps. |

Préparez une liste de 20 annonces courantes (10 russes, 10 anglaises) et notez pour chacune : correcte / compréhensible / fausse.

## 2. Dans l'application

| # | Action | Résultat attendu |
| --- | --- | --- |
| 2.1 | **Audio & IA**, carte « Traduction locale · Opus-MT » | « Russe → français : prêt · chargé en … ms », idem pour l'anglais. Sans modèles : « modèle absent », puis **Rechercher les modèles** après installation. Aucune mention de DeepL, d'Azure ni de clé API. |
| 2.2 | Champ de test : « Traduire » sur la phrase proposée | La traduction française et son temps s'affichent. |
| 2.3 | **Accueil → Démarrer**, un joueur parle russe | La ligne apparaît d'abord en russe (« traduction en cours »), puis passe en **FINAL** : texte français, original en dessous, `RU → FR`. Le sous-titre indique « VOIX · RUSSE → FRANÇAIS ». |
| 2.4 | Overlay en jeu | « VOIX • RUSSE → FRANÇAIS », la phrase française, l'original en petit (format bilingue). |
| 2.5 | Désactiver « Traduire les voix en français » | Les nouvelles lignes restent en langue originale avec « Non traduit — traduction désactivée ». |
| 2.6 | Supprimer le dossier `opus-mt-en-fr` puis parler anglais | « Non traduit — modèle de traduction absent » : jamais de texte français inventé. |
| 2.7 | **Ctrl+Alt+F10** pendant qu'une phrase est traduite | Aucune traduction n'arrive après l'arrêt. |
| 2.8 | Glossaire : laisser « smoke » en *Conserver*, dire « throw smoke mid » | « smoke » reste tel quel dans la traduction. Si le modèle perd le repère, la phrase est traduite sans protection (le terme peut alors être traduit). Notez combien de fois sur 10. |
| 2.9 | Gestionnaire des tâches pendant une partie | CPU et mémoire de `crosstalk.exe` avec les deux modèles chargés (environ 600 Mo de plus attendus). FPS de CS2 avec / sans traduction. |
| 2.10 | **Diagnostic** | Ligne « Traduction locale · Opus-MT » avec l'état réel et le p50. |

## 3. Compte rendu

```
Commit : …            Windows : … (build …)      CPU : …      RAM : …
SHA-256 pytorch_model.bin ru-fr : …   en-fr : …
1.1 / 1.2 : traductions correctes ? …
1.3 : p50 … ms (CS2 fermé) · 1.4 : p50 … ms (CS2 lancé) · 1.5 exact : … ms
20 annonces : correctes … / compréhensibles … / fausses …
2.1 – 2.10 : ✅ / ❌ + remarques (sans le texte des joueurs)
Glossaire 2.8 : terme conservé … / 10
Mémoire crosstalk.exe : … Mo · FPS sans / avec : … / …
```
