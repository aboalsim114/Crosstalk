# Traduction en ligne par un LLM (facultative)

Crosstalk traduit sur votre PC avec Opus-MT, sans clé ni réseau. En option, vous pouvez confier les phrases au LLM de votre choix, avec **votre propre fournisseur et votre propre clé API**. La fonction est **désactivée par défaut**. Opus-MT reste toujours là : il prend le relais à la moindre difficulté.

Réglage : **Audio & IA › Traduction en ligne · LLM**.

## Fonctionnement

Pour chaque ligne finale (voix transcrite ou message du chat) :

1. Les annonces Counter-Strike connues (« rush B », « eco »…) sont traduites par le lexique intégré, sans modèle. C'est le cas uniquement vers le français.
2. Sinon, si la traduction en ligne est active, le texte est envoyé au LLM choisi. Le délai maximal est de **3,5 s**.
3. Si le LLM échoue, la même ligne est traduite par **Opus-MT** sur le PC. C'est le cas si :
   - le réseau est absent ;
   - le quota est dépassé (429) ;
   - le crédit est épuisé (402) ;
   - la clé est refusée (401/403) ;
   - le modèle est introuvable (404) ;
   - le fournisseur renvoie une erreur ;
   - le délai est dépassé ;
   - la réponse est illisible ou incohérente.

Les **lignes en direct** (provisoires, pendant que le joueur parle) restent toujours traduites localement : elles changent plusieurs fois par seconde.

Une réponse est jugée **incohérente** si elle est vide, beaucoup plus longue ou beaucoup plus courte que la phrase, si elle contient des explications sur plusieurs lignes, ou si elle refuse ou commente (« I'm sorry », « en tant qu'IA »…). Les guillemets et préfixes du type « Traduction : » sont retirés.

Le glossaire s'applique aussi au LLM. Les termes protégés sont remplacés par des codes (`KZ1`, `KZ2`…) que le LLM doit recopier. S'il les abîme, la phrase est retraduite sans protection, comme avec Opus-MT.

Le test de traduction d'Audio & IA indique le moteur utilisé : **LLM en ligne**, **Opus-MT local** ou **lexique Counter-Strike**. La carte affiche aussi l'état (actif, ou le problème rencontré), le nombre d'appels et d'échecs, la dernière latence et la dernière erreur.

Les traductions sont gardées en cache. Le cache est vidé quand vous changez de fournisseur, d'adresse, de modèle ou de clé.

## Confidentialité

- **Seul le texte de la phrase est envoyé**, accompagné d'une consigne fixe : traduire de telle langue vers telle autre, sans commentaire.
- **Jamais l'audio.** La voix est transcrite sur votre PC par whisper.cpp ; seul le texte transcrit peut partir.
- **Jamais les pseudos.** Le nom de l'expéditeur d'un message de chat ne fait pas partie du texte envoyé à la traduction.
- Le fournisseur reçoit ces textes et les traite selon **ses propres conditions** (conservation, entraînement…). Lisez-les avant d'activer la fonction.
- Avec un **serveur local** (Ollama, LM Studio), rien ne quitte votre réseau.

### La clé API

- Elle est rangée dans le **Gestionnaire d'identifiants Windows**, sous l'identifiant générique `Crosstalk/llm-api-key`, lié à votre compte Windows.
- Elle n'est **jamais** écrite dans `config.json`, ni dans le journal, ni dans le statut envoyé à l'interface, ni dans un message d'erreur.
- Le champ de saisie est un champ mot de passe, vidé après l'enregistrement ; la clé n'est jamais réaffichée.
- **Effacer** la supprime du Gestionnaire d'identifiants. Vous pouvez aussi la retrouver ou la supprimer dans **Panneau de configuration › Gestionnaire d'identifiants › Informations d'identification Windows**.
- Les erreurs affichées viennent uniquement du code HTTP. Le corps des réponses d'erreur est ignoré, car certains fournisseurs y recopient une partie de la clé.
- Une adresse en `http://` n'est acceptée que pour un serveur local : une clé ne circule jamais en clair sur Internet.

## Mise en route (Audio & IA › Traduction en ligne)

Aucune connaissance technique n'est nécessaire : l'adresse et le modèle sont remplis pour vous.

1. **Choisissez un service** dans la liste.
2. **Créez votre clé** : le bouton « Obtenir ma clé … ↗ » ouvre la bonne page du service dans votre navigateur. Connectez-vous (ou créez un compte), créez une clé et copiez-la.
3. **Collez-la** dans le champ « Clé API », puis cliquez sur « Enregistrer » (ou appuyez sur Entrée).
4. Cliquez sur **« Tester la connexion »**. Une phrase d'exemple (« Двое идут на B, двое на B ») est traduite avec vos réglages. Vous voyez la traduction et le temps de réponse, ou l'erreur avec ce qu'il faut faire (clé refusée, crédit épuisé, quota atteint, modèle introuvable, réseau, délai dépassé…). Le test marche même avant d'activer l'interrupteur.
5. Activez **« Traduire avec un LLM en ligne »**.

Une seule clé est gardée. En changeant de service, collez la clé du nouveau service.

### Services proposés

