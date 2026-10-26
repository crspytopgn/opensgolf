# HD graphics (optional)

SimGolf's art was drawn for 800×600. On today's screens the port scales it up, and it gets soft. **HD mode** draws the same
art upscaled ×4 (or ×2), by the game's built-in pixel-art upscaler (xBRZ) or an AI upscaler such as Real-ESRGAN, plus
trilinear filtering and anti-aliasing.
Everything stays where it was: the layouts, the sprite cuts and the click areas are the classic ones.

**Classic is the default and is never changed by HD.** With HD off, the game runs exactly the code it ran before HD
existed and produces the same pixels (checked by rendering the same scenes with and without the HD code and comparing the
PNGs byte for byte). HD is opt-in, and switching back to Classic restores the classic output exactly.

## Legal note

The game's art belongs to Electronic Arts and the port reads it at run time from your own copy. An upscaled pack is a
derivative of that art, so:

- you make it **yourself, locally, from your own copy**, in the game (Preferences → Graphics → *Make HD graphics from my
  SimGolf files...*) or with `tools/hd_pack`;
- it is **never** committed to this repository, bundled with a release, put into `web/dist`, or offered for download by
  this project; `.gitignore` excludes `HD/` and pack folders;
- **do not share or upload your pack.**

The repository ships only the pack makers and the loader. The game's own upscaler is built in (xBRZ, see the README's
licence section); the AI upscaler is a separate download you get yourself if you want it (Real-ESRGAN ncnn-vulkan,
BSD-3-Clause, <https://github.com/xinntao/Real-ESRGAN/releases>). The game never downloads anything for HD.

## Making a pack

**In the game (one click).** With your SimGolf files in use, System Functions → Preferences → Graphics → *Make HD graphics
from my SimGolf files...*. The game says how many pictures, how much space and how long, then makes the pack in the
background while you play, shows how far it is (the box can be closed; the Preferences line shows the percentage), can be
stopped and carries on later where it stopped, and switches HD on when it is done. The choices:

| | scale | upscaler | size | time (4 cores) |
|---|---|---|---|---|
| desktop, Android: *Make it* | ×4 | xBRZ (built in) | about 0.8 GB | about a minute |
| desktop, Android: *Make a smaller one* | ×2 | xBRZ | about 0.4 GB | about half a minute |
| desktop with realesrgan-ncnn-vulkan on PATH or next to the game | ×4 | Real-ESRGAN (colours) + xBRZ (transparency) | about 1.6 GB | tens of minutes to hours |
| browser | ×2 | xBRZ, a few pictures a frame | about 0.4 GB | several minutes |

Natively the pack goes to `<save folder>/HD` (or `--hd-pack DIR`); in a browser the page keeps it in its storage with the
game files (IndexedDB, as a picked pack). The built-in maker writes exactly what `hd_pack.py` writes (the same files, sizes
and manifest; `method` is `xbrz`), so everything below holds for either. xBRZ is made for pixel art: edges and outlines stay crisp and diagonals smooth, colour keys
become real transparency before scaling (an outline never takes the magenta), and clear pixels carry the nearest opaque
colour so filtering adds no fringes. Real-ESRGAN invents more detail but softens the drawing; try both.

Test hook: `opensgolf --game <folder> --make-hd [--hd-pack DIR] [--hd-scale 2|4] [--hd-method xbrz|realesrgan]` makes the
pack without a window and says how long it took (docs/PLAYING.md lists the rest).

**With the script.** See [tools/hd_pack/README.md](../tools/hd_pack/README.md). In short:

```sh
pip install Pillow
# with Real-ESRGAN (realesrgan-ncnn-vulkan on PATH or next to the script):
python3 tools/hd_pack/hd_pack.py --game <your SimGolf folder> --out <save folder>/HD
# without AI (Lanczos, for testing):
python3 tools/hd_pack/hd_pack.py --game <your SimGolf folder> --out <save folder>/HD --method lanczos
```

The save folder is `%APPDATA%\OpenSGolf` on Windows, `~/Library/Application Support/OpenSGolf` on macOS and
`~/.local/share/opensgolf` (or `$XDG_DATA_HOME/opensgolf`) on Linux, or `OPENSGOLF_SAVE_DIR` when set.

## Smaller packs

Pictures in a pack may be WebP instead of PNG (same name with `.webp`). At quality 95 a pack is about a tenth of
the size and looks the same, which helps when moving a 4x pack between machines.

## Turning it on

- **In the game:** System Functions → Preferences → *HD graphics (off: Classic)*. Switching reloads all the art: the
  interface, the terrain textures and the sprites. The choice is saved in `settings.json` in the save folder.
- **Command line:** `--hd` or `--classic` for one run (overriding the saved choice), and `--hd-pack DIR` for a pack
  somewhere other than `<save folder>/HD`.
- **Browser:** under *Optional: HD graphics* on the start page, pick the pack folder or a .zip of it. It is kept in the
  browser like the game files, and the game then starts in HD. Preferences switch it off and on.

If HD is chosen and no pack is found, the game says how to make one and stays Classic.

Content packs (docs/MODDING.md) sit above the HD pack: when an enabled mod replaces a picture, terrain texture or sprite,
the mod's file is used, loaded the classic way (or at its own resolution for a high-resolution pack of your own art,
docs/MODDING.md section 11), and the HD pack's upscale of the original is not. Everything else is as described here.

## What changes in HD

| | Classic | HD |
|---|---|---|
| Interface, info screens, heads, theme tiles | original PCX | pack PNG at ×scale, same on-screen size |
| Terrain textures | original BMP/TGA, mipmapped | pack PNG at ×scale, mipmapped |
| Buildings, trees, scenery, animals, water, bridges, flowers... | original FLC frames | pack frames at ×scale, mipmapped; colour variants recoloured from the pack's frames |
| Golfers, celebrities, staff, bodies, sprite shadows | original, bilinear | original, bilinear (not upscaled, see below) |
| Anti-aliasing | none | 4× MSAA (when the renderer can resolve multisampled targets) |
| Texture filtering | bilinear | trilinear (mipmaps) for pack textures |

How it works: each loader (`ui::load_pcx`, `load_pcx_alpha`, `split_overlay`, the terrain `texture_for` and the sprite
loader) first checks `hd::active()`. Only in HD does it look the file up in the pack's manifest. It then creates the
texture at the pack's resolution but reports the original size, so all drawing code keeps working in the original's pixel
units (texture coordinates are relative). Anything the pack lacks, or whose size does not match, loads the classic way.
The code is in `crates/simgolf/src/hd.rs`.

## Limitations

- People (golfers, celebrities, staff) and the body sheets are recoloured per person from palettes at run time, so they
  are not upscaled. They are drawn from the classic art with smooth filtering.
- The hole signposts, portraits and accomplishment snapshots stay classic.
- A file loaded both with and without its key colour shows the pack's colours under the key (the classic magenta would
  show there instead). Some files contain both 255,0,255 and 248,0,248, and the pack treats both as transparent.
- Palette variants of sprites are made by adding the palette change, spread smoothly, to the default-palette upscale. This
  is close to, but not exactly, an upscale of the variant.
- A ×4 pack is large (about 0.8 GB on disk with xBRZ, 1.6 GB with Real-ESRGAN or Lanczos), and HD sprites need much more
  video memory than classic ones. Use ×2 on small GPUs and in browsers.
- The quality depends on the upscaler. The Lanczos path of hd_pack.py is for testing only.
