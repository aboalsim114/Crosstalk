# Protocole de test : Mes langues

À exécuter sur Windows 10/11 x64. Rien de ce protocole n'a encore été exécuté sur Windows (voir [languages.md](../languages.md), Limites). Ne mettez dans la PR que des compteurs et des phrases inventées, jamais les messages ni les pseudos de vrais joueurs.

| # | Action | Attendu |
|---|---|---|
| 1 | Nouvelle installation sur un Windows en russe (ou `Paramètres › Heure et langue` réglé sur le russe) | L'installateur télécharge `opus-mt-en-ru` (anglais → russe) et pas `opus-mt-ru-fr`. |
| 2 | Premier lancement, « Entrer » | Fenêtre « Vos langues » : « Русский » présélectionné, « Русский » grisé parmi les coéquipiers, « EN → RU ». Aucun téléchargement tant que « Continuer » n'est pas cliqué. |
| 3 | Cocher « Deutsch », Continuer | « DE → RU · via l'anglais ». La bannière télécharge `opus-mt-de-en` puis, pour « Répondre », `opus-mt-ru-en` et `opus-mt-en-de`. |
| 4 | Audio & IA › test « Traduire » (EN) | Phrase en russe correcte. Notez le temps. |
| 5 | Partie : un coéquipier parle anglais | Ligne `EN → RU`, overlay « ГОЛОС • АНГЛИЙСКИЙ → РУССКИЙ » si l'interface est en russe. |
| 6 | Un coéquipier parle russe | Ligne affichée telle quelle, « Déjà dans votre langue — non traduit ». |
| 7 | Chat : « wartet, ich gehe links » | Détecté allemand, traduit en russe en passant par l'anglais ; noter le temps (environ le double d'une paire directe). |
| 8 | Chat dans une langue non cochée (ex. « espera, vamos ») | Affiché tel quel, « langue non choisie (Langues) ». |
| 9 | Répondre (Ctrl+Alt+F7) | Choix « English » / « Deutsch » ; écrire en russe ; le texte copié est en anglais ou en allemand. Dicter (Ctrl+Alt+F6) en russe : Whisper transcrit en russe. |
| 10 | Revenir à « Français » | Retour au comportement d'avant : lexique CS instantané (« rush B » → « rush B »), modèles ru → fr et en → fr. |

Compte rendu : pour 10 phrases par paire testée, notez juste / compréhensible / faux, le temps médian et, pour les paires passant par l'anglais, la différence de temps.
