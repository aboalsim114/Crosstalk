# Traduction : moteur et test manuel (#10)

Le panneau **Audio → Moteur de traduction · DeepL** teste un texte anglais ou
russe vers le français. Cette étape ne raccorde pas les transcriptions vocales
au moteur : la publication dans la session et l’overlay relève de #11.

## Configuration Windows

1. Choisir le type de compte **DeepL API Free** ou **API Pro**.
2. Saisir la clé directement dans le champ masqué de l’application, puis
   l’enregistrer. Ne pas la mettre dans Git, un rapport, une capture ou le chat.
3. Activer explicitement les tests DeepL. L’enregistrement d’une clé n’active
   pas les requêtes et l’activation est remise à zéro au démarrage.
4. Choisir la langue et tester une phrase non sensible. Le texte est envoyé à
   DeepL uniquement après activation et lancement du test.

Les clés Free et Pro sont séparées dans le Gestionnaire d’identification
Windows. L’interface ne peut pas relire la clé. Supprimer la clé ou changer de
compte désactive le moteur et invalide les résultats en cours.

## Contrat vérifiable

- Les termes du glossaire sont protégés par des balises XML : « Conserver »
  restaure leur orthographe et « Traduire » impose le rendu configuré.
- Le cache conserve en mémoire au plus 1 000 traductions réussies ; une
  modification de configuration ou de clé l’invalide.
- Le budget total de cinq secondes inclut l’attente d’un emplacement et une
  éventuelle seconde tentative. Les erreurs d’authentification et de quota
  n’entraînent pas de nouvelle tentative.
- Un échec affiche **ORIGINAL**, avec une raison générique. Il ne présente pas
  le texte original comme une traduction française.
- Un texte déclaré français reste local, même sans clé et sans activation.
- Désactiver pendant une requête empêche son résultat tardif d’être affiché.

## Vérification locale sans compte

Les tests Rust utilisent un serveur HTTP local simulé et une entrée temporaire
isolée du Gestionnaire d’identification pour le test de stockage Windows.
Ils ne nécessitent pas de compte DeepL ni de clé réelle. Les tests du panneau
utilisent des réponses simulées et une clé fictive.

```powershell
npm run typecheck
npx vitest run --pool=threads --maxWorkers=1
npm run build
cd src-tauri
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Le pool `threads` est une alternative pour le contrôle local : sur le PC de
validation, le pool habituel de processus échoue à démarrer ses workers.
La configuration des tests du dépôt n’a pas été modifiée pour contourner cela.

## Validation réelle à effectuer avec un compte

Tester une phrase russe et une phrase anglaise, une expression du glossaire,
une répétition servie par le cache et la désactivation pendant une requête.
Vérifier le retour à l’original avec une clé invalide et vérifier que le
français ne consomme aucune requête. Ne publier que des durées et des compteurs,
pas la clé ni des propos de joueurs.

L’objectif de latence p50 ≤ 500 ms nécessite des mesures avec le service réel.
Les tests simulés ne démontrent pas cet objectif. Azure et la traduction locale
ne sont pas implémentés dans cette étape.
