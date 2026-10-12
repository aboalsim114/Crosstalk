# Protocole de test : mise à jour depuis l'application

Fonctionnement : au lancement (après 8 s) puis toutes les 6 h, Crosstalk lit `latest.json` (publié par `scripts/publish-dev-build.sh` à côté de l'installateur). Si le numéro de build publié est plus grand que le sien, un bandeau propose la mise à jour. L'installateur est téléchargé dans `%TEMP%\Crosstalk-update`, vérifié (taille exacte, SHA-256), puis lancé en silencieux (`/S /LAUNCH`) : même dossier, raccourci du bureau gardé ou non comme avant, modèles et réglages conservés, puis Crosstalk redémarre.

> **Prérequis** : le flux doit être accessible sans compte GitHub. Tant que le dépôt est privé, la recherche échoue (404) et À propos l'explique. Pour tester avant cela, pointez l'application vers un flux accessible : variable d'environnement `CROSSTALK_UPDATE_URL` (URL HTTPS de `latest.json` sur `raw.githubusercontent.com` ou `github.com`).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | À propos : version, build et commit | Les valeurs réelles de la version installée (plus de date figée). |
| 2 | Dépôt privé, aucun `CROSSTALK_UPDATE_URL` : « Rechercher une mise à jour » | « Mises à jour indisponibles : le dépôt de publication est privé… ». Aucun bandeau. |
| 3 | Flux accessible annonçant un build plus récent | Bandeau « Nouvelle version disponible · build N » avec les titres des dernières PR ; « Plus tard » le masque jusqu'à la version suivante. |
| 4 | « Mettre à jour » | Progression en % et Mo, puis « Mise à jour prête ». |
| 5 | Session démarrée, puis « Redémarrer et installer » | Bouton grisé : « Arrêtez la session pour installer ». |
| 6 | Session arrêtée, « Redémarrer et installer » | Crosstalk se ferme ; quelques secondes plus tard il redémarre dans la nouvelle version (À propos : nouveau build). Même dossier, réglages et modèles conservés, aucun nouveau téléchargement de modèle. |
| 7 | Modifier un octet du fichier `latest.json` servi (empreinte) | « Mise à jour impossible : empreinte SHA-256 invalide : installateur refusé ». Rien n'est installé. |
