# Mes langues : traduire dans sa propre langue

![Premier lancement sur un Windows en russe : le russe est proposé comme langue de lecture](assets/languages-setup-ru.png)

Crosstalk traduit les voix et le chat de CS2 **dans la langue de l'utilisateur**, depuis les langues de ses coéquipiers. Au premier lancement, une fenêtre « Vos langues » demande :

- **Je lis les traductions en** : français, English, Русский, Deutsch, Español ou Українська. La langue de Windows est proposée d'office.
- **Mes coéquipiers parlent** : une ou plusieurs langues parmi Français, English, Русский, Deutsch, Español, Українська, Polski et Türkçe.

Rien n'est téléchargé avant ce choix. Tout se change ensuite dans **Langues & glossaire › Mes langues**.

![Langues & glossaire › Mes langues](assets/languages-page.png)

## Ce qui est traduit

| Ligne entendue ou lue | Résultat |
|---|---|
| Dans une langue de coéquipier choisie | Traduite dans votre langue |
| Dans votre propre langue | Affichée telle quelle, jamais traduite |
| Dans une autre langue | Affichée telle quelle, avec « langue non choisie » |

- Whisper ne reconnaît que les langues choisies et la vôtre : il confond moins les langues sur les annonces courtes.
- Le chat écrit reconnaît les mêmes huit langues, d'après l'alphabet, les lettres propres à chaque langue (ukrainien « і ї є ґ », polonais « ą ę ł », turc « ğ ı ş »…) et des mots fréquents. Un message trop court pour savoir reste tel quel.
- Les annonces CS du lexique (« rush B », « two on A »…) restent instantanées **quand vous lisez en français** : leur traduction est écrite en français. Dans les autres langues, le modèle les traduit.
- Les termes du glossaire marqués « Conserver » sont gardés dans toutes les langues. Les rendus imposés fournis avec Crosstalk sont en français : ils ne sont appliqués qu'en français. Ceux que vous ajoutez sont appliqués partout.

## Modèles

Chaque paire de langues utilise un modèle Helsinki-NLP Opus-MT (licence Apache-2.0, environ 300 Mo), qui tourne sur votre PC.

- 37 modèles sont référencés. Ils couvrent toutes les combinaisons entre français, anglais, russe, allemand, espagnol et ukrainien, ainsi que le polonais et le turc vers ces langues.
- Sans modèle direct, la traduction **passe par l'anglais** : deux modèles à la suite, par exemple allemand → anglais → russe. La carte « Mes langues » l'indique (« via l'anglais »). C'est un peu plus lent et un peu moins précis.
- Il n'existe pas de modèle **vers** le polonais ni le turc : ce sont des langues de coéquipiers seulement.
- Seuls les modèles des langues choisies sont téléchargés, ainsi que ceux de la fonction « Répondre ». La carte « Mes langues » indique la taille qui reste à télécharger et propose **Télécharger**.
- L'installateur installe à l'avance les modèles vers la langue de Windows, depuis le russe et l'anglais. L'application télécharge le reste après votre choix.

## Répondre dans leur langue

Vous écrivez ou dictez **dans votre langue**, et Crosstalk traduit vers la langue de vos coéquipiers. Les langues de réponse possibles sont celles de vos coéquipiers vers lesquelles un modèle existe, la vôtre exceptée.

Les phrases CS toutes faites (« Éco », « Drop-moi une arme »…) restent proposées en français uniquement, vers l'anglais ou le russe.

## Limites

- **Non testé sur un vrai PC Windows ni en partie.** Les nouveaux modèles n'ont pas pu être téléchargés depuis l'environnement de développement, qui n'a pas accès à Hugging Face. Ils ont le même format que les modèles ru → fr et en → fr déjà utilisés (même architecture Marian, vocabulaire partagé, aucun jeton de langue). Les tailles exactes de leurs fichiers viennent de Hugging Face.
- Le passage par l'anglais double le temps de traduction.
- Les écrans Audio & IA, Historique, Diagnostic et la fenêtre « Répondre » restent en français (traduction de l'interface en cours, voir [i18n.md](i18n.md)).
