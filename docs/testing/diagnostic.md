# Protocole de test : journal et rapport de diagnostic

Sur Windows, avec l'application installée.

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Lancer Crosstalk, ouvrir **Diagnostic** | Le journal (à droite) montre des lignes réelles horodatées : version et build, `cs2`, `asr` (Parakeet, CPU), `translation` (ru→fr, en→fr), `chat`. Le libellé indique « LOGS · 7 JOURS ». |
| 2 | Lancer puis fermer CS2, démarrer puis arrêter une session | Lignes `cs2.exe lancé (PID …)`, `capture : cs2.exe ciblé…`, `capture arrêtée (user)`, `cs2.exe non lancé`. Une ligne par changement, pas de répétition. |
| 3 | Laisser passer des phrases (voix et chat) | **Aucune** phrase, traduction ni pseudo n'apparaît dans le journal. |
| 4 | Cliquer **Dossier des journaux** | L'Explorateur ouvre `%LOCALAPPDATA%\Crosstalk\logs` avec `crosstalk-<date>.log` ; le contenu correspond à l'écran. Le nom d'utilisateur Windows est remplacé par `[utilisateur]` dans les chemins. |
| 5 | Cliquer **Tests…** / **Relancer** | Les tests se relancent réellement (nouvelle recherche de CS2 et de `console.log`, raccourcis relus). |
| 6 | Cliquer **Exporter le rapport** | Message « Rapport exporté : C:\Users\…\Documents\Crosstalk-diagnostic-….txt ». Le fichier contient version, build Windows, liste des modèles, tests intégrés et journal ; ni audio, ni texte de partie, ni pseudo, ni nom d'utilisateur. |
| 7 | Cliquer **Signaler un problème** | Le navigateur ouvre une nouvelle issue GitHub pré-remplie (version, build, Windows) ; on peut y glisser le rapport. |
| 8 | Placer un fichier `crosstalk-2020-01-01.log` dans le dossier, relancer | Il est supprimé au démarrage (plus de 7 jours). |
