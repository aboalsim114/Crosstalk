# Protocole de test : installateur (design « Crosstalk Installer »)

À exécuter sur un PC **Windows 10 22H2 ou Windows 11 x64**, avec un compte utilisateur standard (sans droits administrateur). Notez pour chaque ligne ✅ ou ❌, avec une remarque.

Installateur : `Crosstalk-setup.exe` (lien du README) ou `dist-test\Crosstalk-<commit>-setup.exe`.

## 1. Installation

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1.1 | Lancer `Crosstalk-setup.exe` (SmartScreen : « Informations complémentaires » > « Exécuter quand même ») | Fenêtre sans bordure aux coins arrondis, casque 3D, intro avec barres cinéma, « EVERY VOICE. ONE LANGUAGE. ». Version réelle en bas à droite. |
| 1.2 | Glisser la fenêtre par la barre du haut ; bouton « — » | La fenêtre se déplace ; elle se réduit dans la barre des tâches. |
| 1.3 | « INITIALISER L'INSTALLATION » | Écran CONFIGURATION : `C:\Users\<vous>\AppData\Local\Crosstalk`, espace requis (environ 1,3 Go : application et modèles), espace réellement disponible sur le disque. |
| 1.4 | « Parcourir… » puis choisir `D:\Jeux` (ou un autre dossier) | Sélecteur de dossier Windows ; la destination devient `D:\Jeux\Crosstalk`. |
| 1.5 | Lien « conditions d'utilisation », puis Échap | La fenêtre s'ouvre, puis se ferme. |
| 1.6 | Décocher « Créer un raccourci sur le bureau », puis « INSTALLER CROSSTALK » | Écran INSTALLATION : le pourcentage avance, les phases s'allument une à une. Pendant l'extraction, le sous-titre affiche le fichier en cours et les Mo (« Transcription (Whisper small) · 420 / 1 250 Mo »). Notez la durée totale et le débit. |
| 1.7 | Fin | Écran « YOU'RE READY TO CONNECT. ». Raccourci dans le menu Démarrer, **pas** sur le bureau. Fichiers dans le dossier choisi : `crosstalk.exe`, `WebView2Loader.dll`, trois DLL MinGW, `uninstall.exe`. Modèles dans `%LOCALAPPDATA%\Crosstalk\models`. |
| 1.8 | Cocher « Lancer automatiquement au démarrage de Windows », puis « LANCER CROSSTALK » | L'installateur se ferme et Crosstalk s'ouvre. Il ne propose **pas** de retélécharger les modèles (pas de bannière « Préparation »). Dans Crosstalk, l'interrupteur de démarrage automatique est activé. |
| 1.9 | Paramètres Windows › Applications | « Crosstalk » apparaît, avec sa version et sa taille. |

## 2. Cas particuliers

| # | Action | Résultat attendu |
| --- | --- | --- |
| 2.1 | Pendant le téléchargement : « ANNULER » | Retour à CONFIGURATION ; aucun fichier de l'application dans le dossier. Relancer l'installation : le téléchargement **reprend** où il s'était arrêté. |
| 2.2 | Pendant le téléchargement : couper le Wi-Fi | Statut « ERREUR », message clair, bouton « RÉESSAYER ». Rétablir le réseau puis réessayer : l'installation se termine. |
| 2.3 | Bouton ✕ pendant l'installation, puis « QUITTER » | « Installation annulée… » puis fermeture. Rien n'est installé ; pas d'entrée dans Applications. |
| 2.4 | Crosstalk ouvert, relancer l'installateur et installer | Message « Crosstalk est ouvert : quittez-le… ». Après avoir quitté Crosstalk et réessayé, la mise à jour réussit. |
| 2.5 | Réinstaller par-dessus une installation existante | « Crosstalk 0.x.y est déjà installé : cette version le remplace. » ; aucun téléchargement de modèle (espace requis ≈ 30 Mo). |
| 2.6 | `Crosstalk-setup.exe /S` dans un terminal | Installation silencieuse, sans fenêtre (code de sortie 0). |
| 2.7 | PC sans WebView2 (machine virtuelle Windows 10 neuve), si disponible | Message proposant d'installer WebView2, puis l'installateur s'ouvre. |

## 3. Désinstallation

| # | Action | Résultat attendu |
| --- | --- | --- |
| 3.1 | Applications › Crosstalk › Désinstaller | « Désinstaller Crosstalk ? » ; puis « Supprimer aussi les modèles… ? ». Répondre **Non**. |
| 3.2 | Vérifier | Dossier de l'application, raccourcis, entrée de démarrage automatique et entrée Applications supprimés. `%LOCALAPPDATA%\Crosstalk\models` conservé. |
| 3.3 | Réinstaller, puis désinstaller en répondant **Oui** | Les modèles et `%LOCALAPPDATA%\app.crosstalk.desktop` sont aussi supprimés. |

## 4. Compte rendu

```
Commit : …   Windows : … (build …)   Disque : SSD/HDD   Connexion : … Mb/s
1.1 – 1.9 : ✅/❌ + remarques     Durée de l'installation : … min
2.1 – 2.7 : ✅/❌ + remarques
3.1 – 3.3 : ✅/❌ + remarques
Rendu : fluidité de la scène 3D (saccades ?), polices, coins arrondis : …
```
