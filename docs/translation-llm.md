# Traduction IA : un LLM, en ligne ou sur votre PC

Crosstalk traduit **uniquement avec un LLM** : la voix des coéquipiers, le chat écrit et vos réponses (« Réponds dans leur langue », écrite ou dictée). Vous choisissez ce LLM :

- **Cloud** : un fournisseur en ligne, avec **votre propre clé API** ;
- **Local** : **Ollama** ou **LM Studio**, déjà installés sur votre PC, sans clé et hors ligne.

Il n'y a plus de modèle de traduction local intégré (Opus-MT a été retiré) : aucun modèle de traduction n'est téléchargé, ni par l'installateur, ni par l'application, et Crosstalk ne télécharge jamais de LLM. **Tant qu'aucun moteur n'est connecté, les phrases restent dans leur langue d'origine**, avec la mention « aucun moteur de traduction connecté » ; la transcription, l'overlay, le radar et l'historique continuent de fonctionner.

Réglage : **Audio & IA › TRADUCTION IA**. Au premier lancement, Crosstalk propose : **API Cloud**, **Ollama local**, **LM Studio local** ou **Configurer plus tard**.

## Cloud — Fournisseur API

1. Choisissez le **fournisseur**. L'adresse et un modèle conseillé sont remplis pour vous.
2. **Clé API** : « Obtenir ma clé … ↗ » ouvre la page du fournisseur. Collez la clé, puis « Enregistrer ».
3. **Modèle** : « Lister les modèles » interroge le fournisseur (`GET …/models`). Choisissez-en un, ou saisissez son nom.
4. Cochez **l'accord d'envoi** : le texte des phrases part chez ce fournisseur, qui le traite selon ses propres conditions. Sans cet accord, « Connecter » est refusé.
5. **Tester la connexion** (phrase d'exemple) et **Tester une traduction** (votre phrase, par exemple « Mon équipe ne sait pas jouer. » du français vers le russe).
6. **Connecter**. « Déconnecter » arrête aussitôt les envois. Décocher l'accord déconnecte aussi.

| Fournisseur | Adresse de base | Modèle proposé | Page de la clé |
| --- | --- | --- | --- |
| OpenAI | `https://api.openai.com/v1` | `gpt-4o-mini` | platform.openai.com/api-keys |
| OpenRouter | `https://openrouter.ai/api/v1` | `openai/gpt-4o-mini` | openrouter.ai/keys |
| Mistral AI | `https://api.mistral.ai/v1` | `mistral-small-latest` | console.mistral.ai/api-keys |
| Groq | `https://api.groq.com/openai/v1` | `llama-3.1-8b-instant` | console.groq.com/keys |
| Anthropic (Claude) | `https://api.anthropic.com/v1` | `claude-haiku-5-5` | console.anthropic.com/settings/keys |
| Ollama Cloud | `https://ollama.com/v1` | `gemma4:31b` | ollama.com/settings/keys |
| Autre compatible OpenAI | à saisir (`https://` obligatoire) | à saisir | selon le service |

Les modèles proposés sont des suggestions : les catalogues changent. Une seule abstraction couvre tous les services compatibles OpenAI ; Anthropic a la sienne.

### La clé API

- Rangée dans le **Gestionnaire d'identifiants Windows** (identifiant générique `Crosstalk/llm-api-key`), liée à votre compte Windows.
- **Jamais** écrite dans `config.json`, le journal, le statut envoyé à l'interface, un message d'erreur ou un test.
- Champ mot de passe, vidé après l'enregistrement, jamais réaffiché. **Effacer** la supprime du Gestionnaire d'identifiants.
- Les erreurs affichées ne viennent que du code HTTP : le corps des réponses d'erreur est ignoré (certains fournisseurs y recopient une partie de la clé).
- Une adresse `http://` n'est acceptée que pour ce PC ou le réseau local ; les redirections sont refusées.

## Local — LLM installé sur ce PC

Crosstalk interroge seulement votre machine (délai 1,5 s) :

- **Ollama** : `http://localhost:11434/api/tags`, puis l'API `http://localhost:11434/v1` ;
- **LM Studio** : `http://localhost:1234/v1/models` (les modèles d'« embedding » sont ignorés) ;
- **Adresse manuelle (avancé)** : un autre serveur compatible OpenAI sur ce PC ou le réseau local (`http://192.168.…`).

| État détecté | Ce que montre la carte |
| --- | --- |
| Serveur démarré avec des modèles | « Détecté · N modèles » ; liste des modèles réels, le premier est proposé |
| Serveur démarré sans modèle | « Démarré, aucun modèle disponible » et la commande à taper (`ollama pull qwen2.5:7b`) ou « chargez un modèle dans LM Studio » |
| Serveur absent | « Non détecté sur ce PC », bouton « Télécharger Ollama ↗ » / « Télécharger LM Studio ↗ » (page officielle) |

Aucune clé n'est demandée, aucun accord d'envoi : rien ne quitte le PC (ou le réseau local). « Actualiser les modèles locaux » relance la détection. Après la connexion, une requête de préchauffage charge le modèle en mémoire pour que la première vraie phrase ne paie pas ce délai.

**TranslateGemma** (via Ollama ou LM Studio) est reconnu par son nom : il reçoit son format d'invite officiel (un seul message utilisateur, sans message système).

Un modèle local tourne sur votre carte graphique, en même temps que CS2 et la reconnaissance vocale (sur le processeur) : vérifiez la latence avec « Tester une traduction » et surveillez vos FPS.

## Ce que voit la carte

Fournisseur, modèle, type (Cloud ou Local), serveur local (Ollama, LM Studio), état de la connexion, langues traduites (par exemple `EN, RU → FR`), nombre de traductions, appels et échecs, dernière latence, dernière erreur, et la pause en cours après une erreur de quota. L'usage facturé n'est pas affiché : les API ne le renvoient pas de façon uniforme.

## Un seul service pour tout

Toutes les fonctions passent par le même service (`src-tauri/src/translation/service`) et le même client LLM (`src-tauri/src/translation/llm`) :

`WASAPI → Silero VAD → Parakeet (sherpa-onnx) → LLM → Overlay`, ainsi que le chat, « Réponds dans leur langue » (écrite et dictée), les traductions provisoires et finales, l'historique, le mode streamer, le radar, le résumé de partie, le test de traduction et le diagnostic.

Pour chaque ligne finale :

1. Les annonces Counter-Strike du lexique intégré (« rush B », « eco »…) sont traduites sans appel, quand vous lisez en français. Les phrases CS toutes prêtes de « Répondre » aussi (français → anglais ou russe).
2. Sinon la ligne part au LLM connecté, après le cache (1 000 phrases, vidé à chaque changement de fournisseur, d'adresse, de modèle ou de clé).
3. Si la réponse n'arrive pas à temps ou échoue, **la ligne reste dans sa langue** avec la raison affichée. Il n'y a pas d'autre moteur derrière.

### Temps réel

- **File bornée** (8 lignes) traitée sur un thread dédié : ni le thread audio, ni le thread principal de Tauri, ni React n'attendent le réseau. Les commandes lentes (tests, liste des modèles, détection locale) sont asynchrones.
- **Les finales passent avant les provisoires** ; un provisoire remplace le précédent dans la file.
- **Purge** : à l'arrêt ou au changement de réglages, les lignes en attente deviennent obsolètes et ne sont pas envoyées.
- **Délai maximal** d'une ligne : 8 s d'attente au total ; délai de requête 6 s pour le cloud, 15 s en local (premier chargement du modèle). Au-delà : « délai dépassé : la ligne reste dans sa langue ».
- **Provisoires** : jamais envoyés à un fournisseur cloud (ils changent plusieurs fois par seconde et coûteraient autant d'appels payants) ; en local, au plus un toutes les 1,5 s. La ligne finale est toujours traduite.
- **Quota / limite de débit (429)** : pause de 20 s sans aucun appel, affichée sur la carte.
- **Clé refusée ou crédit épuisé (401, 402, 403)** : plus aucun appel jusqu'au changement de clé ou de réglages (un test réussi lève aussi le blocage).
- **Serveur local arrêté** pendant une partie : les lignes restent en langue d'origine avec « réseau indisponible » ; dès que le serveur revient, la ligne suivante est traduite, sans rien relancer.
- Pas de bascule automatique vers un autre moteur. Un LLM de secours n'existe pas encore : il ne serait ajouté que configuré et autorisé explicitement par l'utilisateur.

### Consigne envoyée (prompt CS2)

Traduction fidèle et naturelle, sans invention ni omission, sans explication, commentaire, guillemets ni variante ; garder les négations, les nombres, les positions (A, B, mid, banana, apps…), les armes et objets (AWP, AK, smoke, molotov…), le vocabulaire CS2 (rush, rotate, eco, plant…) et les codes `KZ1`, `KZ2` du glossaire ; traduire les insultes telles quelles. Le texte de la ligne est présenté comme du texte à traduire, jamais comme des instructions. La réponse est vérifiée : vide, beaucoup trop longue ou trop courte, sur plusieurs lignes, refus ou commentaire (« I'm sorry », « en tant qu'IA »…) → rejetée, la ligne reste dans sa langue. Les guillemets et préfixes du type « Traduction : » sont retirés.

### Requêtes

Compatible OpenAI : `POST {adresse}/chat/completions`, en-tête `Authorization: Bearer <clé>` (absent sans clé), corps `model`, `temperature: 0`, `stream: false`, `max_tokens`, `messages` = consigne système + phrase.

Anthropic : `POST {adresse}/messages`, en-têtes `x-api-key` et `anthropic-version: 2023-06-01`, corps `model`, `max_tokens: 1024`, `system`, `messages` ; pas de `temperature` (refusée par les modèles Claude récents) ; `stop_reason: "refusal"` est un échec.

Le streaming n'est pas utilisé : une phrase de jeu tient en quelques jetons, la réponse complète arrive en une fois et le streaming n'apporterait qu'un affichage mot à mot d'une ligne déjà courte.

## Confidentialité

- **Seul le texte de la ligne est envoyé**, avec la consigne fixe. **Jamais l'audio** (transcrit sur le PC par Parakeet via sherpa-onnx), **jamais les pseudos** des joueurs.
- En **Local**, les transcriptions ne vont qu'au serveur choisi sur ce PC ou le réseau local.
- En **Cloud**, l'accord d'envoi est demandé avant la première connexion. Lisez les conditions du fournisseur (conservation, entraînement…).

## Anciens modèles Opus-MT

Les dossiers `opus-mt-*` laissés par les versions précédentes dans `%LOCALAPPDATA%\Crosstalk\models` ne sont **pas** supprimés automatiquement. La carte « Traduction des parties » (Audio & IA) indique leur nombre et leur taille et propose **« Libérer l'espace »** ; seuls ces dossiers sont supprimés, et seulement sur ce clic.

## Code et tests

- `src-tauri/src/translation/llm/` : `config` (fournisseur, adresse, type), `keys` (Gestionnaire d'identifiants), `transport` (HTTP, sans redirection), `prompt` (consignes, TranslateGemma, liste des modèles), `validate`, `local` (Ollama, LM Studio), `client` (appels, pauses, statistiques, tests).
- `src-tauri/src/translation/service/` : file, cache, glossaire, priorités, délais, purge.
- Les tests utilisent un transport et des serveurs simulés : aucun appel réseau, aucune vraie clé. Voir [testing/translation.md](testing/translation.md) et le [rapport de migration](reports/llm-only-translation-migration.md).
