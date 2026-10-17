# Smart Comms et multilingue : rapport

Suite de [smart-comms-feasibility.md](smart-comms-feasibility.md) (analyse et plan approuvés).

## Ce qui est livré

| Objectif | Livré | Limite |
| --- | --- | --- |
| 1. Détection automatique | La langue parlée est détectée parmi les 8 langues du modèle (en, fr, ru, de, es, pt, uk, pl) ; plus de choix des langues sources. Confiance < 0,7 → ligne « langue incertaine » (`auto`). | Parakeet ne reconnaît ni l'arabe, ni le chinois, ni le japonais, ni le turc à la voix (vocabulaire sans ces alphabets) : moteur inchangé, aucun remplacement sans accord. |
| 2. Traduction multilingue | 12 langues cibles (dont pt, ar, zh, ja) ; ar/zh/ja/tr traduits depuis le chat écrit. Arabe affiché de droite à gauche, polices CJK/arabes. | Seules les langues réellement prises en charge sont annoncées. |
| 3. Même langue masquée | Option « Masquer les phrases déjà dans ma langue », activée par défaut, désactivable (Langues › Mes langues). Masque overlay, Script, sous-titre, stream ; l'historique, le récap et le radar gardent la ligne. | Une langue incertaine n'est jamais masquée ; si le LLM la rend inchangée, elle reste affichée avec une mention. |
| 4. Smart Comms | `translation::guard` : nombre de joueurs (2 à 10, chiffres et mots dans 12 langues), sites A/B, gauche/droite. Fait changé → nouvel essai avec une consigne plus ferme → sinon ligne d'origine + « traduction écartée : … ». Ligne provisoire fautive non affichée. Directions ajoutées au prompt. | « un/one » (aussi article), nombres > 10 et un « A » en début de phrase ne sont pas vérifiés : dans le doute, la traduction passe. |
| 5. Latence | Instrumentation par étape (fin de phrase, attente, calcul, affichage) dans le journal 30 s et l'écran Audio & IA ; banc `latency_bench`. Correctif : plus de ligne provisoire calculée une fois la voix en pause. | Mesures faites sur le conteneur de développement (Linux, 4 cœurs), pas sur Windows avec CS2. |

L'intégration Ollama Cloud et la chaîne audio ne changent pas : la traduction reste dans le même client LLM (seul un second essai plus strict s'ajoute), la capture et la VAD sont intactes. Ollama Cloud ne peut pas retarder l'apparition du texte : la transcription s'affiche avant la traduction.

## Latence : avant / après

Banc `latency_bench` : 60 clips du corpus joués en temps réel dans le vrai `SpeechService`, 4 fils de reconnaissance, pause de 1,2 s entre clips. « Charge N » : N fils de calcul occupés pendant l'essai (pour simuler un jeu). Les deux binaires ne diffèrent que par le correctif ; exécutés l'un après l'autre, sans autre activité. Valeurs p50 / p95 en ms.

| Charge | Version | Fin de phrase | Attente | Calcul | **Total** |
| --- | --- | --- | --- | --- | --- |
| 0 | avant | 515 / 597 | 0 / 11 | 184 / 261 | **704 / 802** |
| 0 | après | 509 / 599 | 0 / 0 | 169 / 230 | **687 / 795** |
| 4 | avant | 517 / 611 | 0 / 353 | 529 / 799 | **1070 / 1579** |
| 4 | après | 520 / 611 | 0 / 5 | 520 / 704 | **1065 / 1187** |
| 8 | avant | 529 / 635 | 0 / 768 | 864 / 1079 | **1417 / 2198** |
| 8 | après | 521 / 620 | 0 / 188 | 870 / 1116 | **1391 / 1950** |

50 lignes finales sur 60 clips dans chaque essai (les 10 autres : clips trop courts ou bruités écartés, identique avant/après), aucune ligne abandonnée (trop tard / file pleine).

Lecture :
- Le retard intermittent venait de **l'attente** : la phrase finie patientait derrière une ligne provisoire encore en calcul. Le correctif fait tomber l'attente p95 de 353 à 5 ms (charge 4) et de 768 à 188 ms (charge 8) ; le total p95 baisse de 25 % (charge 4) et 11 % (charge 8). Le p50 ne change pas.
- Restent : un plancher fixe d'environ 0,5–0,6 s (silence qui clôt la phrase + pas de la VAD), et le calcul qui ralentit quand le processeur est partagé. Les réduire demande de changer le découpage ou le moteur : pas fait sans mesure sur CS2 et sans accord.
- Lignes provisoires : 15 → 5 à charge nulle (celles calculées pendant les pauses ont disparu).

## Tests automatisés

- Rust : 302 tests (dont `translation::guard` avec un corpus d'appels fidèles / altérés, le second essai et l'abandon dans le service, la langue incertaine), clippy et fmt.
- Interface : 259 tests Vitest (masquage sûr / incertain / option désactivée, overlay sans doublon, traduction écartée, détection), `tsc`.
- `check:anticheat`, `check:architecture`, `check:release-notes`.

## Non fait ici

**Aucun essai avec du vrai son de CS2 sur Windows** : l'environnement de développement n'a ni Windows ni CS2. Le protocole est dans [docs/testing/speech-recognition.md](../testing/speech-recognition.md), section « 2 ter » (latence par étape dans le journal, langues, masquage, appels avec nombres/sites/côtés).
