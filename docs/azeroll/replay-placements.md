# Replay placements still to apply (D33): L3 only

L1 is placed (`lions-pride-gen.py`, `lions-pride-npcs.toml`). L2 and L3 were held back because other
tracks were editing `market-day-gen.py` and `stormwind-gen.py` while the replay layer was built. Both
snippets below are written against the generators as they stood on 2026-11-03 ~04:45 and have **not**
been run: the geometry needs one capture each (see replay-design.md §10 #2, #3, #5) before it ships.
Everything else is in the tree: `props.csv` already has the `XmasGift02` row (extent 0.88), the schema
accepts all of it, and `design.rs` refuses a recipe whose `[present]` and `present = true` item do not
pair up.

## L2, Market Day Mayhem: DONE (2026-11-03)

Placed in `market-day-gen.py` and `goldshire-npcs.toml`; see the D33 paragraph in ARCHITECTURE.md for the measured numbers (eight crates, jambs 1.5 apart, Twinkle's `catch = [1.25, 1.88]`).

## L3, Stormwind or Bust! (`stormwind-gen.py`, `stormwind-npcs.toml`, `campaign.rs`)

1. Palette: `'gift'` with extent 0.88 (`props.csv` has it; add `'gift': r'World\Generic\PassiveDoodads\Christmas\XmasGift02.m2'`
   to the generator's model table).

2. After `[goal]`:

```
[present]
id = "thinking-helm"
title = "the Thinking Helm"
display = 4953
```

3. The parcel, under the Trade canal bridge (measure the deck clearance and the canal bed with
   `WOW_AZEROLL_SURVEY` first: the bed is expected at z 96-97, the deck at about 100.6):

```python
scene("Lost in the post", [-8753.2, 687.6, 96.5],
      "Fizzwick's third parcel: THROWN into the canal, on the bed beneath the Trade canal bridge's deck. The deck is permanent (D27), so this is a size window: grab it while you still fit (D 4.0 .. ~clearance).",
      indoors=True)
item('line', 'gift', 1, at=4.0, spacing=1.0, present=True, name="Fizzwick's Parcel", yaw=0.0)
```

   Fallback if the bed is not rollable: the Cathedral Square southern terrace behind the south bell tower, same
   scene name, as a ramp reward (replay-design.md 3.3).

4. Challenges (replace `speed`, `convoy`, `unrammed`, keep four at most):

```
[[challenges]]
id = "shops"
title = "Shopping spree: every shop in the Trade District"
kind = "collect-all"
names = ["Trade District cobbler", "Trade District tailor", "Trade District apothecary", "Trade District general store", "Trade District provisions", "Trade District workshop", "Flower shop", "Wine shop", "Canal-side shop"]

[[challenges]]
id = "bells"
title = "Ring the bells: all three Cathedral Square bell towers"
kind = "collect-all"
names = ["Cathedral Square west bell tower", "Cathedral Square south bell tower", "Cathedral Square east bell tower"]

[[challenges]]
id = "fair"
title = "Green city: every tree at the Cathedral tree fair"
kind = "scene"
prefix = "Cathedral tree fair"
radius = 60

[[challenges]]
id = "watch"
title = "The whole watch: every guard in the city"
kind = "collect-all"
species_list = ["Valley guard", "Trade District guard", "Cathedral guard"]
```

   Check the piece names against `stormwind-pieces.toml` and the guard species against the generator's herds
   (the names above are the design's); `progress::planned_piece` counts a city piece once, by its `name`.
   `convoy` is a kept id in the saves: if the Stormwind track wants to keep it, make it the fourth.

5. `campaign.rs`, the `stormwind` entry: `cast: Some(include_str!("stormwind-npcs.toml"))` (the recipe's
   `[level] cast = "stormwind"` is already there). New file `stormwind-npcs.toml`, Thrum alone:

```toml
# Stormwind's cast: Grand-Uncle Thrum Fizzwick, haggling in the Trade market (replay-design.md 4.3).
# The guards and citizens stay herds. A dwarf, by marriage; display 1626 at scale 2.2 stands about 3.5 yd.
[[npc]]
name = "Grand-Uncle Thrum"
role = "cousin"
cousin = "thrum"
scene = "trade market"
activity = "bark"
carry = "mug"
display = 1626
scale = 2.2
catch = [7.5, 11.3]
caption = "Three silver? THREE? For a HAT? I will roll you up myself, laddie."
at = [-8838.0, 626.0, 94.957]
yaw = 1.0
indoors = true
```

   Measure his axis from the `azeroll NPC:` line and set `catch[0]` = 2.15 x axis. Note `level::TierTable`'s
   `adults` representative then exists for L3, which changes its tier reps: check `reps` in the recipe.
