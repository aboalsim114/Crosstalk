# Voix IA en jeu

Vous parlez dans votre langue, vos coéquipiers vous entendent dans la leur.

**Ctrl+Alt+F6**, puis dites « Deux en B, attention au flash ». Crosstalk :

1. transcrit la phrase (Whisper, sur ce PC) ;
2. la traduit dans la langue de vos coéquipiers (phrases CS toutes prêtes, sinon le LLM connecté dans Audio & IA › Traduction IA) ;
3. la fait dire par une **voix neuronale locale** (Piper) dans le vocal de CS2 : « Двое на B, осторожно, флешка ».

La traduction reste aussi copiée pour le chat, comme avant.

**Désactivée par défaut.** La voix est produite sur le PC : pas de compte, pas de clé, pas de quota.

## Mise en route (Langues › Voix IA en jeu)

1. **VB-CABLE** (câble audio virtuel gratuit de VB-Audio) : « Télécharger VB-CABLE ↗ » ouvre `https://vb-audio.com/Cable/`.
   - Installez-le en administrateur (`VBCABLE_Setup_x64.exe`), puis redémarrez le PC.
   - Il n'est pas inclus dans l'installateur de Crosstalk : sa licence ne le permet pas.
2. **Votre vrai micro** : choisissez-le dans la liste. Crosstalk le fait passer dans le câble, vos coéquipiers vous entendent comme avant.
3. **Les voix** : une par langue de coéquipier (2 au choix quand Piper en propose, femme ou homme). « Télécharger » récupère le moteur Piper (22 Mo, une fois) et les voix (environ 63 Mo chacune). « ▶ Écouter » les joue dans votre casque.
4. **Dans Windows** : Paramètres › Système › Son › Entrée, choisissez **CABLE Output (VB-Audio Virtual Cable)** comme micro par défaut. CS2 entend alors le câble : votre voix et la voix IA.
5. Activez **« Dire mes réponses en jeu avec une voix IA »**, puis « Dire en jeu » pour un essai.

**Transmission dans CS2.**
- En micro ouvert : c'est automatique, rien n'est appuyé.
- En appui pour parler : gardez votre touche vocale enfoncée pendant que la voix IA parle, **ou** activez l'appui automatique (ci-dessous).

## Appui automatique pour parler (facultatif)

Option **« Appuyer automatiquement pour parler dans CS2 »**, désactivée par défaut. Crosstalk maintient alors votre touche vocale pendant exactement le temps de la phrase :

1. **Préparation voix** : transcription, traduction et synthèse Piper sont **terminées** avant tout appui.
2. **Activation micro** : appui sur la touche (`K` par défaut, modifiable), puis un court délai (250 ms par défaut, réglable de 50 à 1 000 ms) pour que CS2 ouvre son micro.
3. **Transmission** : la phrase est jouée dans le câble. Crosstalk suit ce que le câble a **réellement joué** (tampon du périphérique compris), pas une simple durée de WAV, puis garde la touche une petite marge (350 ms par défaut, 100 à 1 500 ms) pour ne pas couper la dernière syllabe.
4. **Terminé** : la touche est relâchée, une seule fois.

**Conditions** : CS2 **au premier plan**, « CABLE Output » choisi comme micro (étape 4), et la touche réglée identique à votre bind CS2 (Paramètres › Clavier/Souris › Communication › « Utiliser le micro »).

**La touche est toujours relâchée** : fin normale, erreur, « Annuler la transmission », perte du premier plan (Alt+Tab), changement de réglage, arrêt de la voix IA ou fermeture de Crosstalk. Les phrases successives passent l'une après l'autre, chacune avec son appui.

**Rien n'est appuyé** si l'option est désactivée, si CS2 n'est pas au premier plan, ou si vous tenez déjà la touche vous-même (Crosstalk ne la relâche pas à votre place) ; la phrase est alors jouée normalement et la carte dit pourquoi. Si Windows refuse l'entrée (par exemple CS2 lancé en administrateur et pas Crosstalk), la carte l'indique : maintenez la touche vous-même.

**Tester** : « Tester dans 5 s », passez sur CS2 : la phrase d'essai part touche enfoncée. L'état s'affiche : Préparation voix → Activation micro → Transmission → Terminé.

Touches acceptées : lettres, chiffres, F1–F12, Space, Tab, CapsLock, Shift / Ctrl / Alt (gauche ou droite), Mouse3, Mouse4, Mouse5.

**Comment** : l'appui est émis avec `SendInput` (API Windows documentée pour les entrées synthétiques, en codes de balayage comme un vrai clavier), depuis le processus de Crosstalk. Rien n'est injecté dans CS2, sa mémoire n'est ni lue ni écrite, aucun hook clavier. Le premier plan est vérifié par la fenêtre active (`GetForegroundWindow`) et le nom de son processus (`cs2.exe`). Code : `src-tauri/src/voice/ptt.rs`.

## Voix disponibles

