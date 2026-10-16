# Protocole de test : répondre dans leur langue

Sur Windows, avec l'application installée (installateur neuf : les modèles fr→en et fr→ru sont téléchargés ; après une mise à jour : bouton **Télécharger** dans la carte).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Langues & glossaire › carte « Répondre dans leur langue » | Si les modèles fr→en / fr→ru manquent : bandeau avec **Télécharger** (≈ 610 Mo) ; la progression apparaît comme pour les autres modèles. |
| 2 | Cliquer la phrase rapide **Éco** (langue Russe) | « Эко раунд » s'affiche, « Phrase CS vérifiée · Copié ✓ … » ; Ctrl+V dans le Bloc-notes colle « Эко раунд ». |
| 3 | Taper « deux en B » puis Entrée (Anglais) | « Two B », instantané (phrase CS). |
| 4 | Taper une phrase libre, ex. « on se regroupe au spawn et on attend la fin des smokes » puis Entrée | Premier message : quelques secondes (chargement du modèle) ; puis une traduction anglaise ou russe, « smoke » conservé ; messages suivants en moins d'une seconde environ. |
| 5 | Lancer CS2 (fenêtré sans bordures), en partie cliquer sur le bouton latéral **Mouse 4** de la souris | La fenêtre « Répondre dans leur langue » apparaît en bas au centre, le champ a le focus. La langue proposée est celle entendue le plus récemment chez les coéquipiers (russe si les dernières phrases étaient en russe). |
| 6 | Taper « drop moi stp », Entrée | Traduction copiée, la fenêtre se ferme seule après ~2 s et le jeu reprend le focus. Dans CS2 : **U** (équipe), **Ctrl+V**, **Entrée** : le message part dans le chat. |
| 7 | Mouse 4 puis **Échap** | La fenêtre se ferme sans rien copier. |
| 8 | Icône de notification › « Répondre dans leur langue » | Même fenêtre. |
| 9a | En partie, en avançant accroupi (W et Ctrl maintenus), cliquer **Mouse 4** | La fenêtre s'ouvre : les touches déjà enfoncées n'annulent pas le raccourci. |
| 9b | Garder Mouse 4 enfoncé plus d'une demi-seconde ; appuyer sur R pendant le clic | Rien ne s'ouvre : seul un clic bref, sans nouvelle touche pendant le clic, compte. |
| 9c | Cliquer **Mouse 5**, parler, cliquer Mouse 5 à nouveau | La dictée démarre puis s'arrête. |
| 9 | Diagnostic › Raccourcis globaux | « Mouse 4 / Mouse 5 / Ctrl+Alt+F8 / F9 / F10 · aucun conflit » (ou le raccourci en conflit). |

Les boutons latéraux sont lus avec `GetAsyncKeyState` (état des boutons, sans hook ni accès au jeu) ; tout le clavier n'est lu que pendant un clic sur l'un des deux. CS2 reçoit toujours le clic : si Mouse 4 ou Mouse 5 y est lié à une commande, elle s'exécute aussi. Crosstalk n'envoie aucune touche au jeu : l'utilisateur colle et envoie lui-même le message.
