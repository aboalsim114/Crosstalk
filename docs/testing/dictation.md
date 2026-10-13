# Protocole de test : réponse vocale (dictée)

Sur Windows, avec un micro branché (micro par défaut de Windows, rôle « communications ») et les modèles de réponse fr→en / fr→ru installés.

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Hors partie : Langues & glossaire › Répondre dans leur langue › **🎙 Dicter** | La fenêtre de réponse s'ouvre : « Parlez en français… », jauge du micro, compteur. L'icône micro de Windows apparaît dans la barre des tâches. |
| 2 | Dire « deux en B, je drop l'AWP » puis se taire | Après ~1 s de silence : « Transcription et traduction… », puis « « deux en B, je drop l'AWP » » et la traduction (russe ou anglais), « Copié ✓ ». La fenêtre se ferme après 2,5 s ; l'icône micro de Windows disparaît. Ctrl+V dans le Bloc-notes colle la traduction. |
| 3 | En partie (CS2 fenêtré sans bordures) : **Ctrl+Alt+F6**, parler, se taire | Même chose par-dessus le jeu ; puis U, Ctrl+V, Entrée : le message part dans le chat d'équipe. |
| 4 | Ctrl+Alt+F6, parler, puis Ctrl+Alt+F6 (ou « Terminer », ou Échap) avant la fin | La dictée s'arrête tout de suite et traduit ce qui a été dit. |
| 5 | Ctrl+Alt+F6 puis rester silencieux | Après 5 s : « Aucune parole entendue : vérifiez le micro par défaut de Windows. » |
| 6 | Débrancher le micro, Ctrl+Alt+F6 | « Aucun micro actif… », sans plantage. |
| 7 | Icône de notification › « Dicter une réponse » | Même comportement. |
| 8 | Diagnostic › journal | `dictation: réponse dictée traduite (ru, model)` ; jamais le texte dit. |

Aucun audio n'est écrit sur disque ni envoyé : le micro n'est ouvert que pendant la dictée.