| Langue | Voix | Taille |
| --- | --- | --- |
| Russe | Irina (femme), Dmitri (homme) | 63 Mo chacune |
| Anglais | Amy (femme), Ryan (homme) | 63 Mo |
| Français | Siwis (femme), Tom (homme) | 63 Mo |
| Allemand | Thorsten (homme) | 63 Mo |
| Espagnol | Davefx (homme) | 63 Mo |
| Polonais | Gosia (femme), Darkman (homme) | 63 Mo |
| Turc | Dfki (homme) | 63 Mo |
| Ukrainien | Lada (femme) | 21 Mo |

Les voix viennent de [rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices) (v1.0.0). Chaque voix a sa propre licence, indiquée dans sa fiche (`MODEL_CARD`).

Le moteur est Piper 2023.11.14-2 pour Windows (licence MIT), depuis les publications GitHub de `rhasspy/piper`. Son archive est vérifiée par taille et SHA-1. Les voix sont vérifiées par leur taille exacte, comme les modèles de traduction.

## Fonctionnement

```
micro réel ──┐
             ├─► mélange ─► CABLE Input ══ VB-CABLE ══► CABLE Output ─► CS2 (micro)
voix Piper ──┘                └─► (option) votre casque
```

- **Piper** (`src-tauri/src/voice/piper.rs`) reste lancé pour la voix choisie, en mode `--json-input` : une ligne JSON par phrase, Piper écrit le WAV et renvoie son chemin.
  - Mesuré sur la machine de développement (Linux, processeur) : 0,6 s pour la première phrase, chargement du modèle compris, puis 0,08 s par phrase.
  - Sous Wine (émulation Windows), même code avec le vrai `piper.exe` : 3,7 s puis 0,6 s.
- **Le routeur** (`src-tauri/src/voice/router.rs`, Windows) lit votre micro et l'écrit dans « CABLE Input », avec un tampon de 40 ms côté sortie. Il y ajoute la voix IA, rééchantillonnée au format du câble.
  - Les volumes du micro et de la voix sont réglables séparément.
- **La dictée** (Ctrl+Alt+F6) enregistre le micro choisi à l'étape 2, et non le micro par défaut de Windows, qui est désormais le câble.
- Une phrase dite apparaît dans la fenêtre Répondre (« 🔊 Dit en jeu par la voix IA »). Une réponse tapée peut aussi être dite avec le bouton « 🔊 Dire en jeu ».

## Respect du jeu et des règles

- Crosstalk ne lit ni ne modifie CS2. La voix passe par un périphérique audio, comme un micro ou un changeur de voix.
- Aucune touche n'est simulée, sauf si vous activez l'appui automatique : la touche vocale est alors appuyée par `SendInput`, uniquement pendant la phrase et seulement quand CS2 est au premier plan.
- **Certains serveurs, ligues ou plateformes interdisent les voix de synthèse ou les modificateurs de voix** : vérifiez leur règlement.

## Testé, non testé

**Testé :**
- **Linux, avec le vrai moteur Piper** (version Linux, même version) et une voix russe : deux phrases synthétisées par un seul processus.
- **Sous Wine, avec le vrai `piper.exe` de Windows** :
  - lancement par le code de l'application (fenêtre masquée, chemins Windows) ;
  - deux phrases synthétisées ;
  - extraction de la vraie archive du moteur.
- **Le reste de la chaîne**, par des tests automatiques : mélange, rééchantillonnage, file d'attente, catalogue, détection de VB-CABLE, réglages et interface.
- **La compilation Windows** du routeur et de la sortie audio (clippy, cible `x86_64-pc-windows-gnu`).

**Non testé :**
- **Le son réel** : le routage du micro, la sortie dans VB-CABLE et l'écoute par CS2 demandent un PC Windows avec carte son. Il n'y en a pas dans l'environnement de développement.
- **La latence et la qualité perçue** en partie.
- **L'appui automatique dans un vrai CS2** : on ne sait pas encore si CS2 accepte les appuis `SendInput` pour la touche vocale, ni si les coéquipiers reçoivent la phrase entière. Testé seulement par des tests automatiques (ordre appui → délai → audio → vidange du tampon → marge → relâchement, annulation, perte du premier plan, fermeture, file de phrases, touche déjà tenue) et la compilation Windows. Si CS2 ignorait ces appuis, la carte le montrera par l'absence de voix chez les coéquipiers : maintenez alors la touche vous-même ou passez en micro ouvert.
- **Les voix v1.0.0 téléchargées depuis Hugging Face** : l'environnement de développement n'y a pas accès. Les tests ont utilisé une voix russe plus ancienne du même moteur (GitHub). Les tailles du catalogue viennent des métadonnées publiées.

## Limites connues

- **Les callouts anglais dans une phrase russe** (« B », « smoke ») sont prononcés avec les règles de la langue de la voix.
- **Une seule voix parle à la fois** : les phrases se suivent dans l'ordre.
- **Appui automatique** : le bouton « Dire en jeu » de la carte est cliqué dans Crosstalk, donc CS2 n'est pas au premier plan : utilisez « Tester dans 5 s » ou Ctrl+Alt+F6 en jeu.
- **Si le micro choisi est débranché**, son passage s'arrête : la carte affiche l'erreur. Choisissez de nouveau le micro pour relancer.
