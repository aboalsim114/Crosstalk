# Proposition de projet

_Started 2026-10-11 14:20 UTC_

---

## User

Continuing from "Rust desktop app design".

## Assistant

We were building a Rust desktop app to translate CS2 (Counter-Strike 2) game audio and chat in real time, with cinematic UI design. The project started with a design tool mockup, then evolved to include a 3D object and premium art direction after you requested motion design and cinematic experience.

We made several key decisions along the way:
- Chose to capture system audio at the OS level (WASAPI on Windows) rather than trying to hook directly into CS2, following Unitra AI's approach.
- Prioritized a voice-separation pipeline to isolate player speech from in-game noise (tirs, explosions) using DeepFilterNet, mid/side filtering, and Silero VAD.
- Deferred the cinematic 3D design to post-1.0, keeping the first release focused on core functionality: text chat translation and speech recognition with transcription.
- Split the work into an MVP (chat + basic overlay, ~1–2 weeks) and iterative versions adding voice recognition (~1–2 months) and design.

The current state: We have a complete **Cahier des charges.dc.html** (specifications document) covering 12 sections—context, functional/non-functional requirements, a 7-step audio cleaning pipeline, Rust architecture with crates (wasapi, cpal, whisper-rs, deep_filter, silero-vad), art direction, testing, and a revised 16-week timeline for v1.0. The cinematic mockup file **CS2 Live Translator v2 Cinematic.dc.html** remains as a design reference but is not part of the development spec yet.

Open next steps: Build a detailed module breakdown for the Rust codebase (audio capture, speech recognition, translation API calls, overlay rendering, settings panel); lock the API choice (DeepL vs. Azure for translation); confirm Windows version support; and begin the MVP phase with text chat translation.

Files involved: **Cahier des charges.dc.html**, **CS2 Live Translator v2 Cinematic.dc.html**, **doc-page.js**.

## User

propose un pour le projet

## Assistant



## User

propose un nom en anglais  pour le projet

## Assistant

Voici quelques noms en anglais :

1. **Crosstalk** : joue sur « cross » (Counter-Strike) et « crosstalk », les voix qui se superposent. Court et facile à retenir.
2. **Callout** : le terme qu'on utilise en CS pour annoncer une info à son équipe. Très parlant pour les joueurs.
3. **Comms** : l'argot des joueurs pour les communications d'équipe. Simple et direct.
4. **Lingo** : ton décontracté, évoque la langue et le jargon.
5. **Relay** : on relaie la voix d'une langue à une autre. Sobre et assez technique.
6. **Babelfire** : mélange Babel (traduction) et fire (le jeu de tir). Le plus cinématique de la liste.

Ma recommandation est **Callout**. C'est du vocabulaire natif de CS2, le nom dit tout de suite à quoi sert l'app, et il marche bien comme nom de binaire Rust (`callout.exe`, crate `callout`).

Je peux renommer le projet et mettre à jour le cahier des charges et la maquette avec ce nom.

