# Protocole de test — capture audio (feat/audio-capture)

À exécuter sur un PC **Windows 10/11 x64** avec Counter-Strike 2 installé. Les tests automatisés (`scripts\validate.ps1`, en local) couvrent la logique, la compilation Windows et quelques appels réels (énumération des processus et des périphériques). En revanche, **ils ne testent pas la capture du son de CS2** : ces étapes sont manuelles.

Notez le résultat de chaque étape (✅ / ❌ + remarque) dans le tableau final, puis collez-le en commentaire de la PR.

## 0. Préparation

1. Sur le commit de la PR, construisez le paquet de test : `powershell -ExecutionPolicy Bypass -File scripts\validate.ps1` (voir CONTRIBUTING.md, « Validation locale »).
2. Le dossier `dist-local\<commit>\` contient :
   - `crosstalk-audio-probe.exe`, l'outil de diagnostic en ligne de commande ;
   - `crosstalk.exe` et les installateurs (`*-setup.exe`, `*.msi`).
3. Installez Crosstalk avec `*-setup.exe`. Si une version précédente est installée, elle est mise à jour.
4. Notez votre version de Windows : `Win + R`, puis `winver`. Exemple : 22H2 build 19045, ou 23H2 build 22631.
5. Dans CS2, utilisez le mode d'affichage **fenêtré sans bordures**. Le volume du jeu doit être audible.

## 1. Outil de diagnostic (sans interface)

Ouvrez un terminal (PowerShell) dans le dossier `dist-local\<commit>\`.

| # | Commande | Résultat attendu |
| --- | --- | --- |
| 1.1 | `.\crosstalk-audio-probe.exe --list` | La ligne « Windows build » montre votre build. La liste des périphériques de sortie s'affiche, celui par défaut est marqué `*`. |
| 1.2 | CS2 **fermé** : `.\crosstalk-audio-probe.exe` | « CS2 : non détecté », puis l'erreur `cs2_not_running`. **Aucune** capture du périphérique ne démarre. |
| 1.3 | CS2 **lancé** (menu principal, musique audible) : `.\crosstalk-audio-probe.exe --seconds 15` | « CS2 : cs2.exe, PID … », puis « Source : cs2.exe (PID …), capture par processus ». Les barres de niveau bougent avec la musique. À la fin : « signal reçu ». |
| 1.4 | Même chose, mais en coupant le son de CS2 pendant la capture et en lançant une vidéo YouTube | Le niveau tombe au silence : seul le son de CS2 est capturé, pas celui du navigateur. |
| 1.5 | `.\crosstalk-audio-probe.exe --mode device --seconds 10` avec une vidéo YouTube | Source : « périphérique … ». La vidéo fait bouger le niveau, puisque ce mode capte tout le son du périphérique. |
| 1.6 | `.\crosstalk-audio-probe.exe --seconds 20 --wav test.wav`, CS2 lancé, avec quelqu'un qui parle dans le chat vocal (voir §3) | `test.wav` contient le son de CS2 et la voix, sans autre application. Vous pouvez supprimer ce fichier après écoute. |
| 1.7 | Notez les durées « Ouverture en … ms » et « Arrêt en … ms » | Arrêt **< 200 ms**. |

**Windows 10 22H2 (build 19045) :** la capture par processus n'y est pas documentée par Microsoft. Le résultat de 1.3 est exactement l'information à remonter :
- l'erreur `process_loopback_unavailable` → Crosstalk utilisera le repli ;
- ou une capture réussie → très bonne nouvelle.

## 2. Application Crosstalk

| # | Action | Résultat attendu |
| --- | --- | --- |
| 2.1 | Lancez Crosstalk, CS2 fermé. Écran **Audio & IA** | Le libellé en haut à droite indique « CS2 · NON DÉTECTÉ ». La jauge « Niveau d'entrée · réel » indique « aucune capture ». |
| 2.2 | Lancez CS2 | En **2 s au plus**, le libellé passe à « CS2 · DÉTECTÉ ». Le PID apparaît dans l'option « Audio de CS2 uniquement ». |
| 2.3 | Cliquez **Tester la capture 10 s**, musique du menu CS2 audible | La jauge réagit au son. À la fin : « Signal reçu · crête … dBFS ». |
| 2.4 | **Accueil**, puis **Démarrer** | La carte « Capture » passe à « Process loopback », la carte « Counter-Strike 2 » à « Détecté · PID ». L'indicateur en haut à droite affiche « ● SIGNAL AUDIO » quand le jeu émet du son, « silence » sinon. L'anneau d'onde de la scène 3D suit le niveau. |
| 2.5 | Vérifiez l'étiquette **SIMULÉ** à côté de « Script » et la mention **DÉMO ·** sur le sous-titre | Les répliques affichées sont des exemples : la transcription arrive dans la prochaine fonctionnalité. |
| 2.6 | **Pause**, puis **Reprendre** | La capture s'arrête (« Inactive »), puis reprend. |
| 2.7 | **Ctrl+Alt+F10** pendant que le jeu a le focus | La capture s'arrête immédiatement. Audio & IA ou l'Accueil indiquent « Arrêt d'urgence · tampons purgés ». |
| 2.8 | Session démarrée, **fermez CS2** | La capture s'arrête : « CS2 fermé · capture arrêtée ». |
| 2.9 | Audio & IA : choisissez **Périphérique de sortie**, sélectionnez votre casque, puis **Tester la capture 10 s** | L'avertissement « capte tout le son du périphérique » s'affiche, et la jauge réagit à toutes les applications. |
| 2.10 | Capture en cours sur un casque **USB** : débranchez-le, puis rebranchez-le | Crosstalk tente de rouvrir la capture : soit elle reprend, soit l'erreur « périphérique retiré » s'affiche. **Aucun plantage.** |
| 2.11 | Écran **Diagnostic** | La ligne « Process loopback » montre votre vrai build et l'état de la capture. |
| 2.12 | Fermez la fenêtre (✕) pendant la capture | La fenêtre se masque dans la zone de notification et la capture continue. Ctrl+Alt+F10 l'arrête toujours. |

## 3. Obtenir une voix dans CS2 sans coéquipier (optionnel)

Avec un micro virtuel (par exemple VB-Cable) choisi comme micro dans CS2, tapez `voice_loopback 1` dans la console : vous entendez votre propre voix telle que le jeu la restitue. Lisez un enregistrement russe dans le micro virtuel, puis lancez l'étape 1.6. Ainsi, la voix passe par le vrai chemin audio de CS2. Pensez à taper `voice_loopback 0` après le test.

## 4. Résultats à remonter

```
Windows : … (build …)          GPU/CPU : …
| Étape | Résultat | Remarque |
| 1.1 | | |
| 1.2 | | |
| 1.3 | | source obtenue : processus / repli |
| 1.4 | | |
| 1.5 | | |
| 1.6 | | |
| 1.7 | | ouverture … ms / arrêt … ms |
| 2.1 – 2.12 | | |
```
