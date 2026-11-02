# Scene format (goldshire.toml)

A scene is a few **item groups** that tell one small story: a spill, a heap, a laid table, a stray.
`ARCHITECTURE.md` D14 has the recipe basics; this file is the vocabulary for making them look lived-in.
Everything is deterministic: a group's layout is seeded by its scene name and its position in the
scene, so an edit moves only what it touches.

## Mixed models

`models = [...]` replaces `model` and picks per prop (`pick = 'random'`, the default, or `'cycle'`).
Repeat an entry to weight it. A scene should never be one model times fifty: mix 4-10 models and
add one or two single-prop punchlines (`count = 1`, `offset = [x, z]`).

## Arrangements

| `arrangement` | `spacing` means | notes |
|---|---|---|
| `cluster` | radius holding ~80% of the props | dense centre, falloff, ~12% stragglers; `gap` keeps props apart |
| `heap` | base radius | props rest on props in three layers, each `step` yards higher and narrower |
| `table-set` | half-width of the tabletop | `table = 'model'`, `tables`, `table_gap`, `surface` (top height) put the props on a table or bench; with no `table` it is a floor cluster at `surface` |
| `spill` | average step | a wandering trail of uneven steps along `heading`, turning up to `bend` rad per prop |
| `line`, `grid`, `ring` | step / step / radius | regimented on purpose (fences); `wobble` roughens them |
| `scatter` | density | even density with random positions |
| `pile` | density | the original sunflower spiral |

Per group: `offset = [east, south]` from the scene anchor, `lift` (base height), `yaw` (fixed; omit for
random), `tilt` (random lean up to N rad, for dropped things), `topple` and `topple_lift` (fraction lying on
their side, and how far a toppled prop is raised), `scale`, `jitter` (random scale, plus or minus a
fraction). Headings are radians in Bevy's ground plane: 0 east, pi/2 south, pi west, 3pi/2 north.

## Trails

`models`, `wobble` (drift off the straight line, yards), `variance` (uneven spacing, fraction of
`spacing`), `clump` (chance a drop is a pair), `jitter`, `tilt`.

## Rules the loader and other systems rely on

- Sizes still come from pickup thickness x scale: pick scale so each prop lands in the tier you want.
  Check the `azeroll VALIDATE` table in the log after an edit.
- Tabletop props sit about a yard up; a small ball cannot reach them until it grows. The inn is
  enclosed, so keep what is *in* the room inside the D10 budget (`azeroll D10 PASS`).
- `progress.rs` finds the scene named exactly `Inn supper spill` and counts its single-`model` groups for
  the supper challenge: every Pie and Turkey Leg within 4 yd of its anchor. Keep those reachable
  (on the floor) and keep the name.
- Fence and wagon gates (`solid = true`) use `line` with a fixed `yaw` and no `wobble` so they stay exact.
