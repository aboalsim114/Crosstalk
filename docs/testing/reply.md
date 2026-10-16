# Protocole de test : répondre dans leur langue

Sur Windows, avec l'application installée (installateur neuf : les modèles fr→en et fr→ru sont téléchargés ; après une mise à jour : bouton **Télécharger** dans la carte).

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Langues & glossaire › carte « Répondre dans leur langue » | Si les modèles fr→en / fr→ru manquent : bandeau avec **Télécharger** (≈ 610 Mo) ; la progression apparaît comme pour les autres modèles. |
| 2 | Cliquer la phrase rapide **Éco** (langue Russe) | « Эко раунд » s'affiche, « Phrase CS vérifiée · Copié ✓ … » ; Ctrl+V dans le Bloc-notes colle « Эко раунд ». |
| 3 | Taper « deux en B » puis Entrée (Anglais) | « Two B », instantané (phrase CS). |
| 4 | Taper une phrase libre, ex. « on se regroupe au spawn et on attend la fin des smokes » puis Entrée | Premier message : quelques secondes (chargement du modèle) ; puis une traduction anglaise ou russe, « smoke » conservé ; messages suivants en moins d'une seconde environ. |
| 5 | Lancer CS2 (fenêtré sans bordures), en partie appuyer sur **Alt** | La fenêtre « Répondre dans leur langue » apparaît en bas au centre, le champ a le focus. La langue proposée est celle entendue le plus récemment chez les coéquipiers (russe si les dernières phrases étaient en russe). |
| 6 | Taper « drop moi stp », Entrée | Traduction copiée, la fenêtre se ferme seule après ~2 s et le jeu reprend le focus. Dans CS2 : **U** (équipe), **Ctrl+V**, **Entrée** : le message part dans le chat. |
| 7 | Alt puis **Échap** | La fenêtre se ferme sans rien copier. |
| 8 | Icône de notification › « Répondre dans leur langue » | Même fenêtre. |
| 9a | En partie, en avançant (W maintenu), taper **Alt** seul | La fenêtre s'ouvre : les touches déjà enfoncées n'annulent pas le raccourci. |
| 9b | **Alt+Tab**, **Alt Gr+E** (€), garder Alt enfoncé plus d'une demi-seconde, **Ctrl** puis Alt | Rien ne s'ouvre : seul un appui bref sur Alt, sans autre touche, compte. |
| 9c | Taper **Alt Gr** seul, parler, taper Alt Gr à nouveau | La dictée démarre puis s'arrête (comme l'ancien Ctrl+Alt+F6). |
| 9 | Diagnostic › Raccourcis globaux | « Alt / Alt Gr / Ctrl+Alt+F8 / F9 / F10 · aucun conflit » (ou le raccourci en conflit). |

Alt et Alt Gr sont lus avec `GetAsyncKeyState` (état des touches, sans hook clavier ni accès au jeu), seulement autour d'un appui sur l'une des deux. Crosstalk n'envoie aucune touche au jeu : l'utilisateur colle et envoie lui-même le message.
