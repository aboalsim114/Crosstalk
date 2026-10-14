# « Focus voix » : mesures et protocole

## Principe (cahier des charges A-01)

Dans CS2, le chat vocal est joué **au centre** : le même son à gauche et à droite. Les tirs, pas et explosions sont **spatialisés** : plus forts d'un côté, légèrement décalés dans le temps de l'autre.

Pour chaque fréquence de chaque tranche de 21 ms, Crosstalk compare les spectres gauche et droit :
- ce qui est identique des deux côtés (la voix) est gardé ;
- ce qui diffère est atténué, jusqu'à −20 dB.

Les sons exactement au centre, comme ses propres tirs, ne peuvent pas être séparés de la voix et restent.

Implémentation : `src-tauri/src/speech/focus.rs`. Le traitement se fait avant la conversion en 16 kHz, avec une FFT de 1024 points et un recouvrement de 50 %. Il ajoute 10,7 ms de latence. Désactivé par défaut (Audio & IA › Détection de parole › **Focus voix**).

## Mesures (Linux, banc `focus_bench`)

```
cargo run --release --example focus_bench -- --voice corpus/jfk.wav --vad corpus/ggml-silero-v5.1.2.bin [--peak 1.0]
```

Conditions de mesure :
- **voix réelle** : `jfk.wav`, 11 s, répétée 3 fois et placée au centre ;
- **bruit** : combat **synthétique** du banc d'essai (tirs, rafales, explosions), venant de plusieurs directions avec des différences de niveau et de délai entre les oreilles ;
- **VAD** : le vrai Silero de l'application.

Le RSB (rapport signal/bruit) est mesuré sur les passages de voix, par rapport à la voix propre. Il compte donc aussi les déformations de la voix.

| Scénario (crête 1,0) | RSB sans focus | RSB avec focus | Gain | VAD sans → avec (segments sur la voix / faux) |
|---|---|---|---|---|
| Combat sur les côtés | 12,7 dB | 20,4 dB | **+7,7 dB** | 12 / 0 → 12 / 0 |
| Combat varié (côtés + presque devant) | 10,1 dB | 15,6 dB | **+5,6 dB** | 12 / 0 → 12 / 0 |
| Avec ses propres tirs (au centre) | 9,7 dB | 13,6 dB | **+3,9 dB** | 12 / 0 → 12 / 0 |

Avec une crête de 0,5, les gains sont de +5,8, +4,6 et +3,6 dB. **Coût : 0,8 % d'un cœur CPU.** Une voix seule au centre ressort inchangée : RSB supérieur à 50 dB (test `a_centred_voice_passes_unchanged`).

Historique de la mise au point, mesuré avec le même banc :
- un filtre adaptatif NLMS (référence : différence gauche/droite) **dégradait** la voix, avec un RSB passé de 18,7 à 1,1 dB ;
- avec une régularisation, il n'apportait que +1 à +2 dB ;
- d'où le choix du masque fréquentiel.

## Ce qui n'est pas encore mesuré

- **Le WER**, c'est-à-dire l'effet réel sur Whisper : aucun modèle Whisper n'est téléchargeable dans l'environnement de développement.
- **De vrais sons de CS2**, et l'hypothèse elle-même : que la voix soit bien au centre dans CS2. Si la voix était spatialisée, le focus l'atténuerait.

## Protocole sur Windows

| # | Action | Résultat attendu |
| --- | --- | --- |
| 1 | Audio & IA › Détection de parole | « Focus voix » désactivé, avec la mention « Désactivé : … mono ». |
| 2 | Activer, démarrer une session dans CS2 | « Activé · X dB de sons latéraux retirés… », qui augmente pendant les combats et reste proche de 0 quand seuls des coéquipiers parlent. |
| 3 | Comparer une partie avec et sans le focus (même modèle) | Moins de lignes rejetées et des transcriptions plus justes pendant les combats. Les rejets sont visibles sous le seuil de détection. |
| 4 | Un coéquipier parle sans combat | Aucune différence de transcription avec ou sans focus (la voix est au centre). |
| 5 | Capture d'un périphérique mono | « En attente d'un son stéréo » : aucun effet. |
