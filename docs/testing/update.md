# Protocole de test : mise à jour depuis l'application

Fonctionnement : au lancement (après 8 s) puis toutes les 6 h, Crosstalk lit `latest.json` (publié par `scripts/publish-dev-build.sh` à côté de l'installateur). Si le numéro de build publié est plus grand que le sien, un bandeau propose la mise à jour. L'installateur est téléchargé dans `%TEMP%\Crosstalk-update`, vérifié (taille exacte, SHA-256), puis lancé en silencieux (`/S /LAUNCH`) : même dossier, raccourci du bureau gardé ou non comme avant, modèles et réglages conservés, puis Crosstalk redémarre.

**Nouveautés** : `release-notes.json` (rédigé pour les joueurs, une entrée par PR visible) est publié dans `latest.json` (`news`) avec le build qui a apporté chaque entrée (`news.json` sur la branche `downloads` garde cette attribution). `notes` reprend les phrases de la dernière entrée pour les versions plus anciennes de l'application.

> **Prérequis** : le flux doit être accessible sans compte GitHub. Le dépôt est public depuis le 13/10/2026 : `latest.json` et l'installateur se téléchargent sans compte. S'il redevenait privé, la recherche échouerait (404) et À propos dirait « Impossible de vérifier les mises à jour pour le moment ». `CROSSTALK_UPDATE_URL` permet de pointer vers un autre flux (HTTPS, `raw.githubusercontent.com` ou `github.com`).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | À propos : version | « Version 0.4.1.N · Windows » (N : build installé). Aucun commit, aucun lien, aucune empreinte affichés. |
| 1 bis | Premier lancement après une mise à jour | Fenêtre « Quoi de neuf ? » avec les nouveautés depuis la version précédente, une seule fois (« C'est parti » la ferme ; elle ne revient pas au lancement suivant). |
| 2 | Build installé = dernier build publié : « Rechercher une mise à jour » | « Crosstalk est à jour. » Aucun bandeau. |
| 3 | Flux accessible annonçant un build plus récent | Bandeau « Nouvelle version disponible » avec les titres des nouveautés (`release-notes.json`) ; À propos liste « Ce que la mise à jour apporte ». « Plus tard » masque le bandeau jusqu'à la version suivante. |
| 4 | « Mettre à jour » | Progression en % et Mo, puis « Mise à jour prête ». |
| 5 | Session démarrée, puis « Redémarrer et installer » | Bouton grisé : « Arrêtez la session pour installer ». |
| 6 | Session arrêtée, « Redémarrer et installer » | Crosstalk se ferme ; quelques secondes plus tard il redémarre dans la nouvelle version (À propos : nouveau build). Même dossier, réglages et modèles conservés, aucun nouveau téléchargement de modèle. |
| 7 | Modifier un octet du fichier `latest.json` servi (empreinte) | « Mise à jour impossible : le fichier téléchargé était incomplet : il a été supprimé. Réessayez. » Rien n'est installé ; le détail technique est dans le journal. |
