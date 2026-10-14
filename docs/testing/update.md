# Protocole de test : mise à jour depuis l'application

Fonctionnement : au lancement (après 8 s) puis toutes les 6 h, Crosstalk lit `latest.json` (publié par `scripts/publish-dev-build.sh` à côté de l'installateur). Si le numéro de build publié est plus grand que le sien, un bandeau propose la mise à jour. L'installateur est téléchargé dans `%TEMP%\Crosstalk-update`, vérifié (taille exacte, SHA-256), puis lancé en silencieux (`/S /LAUNCH`) : même dossier, raccourci du bureau gardé ou non comme avant, modèles et réglages conservés, puis Crosstalk redémarre.

> **Prérequis** : le flux doit être accessible sans compte GitHub. Le dépôt est public depuis le 13/10/2026 : `latest.json` et l'installateur se téléchargent sans compte. S'il redevenait privé, la recherche échouerait (404) et À propos l'expliquerait. `CROSSTALK_UPDATE_URL` permet de pointer vers un autre flux (HTTPS, `raw.githubusercontent.com` ou `github.com`).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | À propos : version, build et commit | Les valeurs réelles de la version installée (plus de date figée). |
| 2 | Build installé = dernier build publié : « Rechercher une mise à jour » | « Crosstalk est à jour. » Aucun bandeau. |
| 3 | Flux accessible annonçant un build plus récent | Bandeau « Nouvelle version disponible · build N » avec les titres des dernières PR ; « Plus tard » le masque jusqu'à la version suivante. |
| 4 | « Mettre à jour » | Progression en % et Mo, puis « Mise à jour prête ». |
| 5 | Session démarrée, puis « Redémarrer et installer » | Bouton grisé : « Arrêtez la session pour installer ». |
| 6 | Session arrêtée, « Redémarrer et installer » | Crosstalk se ferme ; quelques secondes plus tard il redémarre dans la nouvelle version (À propos : nouveau build). Même dossier, réglages et modèles conservés, aucun nouveau téléchargement de modèle. |
| 7 | Modifier un octet du fichier `latest.json` servi (empreinte) | « Mise à jour impossible : empreinte SHA-256 invalide : installateur refusé ». Rien n'est installé. |
