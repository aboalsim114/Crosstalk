# Protocole de test : chat écrit de CS2 (console.log)

À exécuter sur un PC **Windows 10/11** avec Steam et Counter-Strike 2. Crosstalk ne fait que **lire** des fichiers : les options de lancement Steam (pour vérifier `-condebug`) et `console.log`. Rien n'est modifié, injecté ni intercepté.

> Les pseudos et les messages des joueurs sont des données personnelles : ne collez pas de lignes de chat dans le dépôt ni dans la PR, seulement des compteurs. Exception : si le format ne correspond pas aux règles (§3), une ligne **anonymisée** (pseudo remplacé par `Joueur`) suffit pour corriger la règle.

## 1. Détection (sans lancer CS2)

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1.1 | **Langues & glossaire**, carte « Chat écrit · console.log », sans `-condebug` dans les options de CS2 | « -condebug absent », le chemin de `console.log` dans la bonne bibliothèque Steam, et la consigne : Steam › CS2 › Propriétés › Options de lancement. |
| 1.2 | Ajouter `-condebug` dans Steam, puis **Rechercher** | « En attente de console.log » (le fichier est créé au lancement de CS2) ou « Prêt ». **Diagnostic** : `-condebug` OK. |
| 1.3 | CS2 installé dans une seconde bibliothèque (ex. `D:\SteamLibrary`) | Le chemin pointe vers cette bibliothèque. |
| 1.4 | Saisir un faux dossier, puis le vrai dossier de CS2 dans « Dossier de CS2 » | Faux : « En attente de console.log » avec le chemin saisi. Vrai : le bon `console.log`. **Diagnostic** indique « (chemin manuel) ». Vider le champ revient à la détection automatique. |

## 2. Partie réelle

| # | Action | Résultat attendu |
| --- | --- | --- |
| 2.1 | Lancer CS2 (avec `-condebug`), **Accueil → Démarrer**, rejoindre une partie | Carte : « Chat suivi ». Les messages déjà présents dans `console.log` avant le démarrage ne sont **pas** affichés. |
| 2.2 | Un coéquipier écrit en russe (ou chat `say` en russe depuis un second compte) | Ligne **CHAT · ÉQUIPE CT/T** (ou TOUS) avec le pseudo, d'abord « traduction en cours », puis en français avec l'original dessous. Overlay : « CHAT • … → FRANÇAIS ». |
| 2.3 | Message en anglais (« they are going B ») | Traduit en français. |
| 2.4 | Message en français | Affiché tel quel, « Déjà en français — non traduit ». |
| 2.5 | « gg », « ez », « 1v3 » | Affichés tels quels avec « Non traduit — langue non reconnue » (aucune traduction inventée). |
| 2.6 | Saisir votre pseudo, « Ignorer mes messages » activé, puis écrire dans le chat | Votre ligne n'apparaît pas ; le compteur « des vôtres ignorés » augmente. Désactivez l'option : vos lignes apparaissent. |
| 2.7 | Le même message envoyé deux fois en moins de 2 s | Une seule ligne ; compteur « doublon ». |
| 2.8 | Joueur mort qui écrit | Pseudo suivi de « (mort) ». |
| 2.9 | Mode « Voix » du Script | Les lignes de chat ne s'affichent pas ; mode « Texte » ou « Mixte » : elles s'affichent. |
| 2.10 | **Arrêter**, écrire dans le chat, **Démarrer** | Rien n'est affiché pour les messages écrits pendant l'arrêt. |
| 2.11 | Quitter puis relancer CS2 pendant la session | Le suivi reprend dans le nouveau `console.log` (fichier recréé au lancement). |

## 3. Si aucune ligne n'apparaît

1. Ouvrez `console.log` (chemin affiché dans la carte) pendant la partie et cherchez une ligne de chat.
2. Les règles intégrées attendent `[ALL]`, `[CT]` ou `[T]`, le pseudo, un éventuel `﹫Lieu` ou `[DEAD]`, puis `: message`, avec éventuellement un horodatage devant.
3. Si le format diffère, copiez `src-tauri/src/chat/rules.json` dans `%APPDATA%\Crosstalk\chat-rules.json`, adaptez l'expression régulière, puis **Rechercher** : la carte affiche la version des règles utilisée (ou l'erreur). Remontez la ligne anonymisée pour corriger les règles intégrées.

## 4. Compte rendu

```
Commit : …   Windows : …   Steam : bibliothèques … · CS2 dans …
1.1 – 1.4 : ✅/❌ + remarques
2.1 – 2.11 : ✅/❌ + remarques (sans le texte des joueurs)
Règles : intégrées / fichier utilisateur · lignes reconnues : … / …
```

## Jeu en français (ou en russe)

| # | Action | Résultat attendu |
| --- | --- | --- |
| F1 | CS2 en français, partie avec chat d'équipe et général | Les lignes `[AT] Nom @Lieu : message`, `[GÉNÉRAL] Nom : message` et `Nom [MORT(E)] : message` apparaissent dans le Script, traduites (russe/anglais → français). |
| F2 | Diagnostic › Parseur de chat | Règles `cs2-2026.10b`. Si une ligne de chat n'est pas reconnue : avertissement et, dans le journal, sa **forme** seulement (`[TAG] x @x : x`), jamais le texte ni le pseudo. |
