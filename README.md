<img src="assets/icon.svg" width="64" height="64" alt="">

# Azeroll

**Everything is Bind on Pickup.**

A Katamari Damacy-style game set in vanilla World of Warcraft. A runaway Gnomish Reclamation Device,
pushed by a very small gnome, starts out rolling up mugs and pies outside the Lion's Pride Inn in
Goldshire. It grows through chickens, villagers, wagons and trees until it is big enough to roll up
Hogger.

Azeroll runs on a from-scratch WoW 1.12.1 client in Rust and [Bevy](https://bevy.org): the
[benilla](https://github.com/samwhosung/benilla) engine, by way of its browser fork
[wenilla](https://github.com/Arnesen/wenilla). The idea of turning that client into a server-less
game came from [Azeroth Survivors](https://github.com/outthislife/azeroth-survivors-re). There's no
server, login or account.

## Play it

You need a World of Warcraft 1.12.1 (build 5875) client's `Data` folder. This repo ships no Blizzard
content.

```bash
make fetch-data                                   # optional: download the 1.12.1 MPQs into wow-data/Data
WOW_DATA=$PWD/wow-data/Data cargo azeroll         # native, optimized `play` profile
scripts/win-build.sh                              # cross-compile a Windows package to ~/azeroll-win.zip
scripts/web-setup.sh && scripts/web-build.sh      # browser (WebGPU) build into web/dist
```

The world takes a minute or two to load the first time.

### The campaign

You start on the hub, the planet Azeroth, with three requests from the gnome Fizzwick: **Last Call at
the Lion's Pride** (an inn, roll up 2 yd 14 in), **Market Day Mayhem** (Goldshire's fair, roll up the
Prize Pumpkin) and **Stormwind or Bust!** (eat the city, then the Cathedral). Finish one to open the
next. A win is graded (Scraped by, Respectable, Splendid, Preposterous!) and names a star on the hub.

On a size request the goal is only the start: keep rolling until the candle goes out, or hold
Enter to call it. The final size is what is graded. When it ends, Fizzwick's Reclamation Beam lifts
the clump into the light and the results dock beside it. Fizzwick's Catalogue (`C` on the hub) logs
every kind of thing you have ever rolled up.

### Controls

| Key | Action |
| --- | --- |
| W / S | roll forward and back |
| A / D | turn (the camera stays behind the ball) |
| Shift | dash (hold for a wind-up and a harder burst) |
| Space | quick 180° turn |
| Q / E, mouse drag, wheel | swing and tilt the camera, zoom |
| Esc | pause (Esc again resumes; **L** on the pause screen leaves for the hub) |
| R | start the level over (play, pause or results) |
| Hold Enter (after the goal of a size request) | call the run early; the clock rim fills |
| Hold Backspace | get unstuck |
| Enter / Space / Esc during the send-off | skip it |
| Results: Enter / R / F | back to the hub / roll again / Free Roll (after a win), untimed |
| Hub: W A S D or arrows, Enter | choose a level, roll it |
| Hub: F | Free Roll a level you have won |
| Hub: **C** | Fizzwick's Catalogue (W/S list, A/D level, Esc or C closes) |

### Gamepad

Twin-stick, as Katamari: each stick is a tank track. Both sticks up roll forward (speed by throw),
one up and one down spin on the spot, both sideways strafe. A pad touched on the title starts the run.

| Pad | Action |
| --- | --- |
| both sticks up / down | roll forward / back |
| one stick up, one down | turn on the spot |
| both sticks sideways | strafe roll |
| `A` (hold, release) | dash with a wind-up |
| sticks swapped up/down three times in a second | the dash gesture |
| `X` or click both sticks | quick 180° turn |
| `L1` / `R1`, d-pad up / down, `L2` / `R2` | swing the camera, zoom, peek |
| `Start` | pause |
| `Y` (hold) | call a won size run, or skip the send-off |
| `Select` (hold) | get unstuck |
| menus: d-pad or left stick, `A` / `Start`, `B`, `X`, `Y` | move, confirm (Enter), back (Esc), restart (R; `C`, the Catalogue, on the hub), Free Roll (F) or leave (L, on pause) |

Simple mode (`Select` on the pause screen): the left stick steers like a car and the right stick
works the camera. Details are in [`ARCHITECTURE.md`](docs/azeroll/ARCHITECTURE.md) (D32).

## What's in the Goldshire level

- Katamari collection: an object rolls up once the ball's diameter is 2.15 times the object's
  longest side. Somewhat bigger things get knocked loose, and anything bigger than that is a wall.
  The ball grows by the real volume of what it collects, and lopsided loads make it wobble.
- A hand-built Goldshire: the inn's supper spill, the coop raid, the merchant unloading, the smithy,
  the woodcutters, the farms and the Riverpaw camp, joined by trails of breadcrumbs.
- Authentic Goldshire NPCs who ignore you, then worry, then panic. Rolled-up villagers flail on
  the ball. Animals scatter, and Hogger shows up early to remind you who's in charge.
- A Katamari-style HUD (size rosette, clock, pickup bubble, Prince portrait), size-milestone camera
  beats, pickup and material sounds, records, challenges and Free Roll.
- Original music by the project's author in `assets/music/`.

Design notes, decisions and research are in [`docs/azeroll/`](docs/azeroll/), starting with
[`ARCHITECTURE.md`](docs/azeroll/ARCHITECTURE.md).

## Legal

Azeroll is a free fan project. It is not affiliated with or endorsed by Blizzard Entertainment or
Bandai Namco, and it ships no Blizzard game files. World of Warcraft is a trademark of Blizzard
Entertainment, Inc.; Katamari Damacy is a trademark of Bandai Namco Entertainment. Azeroll borrows
Katamari's ideas, not its code, music or art.

The engine code is licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your
option. Vendored components under `third_party/` keep their own upstream licenses.
`docs/azeroll/data/goldshire-npcs.json` holds factual spawn data (ids, coordinates, names) extracted
from the GPL-3.0 CMaNGOS classic-db.
