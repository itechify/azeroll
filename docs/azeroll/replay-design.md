# The replay layer: parcels, cousins and challenges

Design for gap-audit-2 **#15** (Royal Presents and cousins) and a refresh of each level's
`[[challenges]]`. Written 2026-11-03 against D21 (recipes), D24 (hub), D29 (climbable props),
D31 (ritual, Fizzwick in play, the Catalogue), the level sections for L1–L3, the generators under
`docs/azeroll/data/`, the captures in `/home/jjd/az-patches/{l1r,l2r,l2r2,city,ritual2,hub2}`, and the
1.12 client's own DBCs (read with the prebuilt `mpqx` from the shared target; nothing was built or run).
Design only.

**What it adds, in one paragraph.** Every level hides one **parcel** (Katamari's Royal Present: a
real Winter Veil gift box, `XmasGift02.m2`) in a spot you reach small, or by a ramp, or by coming
back big; rolling it up unlocks an **outfit** for the Prince (a gnome display with a monocle, a
bandana, an engineer's helm). Every level also hides one **cousin** of Grand Tinker Fizzwick, a small
gnome (and one enormous dwarf) with a silly name who ignores you until you are the right size, then
leads you a chase; rolling one up unlocks them as an alternate **pusher**. Both live in the Catalogue
(D31), where Enter wears an outfit or picks a pusher. The challenges become four Katamari-flavoured
tasks per level, each decidable from the pickup log. Nothing here touches the HUD's wordlessness: the
only new play-screen words are a 2.6 s `PARCEL!` / `COUSIN!` in the existing banner slot and one
Fizzwick pop-in.

**Reading order.** §1 the reference, §2 the reward set (with the asset ids), §3 parcels, §4 cousins,
§5 challenges, §6 UI, §7 copy, §8 data shapes, §9 the implementation brief, §10 acceptance.

---

## 1. What Katamari does (the parts we copy)

- **Royal Presents.** Almost every stage hides one present from the King; collecting it unlocks an
  accessory the Prince wears while rolling (KeenGamer's REROLL guide: scarf, chef's hat, champion's
  belt, camera, headphones, apron, guitar, aloha set, ducky, crown, superhero scarf, running top,
  mawashi, wig, cool mask) [C]. They are placed as a lure that moves (Santa's eraser sleigh, a crow,
  a balloon, a monster truck, Jumboman) or a reward on a surface you must climb (the kitchen table
  edge, a cuckoo clock, a smokestack, a beach umbrella) or a pocket you only fit into small ("it's
  relatively small, so don't get too big before you grab it"; a cave past campfires) [C]. Presents are
  worn from a presents menu in the hub; REROLL+ puts them on a second page of that menu [S].
- **Cousins.** One per stage, hidden in a vignette: Marcy runs under the kotatsu you start beside;
  Havana rolls an orange with the mice; Nik is "big, so you'll have to get the katamari close to 1 m";
  Miso rides a cow and you "bump the cow and grab her while it stands still"; Lalala hides in the
  doghouse and "once you've picked up too much, the doghouse is hard to enter"; Odeko hangs on a hook
  in the rafters [C]. Each counts as an object for the Collection and 100 % [C]. In the later games they
  are playable in place of the Prince [F].
- **The lesson for placement** (katamari-room-design.md P11): a present is a lure that moves, or a
  reward on a surface with a ramp, or a pocket with a size window. The return path must exist.

Evidence tags as in the other notes: [C] a fetched page, [S] snippet, [F] fan claim, [I] inference.

---

## 2. The reward set

The Prince is a gnome display (`creatures.rs` picks the lowest male-gnome NPC appearance, D7/D20a)
and the commissioner is another (`hud_models::commissioner_display`, D24). A **Prince outfit is
therefore a display-id swap**: no attachment work, no new assets. The ids below are real rows of this
client's `CreatureDisplayInfo` + `CreatureDisplayInfoExtra` (race 7 gnome / 3 dwarf), read from
`patch-2.MPQ` on 2026-11-03; the implementer verifies each by eye with the override in §9.1 before it
ships, and swaps any that reads badly for another row from the same table (there are 328 gnome and 772
dwarf-male rows; the dump script is `/tmp/az-replay-tools/dbc2.py`, re-creatable from §9.1).

| reward | what the player sees | asset | unlocked by |
|---|---|---|---|
| **The Monocle** | the Prince with a monocle and a tailor's coat | display **5377** (gnome M, helm 14852 `Helm_Monocle_B_01`, chest 13440) | L1 parcel |
| **Mirela's Bandana** | a red pirate bandana and a traveller's shirt | display **4873** (gnome M, helm 15491 `Helm_Cloth_B_01Pirate`, chest 12969) | L2 parcel |
| **The Thinking Helm** | the big gnomish engineering helmet | display **4953** (gnome M, helm 15371 `Helm_Engi_B_01`, chest 13440) | L3 parcel |
| **Pippa Fizzwick** (pusher) | a small gnome girl with pigtails pushing the ball | display **3121** (gnome F, hair 4, face 5; alternates 3106, 3124) | L1 cousin |
| **Twinkle Fizzwick** (pusher) | a gnome girl in goggles | display **5378** (gnome F, helm 12139 `Helm_Goggles_C_01`) | L2 cousin |
| **Grand-Uncle Thrum** (pusher) | a dwarf in goggles with a barrel chest | display **1626** (dwarf M, helm 13800 `Helm_Goggles_B_04`, chest 13803; alternate 1410, the dwarf monocle) | L3 cousin |
| **The Royal Fireworks** (hub trinket) | fireworks burst over the planet on every hub arrival | `select/art.wgsl` only (sparkle bursts, sums of sines, like the clouds) | all three parcels |
| **The Family Portrait** (hub trinket) | the three cousins stand behind Fizzwick in his window and cheer with him | three more units on `COMMISSIONER_STAGE` (D24), scale 0.7, behind and beside him | all three cousins |

**The parcel model.** `World\Generic\PassiveDoodads\Christmas\XmasGift02.m2` (patch.MPQ; the Winter
Veil gift box with a bow, GameObjectDisplayInfo 5255). Vertex extents 0.84 × 0.84 × 0.88 yd, origin
on its base (min z 0.01). Siblings 01/03/04/06 are flatter boxes (0.71 × 0.91 × 0.42, 0.75 × 1.39 ×
0.34) and 05 is a second cube; use 02 for all three so the silhouette is learnable, and tint nothing.
Add it to `props.csv` and to each generator's model table with extent **0.88**.

**Why outfits, not ball skins.** The user's list also allowed a new core skin and hub trinkets. A
display swap is a solved path (the commissioner already does it) and the corner Prince (D20a) mirrors
the world Prince's display for free, so the reward shows in every frame of play. A shell skin would
need a material lane in `lod.rs` nobody has opened yet; it stays on the deferred list (§9.9). The two
hub trinkets are the completion rewards, because Katamari's collection screen is where completion is
felt (the bunny ears for 100 % in REROLL+ [S]).

**Rule.** Outfits dress the Prince only. Choosing a cousin as the pusher hides the outfit (the
cousin wears their own clothes); choosing the Prince again restores it. One `pusher`, one `outfit`,
both in the save (§8).

---

## 3. Parcels (Fizzwick's mis-delivered post)

**The fiction.** Fizzwick ordered three parts from Gnomeregan by the Stormwind post. The post lost all
three. One is in the inn's fireplace, one is in a chicken coop, one is in a canal. The Catalogue name
of each is **Fizzwick's Parcel** (the level tells them apart), category Odds and ends.

**Common rules.**
- One per level, `present = true` on a scene item of `XmasGift02.m2`, with a `[present]` table that
  names the reward (§8). Never `solid`, never the goal.
- **No marker, no UI.** It is foreshadowed by placement: in a landmark's sight line, with the room's
  story pointing at it (hens going in and out, soot, a bridge you pass under). Its bow is the brightest
  thing in its pocket. The one concession to visibility is the existing item-bubble: the first time the
  parcel enters the pickup search the bubble does nothing; the first time it is rolled up the bubble
  shows it with its name, as for any item.
- **The moment.** On `Collected` of the present entity: the banner slot (D31, 14 %) shows `PARCEL!`
  for 2.6 s in gold; the discovery sting (`TaxiNodeDiscovered` 1519, the tier sting); the rosette
  blooms; Fizzwick pops in with the level's parcel line (§7.1) on a **bonus slot** outside the
  3-per-run budget (priority just under the goal). The clump keeps rolling.
- **It is yours at the touch,** not at the results: a shed parcel (knocked off by a ram) is still
  found, and the unlock is merged into the save the way the Catalogue is (idempotent, on pause, restart,
  leaving and results; D31). Free Roll finds count (the Catalogue rule: a log, not a score). A debug
  start (`persist = false`) does not save it.
- **The autopilot never targets a parcel or a cousin.** They are human content and must not move the
  pacing numbers. Fixtures start the ball at the spot instead (§10).
- **The results and hub** say what was found (§6).

### 3.1 L1, Last Call at the Lion's Pride: in the fireplace

| | |
|---|---|
| where | inside the cold **firebox**, 1.0 yd in from its lip, on the firebox floor: WoW (−9457.5, 22.3, 57.0). The firebox is set in the pit's north wall at x −9458.5..−9456, y 20..24.5, floor 57.0 = 0.66 above the pit boards (L1 section). It sits behind the ash pail and the kettle of scene *The hearth has gone out* (anchor (−9459.6, 22.3, 56.339)). |
| why there | Farley hid the post from the regiment in the one place a soldier never looks. The fireplace is the room's north landmark (P9), 8 yd from the start, framed by the two book stairs the player already climbs (`l1r/04-hearth-bookstairs.png`): a red-and-gold box in a black firebox is seen from the nursery. |
| how you reach it | **Small:** up the existing seven-book stair into the firebox while the books are still steps (they are edible from D 0.67 and vanish under a ball that size). **Big:** from D ≈ 2.0 the ball steps the 0.66 lip unaided (`0.2 + 0.45 r`) and reaches 1 yd in (a r 1.2 sphere intrudes 1.1 yd through the 2.5 yd opening). Between D 0.67 and 2.0 it is out of reach: the Katamari "come back for it" (MaS1's windowsill, P11), and the goal is 2.4, so every winning run can come back. |
| opens at | D **0.45** (scale 0.238, a 0.21 yd cube): edible from the nursery's end, so the first-minute detour pays. |
| scene | its own scene, *The parcel in the fireplace* (so the hearth's scene challenge, if kept, does not count it), anchor the firebox, `present = true`, `lift` so it sits on the 57.0 floor (every prop snaps to the highest WMO surface under its origin, which is the firebox floor here). |
| reward | **The Monocle** (display 5377): the callback to Fizzwick's "I do not own a monocle" (D31). |
| risks | the book stair must still reach the firebox floor at r 0.22–0.33 (D29 acceptance climbed nine books onto a table at r 0.2); a parcel 1 yd in must not be reachable by a r 0.5 ball from the lip (its sphere reaches 0.47 yd in at the floor's height). Fixture: §10 #1. |

### 3.2 L2, Market Day Mayhem: in the chicken coop

| | |
|---|---|
| where | inside **the coop by the signpost** (scene anchor (−9441, 61, 56.5), 6 yd from the start and in its first frame), at the coop's back wall. The coop becomes a real pocket: six `Crate01` crates at scale 0.6 (extent 0.89, solid, opens at D 1.9) in a U, inner width 1.6 yd, open side toward the beer garden with a **0.95 yd gap**; the parcel at the back, hens inside and out, the burst feed sack at the gap. |
| why there | Katamari's doghouse (Lalala: "once you've picked up too much, the doghouse is hard to enter"). The hens pecking in and out tell you there is an inside. It is the start vignette's third element (supper tray, beer garden, coop), so a curious player meets it before the street pulls them away. |
| how you reach it | **Small:** through the gap at D ≤ 1.05 (the mover's 0.85 r clearance: 0.85 × 0.525 < 0.95/2). The start is D 0.64 and the supper tray brings it to about 0.9 in the first half minute, so the window is the first minute. **Big:** from D 1.9 the crates are food; eat the wall, take the parcel. |
| opens at | D **0.8** (scale 0.42, a 0.37 yd cube). |
| scene | *The coop by the signpost*, extended: the six crates (`solid = true`, `grid`), the parcel `present = true` at the back, the hens' herd unchanged. No `blocker` (the crates must become food). The pocket is on the start's flat (56.5), no steps. |
| reward | **Mirela's Bandana** (display 4873): the wagon's traveller lost a bandana in the post; the parcel was hers. |
| risks | the L2 lesson "a blocker must cover the bait behind it" was about the autopilot hunting things it cannot reach: it never targets presents (§3 rules), and the human sees the gap. A D 1.0 ball inside grows by the parcel's 0.119 × 0.05 ≈ 0.006 r³, nothing. Keep the hens' herd `radius` so they do not pile in the gap. Fixture: §10 #2. |

### 3.3 L3, Stormwind or Bust!: under the Trade canal bridge

| | |
|---|---|
| where | on the **canal bed beneath the Trade canal bridge's deck**, on the bridge's centre line (bridge area centre (−8753.2, 687.6, 100.6); the canal bed's z from the survey, expected ≈ 96–97). Bridge decks and canal beds are permanent (D27), so the deck is a roof the ball can never eat. |
| why there | "The post THREW it in the canal." The route follows the canal to the Cathedral fair (the briefing says so), the ball passes the bridge at D 6–10, and the parcel's bow is in view from the Trade canal bank. It is the one present you must take **while you still fit**: too big and the deck keeps it forever (a true size window, the reverse of L1's). |
| how you reach it | roll down into the canal strip (the 12 yd navigation capsules cover it; a run once "drove beneath the bridge", D27) and under the deck. The clearance under the deck is the gate; the implementer measures it (expected 6–8 yd) and sets `opens at` below it. |
| opens at | D **4.0** (scale 2.11, a 1.9 yd box: Gnomeregan-sized, visible from the bank). |
| scene | a new scene *Lost in the post*, anchor the bridge's centre on the canal bed, `present = true`, `wmo_floors = true` placement (the recipe already opts in). |
| reward | **The Thinking Helm** (display 4953). |
| fallback | if the canal bed is not rollable under the deck (water, a collider), put it on the **Cathedral Square southern terrace** behind the south bell tower, reached by the tree-fair rows (the terrace rises through rows 1–13): a ramp reward instead of a pocket. Same scene name. |
| risks | the deck's underside must not be a camera trap: the D8 lens boom is radius-driven and the cutout handles the deck; a ball under a 6 yd deck at D 5 is the inn at D 2, which works. Fixture: §10 #3. |

---

## 4. Cousins (Fizzwick's relatives)

**The fiction.** Fizzwick has relatives everywhere and introduced none of them. Each is a cast NPC
(`[[npc]]`, `npcs.rs`), `role = "cousin"`, with a display, a scale and a **catch window** in ball
diameters. They are Folk in the Catalogue and count toward the level's `N of M`.

**Behaviour by size** (one new NPC role; the states are the existing ones, D18):

| ball diameter | the cousin | why |
|---|---|---|
| below the window | **ignores you.** Stays `Ambient` in their vignette (their `activity`), never `Notice`, never `Flee`, no panic contagion. The ball cannot roll them up (they are not edible yet) and they are non-solid like every villager (D18), so the ball passes through the vignette. | "too small = they ignore you" |
| inside the window | **the chase.** On the ball within 6 yd: `Notice` (a wave, EmoteWave 67, their one-liner typed over their head as a caption for 2 s: §7.2), then `Flee` with `Style::Circle` round their landmark, 1.3× villager speed, re-fired while the ball is within 10 yd, and they never leave their room or storey (the indoor flee already keeps the spawn storey). Rolled up here = **caught fair**. | the target-size catch, Katamari's Havana and Miso |
| above the window | **swallowed.** `Cower` on sight (crouch, cry), no chase; the touch rolls them up at once. Unlocks all the same, with the other line. | "too big = instantly absorbed"; never punish a big ball |

Both outcomes unlock the pusher. "Caught fair" is recorded (`cousin_fair`) only for the Catalogue's
line and the results stamp; it gates nothing. The window is `[open, 1.5 × open]`; `open` is where the
cousin becomes edible (2.15 × their scaled height), so the implementer sets `scale` from the chosen
`open` as the generators do for props (gnome height 1.375 yd at scale 1, from the Prince's 1.1 yd at
0.8; dwarf male about 1.6 yd: measure once with `survey.rs`).

**The moment** on `Collected` of a cousin: banner `COUSIN!`, the same sting, the bubble shows the
cousin's model with the creature sway and their name, Fizzwick's bonus pop-in (§7.2 caught / swallowed).
The caught cousin rides the clump animated like the newest eight NPCs (`npcs.rs` carried reactions);
they are the one carried unit that **cheers** instead of crying.

**As a pusher.** `Wardrobe::pusher = Some("pippa")` makes `creatures.rs` resolve the Prince's display
from the cousin's id instead of the lowest gnome; everything else about the Prince (scale 0.25,
`Push`, the wind-up, the corner mirror) is unchanged. Thrum at scale 0.25 is a head taller than the
gnomes, which is the joke.

### 4.1 L1: Pippa Fizzwick, counting the rats

| | |
|---|---|
| who | gnome girl, display 3121, scale 0.45 (0.62 yd tall), `activity = "work"` (she is counting), `carry = "cheese"`. |
| where | **the cellar**, in the rats' cheese hoard: (−9466.5, −4.0, 49.8), `indoors = true`, among the 12 rats (herd at (−9467, −6, 49.8) r 4). Her landmark for the chase circle is the Colossal Swiss. |
| window | opens at D **1.33**, window 1.33–2.0: the cellar is routed at D 0.6–2.3 (the cheese band), so a player who follows the cheese trail from the kitchen meets her in the window. The stairwell (3.4 yd) admits any ball to the goal. |
| foreshadow | the rats run lines through the cheese (P10); she is the one shape among them that waves. The kitchen's cheese trail already leads here. |
| caption (over her head, window only) | "Forty-one, forty-two… do NOT roll up the rats, I am COUNTING." |
| Catalogue quip | "Pippa. She was counting the rats. She lost count. She IS the count now." |

### 4.2 L2: Twinkle Fizzwick, apple-bobbing

| | |
|---|---|
| who | gnome girl in goggles, display 5378, scale 0.5 (0.69 yd), `activity = "sit"` pose if the cast has one, else `"applaud"` with her head in the barrel's direction. |
| where | the **greengrocer's apple-bob queue** (scene anchor (−9479, 79, 56.7)): at the apple-bob barrel, (−9481.5, 81.0, 56.7), facing it; the queue of kids beside her. Chase circle round the barrel and the apple pyramid. |
| window | opens at D **1.48**, window 1.48–2.3 (Critters 1.04 → Livestock 2.58): the market phase, minutes one to three. |
| foreshadow | she is in the queue of children, head down; the queue is a vignette the street's crumb line leads to (*Crumbs to the cheese stall*, then the greengrocer). Goggles in a queue of kids read as "someone else's child". |
| caption | "I am NOT lost. I am apple-bobbing. There is a DIFFERENCE." |
| Catalogue quip | "Twinkle, apple-bobbing. Got the apple. Also got rolled up. Mixed day." |

### 4.3 L3: Grand-Uncle Thrum Fizzwick, haggling

| | |
|---|---|
| who | a **dwarf** (by marriage: "we do not discuss it"), display 1626, scale 2.2 (about 3.5 yd tall: the postmortem's "giant" trick, and nine times the Prince), `activity = "bark"` (he is arguing a price), `carry = "mug"`. |
| where | the **Trade market** (scene anchor (−8829.9, 620.2, 94.957)), on the Auction House side of the market: (−8838, 626, 94.957), `indoors = true` (the city's WMO floors). Chase circle round the market's middle. |
| window | opens at D **7.5**, window 7.5–11.3: the ball enters the gate near D 6 and sweeps the market at 6–12. |
| foreshadow | he is the only thing in the market taller than the stalls, shouting. From the gate he is a landmark (P9) before he is food. |
| caption | "Three silver? THREE? For a HAT? I will roll you up myself, laddie." |
| Catalogue quip | "Grand-Uncle Thrum. A dwarf. By marriage. We do not discuss it. He is on the ball now, so we will have to." |

---

## 5. The challenges, refreshed

**Principles.** At most four per level. Each is decided from `Run::finds` (name, species, pickup
order, play time; D31) and the recipe, so every one is unit-testable without the world. Each is a
Katamari joke, not a stat: a thing to do *in* a vignette. **The `speed` challenges go:** the comet
already rewards pace (goal under par, D31) and a second clock on the card was noise. `unrammed` goes
too (no one plans around it). Kept ids keep their save semantics; a kept id with a stricter rule is
simply harder to re-earn, which is fine for a save that already has it.

**Two new `ChallengeKind`s and two list fields** (`design.rs`, §8):

| kind | fields | met when |
|---|---|---|
| `before` | `species` or `name` (the set), `before_species` or `before_name` (the gate) | every member of the set was rolled up and the first pickup of the gate came after the last of them (or the gate was never rolled up) |
| `within` | `scene`, `radius`, `secs` | every prop of the scene was rolled up and `last.t − first.t ≤ secs` |
| `collect-all` (existing) | gains `names = [..]` (a list of display names) and `species_list = [..]` | every one of every listed name / species |
| `scene` (existing) | gains `prefix = "..."` (every scene whose name starts with it) | as today, over the union |

### 5.1 L1, Last Call at the Lion's Pride (300 s, size 2.4)

| id | title | kind | rule |
|---|---|---|---|
| `rats` (kept) | **Pied piper: every cellar rat before the first cheese wheel** | `before` | species `Cellar Rat` before name `Cheese Wheel` |
| `pillows` | **Pillow fight: every pillow in the upstairs lounge** | `scene` | *Pillow fight in the lounge*, radius 6 (the upstairs detour has a reason) |
| `floor` | **Farley's floor: every stool and footstool** | `scene` | *Farley wants his floor back*, radius 9 (the finish band, after the goal: the ritual's "keep rolling") |
| `big` (kept) | **Preposterous: three yards across** | `size` | 3.0 |

Dropped: `speed` (the comet), `hearth` (the parcel lives there now; a scene challenge on it would
spoil the hiding).

### 5.2 L2, Market Day Mayhem (620 s, the Prize Pumpkin)

| id | title | kind | rule |
|---|---|---|---|
| `hogger` (kept) | **Roll up Hogger, who raided the cheese stall** | `collect-all` | species `Hogger` |
| `chickens` (kept) | **Chicken run: every chicken at the fair** | `collect-all` | species `Chicken` |
| `garden` | **Last round: the whole beer garden inside sixty seconds** | `within` | *The Lion's Pride beer garden*, radius 10, 60 s (the tables need the bale steps: it is a routing puzzle, not a sprint) |
| `parade` | **Clean sweep: every runner-up on the street** | `scene` | *Parade of the runners-up*, radius 40 (the run ends on the Prize Pumpkin, so "before the prize" is implicit) |

Dropped: `speed`, `supper` (the cheese stall is on every route), `unrammed`.

### 5.3 L3, Stormwind or Bust! (1800 s, the Cathedral)

| id | title | kind | rule |
|---|---|---|---|
| `shops` | **Shopping spree: every shop in the Trade District** | `collect-all` | `names = ["Trade District cobbler", "Trade District tailor", "Trade District apothecary", "Trade District general store", "Trade District provisions", "Trade District workshop", "Flower shop", "Wine shop", "Canal-side shop"]` (pieces of `stormwind-pieces.toml`) |
| `bells` | **Ring the bells: all three Cathedral Square bell towers** | `collect-all` | `names = ["Cathedral Square west bell tower", "Cathedral Square south bell tower", "Cathedral Square east bell tower"]` |
| `fair` | **Green city: every tree at the Cathedral tree fair** | `scene` | `prefix = "Cathedral tree fair"`, radius 60 |
| `watch` | **The whole watch: every guard in the city** | `collect-all` | `species_list = ["Valley guard", "Trade District guard", "Cathedral guard"]` |

Dropped: `speed` (11:00; the comet par is 15:00), `unrammed`.

---

## 6. UI (kept tiny)

### 6.1 In play: the moment (both parcel and cousin)

```
┌──────────────────────────────────────────────────────────────────┐
│ dial                                     03:12  clock            │
│               PARCEL!            ← banner slot, 26 px, gold, 2.6 s│
│                 ┌────────────────────────────────┐ ╭──────╮      │
│                 │ Found it! The post said "lost". │◁│ Fizz │      │
│                 │ The post LIED.                  │ │ wick │      │
│                 └────────────────────────────────┘ ╰──────╯      │
│                          (ball)                                  │
│  ╭────────╮                                                      │
│  │ parcel │ Fizzwick's Parcel                  Prince corner     │
│  ╰────────╯ ← the existing item bubble                           │
└──────────────────────────────────────────────────────────────────┘
```

Nothing new is drawn: the banner slot, the bubble and the porthole all exist (D31). The cousin's
caption over their head in the window is the NPC caption style already used for the cast (one line).

### 6.2 The results tally: one line each, only when it happened

```
│ │ Best find: Chair                       │ │
│ │ A new star: The Chair                  │ │
│ │ NEW to the Catalogue: 14               │ │
│ │ PARCEL FOUND: the Monocle          NEW!│ │   ← gold; "the Monocle (again)" without the stamp on a repeat
│ │ COUSIN CAUGHT: Pippa Fizzwick      NEW!│ │   ← "swallowed whole" replaces "caught" when above the window
│ │ challenges 2/4 …                       │ │
```

### 6.3 The hub mission card: one line under the verdict

```
│ Best verdict: Splendid  +  comet for speed  ·  Challenges 2/4          │
│ Parcel ✓   Cousin ✓                                                     │   ← found: gold ✓; not yet: a dim "·" and no word
```

Two words and two glyphs; the compact layout (D24) shows the same line. Nothing says *where*. The
Elwynn inset's pin (D24) gains nothing.

### 6.4 The Catalogue: a family section, and Enter equips

The Catalogue page (`select/catalogue.rs`, D31) gains one group **above** the level list, always
visible, with the three outfits and three cousins. Rows are dim until found, and a found row's name is
followed by its state. `Enter` on a found row wears / pushes; `Enter` again on the Prince's own row
restores him. No other key.

```
┌─────────────────────────────────────────────────────────────┐
│  FIZZWICK'S CATALOGUE                        Esc / C closes  │
│  ─────────────────────────────────────────────────────────  │
│  ▸ THE FAMILY, AND THE POST                                  │
│     The Prince                 pushing          ╭──────────╮ │
│       the Monocle              wearing  (I)     │  bubble  │ │
│       Mirela's Bandana         · · ·    (II)    │  model   │ │
│       the Thinking Helm        · · ·    (III)   ╰──────────╯ │
│     Pippa Fizzwick             Enter: push      Pippa        │
│     Twinkle Fizzwick           · · ·            caught fair, │
│     Grand-Uncle Thrum          · · ·            Lion's Pride │
│  I  Last Call at the Lion's Pride     41 of 63  "She was     │
│  ▸ FOOD & DRINK (14)                             counting…"  │
│     …                                                        │
└─────────────────────────────────────────────────────────────┘
```

- A not-found row reads `· · ·` and its level numeral, nothing else: the numerals are the only hint,
  which is Katamari's "a present is somewhere in this stage".
- The detail pane shows the row's model when the loader can (D31's deviation: no model by name yet;
  text until then), the quip, and for a cousin whether they were caught fair.
- The bottom help line gains `Enter wears / pushes` while the cursor is in the family group.
- Completion: when all three parcels are found the group's header gains a small comet glyph and the
  fireworks play on the next hub arrival; all three cousins, the family portrait (§2).

### 6.5 Hub news (D24 *Celebration*)

A run that found a parcel or a cousin adds a line to the arrival news, after the star and before the
unlock: Fizzwick cheers and types one of §7.3. The chosen card does not move.

---

## 7. Copy (`lines.rs`; ≤ 68 characters a line; `pick` never repeats in a session)

### 7.1 PARCEL — the parcel rolled up (pop-in, bonus slot)

*Lion's Pride*
- "Found it! The post said 'lost'. The post LIED. It was in the FIRE."
- "My parcel! In a fireplace! Farley has questions to answer. Later."
- "That is my post! Unopened! Singed! A monocle, if the label is right."

*Market Day*
- "The parcel! In a CHICKEN COOP. The chickens have been sitting on it."
- "Mirela's bandana, by chicken. The post office owes me an explanation."
- "A parcel among hens. They were keeping it warm. That is not how post works."

*Stormwind*
- "The parcel! Under a BRIDGE! In a CANAL! The post THREW it. I knew it."
- "Fished out of the canal by a ball. Stormwind's post is a disgrace."
- "My helm! Under the Trade bridge! Dry, mostly. I shall think in it."

*Generic*
- "A parcel! Addressed to ME. Opened by YOU. I shall allow it, this once."
- "Post! Years late! Gnomeregan quality, Stormwind delivery."

### 7.2 COUSIN — caught fair (pop-in) / swallowed (pop-in) / the caption over their head

*Lion's Pride, Pippa*
- caught: "PIPPA! My cousin! She was counting rats. She may stop now."
- swallowed: "Was that Pippa? That was Pippa. You did not even slow down."
- caption: "Forty-one, forty-two… do NOT roll up the rats, I am COUNTING."

*Market Day, Twinkle*
- caught: "Twinkle! Cousin! Out of the barrel at last, and onto a ball."
- swallowed: "Twinkle went in with the apples. She will be fine. She is small."
- caption: "I am NOT lost. I am apple-bobbing. There is a DIFFERENCE."

*Stormwind, Thrum*
- caught: "Grand-Uncle THRUM. A dwarf. By marriage. He owes me three silver."
- swallowed: "That was Thrum. All of him. The haggling is over, I suppose."
- caption: "Three silver? THREE? For a HAT? I will roll you up myself, laddie."

*Generic*
- caught: "A RELATIVE! On the ball! Family reunions are usually worse."
- swallowed: "Someone from the family went by. Never mind. They will write."

### 7.3 NEWS — on the hub after a run that found one (Fizzwick cheers)

- parcel: "The post arrived! Years late and by katamari, but it ARRIVED."
- cousin: "A cousin, found! The family grows. The family is on a ball."
- all parcels: "ALL my post! Fireworks! I have fireworks. I always have fireworks."
- all cousins: "The whole family, assembled. Nobody is speaking to anybody. Perfect."

### 7.4 EQUIP — the Catalogue, typed in Fizzwick's window when Enter is pressed

- the Monocle: "A monocle. On the Prince. I do not own one. I am not bitter."
- Mirela's Bandana: "Very piratical. Mirela will want it back. Mirela can roll for it."
- the Thinking Helm: "The Thinking Helm! Think big thoughts. Round ones, preferably."
- Pippa: "Pippa pushes now. She is small. She is also DETERMINED."
- Twinkle: "Twinkle pushes now. Mind the goggles, they are not for swimming."
- Thrum: "Thrum pushes now. He is a dwarf. The ball will not notice. I will."
- the Prince again: "The Prince is back on the ball. Where he belongs. Push, boy."

### 7.5 CATALOGUE quips (added to `CATALOGUE_ITEM`)

- Fizzwick's Parcel: "Addressed to me, opened by you, logged by both of us."
- Pippa Fizzwick: "Pippa. She was counting the rats. She lost count. She IS the count now."
- Twinkle Fizzwick: "Twinkle, apple-bobbing. Got the apple. Also got rolled up. Mixed day."
- Grand-Uncle Thrum: "Grand-Uncle Thrum. A dwarf. By marriage. We do not discuss it. He is on the ball now, so we will have to."

---

## 8. Data shapes

### 8.1 Recipe (`design.rs`)

```toml
# One per level. The scene item it points at is the parcel; `present = true` marks it.
[present]
id = "monocle"                 # the Wardrobe key; stable, stored in saves
title = "the Monocle"
display = 5377                 # the gnome display the Prince wears for it

[[scenes]]
name = "The parcel in the fireplace"
anchor = [-9457.5, 22.3, 57.0]
indoors = true
[[scenes.items]]
model = 'World\Generic\PassiveDoodads\Christmas\XmasGift02.m2'
arrangement = "line"
count = 1
scale = 0.238                  # opens at D 0.45 (extent 0.88)
present = true                 # exactly one per recipe; design.rs asserts it when [present] exists
name = "Fizzwick's Parcel"     # the display-name override (add `name` to Item if it lacks one)

# Challenges: the two new kinds and the list fields.
[[challenges]]
id = "rats"
title = "Pied piper: every cellar rat before the first cheese wheel"
kind = "before"
species = "Cellar Rat"
before_name = "Cheese Wheel"

[[challenges]]
id = "garden"
title = "Last round: the whole beer garden inside sixty seconds"
kind = "within"
scene = "The Lion's Pride beer garden"
radius = 10
secs = 60

[[challenges]]
id = "shops"
title = "Shopping spree: every shop in the Trade District"
kind = "collect-all"
names = ["Trade District cobbler", "Trade District tailor"]   # …

[[challenges]]
id = "fair"
title = "Green city: every tree at the Cathedral tree fair"
kind = "scene"
prefix = "Cathedral tree fair"
radius = 60
```

### 8.2 Cast (`*-npcs.toml`, `npcs.rs`)

```toml
[[npc]]
name = "Pippa Fizzwick"
role = "cousin"                # new Role; behaves as a villager outside the window
cousin = "pippa"               # the Wardrobe key; stable
scene = "last call"
activity = "work"
carry = "cheese"
display = 3121
scale = 0.45                   # new, default 1.0; the generator sets it from `catch[0]`
catch = [1.33, 2.0]            # ball diameters: [opens, swallowed-above]
caption = "Forty-one, forty-two… do NOT roll up the rats, I am COUNTING."
at = [-9466.5, -4.0, 49.8]
yaw = 2.0
indoors = true
```

L3 has no cast file today; add `stormwind-npcs.toml` with Thrum alone (the guards and citizens stay
herds), and `cast = "stormwind"` in `LevelEntry`.

### 8.3 Save (`progress.rs`, `[levels.<id>]` and a new top-level table; all `serde(default)`)

```toml
[levels.lions-pride]
wins = 2
# …
parcel = true                  # found at least once
cousin = true
cousin_fair = true             # caught inside the window at least once

[wardrobe]
outfit = "monocle"             # Option<String>; a key no recipe knows is ignored, not an error
pusher = "pippa"               # Option<String>; absent = the Prince
```

```rust
// progress.rs
pub(super) struct LevelBest { /* … */ pub parcel: bool, pub cousin: bool, pub cousin_fair: bool }
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub(super) struct Wardrobe { pub outfit: Option<String>, pub pusher: Option<String> }
struct SaveFile { levels, catalogue, #[serde(default)] wardrobe: Wardrobe }

// Run (per run; merged like the Catalogue, idempotently, on pause / restart / leave / results)
pub parcel_found: bool,
pub cousin: Option<(String /* key */, bool /* fair */)>,

// Outcome
pub parcel: Option<(String /* title */, bool /* new */)>,
pub cousin: Option<(String /* name */, bool /* fair */, bool /* new */)>,

// A resource both creatures.rs and hud_models.rs read at OnEnter(Ready):
#[derive(Resource)] pub(super) struct Wardrobe { outfit: Option<u32> /* display */, pusher: Option<u32> }
```

`Campaign` gains, per level, `present: Option<PresentInfo { id, title, display }>` and
`cousin: Option<CousinInfo { id, name, display }>` from the recipe and the cast, so the hub, the
Catalogue and the wardrobe resolve keys to displays without reparsing.

---

## 9. Implementation brief (Sonnet 5.5), in build order

Everything is in `azeroll/`; no engine edits. Each step leaves the game playable and tested.

### 9.1 Step 1: the data (`design.rs`, `campaign.rs`, `progress.rs`, `npcs.rs` parse only)

- `Item` gains `present: bool` and (if absent) `name: Option<String>`; `Design` gains
  `present: Option<PresentSpec { id, title, display }>`; a recipe with `[present]` must have exactly
  one `present = true` item (a `design.rs` test per shipped recipe, like the barrier assertions).
- `ChallengeKind::{Before, Within}`; `ChallengeSpec` gains `before_species`, `before_name`, `names`,
  `species_list`, `prefix` (all `default`). `deny_unknown_fields` stays.
- `npcs::Spec` gains `cousin: Option<String>`, `scale: f32` (default 1.0), `catch: Option<[f32; 2]>`,
  `caption: Option<String>`; `Role::Cousin`.
- `LevelBest.{parcel, cousin, cousin_fair}`, `SaveFile.wardrobe`, `Progress::wardrobe()` /
  `set_wardrobe()`; the save round-trip test gains an old save without the fields and a new one.
- `campaign::LevelInfo.{present, cousin}` (from the recipe and the cast at `Campaign::load`);
  `catalogue_names` includes the parcel's name and the cousin's name.
- A `Wardrobe` resource built from the save at startup and on every `set_wardrobe`.
- Env for verification: `WOW_AZEROLL_PRINCE_DISPLAY=<id>` overrides the Prince's display (as
  `WOW_AZEROLL_COMMISSIONER` does), so the three outfit ids and three cousin ids can be eyeballed in
  Ready before any of the rest exists. Log `azeroll prince: display N (outfit|pusher|default)`.
- Re-create the DBC dump if a different id is wanted: `mpqx <data> DBFilesClient\CreatureDisplayInfo.dbc`,
  `…Extra.dbc`, `…ItemDisplayInfo.dbc`, then filter race 7 / 3 rows and print helm (`extra[8]`) and
  chest (`extra[11]`) item display ids with `ItemDisplayInfo`'s model name.

### 9.2 Step 2: the Prince's wardrobe (`creatures.rs`, `hud_models.rs`)

- `Catalog::prince` resolves, in order: `WOW_AZEROLL_PRINCE_DISPLAY`, `Wardrobe::pusher`,
  `Wardrobe::outfit`, the lowest gnome. Re-resolve on `OnEnter(Ready)` (the Prince spawns per run; the
  world Prince is despawned with the run roots, so no live swap is needed).
- `stage_prince` already copies the world Prince's display; clear its `Local<Option<u32>>` on
  `OnEnter(Ready)` so the corner follows a change.
- Test: a wardrobe with a pusher wins over an outfit; an unknown key falls through to the default.

### 9.3 Step 3: the parcel in play (`collect.rs` read-only, `level.rs`, `progress.rs`, `hud.rs`, `fizz.rs`, `sfx.rs`)

- `props::assemble` tags the `present = true` entity with a `Present` marker; a `Collected` whose
  entity has it sets `run.parcel_found = true` and writes `ParcelFound`.
- `hud.rs`: `ParcelFound` → the banner slot `PARCEL!` (gold, 2.6 s, the goal/tier slot: the later
  replaces the earlier); the rosette bloom; `sfx.rs`: `TaxiNodeDiscovered` 1519 at full level.
- `fizz.rs`: a **bonus slot**: `ParcelFound` and `CousinCaught` bypass `POPS_PER_RUN` and `POP_GAP`
  (one each per run), priority below the goal only; lines from `lines::PARCEL` / `COUSIN_FAIR` /
  `COUSIN_SWALLOWED` with the cheer voice.
- `progress::merge_finds` (the Catalogue's idempotent merge) also writes `parcel`, `cousin`,
  `cousin_fair` into `LevelBest`; `record_results` fills `Outcome.parcel / cousin` (new = the flag was
  false before this run); `verdict.rs` draws the two tally lines (§6.2).
- Shedding: a shed parcel stays found (`run.parcel_found` is sticky) and the shed entity is a normal
  loose collectible again; rolling it up twice is harmless.
- The autopilot (`auto.rs`): exclude `Present` and `Role::Cousin` from target scoring and the census
  (a `human_only` predicate next to the blocker mask), so the pacing numbers do not move.

### 9.4 Step 4: cousins (`npcs.rs`, `hud.rs` caption)

- `Role::Cousin` spawns as a villager with `scale`; the size window drives a small override in the
  state machine: below `catch[0]` the cousin never leaves `Ambient` (skip Notice/Flee transitions and
  ignore panic contagion); inside, `Notice` (EmoteWave 67 + the caption for 2 s) then `Flee`
  `Style::Circle` at 1.3× villager speed, re-fired while the ball is within 10 yd, clamped to the
  spawn storey as today; above `catch[1]`, `Cower` on sight.
- Edibility: the cousin's `Collectible.pickup` is their scaled height, so the engine's own roll-up
  rule makes "below the window" un-rollable with no special case; "fair" = `ball diameter < catch[1]`
  at `Collected`. `CousinCaught { key, fair }` → banner `COUSIN!`, sting, pop-in, the bubble with the
  creature sway (as any NPC).
- Carried: the cousin plays Cheer (68) instead of the cry pulses.
- Tests: the window transitions on a scripted ball diameter; `fair` at the boundary; the caption fires
  once per window entry.

### 9.5 Step 5: the challenges (`design.rs`, `progress.rs::challenges_met`)

- Implement `Before` and `Within` over `Run::finds` (name, species, order, play time): `Before` is
  met if the set is complete and `max(t of set) < min(t of gate)` or the gate is absent; `Within` is
  met if the scene is complete and `max(t) − min(t) ≤ secs`. `names` / `species_list` / `prefix`
  widen the existing kinds.
- Replace the three recipes' `[[challenges]]` per §5 (through the generators for L1 and L2; L3's
  recipe is generated by `stormwind-gen.py` too).
- Tests: each new kind on a synthetic `finds` list; the three shipped recipes parse and have ≤ 4.

### 9.6 Step 6: the hub and the Catalogue (`select.rs`, `select/hub.rs`, `select/catalogue.rs`, `lines.rs`)

- `CardView` gains `parcel: Option<bool>`, `cousin: Option<bool>` (None when the recipe has none);
  `hub.rs` adds one `Field::Finds` line under the verdict (§6.3), wide and compact.
- `catalogue.rs`: the family group (§6.4) above the level list: six rows from `Campaign` +
  `Progress`; `Enter` on a found row calls `set_wardrobe` and types the §7.4 line in Fizzwick's
  window; the help line changes while in the group. `rows()` stays pure and tested.
- `Celebration` gains `parcel` / `cousin` / `all_parcels` / `all_cousins` news with §7.3; the
  fireworks (art.wgsl bursts for 4 s on arrival) and the family portrait (three units on the
  commissioner stage, cheering on news) are the two completion trinkets; both read `Progress` only.

### 9.7 Step 7: the recipes (the generators, then the casts)

- `lions-pride-gen.py`: add `gift` to the model table (`XmasGift02`, 0.88); the scene *The parcel in
  the fireplace* (§3.1); drop `hearth` and `speed`, add `pillows` and `floor`; **the hearth's book
  stair is unchanged.** `lions-pride-npcs.toml`: Pippa (§4.1).
- `market-day-gen.py`: the coop as a solid-crate U with the parcel (§3.2); challenges per §5.2;
  `goldshire-npcs.toml`: Twinkle in the apple-bob queue (§4.2). Re-run the three primary autopilot
  runs: the win time must stay within the 37–39 % band (the coop is not on the bot's menu).
- `stormwind-gen.py`: *Lost in the post* under the Trade bridge (§3.3) after measuring the deck's
  clearance and the canal bed with `WOW_AZEROLL_SURVEY`; the fallback terrace placement if the bed is
  not rollable; challenges per §5.3; new `stormwind-npcs.toml` with Thrum (§4.3). Re-run one normal
  start: the 5:30–6:00 win and zero Unstuck must hold.

### 9.8 Edge cases (each needs a test or a capture)

- [ ] Free Roll finds a parcel or a cousin: the moment plays (banner, sting, pop-in), the unlock is
      saved, no results screen is needed (the merge on `OnExit(Playing)`).
- [ ] Restart (R) or Leave (L) after a find and before the results: still saved (idempotent merge).
- [ ] `WOW_AZEROLL_DEBUG_RADIUS` run: `persist = false`, nothing saved, the moment still plays.
- [ ] The parcel knocked off by a ram: still found; the loose box rolls up again silently.
- [ ] A cousin rolled up above the window: `fair = false`, unlocked, the swallowed line.
- [ ] A cousin fleeing never leaves the storey (L1 cellar) or the market (L3); never solid.
- [ ] Pusher + outfit both set: the pusher wins; the Catalogue shows the outfit as owned, not worn.
- [ ] A save whose `wardrobe` names a key no recipe has: ignored, the Prince is default, no panic.
- [ ] The L2 Prize Pumpkin captured while the coop still has its parcel: the run ends; the parcel is
      simply not found (no "missed" message anywhere; Katamari says nothing either).
- [ ] L3 at D 12+ under the Trade bridge: the ball cannot enter; the parcel is not reachable by
      contact through the deck (it is at the bed's centre, 3+ yd in); nothing stalls (not a bot target).
- [ ] Pops: a run with the goal, a parcel, a cousin and sixty seconds shows five pop-ins at most
      (3 + 2 bonus); the gap rule still applies between the bonus pops and the next budgeted one.
- [ ] Narrow aspect (0.92) and 1.78: the card's finds line and the Catalogue's family group fit;
      `PARCEL!` never touches the dial's label.
- [ ] wasm check after every step (`cargo check --target wasm32-unknown-unknown -p wenilla
      --no-default-features --features webgpu`).

### 9.9 Deferred (not in this track)

- A ball shell skin (the "pumpkin core") as a fourth reward: needs a material lane in `lod.rs`.
- The Catalogue's 3D model by name (D31's open deviation): the family rows would show the displays.
- Cousin-specific pushing animations (Thrum should shove with his shoulder).

---

## 10. Acceptance (captures and tests)

Captures under `/home/jjd/az-patches/replay/`, native, the user's ~0.92 aspect and
`WOW_AZEROLL_UI_ASPECT=1.78`. New switches: `WOW_AZEROLL_PRINCE_DISPLAY=<id>`,
`WOW_AZEROLL_FIND_TEST=parcel|cousin` (the autopilot drives to the level's parcel or cousin from the
fixture start, for captures only; off by default), `WOW_AZEROLL_WARDROBE=<outfit>,<pusher>` (seed the
resource without a save). Fixtures are generator variants as `MD_CAPTURE` is today
(`LP_CAPTURE=parcel|cellar`, `MD_CAPTURE=coop|bob`, `SW_CAPTURE=bridge|market`).

| # | capture | what must be true |
|---|---|---|
| 1 | L1, fixture r 0.25 at the hearth's book stair foot, `FIND_TEST=parcel` | the ball climbs the books into the firebox and takes the parcel at D ≤ 0.67; the frame of the moment: `PARCEL!` at 14 %, the bubble with the gift box named "Fizzwick's Parcel", Fizzwick's porthole; a second fixture at r 0.5 at the lip cannot reach it; a third at r 1.05 can |
| 2 | L2, fixture r 0.45 at the coop's gap | the ball enters, takes the parcel, exits; a fixture at r 0.6 is refused by the gap (no stall over 5 s: the bot does not target it) and takes it after eating a crate at D 1.9 |
| 3 | L3, fixture r 2.5 on the Trade canal bank | the ball rolls under the deck and takes the parcel; a fixture at r 5 cannot enter; the survey numbers (deck clearance, bed z) are in the capture folder's README |
| 4 | L1 cellar, fixture r 0.6 then r 0.7 | at D 1.2 Pippa keeps counting (Ambient) while the ball passes through her; at D 1.4 she waves, the caption shows, she circles the Colossal Swiss and is caught: `COUSIN!`, the bubble with her model, the caught line; a fixture at r 1.1 swallows her with the other line |
| 5 | L2 greengrocer, L3 Trade market | the same three states for Twinkle and Thrum; Thrum visibly taller than the stalls from the gate |
| 6 | results after #1 and #4 | the two tally lines with `NEW!`; after a repeat run, without the stamp |
| 7 | hub after #6 | the news line and Fizzwick's cheer; the card's `Parcel ✓   Cousin ✓` line; compact and wide |
| 8 | Catalogue, family group | six rows, two found; Enter on the Monocle types the equip line; Enter on Pippa; the Prince's row restores |
| 9 | Ready with `WARDROBE=monocle,` / `,pippa` / `thinking-helm,thrum` | the corner Prince and the world Prince wear the monocle; Pippa pushes; Thrum pushes (the pusher wins over the helm) |
| 10 | each outfit and cousin id in Ready via `PRINCE_DISPLAY` | six frames for the user to approve the looks; any that reads badly is swapped for another row of the same table |
| 11 | hub with all parcels / all cousins seeded | the fireworks over the planet; the three cousins behind Fizzwick, cheering on arrival |
| 12 | L1, L2, L3 primary autopilot runs on the new recipes | L1 ≤ 40 % of 300 s, L2 37–39 % of 620 s, L3 ≈ 6 min; 0 Unstuck; the bot never targets a parcel or a cousin (grep the telemetry for their names) |

Tests (`cargo test -p benilla-app --lib azeroll`): the recipe assertions (one `present` item per
`[present]`, ≤ 4 challenges, the dropped ids gone); the save round trip with and without the new
fields; the wardrobe resolution order; the cousin window state machine; `Before` and `Within` on
synthetic finds; `names` / `species_list` / `prefix` widening; `CardView` finds; the Catalogue rows
with the family group; `Celebration` with the new news; `lines::pick` on every new pool.

Not verifiable on the build host, for the user's Windows playtest: whether the six displays look
right; whether the L1 firebox window (grab it before D 0.67 or after 2.0) reads as a trick or as a
bug; whether Thrum at 3.5 yd is funny or alarming; whether the fireworks are too much on every hub
arrival (if so: first arrival only).

---

## Sources

- KeenGamer, [Katamari Damacy REROLL Royal Present guide](https://www.keengamer.com/articles/guides/katamari-damacy-reroll-royal-present-guide) and [cousins guide](https://www.keengamer.com/articles/guides/katamari-damacy-reroll-cousins-guide) [C]
- GamePretty, [We Love Katamari REROLL+ Royal Presents guide](https://gamepretty.com/we-love-katamari-reroll-royal-reverie-royal-presents-guide/) [S]
- Katamaripedia, [Make a Star 1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1) (Marcy under the kotatsu) [C]
- Wowpedia, [Winter Veil Gift](https://wowpedia.fandom.com/wiki/Winter_Veil_Gift) (the gifts exist in 1.12; the model path comes from this client's `GameObjectDisplayInfo.dbc`, rows 5254–5259) [C]
- In-repo: katamari-room-design.md (P6, P11, §5, §6), ritual-design.md (§3 pops, §4 Catalogue), ARCHITECTURE.md D21, D24, D28a, D29, D31, the L1 and L3 sections, `docs/azeroll/data/*-gen.py`.
