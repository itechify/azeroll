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

| Key | Action |
| --- | --- |
| W / S | roll forward and back |
| A / D | turn (the camera stays behind the ball) |
| Shift | dash |
| Space | quick 180° turn |
| Mouse drag / wheel | adjust the camera |
| Esc / R | pause / restart |
| Hold Backspace | get unstuck |
| F (after a win) | Free Roll, untimed |

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
