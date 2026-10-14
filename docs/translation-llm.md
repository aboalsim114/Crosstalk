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

## Choisir un fournisseur

| Fournisseur | Type | Adresse de base | Clé |
| --- | --- | --- | --- |
| OpenAI | Compatible OpenAI | `https://api.openai.com/v1` | oui |
| OpenRouter (accès à de nombreux modèles) | Compatible OpenAI | `https://openrouter.ai/api/v1` | oui |
| Mistral | Compatible OpenAI | `https://api.mistral.ai/v1` | oui |
| Groq | Compatible OpenAI | `https://api.groq.com/openai/v1` | oui |
| Anthropic | Anthropic | `https://api.anthropic.com/v1` | oui |
| Ollama (local) | Compatible OpenAI | `http://localhost:11434/v1` | non |
| LM Studio (local) | Compatible OpenAI | `http://localhost:1234/v1` | non |

Conseils :

- **Prenez un petit modèle rapide.** Une phrase de jeu est courte ; un gros modèle ajoute surtout de la latence. Au-delà de 3,5 s, Opus-MT prend le relais.
- **Évitez les modèles de raisonnement** (séries « o » et GPT-5 d'OpenAI, par exemple). Ils sont lents pour cet usage, et certains refusent les paramètres `temperature` ou `max_tokens` envoyés en mode compatible OpenAI. L'erreur 400 qui en résulte fait passer chaque ligne par Opus-MT.
- La latence dépend de votre connexion et de la charge du fournisseur. Elle est affichée dans la carte (« dernière latence »).
- Les noms de modèles changent souvent : recopiez l'identifiant exact donné par votre fournisseur.
- Le coût dépend du fournisseur. Une phrase pèse quelques dizaines de jetons, plus la consigne.

## Exemples de configuration

Les noms de modèles ci-dessous sont des exemples ; vérifiez ceux que propose votre compte.

**OpenAI**
- Fournisseur : Compatible OpenAI
- Adresse : `https://api.openai.com/v1`
- Modèle : `gpt-4o-mini`
- Clé : votre clé `sk-…` (platform.openai.com › API keys)

**OpenRouter**
- Fournisseur : Compatible OpenAI
- Adresse : `https://openrouter.ai/api/v1`
- Modèle : l'identifiant affiché sur openrouter.ai/models, au format `fournisseur/modèle` (par exemple `openai/gpt-4o-mini`)
- Clé : votre clé `sk-or-…` (openrouter.ai › Keys)

**Anthropic**
- Fournisseur : Anthropic
- Adresse : `https://api.anthropic.com/v1`
- Modèle : un petit modèle rapide, `claude-haiku-5-5` (Haiku actuel) ou `claude-haiku-4-5`
- Clé : votre clé `sk-ant-…` (console.anthropic.com › API Keys)

Sur les modèles Claude récents, la réflexion (« thinking ») est activée par défaut. Elle peut allonger un peu la réponse : surveillez la dernière latence dans la carte.

**Ollama (sur votre PC, sans clé)**
1. Installez Ollama, puis téléchargez un modèle : `ollama pull qwen2.5:7b`.
2. Fournisseur : Compatible OpenAI
3. Adresse : `http://localhost:11434/v1`
4. Modèle : `qwen2.5:7b`
5. Clé : aucune

Un modèle local tourne sur votre carte graphique, en même temps que CS2 et Whisper. Vérifiez que la latence reste sous 3,5 s et que vos FPS ne chutent pas.

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
