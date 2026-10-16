# Protocole de test : Mes langues

À exécuter sur Windows 10/11 x64, avec un moteur connecté dans Audio & IA › Traduction IA (voir [translation.md](translation.md)). Rien de ce protocole n'a encore été exécuté sur Windows (voir [languages.md](../languages.md), Limites). Ne mettez dans la PR que des compteurs et des phrases inventées, jamais les messages ni les pseudos de vrais joueurs.

| # | Action | Attendu |
|---|---|---|
| 1 | Nouvelle installation sur un Windows en russe | L'installateur ne télécharge aucun modèle de traduction. |
| 2 | Premier lancement, « Entrer » | Fenêtre « Vos langues » : « Русский » présélectionné, « Русский » grisé parmi les coéquipiers, « EN → RU ». Puis la fenêtre « Moteur de traduction ». |
| 3 | Cocher « Deutsch » | « DE → RU » direct, rien à télécharger. |
| 4 | Audio & IA › test « Traduire » (EN) | Phrase en russe correcte. Notez le temps. |
| 5 | Partie : un coéquipier parle anglais | Ligne `EN → RU`, overlay « ГОЛОС • АНГЛИЙСКИЙ → РУССКИЙ » si l'interface est en russe. |
| 6 | Un coéquipier parle russe | Ligne affichée telle quelle, « Déjà dans votre langue — non traduit ». |
| 7 | Chat : « wartet, ich gehe links » | Détecté allemand, traduit en russe directement. |
| 8 | Chat dans une langue non cochée (ex. « espera, vamos ») | Affiché tel quel, « langue non choisie (Langues) ». |
| 9 | Répondre (Mouse 4 / Ctrl+Alt+F7) | Choix « English » / « Deutsch » ; écrire en russe ; le texte copié est en anglais ou en allemand. Dicter (Mouse 5 / Ctrl+Alt+F6) en russe : la reconnaissance vocale transcrit en russe. |
| 10 | Lire en polonais ou en turc | Les traductions arrivent en polonais / turc (possible maintenant avec le LLM). |

Compte rendu : pour 10 phrases par paire testée, notez juste / compréhensible / faux et le temps médian.
