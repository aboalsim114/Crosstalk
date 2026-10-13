# Protocole de test : overlay sur deux écrans

Sur Windows, avec deux écrans et CS2 en **fenêtré sans bordures** (le plein écran exclusif cache toute fenêtre par-dessus le jeu).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | CS2 sur l'écran secondaire, Crosstalk sur l'écran principal ; démarrer une session | En moins de 3 s, l'overlay se place dans le coin choisi **de l'écran de CS2**. |
| 2 | Changer le coin (Overlay › coin d'ancrage) | L'overlay change de coin sur l'écran de CS2. |
| 3 | Déplacer CS2 sur l'autre écran (Win+Maj+flèche) | L'overlay suit dans les 3 s. |
| 4 | Fermer CS2 | L'overlay reste où il est (aucune recherche ne le déplace). |
