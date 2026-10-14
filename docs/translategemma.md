# TranslateGemma : installation et utilisation hors ligne

Crosstalk traduit en français les annonces qu'il ne connaît pas avec **TranslateGemma 4B**, un modèle de traduction de Google, **sur votre PC**. Aucun compte, aucune clé API, aucun service à installer à part (pas d'Ollama, pas de Python).

## Ce qui se passe à l'installation

1. `Crosstalk-setup.exe` installe l'application, puis télécharge les modèles dans `%LOCALAPPDATA%\Crosstalk\models` :
   - TranslateGemma 4B, format GGUF Q4_K_M, **2,49 Go** (`translategemma-4b-it\translategemma-4b-it.Q4_K_M.gguf`) ;
   - Whisper (reconnaissance vocale), la détection de parole et Opus-MT (traduction de secours), environ 1,8 Go.
2. La progression affiche le fichier, les Mo reçus, le pourcentage et la vitesse. Une coupure n'est pas grave : le téléchargement reprend où il s'est arrêté.
3. Le fichier est vérifié par son **empreinte SHA-256** avant d'être utilisé. Un fichier partiel ou corrompu n'est jamais pris pour un modèle.
4. L'installateur charge le modèle une fois avec le moteur (`crosstalk-llm.exe`) pour vérifier qu'il fonctionne vraiment. En cas d'échec, l'installation se termine quand même et Crosstalk utilise Opus-MT.

Espace disque nécessaire : environ 4,5 Go pour les modèles. Une réinstallation ou une mise à jour de Crosstalk ne retélécharge pas un modèle déjà vérifié.

## Hors ligne

Une fois les modèles installés, la traduction fonctionne **sans connexion**. Le moteur ne s'adresse à aucun serveur : il parle à Crosstalk par un canal privé (ses entrées et sorties standard), sans port réseau.

Pour un PC sans Internet : installez Crosstalk sur un PC connecté, puis copiez le dossier `%LOCALAPPDATA%\Crosstalk\models\translategemma-4b-it` (le `.gguf` **et** `crosstalk-manifest.json`) au même endroit sur l'autre PC. Sans le manifeste, Crosstalk revérifie le fichier ; pour cela il a besoin d'une connexion (l'empreinte de référence vient de Hugging Face).

## Dans l'application : Audio & IA › Moteur de traduction

- **État** : non installé, téléchargement, vérification, installé, chargement, prêt, trop lent ou erreur.
- **Exécution** : processeur (nombre de threads) ou carte graphique (Vulkan), temps de chargement, mémoire utilisée.
- **Temps mesuré** : p50 / p95 des dernières phrases.
- **Télécharger / Reprendre** : récupère un modèle absent, incomplet ou supprimé.
- **Vérifier / réparer** : relit tout le fichier et compare son SHA-256. Un fichier corrompu est supprimé ; « Télécharger » le remplace.
- **Moteur utilisé** :
  - *Auto* (par défaut) : TranslateGemma s'il est installé et assez rapide, sinon Opus-MT ;
  - *TranslateGemma* : toujours, même lent (Opus-MT seulement s'il échoue) ;
  - *Opus-MT* : TranslateGemma n'est pas chargé.
- **Calcul** :
  - *Auto* : la carte graphique seulement s'il reste assez de mémoire vidéo pour CS2 et Whisper (taille du modèle + 1,5 Go), sinon le processeur ;
  - *Processeur* ou *Carte graphique* pour l'imposer.

## Ressources et CS2

- Le modèle est chargé **une fois** au démarrage de la traduction et gardé pendant la session (3 à 3,5 Go de mémoire).
- Il est libéré dès que la traduction est désactivée ou qu'Opus-MT est choisi.
- Le moteur tourne en **priorité basse** : CS2 et Whisper passent avant. Il utilise la moitié des cœurs logiques, entre 2 et 6.
- Les annonces connues du lexique (« rush B », « two on A »…) restent traduites instantanément, sans le modèle.
- Une phrase qui attend plus de 5 s est affichée non traduite plutôt qu'en retard.
- Si la médiane des dernières traductions dépasse 3,5 s, le mode automatique repasse sur Opus-MT.
- Une traduction qui dépasse 12 s redémarre le moteur.

## Licence

TranslateGemma est fourni par Google sous les [Gemma Terms of Use](https://ai.google.dev/gemma/terms) et sa [politique d'utilisation interdite](https://ai.google.dev/gemma/prohibited_use_policy). Crosstalk ne modifie pas le modèle. Il télécharge la conversion GGUF publiée par mradermacher ([mradermacher/translategemma-4b-it-GGUF](https://huggingface.co/mradermacher/translategemma-4b-it-GGUF)), à partir de [google/translategemma-4b-it](https://huggingface.co/google/translategemma-4b-it).
