# Flappy assets

Bird frames and pipe from [samuelcust/flappy-bird-assets](https://github.com/samuelcust/flappy-bird-assets).

- `bird-sheet.png` — horizontal atlas (up, mid, down) used by `Animation` / `SpriteSheet`
- `bird-up.png` / `bird-mid.png` / `bird-down.png` — source frames (kept for editing)
- `pipe.png` — green pipe
- `bgm.ogg` — looping BGM: “Stage 1” from [4 Chiptunes (Adventure)](https://opengameart.org/content/4-chiptunes-adventure) by Juhani Junkala / SubspaceAudio (CC0)
- `Roboto-Regular.ttf` — UI menus ([Roboto](https://github.com/googlefonts/roboto), Apache 2.0)

Listed in `Game::assets` and loaded via `load_texture` / `audio.load` / `Font::load` (disk on desktop, `fetch` on web). Trunk copies this folder into the web dist. Replace with your own art when shipping.
