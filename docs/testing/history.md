# Protocole de test : historique local

Sur Windows, avec l'application installée et une partie CS2 (ou une vidéo avec des voix et le chat).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Premier lancement, ouvrir **Historique** | Interrupteur **désactivé**. Aucun fichier `%LOCALAPPDATA%\Crosstalk\history.jsonl`. |
| 2 | Lancer une session sans activer l'historique, laisser passer des phrases | Les sous-titres s'affichent ; `history.jsonl` n'existe toujours pas. |
| 3 | Activer l'historique, laisser passer quelques phrases (voix et chat) | Les lignes apparaissent dans la page Historique (les plus récentes en haut), une seule fois chacune, sans les mots « EN DIRECT » provisoires. |
| 4 | Ouvrir `history.jsonl` dans le Bloc-notes | Une ligne JSON par phrase : date, type, langue, original, traduction. Aucun audio. |
| 5 | Taper un mot d'une phrase (ou un pseudo du chat) dans la recherche | Seules les lignes qui le contiennent restent ; « Aucun résultat. » sinon. |
| 6 | Cliquer **TXT**, puis **JSON** | Message « Exporté : C:\Users\…\Documents\Crosstalk-historique-….txt / .json » ; les fichiers s'ouvrent et contiennent les lignes. |
| 7 | Quitter et relancer Crosstalk | Les lignes sont toujours là ; l'interrupteur garde son état. |
| 8 | Cliquer **Effacer** | La liste est vide, `history.jsonl` est vide ou supprimé. |
| 9 | Désactiver l'historique, laisser passer des phrases | Rien n'est ajouté. |

Purge à 7 jours : vérifiée par les tests unitaires (`cargo test history`), avec une horloge simulée. Elle a lieu au démarrage et à chaque ouverture de la page.
