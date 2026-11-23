# Corner Table sound sources

The macOS build embeds a 20-cue mix. Card draws and placements use paper foley,
Mahjong uses ceramic-like taps, and Mahjong Cubes and Block Puzzle use wood.
Minesweeper has separate cues for revealing, placing a flag, and removing it.
Bubble streaks use pitched pops, while gem clears use one, two, or three bells.
An invalid action has a short, low cue and completed boards have their own
stinger. The sidebar's Sound setting controls the whole mix.

## Bundled audio

The paths below are relative to the publishers' archives. The prepared WAV files
are in `apps/desktop/assets/sounds/`.

| Bundled cue | Publisher and archive entry |
| --- | --- |
| `card-place.wav`, `card-slide.wav`, `card-collect.wav` | Kenney Casino Audio 1.1, `Audio/card-place-1.ogg`, `Audio/card-slide-1.ogg`, `Audio/card-fan-1.ogg` respectively |
| `ceramic-tap.wav` | Kenney Impact Sounds 1.0, `Audio/impactPlate_light_000.ogg` |
| `ceramic-pair.wav` | Kenney Impact Sounds 1.0, `Audio/impactPlate_light_002.ogg` and `Audio/impactPlate_medium_003.ogg` |
| `wood-tap.wav` | Kenney Impact Sounds 1.0, `Audio/impactWood_light_000.ogg` |
| `wood-pair.wav` | Kenney Impact Sounds 1.0, `Audio/impactWood_medium_002.ogg` and `Audio/impactWood_light_003.ogg` |
| `reveal.wav`, `flag.wav`, `unflag.wav`, `bubble-land.wav` | Dustyroom Free Casual Game Sounds, `DM-CGS/WAV/DM-CGS-21.wav`, `DM-CGS/WAV/DM-CGS-46.wav`, `DM-CGS/WAV/DM-CGS-47.wav`, `DM-CGS/WAV/DM-CGS-32.wav` respectively |
| `bubble-pop.wav`, `bubble-chain.wav` | Dustyroom Free Casual Game Sounds, `DM-CGS/WAV/DM-CGS-31.wav`, single and layered with ascending pitches respectively |
| `move.wav`, `invalid-move.wav`, `clear.wav`, `complete.wav` | Corner Table's original synthesized cues from commit `2373a4e`, level matched for this mix |
| `gem-clear.wav`, `gem-chain.wav`, `gem-chain-long.wav` | Original Corner Table synthesized bell tones at C♯5, E5 and G♯5, layered in order according to chain length |

[Kenney Casino Audio](https://kenney.nl/assets/casino-audio) and
[Kenney Impact Sounds](https://kenney.nl/assets/impact-sounds) both include a
`License.txt` dedicating the assets under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
[Dustyroom Free Casual Game Sounds](https://dustyroom.com/free-casual-game-sounds/)
includes `DM-CGS/license.pdf`, which identifies the entire archive as CC0.
All three permit commercial use and credit is optional.

## Optional local audio

Place the prepared files listed below next to this README in
`apps/desktop/local-audio/` before building. Keep source archives and downloaded
audio outside the repository. Build with `cargo build --locked -p corner-table`
on macOS. The build embeds only the listed files and uses the corresponding
bundled cue whenever an override is absent. Changes to this folder trigger a
rebuild the next time Cargo runs. Git ignores the prepared WAVs.

| Local filename | Free edition and source WAV |
| --- | --- |
| `move.wav` | Willosuna Cozy UI SFX Lite v2, `wav/click_01.wav` |
| `invalid-move.wav` | Willosuna Cozy UI SFX Lite v2, `wav/error_01.wav` |
| `flag.wav`, `unflag.wav` | Willosuna Cozy UI SFX Lite v2, `wav/toggle_on_01.wav` and `wav/toggle_off_01.wav` |
| `bubble-pop.wav`, `bubble-chain.wav` | REEBOO Labs 27-sound free sample, `Casual_Puzzle_SFX/WAV/pops/bubble_pop_01.wav`, single and layered with ascending pitches respectively |
| `gem-clear.wav`, `gem-chain.wav`, `gem-chain-long.wav` | REEBOO Labs 27-sound free sample, `Casual_Puzzle_SFX/WAV/combos/combo_bell_01.wav` for one wave, adding `combo_bell_02.wav` at 105 ms for two and `combo_bell_03.wav` at 208 ms for three or more |

The included `Casual_Puzzle_SFX/LICENSE.txt` grants royalty-free use in
commercial and noncommercial games, apps and videos in unlimited projects with
optional credit. It allows edits and layering inside projects and forbids
redistribution of the sounds themselves as a library, asset pack or other
collection, or redistribution of edits as sound assets.
[REEBOO Labs free sample](https://reeboolabs.itch.io/casual-puzzle-sfx-free-sample)

Willosuna's included `LICENSE.txt` permits modification and royalty-free use in
unlimited personal and commercial projects, including games, apps, videos,
prototypes and client work. Credit is optional. It prohibits redistributing or
reselling the audio as a sound pack, library or collection, including modified
files, and bars Content ID registration of the sounds and derivative works.
[Willosuna Cozy UI SFX Lite v2](https://willosuna.itch.io/cozy-ui-sfx-lite)

All cues use 44.1 kHz mono 16-bit PCM WAV. Trimming, short fades,
active-window RMS matching and peak limiting keep the clips short and at a
similar level. Use the prepared clips to retain their timing and gain.
