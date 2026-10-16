# Protocole de test : réglages conservés

Sur Windows, avec l'application installée.

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Changer : seuil VAD 70 %, format d'overlay **Minimal**, coin bas-gauche, votre pseudo (Langues & glossaire), un terme ajouté au glossaire, mode du Script **Voix** | Les réglages s'appliquent tout de suite. |
| 2 | Quitter Crosstalk (icône près de l'horloge › Quitter), puis le relancer | Tous les réglages de l'étape 1 sont revenus ; le moteur charge directement **Parakeet TDT 0.6B v3** (Audio & IA). Un ancien réglage `model` Whisper (`base`, `small`…) ou `accel` est ignoré : le modèle par défaut s'applique. |
| 3 | Ouvrir `%APPDATA%\Crosstalk` | `config.json` (version 1) et `glossary.json` y sont, lisibles. Aucune voix, aucun texte de partie dedans. |
| 4 | Remplacer le contenu de `config.json` par `{ cassé`, relancer | Réglages par défaut, un message signale le fichier illisible ; `config.json.bak` garde l'ancien contenu. |
| 5 | Supprimer `%APPDATA%\Crosstalk`, relancer | Réglages par défaut, sans erreur. |
