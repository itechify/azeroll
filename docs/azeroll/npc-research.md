# Goldshire NPCs: authentic cast, Katamari humans, animation and sound mapping

Research for populating the Azeroll Goldshire level with recognisable vanilla NPCs. Machine-readable data is in
[`data/goldshire-npcs.json`](data/goldshire-npcs.json); this file is the reading copy, the proposal and the mapping.

**Evidence grades**, same as [katamari-reference.md](katamari-reference.md):
**[C]** stated by a source I retrieved (link given); **[S]** seen only in a search snippet (page blocked fetch);
**[F]** forum or fan claim; **[I]** my inference or recommendation; **[?]** looked for and not found.
Data facts taken straight from the databases or the client DBC are marked **[DB]** or **[DBC]**.

## 0. Sources, licence, method

| what | where | licence |
|---|---|---|
| Spawns, templates, addons, waypoints, equipment, events | CMaNGOS `classic-db` `Full_DB/ClassicDB_1_12_1_z2815.sql.gz` ([repo](https://github.com/cmangos/classic-db)) | GPL-3.0 (repo `LICENSE.md`) |
| Cross-check of guid, position, movement type, `emote_state` and `stand_state` | vmangos world dump `world_full_14_june_2021.sql` from [brotalnia/database](https://github.com/brotalnia/database) (the dump the vmangos install guide points to) | the vmangos core is GPL-2.0; the dump has no separate licence, so nothing was copied from it beyond cross-check flags and the six kid positions |
| Title and zone cross-check | Wowhead Classic NPC pages (Smith Argus, Kira Songshine) | not used for data; the pages list no coordinates |
| Display ids, models, animations, sound kits | the client's own `dbc.MPQ` + `patch.MPQ` + `patch-2.MPQ` (read only, nothing copied into the repo) | Blizzard data |

The JSON holds ids, coordinates, names and flags only. It is facts, not art, so it can sit in the repo under the project
licence; the GPL-3.0 source is credited in its `meta.primary_source`.

**How it was extracted.** Every `creature` row on map 0 within 300 yd of the Lion's Pride Inn entrance
(-9459.16, 44.37, 56.95) is joined to its template, addon, equipment, waypoint rows and game-event membership. Result:
**244 spawns, 204 outside seasonal events** (the other 40 are Darkmoon Faire Elwynn, Winter Veil and Lunar Festival). The
two databases agree: every Goldshire guid matches, positions to 1 yd, with these exceptions: Kira Songshine starts at a
different point of her loop, Andrew Krighton differs by 1.8 yd, and the six kids stand at slightly different points
(CMaNGOS stacks them on one spot; vmangos spreads them within 4 yd, and the JSON keeps both) [DB].

**One thing that does not exist.** Neither database gives any friendly Goldshire NPC an `emote_state`, and only
Guard Roberts has a `stand_state` (kneeling). Smith Argus does not hammer, Farley does not polish a mug. In 1.12 they stand
[DB]. The only emote states near the inn belong to Darkmoon Faire carnies, which is out of scope. Every work, sit or
drink behaviour below is **our design**, not authenticity.

**Client DBC caution.** `patch.MPQ` overrides `dbc.MPQ` for `CreatureDisplayInfo`, `CreatureDisplayInfoExtra`,
`CreatureModelData`, `AnimationData` and `EmotesTextSound` (the last exists only there). A reader that opens
`dbc.MPQ` alone sees 8,320 displays instead of 10,534 and no voice emotes. Use the chain `patch-2 > patch > dbc`.

**Display verification [DBC].** All 71 display ids used by the 204 regular spawns exist in the client's
`CreatureDisplayInfo.dbc` (so do the seasonal ones). Two kinds:

- **Character-model NPCs** carry an `ExtraDisplayInfoID` into `CreatureDisplayInfoExtra` (race, sex, skin, face, hair,
  and baked helm/shoulder/shirt/chest/legs/boots/wrist/glove/tabard/cape display ids). All 38 distinct Goldshire ones are
  `Character\Human\Male\HumanMale` or `Female\HumanFemale`, with the full human animation set (§4).
- **Creature-model NPCs**, which for people means the kids: `Creature\HumanMaleKid` and `HumanFemaleKid`, which have **only
  Stand, Walk and Run** (§4) and no sound data.

## 1. The Goldshire cast

Positions are WoW yards (+X north, +Y west, +Z up), the survey's convention. "dist" is the distance from the inn
entrance. Display column: `id race+sex/exN` for character models (N is the `CreatureDisplayInfoExtra` id), or `id model`
for creature models; `[...]` is what the template wields (placeholder "Monster -" items from `item_template`).
The JSON has every field, including the 155 animals, monsters and bandits left out here.

| name | title | entry | dist | class | pos (x, y, z) | movement | display |
|---|---|---|---|---|---|---|---|
| Erma | Stable Master | 6749 | 7 | townsfolk | (-9466.0, 48.1, 57.0) | idle | 9257 HuF/ex6411 |
| White Stallion |  | 5403 | 9 | stable_mount | (-9468.9, 46.1, 56.9) | idle | 2410 RidingHorse |
| Priestess Josetta | Priest Trainer | 377 | 11 | townsfolk | (-9460.8, 33.1, 63.9) | idle | 1295 HuF/ex65 [crooked staff] |
| William Pestle |  | 253 | 12 | townsfolk | (-9460.3, 31.9, 57.0) | idle | 5038 HuM/ex59 |
| Palomino |  | 5406 | 14 | stable_mount | (-9472.9, 47.0, 56.9) | idle | 2408 RidingHorse |
| Michelle Belle | Physician | 2329 | 15 | townsfolk | (-9456.8, 29.4, 63.9) | idle | 1296 HuF/ex66 |
| Zaldimar Wefhellt | Mage Trainer | 328 | 16 | townsfolk | (-9471.7, 34.5, 63.9) | idle | 1294 HuM/ex64 [crooked staff] |
| Kira Songshine | Traveling Baker | 3937 | 19 | townsfolk | (-9468.4, 61.7, 56.1) | path 94 pts | 3234 HuF/ex1488 |
| Melika Isenstrider | Assistant Innkeeper | 6778 | 21 | townsfolk | (-9471.9, 27.4, 56.4) | idle | 5527 HuF/ex3681 |
| Innkeeper Farley | Innkeeper | 295 | 28 | townsfolk | (-9462.7, 16.2, 57.0) | idle | 1291 HuM/ex61 |
| Stormwind Guard |  | 1423 | 29 | guard | (-9455.4, 73.7, 57.0) | idle | 3258 HuM/ex346 / 3257 HuF/ex1518 [long sword + Stormwind Guard shield] |
| Marshal Dughan |  | 240 | 30 | townsfolk | (-9465.5, 74.0, 56.8) | idle | 1985 HuM/ex634 [Mace, Spiked Heavy] |
| Keryn Sylvius | Rogue Trainer | 917 | 32 | townsfolk | (-9465.8, 12.7, 63.9) | idle | 1297 HuF/ex67 [Thieves Blade] |
| Brog Hamfist | General Supplies | 151 | 35 | townsfolk | (-9465.3, 9.6, 57.1) | idle | 1292 HuM/ex62 [bread] |
| Barkeep Dobbins | Bartender | 465 | 36 | townsfolk | (-9460.0, 8.4, 57.1) | idle | 3266 HuM/ex1559 [bottle] |
| Stormwind Guard |  | 1423 | 37 | guard | (-9481.3, 74.4, 56.5) | idle | 3258 HuM/ex346 / 3257 HuF/ex1518 [long sword + Stormwind Guard shield] |
| Smith Argus | Journeyman Blacksmith | 514 | 43 | townsfolk | (-9456.6, 87.9, 58.4) | idle | 1288 HuM/ex57 [Tool, Wrench Small] |
| Andrew Krighton | Armorer & Shieldcrafter | 2046 | 43 | townsfolk | (-9464.1, 87.9, 58.3) | path 3 pts | 3340 HuM/ex54 [Mace, Basic Metal Hammer] |
| Toddrick | Butcher | 3935 | 44 | townsfolk | (-9466.4, 0.1, 57.0) | idle | 3236 HuM/ex1495 [Cleaver] |
| Remy “Two Times” |  | 241 | 46 | townsfolk | (-9496.3, 72.8, 56.6) | idle | 3254 HuM/ex682 [Torch] |
| Tomas | Cook | 1430 | 49 | townsfolk | (-9467.0, -4.5, 57.0) | idle | 1293 HuM/ex63 [bread] |
| Bo |  | 797 | 49 | child | (-9460.6, 94.2, 58.4) | path 12 pts | 338 HumanMaleKid |
| Corina Steele | Weaponsmith | 54 | 49 | townsfolk | (-9464.9, 94.0, 58.5) | idle | 1287 HuF/ex56 [Mace, Basic Metal Hammer] |
| Cylina Darkheart | Demon Trainer | 6374 | 50 | townsfolk | (-9467.0, -5.5, 49.9) | idle | 5092 HuF/ex3302 [Staff, Basic Red] |
| Maximillian Crowe | Warlock Trainer | 906 | 51 | townsfolk | (-9472.8, -5.3, 49.9) | idle | 3271 HuM/ex1564 [crooked staff] |
| Tharynn Bouden | Trade Supplies | 66 | 52 | townsfolk | (-9494.1, 83.9, 56.7) | idle | 1298 HuM/ex68 |
| Remen Marcot |  | 6121 | 54 | townsfolk | (-9473.4, -8.6, 49.9) | idle | 4866 HuM/ex3062 |
| Kurran Steele | Cloth & Leather Armor Merchant | 74 | 55 | townsfolk | (-9457.6, 99.7, 58.4) | idle | 1289 HuM/ex60 |
| Lyria Du Lac | Warrior Trainer | 913 | 65 | townsfolk | (-9461.9, 109.3, 57.9) | idle | 1300 HuF/ex70 [Sword, Long Basic, Shield, Large Wooden] |
| Brother Wilhelm | Paladin Trainer | 927 | 65 | townsfolk | (-9468.1, 109.0, 57.7) | idle | 1299 HuM/ex69 [Mace, Ornate Metal Hammer, Shield, Kite Metal] |
| Mark |  | 795 | 91 | child | (-9424.7, 129.1, 59.8) | wander 7 yd | 338 HumanMaleKid |
| Joshua |  | 796 | 92 | child | (-9422.3, 129.1, 59.7) | wander 7 yd | 221 HumanMaleKid |
| Helene Peltskinner | Skinner | 6306 | 138 | townsfolk | (-9380.2, -69.5, 64.5) | idle | 5365 HuF/ex3261 [Dagger, Bowie Knife] |
| Cameron |  | 805 | 140 | child | (-9373.4, -67.4, 69.3) | idle | 262 HumanMaleKid |
| John |  | 806 | 140 | child | (-9373.4, -67.4, 69.3) | idle | 338 HumanMaleKid |
| Dana |  | 804 | 140 | child | (-9373.4, -67.4, 69.3) | idle | 257 HumanFemaleKid |
| Jose |  | 811 | 140 | child | (-9373.4, -67.4, 69.3) | idle | 221 HumanMaleKid |
| Lisa |  | 807 | 140 | child | (-9373.4, -67.4, 69.3) | idle | 252 HumanFemaleKid |
| Aaron |  | 810 | 140 | child | (-9373.4, -67.4, 69.3) | idle | 338 HumanMaleKid |
| Adele Fielder | Journeyman Leatherworker | 1632 | 145 | townsfolk | (-9375.7, -74.5, 64.5) | idle | 3335 HuF/ex256 |
| Spirit Healer |  | 6491 | 168 | service | (-9341.0, 165.0, 61.6) | idle | 5233 SpiritHealer |
| Jason Mathers | Fishmonger | 383 | 175 | townsfolk | (-9378.9, -111.9, 59.0) | idle | 3273 HuM/ex1563 |
| Matt |  | 794 | 176 | child | (-9387.5, -117.4, 58.9) | idle | 338 HumanMaleKid [Item, Fishing Pole] |
| Lee Brown | Fisherman | 1651 | 178 | townsfolk | (-9382.1, -117.0, 59.0) | idle | 3272 HuM/ex1562 [Item, Fishing Pole] |
| Guard Roberts |  | 12423 | 188 | guard | (-9514.6, -135.7, 60.4) | idle, kneel | 3258 HuM/ex346 [long sword + Stormwind Guard shield] |
| Ashley Bridenbecker |  | 11996 | 266 | child | (-9318.4, 270.6, 68.1) | idle | 257 HumanFemaleKid |
| Rob Bridenbecker |  | 11994 | 282 | townsfolk | (-9309.8, 284.6, 70.6) | idle | 11930 HuM/ex8504 |
| Donni Anthania | Crazy Cat Lady | 6367 | 286 | townsfolk | (-9176.5, 0.7, 80.5) | idle | 5086 HuF/ex3300 [Cleaver] |
| Kim Bridenbecker |  | 11979 | 286 | townsfolk | (-9313.1, 291.2, 70.6) | idle | 11929 HuF/ex8503 |

Floors: z 63.9 is the inn's upper floor (the survey's `inn-floor-probes.csv` finds a floor at 63.8 under Josetta,
Michelle and Keryn). z 49.9 is below the ground floor (a cellar the survey probes never reached; they start at z 62), where the warlock
trainers and Remen Marcot stand [DB, I].

**Moving or posed ones** [DB]:
- *Kira Songshine* (3937, Traveling Baker), a 94-point waypoint loop from the inn east along the road toward
  Stormwind and back, 0 wait at every point. The first leg `(-9454.6, 66.6) → (-9337.4, 124.5)` is the useful bit
  (about 130 yd, no waits).
- *Andrew Krighton* (2046, Armorer), 3 points over 1.3 yd: waits 60 s at the anvil, steps, waits 10 s and 5 s.
  He is the only "worker" with a script.
- *Bo* (797, kid), 12 points from the smithy `(-9460.6, 94.2)` south-east to the chickens at `(-9425.6, 129.2)`, 5 s waits
  at both ends, retraced back. Mark and Joshua stand by those chickens (wander 7 yd).
- *Cameron, John, Dana, Jose, Lisa, Aaron* (805, 806, 804, 811, 807, 810): six kids by Helene Peltskinner's tannery at
  `(-9373, -66, 69.2)`. John and Dana carry EventAI that makes them follow Cameron ("follow master") [DB].
- *Guard Roberts* (12423) kneels (`stand_state` 8) at `(-9514.6, -135.7)`, the only authored pose.
- *Matt, Lee Brown and Jason Mathers*: Matt and Lee hold fishing poles (item 1117), by the south lake (177 to 179 yd).

**Not in the 300 yd radius, but you asked.** Stormwind Guard patrols with waypoints are 385 to 496 yd out
(`extended_patrols` in the JSON: 69, 19 and 110 points). Entry 68 "Stormwind City Guard" has no Goldshire spawn; Goldshire's
guards are entry 1423 "Stormwind Guard" (display 3258 male or 3257 female, long sword, kite shield, crossbow). There are
no orphans, drunks or bards in the 1.12 data.
"Remy Two Times" (241) and Remen Marcot (6121) are the closest to drunks by flavour; Barkeep Dobbins holds a bottle.

**Hostile-looking humans** (for a "tough guys" corner): 16 Defias Cutpurse and 5 Defias Bandit, character models,
closest 211 yd out; Cutpurses use display 2361 (male) or 2362 (female) and carry a dagger.

**Animals worth a cameo:** chickens beside the inn (13.8, 14.7 yd), more at the east fence; White Stallion and Palomino
beside Erma (display 2410 and 2408, `RidingHorse`); Bo's chickens; stray wolves, rabbits, deer, fawns and boars within
75 yd (`wildlife`/`critter` in the JSON).

## 2. Proposed cast for the level (33 core, 6 optional)

Principles: keep the authentic spots, so a player who knows Goldshire recognises it; cluster into scenes of 3 to 7 so
each scene is a place the ball arrives at, not a scatter; give every NPC one idle, one "life" loop and a reaction
profile. Positions are the DB's; "invented" marks anything the DB does not contain.

**A. Crossroads guard post (north of the inn, y about 74)** , stand-ground guards, a size gate.
1. Stormwind Guard `(-9455.4, 73.7)`, 3258/3257
2. Stormwind Guard `(-9481.3, 74.4)`, 3258/3257
3. Marshal Dughan `(-9465.5, 74.0)` (1985, mace; he is level 25 and the boss of the post)
4. Remy "Two Times" `(-9496.3, 72.8)` (3254, holding a torch): the loiterer, fits the sleeping drunk (Sleep loop 100)
5. Tharynn Bouden `(-9494.1, 83.9)` (1298, Trade Supplies): merchant at the stall
6, 7. Two **invented** patrol guards (same displays) walking Kira's first leg `(-9454.6, 66.6) → (-9337.4, 124.5)` and back, 4 s waits at each end

**B. Lion's Pride Inn, ground floor and yard.**
8. Innkeeper Farley `(-9462.7, 16.2)` (1291)
9. Barkeep Dobbins `(-9460.0, 8.4)` (3266, bottle)
10. Brog Hamfist `(-9465.3, 9.6)` (1292, bread)
11. Toddrick the butcher `(-9466.4, 0.1)` (3236, cleaver)
12. Tomas the cook `(-9467.0, -4.5)` (1293, bread)
13. Melika Isenstrider `(-9471.9, 27.4)` (5527, assistant innkeeper)
14. William Pestle `(-9460.3, 31.9)` (5038)
15. Erma `(-9466.0, 48.1)` (9257, stable master) with White Stallion and Palomino as props (not pickups; they are 2 yd and need a ride animation set)

**C. Inn upstairs, z 63.9** (class trainers as "adventurers"): 16 Priestess Josetta (1295, staff), 17 Michelle Belle (1296),
18 Zaldimar Wefhellt (1294, staff), 19 Keryn Sylvius (1297, thief blade). A Katamari player needs a path up; if the
ball cannot climb, they become a late-game bonus, or list them as optional.

**D. Smithy and market street (y 85 to 110).**
20. Smith Argus `(-9456.7, 87.9)` (1288, wrench)
21. Andrew Krighton `(-9464.1, 87.9)` (3340, hammer; 60 s anvil wait)
22. Corina Steele `(-9464.9, 94.0)` (1287, hammer)
23. Kurran Steele `(-9457.6, 99.7)` (1289)
24. Lyria Du Lac `(-9461.9, 109.3)` (1300, sword + wooden shield, level 12 warrior trainer)
25. Brother Wilhelm `(-9468.1, 109.0)` (1299, mace + kite shield)

**E. Traveller.** 26. Kira Songshine, the walking baker (3234): the DB loop, trimmed to the first leg plus return.

**F. Kids.** 27. Bo (338, the DB loop), 28. Mark `(-9424.7, 129.1)` (338), 29. Joshua `(-9422.3, 129.1)` (221), plus
the two chickens beside them; 30 to 33. The tannery group Cameron, Dana, John, Lisa at `(-9373, -66)` (262, 257, 338, 252),
following Cameron. These are the best first targets: no animation beyond Run, no resistance (§4).

**Optional, 140 to 180 yd out (only if the level extends there):** Helene Peltskinner (5365, skinner) and Adele Fielder
(3335), the fishers Lee Brown (3272), Jason Mathers (3273) and Matt (kid, 338, fishing pole), and Guard Roberts (kneeling).
Three of these give a "fishing" work loop (§4) that nothing else offers.

**Optional tough corner (200+ yd):** Defias Cutpurses in a small group at the road to the mines (e.g. `(-9472, 254.6)`,
`(-9465, 275.8)`, `(-9486.5, 277.4)`): they `Ram` instead of fleeing (the Hogger pattern in ARCHITECTURE.md, D12).

**Why 33.** It is enough that every part of the 60-yd inn core has someone to scare, few enough that each can keep its
own look (38 distinct human character displays exist within 300 yd, so no two core NPCs need to share a face except the two guards).
The existing Hogger/critter path already spawns display-id units (ARCHITECTURE.md D7).

**Authentic positions vs invented.** Everything in A1-5, B, C, D, E, F is a DB position. Only the two patrol guards,
the work, sit, sleep and gesture behaviours and the voices are ours.

## 3. How Katamari handles people

**The honest summary.** No source I could reach documents the reaction logic: distances, thresholds, speeds, timers.
What the sources do agree on is a three-beat shape: *ignore a small ball, run in terror from a big one, scream and
flail once caught.* Everything numeric in §5 is my design.

### 3.1 Ambient, then ignoring
- At the start of a stage bystanders "sort of just go about their business normally… not even giving a second glance to the weird
  ball of junk rolling around them" ([Destructoid](https://www.destructoid.com/experience-points-21-katamari-damacy/)) **[C]**.
- Players describe people who "just sit there at the table as I roll up their food and eventually their cats"
  ([ResetEra thread](https://www.resetera.com/threads/do-the-people-in-katamari-damacy-games-realize-theres-a-little-green-man-rolling-up-all-their-things-and-animals.473565/)) **[F]**.
  Another poster: before you can roll them up "you're just a nuisance" **[F]**.

### 3.2 Notice and flee
- "But once it's big enough, people will notice it and run away in terror, flailing their arms wildly"
  ([Destructoid](https://www.destructoid.com/experience-points-21-katamari-damacy/)) **[C]**. This is the only documented trigger: *size*, not
  proximity alone.
- "They scream, run, and sometimes wave their arms dramatically as you roll them into your katamari"
  ([Steam REROLL guide](https://steamcommunity.com/sharedfiles/filedetails/?id=3409633275)) **[C]**.
- "You'll often see people and animals run away in fear as the katamari approaches" (summarised from reviews) **[S]**.
- Animals follow the same two stages: "Living and non-living creatures will sometimes attack the katamari if it is smaller
  than them; once the katamari reaches a certain size, the creature may run away"
  ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) **[C]**.
- A middle "noticing" beat, a gaze or a freeze, is **not documented [?]**. Adding one is design (§5).

### 3.3 Voice
- Rolled-up people "scream and yell"
  ([Game Rediscoveries](https://psychopomp.com/fantasy/miscellaneous/game-rediscoveries-katamari-damacy/)) **[C]**; they
  "make funny noises upon being picked up. Usually it's a goofy shouting or laughing sound. The biker punks' cries are especially odd"
  (Destructoid) **[C]**.
- Children "laugh and wiggle" when stuck **[S]**; a "ball… filled with screaming people and flailing limbs"
  ([GameSpew](https://www.gamespew.com/2018/12/katamari-probably-disturbing-game-youll-ever-play/)) **[C]**.
- Tone: reviewers call it playful; Common Sense Media notes "no blood or gore's shown" **[S]**. The screams are
  an event on pickup and while fleeing, not a constant bed **[I]**.
- Whether the voices are a handful of shared clips or per-character is **[?]**.

### 3.4 What a rolled-up person looks like
- "Even when they get rolled up themselves, they keep flailing their arms and legs… like insects that are stuck on their backs"
  (Destructoid) **[C]**.
- "The poor child was stuck to the side of the Katamari, legs wiggling until he was buried in other stuff"
  ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) **[C]**. So the flailing is visible only
  while the person is on the surface; later layers hide them. Matches the Katamari cull rule in
  [katamari-reference.md](katamari-reference.md) §2.
- "You can still see them kicking their feet in fright (or delight, you never know)" (Game Rediscoveries) **[C]**.
- Others on the ball spin with it: "swirling round and round" (GameSpew) **[C]**.

### 3.5 Do some people resist?
- Base-game people-who-fight-back (police, sumo) are **not documented [?]**. I searched for it and found nothing
  in the sources I could reach. The sumo wrestler in *We Love Katamari* is the *player's* ball, not an enemy [S].
- What is documented: *animals and creatures* attack a katamari smaller than them and flee when it is larger (Wikipedia, above)
  **[C]**; and big moving things knock you (§3.7).
- So "armed people fight back" is our extension [I]. Recommendation in §5.

### 3.6 Keeping fleeing targets fun
- The ball outgrows pursuit: "They might run away in terror, but this thing is fast, and picking up speed the more items it devours"
  ([The Gamer](https://www.thegamer.com/katamari-damacy-is-a-horror-in-disguise/)) **[C]**. Azeroll's ball speed is
  `r × 12 / √(1 + 0.65 r)` yd/s (ARCHITECTURE.md): r 0.5 gives 5.2, r 1 gives 9.4, r 1.85 gives 15.0, r 2.8 gives 20.0, r 4 gives 25.3. A human
  runs about 8 yd/s (§5). At the radius where a person becomes edible, the ball is about twice as fast, so
  fleeing cannot be frustrating on open ground; the risk is *getting stuck*, not *being outrun* [I].
- Panic subsides: "Wait for humans to calm down, then pounce. They never see it coming" (Steam guide) **[C]**.
  So a flee is time-bounded and a retarget is rewarded.
- Crowds are handled by force: "Sometimes, you just have to bulldoze through crowds and hope for the best" (Steam guide) **[C]**.
- Cornering and walls: hitting walls at speed sheds items; the community advice is to quick-turn out of dead
  ends rather than reverse **[S]**. Nothing documents what the *fleeing person* does at a wall **[?]**.
- Chase logic, give-up distance, speed ratios: **[?]**. Design in §5.

### 3.7 Groups and scenes
- Sources describe *places*, not choreography: "a campsite where people kayak, swim, eat, and gather around a campfire",
  a family at a table whose cats you roll up ([Katamaripedia object notes](https://katamaripedia.miraheze.org/), search results; the Make a Star level pages
  I fetched, MaS3, 5 and 7, are stubs) **[S]**. The pattern is people doing a *shared activity at a prop*
  that you can scatter. Crowds, couples and school kids as named scene types are **[?]**.
- Progression: you meet "various animals, small children, and eventually fully grown people" in that order **[C]**
  (summary of reviews, e.g. [Game Rediscoveries](https://psychopomp.com/fantasy/miscellaneous/game-rediscoveries-katamari-damacy/)). A *Beautiful Katamari* FAQ line puts humans
  at "around 1m50" [F] (already in level-design-research.md). Our order: kids, then adults, then guards.

### 3.8 The big thing hits the small ball
- "If players are attacked or crash into walls or large objects, the katamari will lose items"
  (Wikipedia: Katamari) **[C]**.
- "If you do take a big knock from something larger than you, then chunks fly off your katamari, shrinking its size and flinging it far, far away"
  (Diabolical Plots) **[C]**.
- "A sharp knock by a moving object… will cause objects to fall off" (Katamari wiki) **[S]**; and sequels warn about
  moving animals, people and vehicles "too large to collect" that can collide with the katamari (Giant Bomb) **[S]**.
- Immunity window, count and direction of the lost items are **[?]**. ARCHITECTURE.md D12 already has the `Ram` contact
  (knock 7 yd/s, 0.35 s stun, sheds on a hard bump) and level-design-research.md §16 a 1.5 s immunity and 2 to 4 items
  per hit; the NPC bump can reuse both.

## 4. WoW 1.12 animations and sounds for each state

All below are read from this client. Looping vs one-shot follows the engine's own rule
(`benilla-formats/src/models/anim.rs:643`: bit 0 of the sequence flags clear = loops, set = clamps) [DBC].

### 4.1 What the models contain

| model | animation ids present |
|---|---|
| `HumanMale` / `HumanFemale` (129 ids each) | everything in the table below, plus combat, spell, swim and sit sets |
| `HumanMaleKid` / `HumanFemaleKid` | **only** Stand (5 variants), Walk, Run |
| `Chicken`, critters | not checked here |

`AnimationData.dbc` has **no Cower**: no `EmoteCower` row; id 175 `DruidCatCower` exists only on druid cat forms. There
is no "scared" or "panic" animation for humans. Also no `Emotes.dbc` row for cry, laugh-loop or cower states beyond the one-shots
below. The two **female** `EmoteCry` (77) and `EmoteChicken` (78) are 0.40 and 0.53 s stubs; the male ones run 4.3 and 3.3 s.

Key clips (male ms / female ms, `L` = loops, `1` = plays once) [DBC]:

| id | name | M / F | note |
|---|---|---|---|
| 0 | Stand | 4 variants 2.7 s / 3 variants, up to 6.7 s | L |
| 4 / 5 / 143 | Walk / Run / Sprint | 1.0 s / 0.67 s / 0.67 s | L; clip speed 2.5 / 6.94 / 11.67 yd/s, the player scales playback to the real speed |
| 11 / 12 | ShuffleLeft / ShuffleRight | 0.5 s / 0.5 s | L, in-place side-steps |
| 14 | Stun | 2.0 s | L, a stagger pose |
| 24, 26, 25 | ShieldBlock, Ready1H, ReadyUnarmed | 1 / L / L | guard poses |
| 30 | Dodge | 1.0 s | 1, a flinch away |
| 37-39, 40 | JumpStart, Jump, JumpEnd, Fall | 0.83 s, 1 s L, 1 s, 1 s L | |
| 42 | Swim | | L, paddling limbs |
| 55 | BattleRoar | 1.0 s | 1 |
| 60 / 64 / 65 | EmoteTalk / TalkExclamation / TalkQuestion | 2.0 / 2.0 / 1.8 s (F 2.7 / 2.8 / 1.8) | 1 |
| 61, 199 | EmoteEat, EmoteEatNoSheathe | 2.6 s | 1 |
| 62, 136 | EmoteWork, EmoteWorkNoSheathe | 2.0 s (F 1.7) | **L** (work loop) |
| 63 | EmoteUseStanding | 1.0 s (F 2.1) | L |
| 66 / 67 / 68 / 70 | Bow / Wave / Cheer / Laugh | 2.2 / 2.7 / 2.5 / 3.3 s | 1 |
| 69 | EmoteDance | five variants, 0.9 to 2.7 s | **L** |
| 73 / 74 / 81 / 82 | Rude / Roar / Shout / Flex | 2.7 / 3.3 / 2.8 / 2.9 s | 1 |
| 75, 114-116 | Kneel, KneelStart / KneelLoop / KneelEnd | 3.3, 1.2 / 1.6 / 1.0 s | KneelLoop L |
| 77 / 78 / 79 / 80 | Cry / Chicken / Beg / Applaud | M 4.3 / 3.3 / 4.7 / 2.7 s; F 0.4 / 0.5 / 4.5 / 3.7 s | 1 |
| 83 / 84 | Shy / Point | M 4.5 / 3.0 s; F 2.0 / 2.2 s | 1 |
| 95 | Kick | 1.0 s | 1 |
| 96-98 | SitGroundDown / SitGround / SitGroundUp | | SitGround L |
| 99-101 | SleepDown / Sleep / SleepUp | | Sleep L |
| 113 | EmoteSalute | 1.5 s (F 2.0) | 1 |
| 121 | Knockdown | 2.0 s | 1, takes the engine's base-animation lock |
| 126 | Whirlwind | | L, a spin |
| 133 / 134 | FishingCast / FishingLoop | 1.0 s / 1.5 s | cast 1, loop L |
| 185 / 186 | EmoteYes / EmoteNo | 2.7 / 3.3 s | 1 |
| 195 | EmoteTrain | 4.3 s (F 4.7) | M loops, F loops too |

### 4.2 How the engine plays them
- `EmoteAnim { entity, anim_id, seq, via_player }` (`creature_anim.rs:422`) plays any `AnimationData` id the model has as a
  one-shot over the gait. **While the unit moves it plays on the SpineLow torso overlay**, so Run plus an arms-up
  one-shot (Cheer 68, Shout 81, Roar 74, Chicken 78) gives "running away waving their arms" without new code
  (`creature_anim.rs` header; `route_oneshot`).
- `UNIT_NPC_EMOTESTATE` loops an `Emotes.dbc` row's `AnimID` as the idle (`emote_anim.rs`, `select.rs::state_emote_anim`). The
  loopable rows: 10 STATE_DANCE (69), 28 STATE_WORK (62), 173 STATE_WORK_NOSHEATHE (136), 69 STATE_USESTANDING (63), 64 STATE_STUN (14),
  93 STATE_STUN_NOSHEATHE (137), 27 STATE_READYUNARMED (25), 333 STATE_READY1H (26), 375 STATE_READY2H (27), 12 STATE_SLEEP,
  13 STATE_SIT, 68 STATE_KNEEL (stand-state poses), 313 STATE_AT_EASE, 26 STATE_STAND.
- Movement flags select the rest: `FALLING | FALLING_FAR` loops **Fall (40)**; `SWIMMING` gives the swim set (Swim 42 when
  moving forward); stand state 1/3/8 plays the sit/sleep/kneel sequences (`select.rs::Special::Pose`).
  A carried NPC (the `Carried` marker, ARCHITECTURE.md D7) has no ground clamp, so setting these flags on it is a
  zero-code way to loop a flail.
- `Knockdown` (121), `LiftOff` and `Land` take the base-animation lock (`select.rs:KNOCKDOWN`), which refuses every other base request until it
  finishes: fine for a trip, wrong for a flail.
- The torso `BodyTwist` (`twist.rs`) turns SpineLow and Head toward an aim, but only the strafe path writes it
  today. A "look at the ball" head turn would need a producer **[I]**.
- `sound/vocal.rs` is **not** the NPC voice. It is the player's UI error speech ("Not enough mana"), 68 lines from
  `VocalUISounds.dbc`. Voice emotes come from `EmotesTextSound.dbc` through `EmoteSounds` (`sound/emote.rs`) and `play_kit_ext`.

### 4.3 State mapping

| state | humans: animation | kids: animation | voice (SoundEntries kit) |
|---|---|---|---|
| **idle** | Stand 0, random variant; the engine picks among 4 (M) or 3 (F) | Stand 0 (5 variants) | none; rare `BORED` whistle (M 7952, F 7951, 6 files), `SIGH` (M 7914, F 7894), `YAWN` (M 7932, F 7931) |
| **ambient gesture** (one-shot every 6 to 20 s) | Talk 60, TalkExclamation 64, TalkQuestion 65, Point 84, Wave 67, Bow 66, Flex 82, Yes 185 / No 186, Salute 113 (guards) | none | optional `HELLO` (M 6162, F 6134), `BYE` (M 6163, F 6135), `THANK` (M 6166, F 6138): the "greeting" lines; chatter between two NPCs = alternate Talk 60 and TalkQuestion 65 with no voice |
| **ambient activity** | smith, butcher, cook, trader: STATE_WORK (62) or the no-sheathe twin (136); trainers: EmoteTrain 195; barkeep: Eat 61/199 (tankard items 2703, 13861 exist); loiterer: SitGround 96-98 or Sleep 99-101; Kira: Walk loop; fishers: FishingCast 133 then Loop 134 (pole item 1117); dance: STATE_DANCE (69) for the mambo track | Walk and Run only: wander at walk speed; Bo and the tannery group follow a leader | `WHISTLE` catcall (M 7968, F 7967) from Remy for a bit of life |
| **notice** (ball at about 20 to 40% of edible size) | stop, turn to the ball (root yaw), one of TalkQuestion 65 / Point 84 / ShuffleLeft 11 or Right 12 | stop and face (no clip) | none, or a `HELPME` mutter at low volume |
| **alarm** (about 40 to 60%) | Dodge 30 (1 s recoil), then TalkExclamation 64 or Shout 81; backing away: Walkbackwards 13 (loops, 2.5 yd/s) | step back at Walk | `INCOMING` (M 2669, F 2681, 2 files), `HELPME` (M 2668, F 2680, 2 files) |
| **flee** | Run 5 base (Sprint 143 above 11 yd/s) + torso overlay one-shot, re-fired about every 2 s: Chicken 78 (males; females only 0.5 s), Cheer 68, Shout 81, Roar 74, Applaud 80. Sprint and Run both loop | Run 5 | `FLEE` (M 2671, F 2683, 2 files each: `HumanMaleFlee01/02.wav`, `HumanFemaleFlee01/02.wav`); kids: no kid voice exists, Gnome female `FLEE` (2841, 3 files) or `HELPME` (2838) are the nearest high-pitched voices **[I]** |
| **cornered / give up** | KneelLoop 115 (kneel 114 first), Beg 79 (4.7 s hands together, the "please no" pose), or Shy 83; Stun 14 loop for a daze. There is no cower clip | Stand | `HELPME` once, then silence |
| **guard stand-ground** | STATE_READY1H 333 (loop 26) or STATE_READYUNARMED 27 (loop 25), then ShieldBlock 24 and BattleRoar 55 (1 s) when the ball is a threat; Salute 113 greeting | n/a | `CHARGE` (M 2670), `ATTACKMYTARGET` (M 2672, 2 files), `OPENFIRE` (M 2678) for a defiant shout. Then Dodge 30, and either fall back to flee or break the gate |
| **stuck on the ball** | loop one of: **Fall 40** (set `FALLING\|FALLING_FAR`), **Stun 14** (STATE_STUN 64), **Whirlwind 126**, **Swim 42** (set `SWIMMING\|FORWARD`), **Dance 69** (STATE_DANCE), plus one-shots re-fired every 1 to 3 s: Kick 95 (1 s), AttackUnarmed 16 (1 s), Chicken 78 (M). Females: prefer Fall, Stun, Whirlwind, Kick, because the female Cry and Chicken are stubs. **No death animations** | no clips: a procedural wiggle, about ±25° roll at 5 to 7 Hz around the contact normal, plus bob; kids have no Stun or Fall | on pickup one `HELPME` (2668 / 2680) or `FLEE`, then wound grunts at 1 per 2 to 4 s: player wound kits `HumanMalePlayerWound` 2942 (3 files, `HumanMaleMainWoundA-C`) and `HumanFemalePlayerWound` 2938 (4 files), or the NPC peasant set `HumanMalePeasantWound` 1290 (males); kids: Gnome female `HELPME`; `CHEER` (M 2677, F 2689) for the delighted ones. Cap the voices per second, the mixer has a bus limiter |
| **bump** (a big NPC runs into a small ball) | the NPC: `INCOMING` as it runs in, then on contact StandWound 8 or Dodge 30, or Knockdown 121 if it should trip over the ball (2 s, locks the base clip), then it stands back up | Stand, then Run away | NPC `INCOMING` before impact; at impact a `HumanMaleWound` (1259, 4 files) or the peasant wound; the ball side needs a non-human "thwump" or the existing Ram hop. The vanilla humans have no "oops" or "lol" kit: `EmotesTextSound` has no `LAUGH`, `CRY`, `COWER` or `PANIC` voice rows at all, only the 26 text emotes listed in §4.4 |

All the `.wav` files named by these kits exist in the client MPQs (checked: `Sound\Character\Human\HumanVocalMale\HumanMaleFlee01.wav`,
`...\HumanVocalFemale\HumanFemaleHelp02.wav`, the `PlayerExertions\HumanMaleFinal` wound set, the `Sound\Character\Gnome\GnomeVocalFemale` flee set, and the
emote whistles) [DBC].

### 4.4 The voiced text emotes (race 1 Human; the same ids exist for all eight races)
`BORED`, `BYE`, `CHEER`, `CONGRATULATE`, `HELLO`, `NO`, `NOD`, `SIGH`, `THANK`, `WELCOME`, `WHISTLE`, `YAWN`, `RASP`
(raspberry, 1 file), `TRAIN`, `HELPME`, `INCOMING`, `CHARGE`, `FLEE`, `ATTACKMYTARGET`, `OOM`, `FOLLOW`, `WAIT`, `HEALME`,
`OPENFIRE`, `FLIRT`, `JOKE` (the "silly" kit, files `HumanMalePissed01-06`): 26 in all. `MOO` is Tauren only.
`VocalUISounds.dbc` is the UI error set (inventory full, out of range, can't do that yet, ...): `err_outofrange02`
and `err_invalidattacktarget01` are the only plausibly comic ones, as a low-priority grumble after a bump **[I]**.

Note the voice chain: `EmotesText` row → `EmotesTextSound(text, race, sex)` → `SoundEntries` kit → file list, weighted pick. The
sound id lookups need race and sex, and the character-model NPC carries both in its `CreatureDisplayInfoExtra` row. A
creature-model NPC (kid) has neither, and `CreatureSoundData` rows 79 and 80 for the kid models are empty [DBC], so kids
need an explicit kit.

## 5. Recommendations for Azeroll

All **[I]** (design, not sourced), tuned against numbers already in ARCHITECTURE.md and level-design-research.md.

1. **Four-stage reaction, keyed on `ratio = ball radius / the NPC's edible radius`** (`Collectible.pickup`, D12):
   ignore under 0.2; notice 0.2 to 0.4 (1 s freeze + look, no fear); alarm 0.4 to 0.6 (recoil, shout, step back);
   flee 0.6 to 1.0 (run, shout every 3 to 5 s); edible at 1.0 and above, still fleeing until caught. A distance gate
   (react only within about 12 yd) keeps a far-away scene idle and cheap.
2. **Flee speed.** Walk 2.5 yd/s; flee at about 6 to 8 yd/s (the template run speed is 1.14286 × 7.0 = 8.0 yd/s;
   the Run clip is 6.94 yd/s). Never faster than 0.9 × the ball's speed at the *same ratio*; the table in §3.6 says the ball is
   already faster at the edible radius.
3. **Fleeing is bounded and steered, not a chase simulation.** Flee away from the ball with ±30° jitter, re-pick every 0.6 s,
   steer along open ground (reuse the creature path code), and **cornered ⇒ cower** (Beg 79 or Kneel) instead of pressing into the wall. After 6 to 8 s
   without the ball inside 15 yd, calm down back to idle. (The Steam guide supports "calm down, then pounce".)
4. **Guards are a size gate, not cowards.** `solid` until ratio ≥ 1; stand-ground poses at ratio 0.6; at ratio ≥ 0.9 they
   `Dodge` and run last. A hostile corner (Defias) uses the existing Ram.
5. **Flail = existing state, not a new animation.** For a carried NPC set the movement flags (Fall) or an emote state (Stun /
   Dance) plus a one-shot every 1 to 3 s; for kids use the procedural wiggle. Stop the one-shots when the NPC is buried.
6. **The bump** (a big NPC colliding with a small ball): fire `INCOMING`, then at contact reuse `collect::Ram`'s numbers
   (a hard bump: 2 to 4 recent objects shed, 1.5 s immunity). The NPC plays Dodge or Knockdown and gets up. Never delete
   the NPC; it is a nuisance, not an enemy.
7. **Verify visually before committing.** I could not run the client (no cargo, per instructions). The clip *names*
   and durations are read from the data; whether Fall, Whirlwind or Swim *look* like flailing on the chosen displays has to
   be checked in-engine. Test on 1291 (Farley), 1300 (Lyria) and 9257 (Erma) first, and all five kid displays: 338, 221,
   262 (male), 257, 252 (female).
8. **Data hygiene.** `goldshire-npcs.json` keeps CMaNGOS values. Where this proposal departs (patrol guards, Remy asleep,
   fishers working), it is in this file, not in the data.

## 6. Files and reproduction

- `data/goldshire-npcs.json`: `meta` (sources, licence, verification), `displays` (71 display ids with DBC details),
  `spawns` (204), `notable_cast`, `extended_patrols`, `kids_group_vmangos_positions`, `seasonal_event_spawns`.
- Reproduction: download the CMaNGOS dump, parse the `INSERT` rows of the tables listed in `meta.primary_source.tables`,
  filter map 0 within 300 yd, join as described in §0, read the DBC via the patch chain. Parser, DBC reader and M2 sequence
  reader were throwaway Python and are not committed; M2 v256 sequences are 68-byte records at the header's animation offset.
