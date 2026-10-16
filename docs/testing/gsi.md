# Protocole de test : carte en cours (Game State Integration)

Sur Windows, avec l'application installée et CS2 détecté (Langues & glossaire › Chat écrit).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Ouvrir **Langues & glossaire**, carte « Carte en cours » | État **Désactivée**. Aucun fichier `gamestate_integration_crosstalk.cfg` dans `…\Counter-Strike Global Offensive\game\csgo\cfg`. |
| 2 | Cliquer **Activer** | Le fichier apparaît dans le dossier cfg (il ne demande que `"map" "1"` : carte, mode, phase du match et score, utilisés aussi par le [résumé de partie](recap.md)) ; état « Activée · en attente de CS2 ». Journal (Diagnostic) : `gsi: carte en cours : en attente de CS2`. |
| 3 | Lancer CS2 (le relancer s'il était ouvert) et rester dans les menus | État « CS2 connecté · dans les menus ». |
| 4 | Lancer une partie (bots ou casual) sur Mirage | État « Carte : Mirage », « 23 callouts de Mirage… ». Journal : `gsi: carte en cours : de_mirage · 23 callouts`. |
| 5 | Annoncer à voix haute (ou faire jouer) « they're in palace, one jungle » | La transcription restitue « palace » et « jungle » (sans indice de carte depuis le changement de moteur : à vérifier en partie) ; la traduction les garde tels quels. |
| 6 | Changer de carte (ex. Inferno) | L'état suit la carte ; « banana », « coffins »… sont reconnus et conservés. |
| 7 | Fermer CS2 | La carte disparaît (« Activée · en attente de CS2 »). |
| 8 | Cliquer **Retirer** | Le fichier cfg est supprimé ; état **Désactivée**. |
| 9 | Vérifier qu'aucun avertissement anti-triche n'apparaît | GSI est l'interface officielle de Valve : aucun accès au processus du jeu. |