| Service | Adresse de base (remplie automatiquement) | Modèle proposé | Clé |
| --- | --- | --- | --- |
| OpenAI | `https://api.openai.com/v1` | `gpt-4o-mini` | oui · platform.openai.com/api-keys |
| OpenRouter (de nombreux modèles) | `https://openrouter.ai/api/v1` | `openai/gpt-4o-mini` | oui · openrouter.ai/keys |
| Mistral AI | `https://api.mistral.ai/v1` | `mistral-small-latest` | oui · console.mistral.ai/api-keys |
| Groq | `https://api.groq.com/openai/v1` | `llama-3.1-8b-instant` | oui · console.groq.com/keys |
| Anthropic (Claude) | `https://api.anthropic.com/v1` | `claude-haiku-5-5` | oui · console.anthropic.com/settings/keys |
| Ollama Cloud | `https://ollama.com/v1` | `gemma4:31b` | oui · ollama.com/settings/keys |
| Ollama sur ce PC | `http://localhost:11434/v1` | un modèle installé (détecté) | non |
| LM Studio sur ce PC | `http://localhost:1234/v1` | le modèle chargé, à saisir | non |
| Autre | à saisir | à saisir | selon le service |

Les modèles proposés sont des suggestions : les catalogues changent souvent. « Réglages avancés » permet de saisir un autre modèle, une autre adresse ou le format d'API (compatible OpenAI ou Anthropic). Modifier l'adresse resélectionne le service correspondant, ou « Autre ».

Les pages de clé ouvertes par l'application viennent d'une liste fixe dans le code (`KEY_PAGES` dans `llm.rs`) : l'interface ne peut pas faire ouvrir une adresse arbitraire.

### Ollama Cloud (avec clé)

Ollama Cloud fait tourner des modèles ouverts sur les serveurs d'Ollama, sans carte graphique de votre côté. Crosstalk l'utilise par son API compatible OpenAI : `https://ollama.com/v1/chat/completions`, avec l'en-tête `Authorization: Bearer <clé>`. La clé se crée sur ollama.com/settings/keys. Les modèles disponibles sont listés sur ollama.com (onglet Cloud) ; recopiez le nom exact dans « Réglages avancés » si vous voulez en changer.

### Ollama sur ce PC (sans clé)

Quand vous choisissez « Ollama sur ce PC », Crosstalk interroge `http://localhost:11434/api/tags` (délai 1,5 s), c'est-à-dire seulement votre propre machine :

- **Ollama détecté avec des modèles** : la liste des modèles installés s'affiche, le premier est choisi. Rien d'autre à faire.
- **Ollama détecté sans modèle** : tapez dans PowerShell `ollama pull qwen2.5:7b`, puis « Vérifier à nouveau ».
- **Ollama absent** : le bouton « Télécharger Ollama ↗ » ouvre ollama.com/download. Installez-le, téléchargez un modèle, puis « Vérifier à nouveau ».

Un modèle local tourne sur votre carte graphique, en même temps que CS2 et Whisper. Vérifiez avec « Tester la connexion » que la latence reste sous 3,5 s, et que vos FPS ne chutent pas.

### Conseils

- **Prenez un petit modèle rapide.** Une phrase de jeu est courte ; un gros modèle ajoute surtout de la latence. Au-delà de 3,5 s, Opus-MT prend le relais.
- **Évitez les modèles de raisonnement** (séries « o » et GPT-5 d'OpenAI, par exemple). Ils sont lents pour cet usage, et certains refusent les paramètres `temperature` ou `max_tokens` envoyés en mode compatible OpenAI. L'erreur 400 qui en résulte fait passer chaque ligne par Opus-MT.
- Sur les modèles Claude récents, la réflexion (« thinking ») est activée par défaut. Elle peut allonger un peu la réponse : surveillez la latence.
- Le coût dépend du service. Une phrase pèse quelques dizaines de jetons, plus la consigne. OpenAI et Anthropic demandent un moyen de paiement ; d'autres services proposent un quota gratuit limité (erreur 429 une fois épuisé).

## Requêtes envoyées

Compatible OpenAI : `POST {adresse}/chat/completions`
- en-tête `Authorization: Bearer <clé>` (absent sans clé) ;
- corps : `model`, `temperature: 0`, `max_tokens`, `messages` = consigne système + phrase.

Anthropic : `POST {adresse}/messages`
- en-têtes `x-api-key: <clé>` et `anthropic-version: 2023-06-01` ;
- corps : `model`, `max_tokens: 1024`, `system` = consigne, `messages` = phrase ;
- pas de `temperature` : les modèles Claude récents refusent une valeur différente de celle par défaut (erreur 400) ;
- `max_tokens` laisse la place à la réflexion, activée par défaut et comptée dans cette limite ;
- une réponse `stop_reason: "refusal"` est traitée comme un échec : Opus-MT prend le relais.

Le code est dans `src-tauri/src/translation/llm.rs`. Les tests utilisent un transport simulé : aucun appel réseau, aucune vraie clé.

## Ce qui reste local

- Capture audio et détection de parole (Silero VAD).
- Transcription (whisper.cpp).
- Lignes en direct.
- Lexique Counter-Strike.
- Secours Opus-MT.
- Réponse dans la langue des coéquipiers (Ctrl+Alt+F6 / F7) : elle utilise toujours Opus-MT et le lexique.
