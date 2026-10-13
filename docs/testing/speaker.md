# Protocole de test : « Qui parle ? » (expérimental)

Sur Windows 10 ou 11, avec CS2 en **plein écran fenêtré** et des coéquipiers qui parlent en vocal.

Cette fonctionnalité n'a pas encore été validée sur une vraie partie. Elle est développée et testée sous Linux (logique, compilation pour Windows) ; la capture d'écran et l'OCR Windows ne peuvent s'exécuter que sur un PC Windows.

## Fonctionnement

Quand un joueur parle, CS2 affiche son pseudo sur l'indicateur de voix, à gauche au-dessus du chat. Pendant que l'audio du jeu contient de la voix et que CS2 est au premier plan, Crosstalk lit **cette petite zone de l'écran** quatre fois par seconde. Il copie les pixels du bureau comme le ferait une capture d'écran, puis les lit avec l'OCR intégré à Windows (`Windows.Media.Ocr`).

Une ligne vocale reçoit un pseudo seulement si :
- un seul pseudo domine l'indicateur pendant qu'elle était prononcée (au moins 60 % des lectures, sans égalité) ;
- ce pseudo a été lu au moins deux fois, ou il est déjà connu par le chat écrit.

Dans tous les autres cas, aucun pseudo n'est affiché.

Garanties :
- aucun accès au processus ni à la mémoire de CS2 (`npm run check:anticheat`) ;
- aucune image écrite sur disque ;
- pseudos gardés en mémoire jusqu'à la fermeture ou la désactivation, jamais écrits dans les journaux ;
- désactivé par défaut.

## Étapes

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Audio & IA › carte « Qui parle ? » | Désactivé. Le journal ne contient aucune ligne `speaker`. |
| 2 | Activer | « En attente de CS2 » si CS2 est fermé ; « En pause » si CS2 est en arrière-plan. Le journal contient `identification des voix activée (OCR Windows xx-XX)`, sans aucun pseudo. |
| 3 | Lancer CS2 en plein écran fenêtré, rejoindre une partie | État « Actif ». |
| 4 | Cliquer **Tester la zone**, passer dans CS2 en moins de 4 s et maintenir la touche micro | En revenant : l'aperçu de la zone lue et « Lu : <votre pseudo> ». |
| 5 | Si le pseudo n'est pas dans l'aperçu | Ajuster gauche / haut / largeur / hauteur, puis refaire le test. Les réglages sont conservés. |
| 6 | Cliquer sur votre pseudo dans « Pseudos lus » | Il devient votre pseudo (le même que dans la carte Chat) et n'est plus attribué. |
| 7 | Un coéquipier parle seul | Script, overlay et historique affichent « <pseudo> — <phrase> ». |
| 8 | Deux coéquipiers parlent en même temps | Aucun pseudo plutôt qu'un pseudo faux. |
| 9 | CS2 en plein écran exclusif | État « Écran noir » avec le conseil de passer en plein écran fenêtré. |
| 10 | Placer l'overlay Crosstalk sur la zone lue | État « Zone recouverte » ; l'overlay est masqué de la lecture et n'est jamais lu comme un pseudo. |
| 11 | Pseudos en cyrillique | Lus seulement si la langue russe est installée dans Windows (Paramètres › Heure et langue › Langue). |
| 12 | Désactiver | Plus aucune lecture de l'écran ; les pseudos en mémoire sont oubliés. |
| 13 | Gestionnaire des tâches pendant une partie | Le coût CPU ne s'ajoute que pendant les phrases (OCR ≈ 4 fois par seconde sur une petite zone). |

## Ce qui reste à mesurer sur de vraies parties

- La position réelle de l'indicateur selon la résolution et le ratio d'écran. La zone par défaut est gauche 0 %, haut 50 %, largeur 25 %, hauteur 30 %.
- La précision : aucun pseudo erroné ne doit être affiché (cahier des charges, précision ≥ 98 %).
- Le taux de lignes sans pseudo.
