# Matrice de test — statut réel des fonctionnalités

Environnement : Windows 11 Famille 26200 (i7-1165G7, sans GPU dédié), CS2 ouvert sur la machine (PID 25872), branche `fix/rea-full-stabilization`.

**Méthode « boîte noire »** :
- **Binaire testé** : une copie de test, build release MSVC avec l'identifiant `app.crosstalk.qa`. Ses dossiers de configuration, de journal, d'historique et d'état GPU sont isolés. Les modèles de l'utilisateur sont seulement lus.
- **Pilotage** : clics dans la vraie interface, via le protocole de débogage de WebView2 (CDP), et lecture des fenêtres par Win32.
- **Installation personnelle** : l'instance Crosstalk 0.2.0 de l'utilisateur, en cours d'exécution, n'a pas été touchée. En conséquence, le port GSI et les raccourcis globaux étaient déjà pris par elle.

Légende : ✅ vérifié en exécution réelle · 🧪 vérifié par test automatisé seulement · ⚠️ partiel · ❌ défaut ouvert · — non testé (raison donnée).

| Fonctionnalité | Comportement attendu | Test exécuté | Résultat | Bug / correction |
|---|---|---|---|---|
| Lancement | Fenêtre, intro, services prêts | 3 lancements chronométrés | ✅ interface en 2,2 à 3,6 s ; Whisper prêt en ~3 s | CT-003 (version « V0.2 ») corrigé |
| Premier lancement | Dialogue « Vos langues » | Clic « Entrer » sur un profil vierge | ✅ dialogue affiché, langue de Windows présélectionnée | — |
| Pages (7) | Aucune page vide, aucune erreur | Visite automatisée et captures, écoute de la console | ✅ 0 exception, 0 erreur de console, pas de défilement horizontal | — |
| Capture CS2 (process loopback) | Son de cs2.exe uniquement | Session démarrée, CS2 ouvert | ✅ « Son de cs2.exe uniquement (PID 25872) · 48 kHz », 1,3 % CPU au repos | — |
| Capture périphérique (repli) | Tout le son de la sortie | Session, corpus joué sur la sortie | ✅ | — |
| Test de capture 10 s | — | — | — non rejoué dans l'application (couvert par F2 / #67) | — |
| VAD + segmentation | Voix détectée, bruits rejetés | Banc `--noise-only 120` | ✅ 0 fausse transcription | — |
| Whisper base (CPU) | Transcription en / fr | Banc et application | ✅ 12/12 dans l'application après correction | CT-001, CT-008 corrigés |
| Whisper small | — | Banc | ✅ WER en 14,3 % (propre), RTF ≈ 0,9 | — |
| Whisper GPU / Auto | Inférence sur la carte graphique | — | ❌ absent de la version MSVC | CT-002 ouvert |
| Russe (voix) | — | — | — aucune voix russe disponible pour générer un corpus | — |
| Traduction ru/en → fr | Lignes traduites | Outil `translate` ; application (en→fr) | ✅ voir performance.md | CT-005 (faux « absents ») corrigé |
| Traduction provisoire | Texte rapide, remplacé ensuite | Outil `--greedy` | ⚠️ outil seulement | — |
| Glossaire (AWP, smoke) | Termes conservés | Application : « … with a AWP, I'm holding the smoke » | ✅ | — |
| Répondre (fr → en) | Traduit et copié | Carte Répondre, saisie et Entrée | ✅ 3,6 s, modèle chargé à la demande, presse-papiers rempli | — |
| Répondre (fr → ru / uk) | — | Outil `translate --to ru/uk` | ⚠️ outil seulement | — |
| Dictée | — | — | — demande un micro et une voix humaine | — |
| Chat écrit (console.log) | Lecture des messages | Statut « Chat suivi » pendant la session | ⚠️ suivi actif constaté ; aucun message injecté (on n'écrit pas dans le fichier de CS2) ; analyse couverte par les tests | — |
| Overlay | Fenêtre transparente, coin choisi | Coin « Bas gauche », redémarrage, position lue par Win32 | ✅ après correction | CT-004 corrigé |
| Mode streamer (OBS) | Page locale, SSE, pseudos et originaux retirés | Activation, page, Host étranger, SSE et ligne de test | ✅ 200 ; Host étranger → 403 ; ligne reçue sans pseudo ni original | CT-009 ouvert (le port reste ouvert après désactivation) |
| Arrêt d'urgence | Capture coupée aussitôt | Clic et sondage du statut | ✅ 35 ms | — |
| Historique | Désactivé par défaut | Lecture de l'interrupteur | ⚠️ désactivé par défaut constaté, écriture non testée | — |
| GSI (carte) | — | — | ⚠️ port 31982 occupé par l'instance de l'utilisateur ; tests automatisés seulement (dont le client lent, #76) | — |
| « Qui parle ? » | — | — | — expérimental, demande une vraie partie avec des joueurs | — |
| Récapitulatif de partie | — | — | — demande une fin de match | — |
| Raccourcis globaux | — | Diagnostic | ⚠️ « AVERT. » : déjà pris par l'instance de l'utilisateur, ce qui est attendu | — |
| Zone de notification, démarrage automatique | — | — | — non testé (créer ou supprimer une entrée de démarrage modifie le registre de l'utilisateur) | — |
| Mises à jour | Vérification du flux | Journal | ✅ « uptodate » (build local 199 ≥ 185 publié) | — |
| Diagnostic et journal | Tests intégrés, journal | Page Diagnostic | ✅ | — |
| À propos | Version, licences | Capture | ✅ après correction | CT-006 corrigé |
| Langues de l'interface (9) | — | Tests Vitest | 🧪 | — |
| Installateur NSIS/MSI (MSVC) | Construit | `validate.ps1` | 🧪 construit ; **non installé** (l'installation écrit dans HKCU et les raccourcis de l'utilisateur) | — |
| Installateur sur mesure | — | Tests `src-installer` | 🧪 | — |
| Dépendances natives | Démarre sur un PC sans redistribuable VC++ | `dumpbin /dependents` | ✅ plus de MSVCP140 ; ❌ non essayé sur un PC vierge | CT-007 corrigé |
| Anti-triche | Aucune API d'accès aux processus | `check:anticheat` et imports du binaire | ✅ | — |
