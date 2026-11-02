# What makes Katamari's levels satisfying: research for Azeroll's Goldshire

Scope: Katamari Damacy (2004 PS2), REROLL (2018 port, same levels) and We Love Katamari (2005), read for
level design. Mechanics (pickup rule, growth law, camera, controls) live in
[katamari-reference.md](katamari-reference.md); this file is about layout, density, pacing and tone.
The last section turns it into rules for our map.

**Evidence grades** (same as katamari-reference.md).
- **[C]** stated by a source I retrieved (link given).
- **[S]** only seen in a search-result snippet, because the page blocked fetch.
- **[F]** fan or community claim.
- **[I]** my inference or design recommendation, not sourced.
- **[?]** looked for, not found.

**How good is the evidence?** Thin on exact layouts. No developer has published a level map, an
object-placement spec or a density number. The Game Developer postmortem says almost nothing about
layout. The best layout sources (StrategyWiki walkthroughs, the speedrun size-bottleneck list, GameFAQs)
returned 403/429 or are off-site documents I could not open. Katamaripedia stage pages are mostly stubs.
So sections 1-7 combine what is documented with what the documented facts imply. Every number in the
final principles is a **[I]** starting value to be tuned by play. That matches the developers' own method:
"the objects were added to the game and rolled up to see if they would work or not"
([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].

---

## 1. Object placement and density

- **Map and object placement were designed together, up front.** The team "carefully considered the map
  design and object placement to work together" before building, then checked memory fit and time to
  reach the goal size ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
  Layout is therefore a function of the growth schedule, not scenery that objects are sprinkled onto [I].
- **No "wallpaper".** Almost everything in the world can be rolled up eventually; clouds, guardrails and
  islands included ([Ethan Hein](https://ethanhein.com/wp/2009/katamari/)) [C]. Every prop is food at some
  size, so a level has no dead decoration, only things you are not big enough for yet.
- **Each stage uses a small slice of its area's inventory.** The house living room holds 145 item types;
  Make a Star 1 (MaS1) uses 30 of them, about 21%. Its first play offers about a dozen kinds: eyedrops, screws,
  pushpins, a crayon, cookies, buttons, lipstick, mandarin pieces, cherries, compasses, hairpins. A replay adds
  roughly 16 more kinds (mosquitoes, stamps, soy-sauce packs, chestnuts, gold coins, matches, film, chocolates)
  ([Katamaripedia MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) [C]. Two lessons: the opening
  menu is small and legible, and the object table can change between attempts.
- **Themed clusters, not random scatter.** Start-table advice: "collect everything on the beginning table
  till you grow 1 size, then roll off the table to collect the line of waiting mice" (search-result summary of a
  player guide, [PS Trophies guide](https://www.playstationtrophies.org/game/katamari-damacy-reroll/guide/)) [S]. The
  kitchen has a table with a belt on its edge, and the floor around it is "littered" with items. A stack of
  sugar cubes or caramels is the early-growth jackpot in a *Beautiful Katamari* casino stage
  ([FAQ](https://www.gamerevolution.com/?p=41286), a different game, [F]). The pattern is a dense nursery, a
  line leading out of it, then themed rooms or yards.
- **Density varies with scale.** Reviews describe tabletops dense with tiny things, then yards and streets
  with fewer, larger items. A small-scale level seems to have more objects per area than a big-scale one,
  because large objects are physically bigger [I]. Rendering is the reason for the cost cap: absorbed objects
  keep drawing, and the game swaps in low-poly shells at about 12 m, 60 m and 200 m
  ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/Katamari)) [C].
- **How much is collectible at each moment?** Not published [?]. Churchley's data says growth stays
  exponential while "the level keeps supplying appropriately sized objects" and sags where one scale runs short,
  visibly around the 10-minute mark for some players, and again at the end when the level is depleted
  ([Ross Churchley](https://rosschurchley.com/2007/exponential-growth-in-katamari-damacy/)) [F]. So the working
  target is "never out of food, never swimming in it".
- **Giants were improvised.** Where the team "couldn't come up with any really large objects" they scaled
  ordinary ones up and prefixed "giant" (giant mushroom, giant daruma)
  ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
  Reusing a model at a larger scale is a legitimate way to fill a tier.

## 2. Size progression

- **The core idea is a ladder of objects that are landmarks, then obstacles, then food.** "At first a bike or
  table is a major feature of the landscape; later it's an obstacle; still later it's an object you can pick
  up" ([Ethan Hein](https://ethanhein.com/wp/2009/katamari/)) [C]. Early on, large objects "delineate the
  landscape" and by mid-level they can be rolled over (critic cited in a search summary) [S]. A kitten that looks
  enormous at the start becomes tiny [S].
- **Graduation is the emotional peak.** The key design moment is the jump to the next object tier: "things that
  once threatened you, such as giant objects that stopped or bounced you, become absorbable"
  ([Affectionate Discourse](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)) [C].
  Reviews of REROLL describe being stopped by "seemingly insurmountable hurdles" at the start and demolishing them
  "a few moments later" ([Worth Playing REROLL review](https://worthplaying.com/article/2021/2/10/reviews/124080-ps4-review-katamari-damacy-reroll/), snippet) [S].
- **Tiers are continuous and overlap.** The scale change is "gradual and continuous" (Hein) [C]. The Make a Star
  8 stage goes from thumbtacks to cars and containers; the final stage reaches whales and a brontosaurus (Hein) [C].
  Pickup depends on the ball's diameter against each object's own threshold, with long thin things (pencils) and
  bounding boxes bending the rule (see katamari-reference.md §3) [C].
- **Gaps are avoided by giving every area several tiers at once.** The documented evidence is indirect:
  growth stays exponential in most runs, so something right-sized is almost always near (Churchley) [F]. The
  speedrun community maintains a list of "size bottlenecks": objects "it's possible to be too small for, if you
  miss too much leading up to them", some marked "hard" (no extra size needed beyond them)
  ([speedrun.com tips thread](https://www.speedrun.com/kd/forums/v69f4), snippet) [S]. Bottlenecks exist, but
  they are placed so a normal route clears them.
- **Large-per-pickup growth later.** "Smaller objects not add much value to size as you get bigger" (Steam
  thread, see katamari-reference.md §1) [F]. A player must climb the ladder; sweeping dust does not work.
- **Takahashi's own critique.** Once the katamari is big, "you instantly forget about when you were small"; the
  team wished the camera had reinforced the scale change more
  ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
  Showing a small thing you used to fear is the cheapest fix [I].

## 3. Spatial structure

- **The start is a dense, safe nursery.** MaS1 is "a relatively standard stage" in the living room, with small
  objects "all over the table and floor" ([Katamaripedia MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) [C].
  Its first play has no time limit and ends automatically at 10 cm [C]. The first level's tabletop with pushpins
  and soy-sauce packets is the standard description of the opening in reviews [S].
- **Expansion outward by room or district, in order of size.** Settings climb room → house → town → city →
  island ([search summary](https://unwinnable.com/2019/01/23/katamri-damacy-keep-going/)) [S]. Stage 12 of
  *Beautiful Katamari* recombines earlier stages as sized districts: TV station at 3 m, school at 3 m, train
  station at 6 m, rich house at 12 m, a 60 m barricade ([FAQ](https://www.gamerevolution.com/?p=41286), different game) [F].
  Districts open on a size schedule. We Love Katamari reviewers say the same: "areas open or close depending on
  the katamari's size" ([Wikipedia: We Love Katamari](https://en.wikipedia.org/wiki/We_Love_Katamari), citing the Sydney Morning Herald) [C].
- **Size gates are objects, and they are secrets.** MaS2's park has 10 cm fences; rolling through them opens
  tunnels with shortcuts ([Katamaripedia MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2)) [C].
  Walls, doors, stairs and slopes also gate: steep slopes are impassable without a dash
  ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C]. REROLL players note you get "stuck
  between the table and the ceiling" if too large indoors ([search summary of a PS Trophies guide](https://www.playstationtrophies.org/game/katamari-damacy-reroll/guide/)) [S]: an indoor level also gates from above.
- **Vertical layers.** Table → floor → kitchen table → rooftop; the cousin Johnson is on a rooftop playing
  pachinko and needs 76.9 cm ([MaS4](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_4)) [C]. Slides and
  ramps (a giraffe-neck slide in MaS3, a ramp of green fences in MaS5) lift you
  ([The Gamer](https://www.thegamer.com/katamari-damacy-reroll-all-kings-presents-locations/)) [C].
- **Landmarks are named and silhouetted.** A giant daikon with crows circling in MaS5; a smokestack with a
  ladder in MaS9; a swing tree with a cat mask in MaS3 ([The Gamer](https://www.thegamer.com/katamari-damacy-reroll-all-kings-presents-locations/)) [C].
- **Revisiting is built in.** Optional prizes per stage (the Royal Present and a cousin, e.g. Marcy at 6.8 cm in
  a 10 cm level, Dipp at 1.375 m in the final stage) sit above or beside the main route and need a larger
  ball than the stage's early game ([MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1),
  [Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon)) [C]. After the goal, "Eternal" variants of MaS4, MaS8 and the
  Moon unlock with no timer and no goal; the player just rolls the whole map ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy)) [C].
  The "return as a giant" payoff is standard. Walkthrough advice: "backtrack to the candy and burger shops
  once you're bigger" (*Beautiful Katamari*) [F].
- **Streaming by size.** Some areas load only when the ball reaches a size breakpoint (the train in the Moon
  stage) (speedrun route-notes snippet) [S]. Pacing gates and memory management coincide.

## 4. Guidance without UI

Mostly undocumented in sources [?]; what the documents do show:

- **Breadcrumb trails.** "Begin on the trail of Shogi pieces" and "a line of waiting mice" are explicit in
  player guides (*Beautiful Katamari* [F]; Katamari Damacy tips [S]). Trails run from nursery to the next
  cluster.
- **Moving set pieces as lures.** MaS2's Santa on a flying sleigh pulled by drumstick-and-magnet reindeer;
  MaS7 drops cars, boulders and sumo wrestlers from the sky; MaS8's present rides behind a truck; a balloon
  carries the MaS6 present ([The Gamer](https://www.thegamer.com/katamari-damacy-reroll-all-kings-presents-locations/)) [C].
  These pull the eye and send the player across the map.
- **Camera as reveal.** The camera zooms out automatically once the ball "occupies the entire screen"
  ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. Each zoom
  reveals what is now in range; the game never says "go there" [I].
- **Sightlines as the main guide.** Larger objects "delineate the landscape" (critic summary) [S]. A big
  landmark seen early is a promise of later food [I].

## 5. Storytelling and humour

- **Set pieces tell a gag.** MaS3's tree with a swing shakes out "apples, baseballs, and bugs" until the present
  falls ([The Gamer](https://www.thegamer.com/katamari-damacy-reroll-all-kings-presents-locations/)) [C]. MaS7's sky of falling sumo wrestlers is absurd
  on purpose.
- **Juxtaposition by scale.** Pushpins on a table beside a cat the size of a building; a school full of children
  you will one day roll [I]. Reviewers say the first human pickup is funny ("giggled when they first picked up a
  human", [Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
- **Everything is cheap, flat and readable.** Objects are a few flat-coloured polygons; "part of the fun is
  seeing how each object is translated" (Hein) [C]. Caricature beats fidelity.
- **Tone.** Developer pillars were reported as novelty, humour, ease of understanding and joy (video-essay
  summary, [S]). The sound director humming in the title sequence was a joke "players came to like", and
  combos pay out in fireworks, confetti and stars rather than numbers
  ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **The collection log.** Each stage has two collectables (a Royal Present and a cousin) and the log screen
  adds "lighthearted learning about Japanese food" ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy), [Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
  Item names are part of the joke [I].
- **Dark undertone.** Takahashi: "when these objects are rolled up and absorbed by the Katamari, they're gone.
  Then I felt empty" ([GDC 2009 write-up](https://www.gamedeveloper.com/game-platforms/gdc-keita-takahashi---the-complete-gdc-lecture)) [C]. Not something we need to copy, but it explains
  the soft, apologetic tone.
- **People reacting.** Documented only as "can be rolled up", with a funny first pickup. Screaming, fleeing
  and reaction animations are expected but not sourced [?].

## 6. Pacing

Level table: see katamari-reference.md §9. In summary:

- **Goal/start ratio rises with the stage:** 2× (MaS1, 3-4 min) up to 300× (the Moon, 25 min). Takahashi's
  "ultimate goal" is to take the 1 m Moon-stage ball past 500 m ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
  Within a stage, time limits are tuned to the goal size rather than to the area travelled [I].
- **Why a timer:** "I couldn't figure out how to make the game fun without a time limit" (postmortem) [C];
  later, "a formula that somehow betrayed my vision" (GDC 2009 write-up) [C]. Time pressure is "the only other
  real obstacle" there.
- **Curve shape:** roughly exponential in time (log chart straight), sagging when right-sized objects run out
  mid-level or at the end (Churchley) [F]. Minute-by-minute it feels like a steady series of "doublings".
- **Climax = a crossing, not a boss.** Late thresholds jump from individual buildings to city blocks, then
  mountains, then land masses; the author calls the crossing "a hugely pleasing climax"
  ([Affectionate Discourse](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)) [C]. *Beautiful Katamari*:
  "less than a minute from 500 m to 1 km" once mainland pieces start landing (FAQ, different game) [F].
- **Final size jump.** The last stretch rewards you with the biggest per-pickup gains of the level, because
  the objects are largest and the ball is biggest. The cooldown continues past the climax (credits roll over the
  ball), per Affectionate Discourse [C].
- **Post-goal freedom:** Eternal maps and "keep rolling" for score after the goal (Diabolical Plots) [C].
- **We Love Katamari variants:** the same map reused with new start sizes and objects (one German review
  complained of recycling, snippet [S]), plus new goals: sumo wrestler rolling food, snowman head, collecting
  children for a school ([Wikipedia](https://en.wikipedia.org/wiki/We_Love_Katamari)) [C]. Replaying a place
  with a different size window is cheap content.

## 7. Hazards

- **No enemies, no death.** Hitting something larger knocks chunks off, shrinking the ball
  ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C]. REROLL: "Each time there's a
  decent collision, objects fly off the katamari and it shrinks" (PlayStation LifeStyle, see
  katamari-reference.md §5) [C].
- **Animals harass the small.** Cats chase and knock things off a small katamari, then flee once it's big enough
  to roll them up ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C]. A REROLL guide
  says "avoid the cat. It's not your friend yet" ([Steam guide](https://steamcommunity.com/sharedfiles/filedetails/?id=3409633275), snippet) [S];
  a cat sits beside the MaS3 slide ([PS Trophies](https://www.playstationtrophies.org/game/katamari-damacy-reroll/guide/), snippet) [S].
  MaS2's living room has the cousin Havana running around the kotatsu, along with mice and rats
  ([MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2)) [C]: moving things
  sit in the nursery-to-first-expansion zone.
- **People as obstacles.** "Around 1m50cm you can send humans flying, and before that, pets are the targets"
  (*Beautiful Katamari* FAQ, different game) [F]: the pickup order is pets first, people later.
- **Large static objects are the other hazard.** Too-big objects bounce you harmlessly early on and can strip
  objects later (Diabolical Plots; katamari-reference.md §5) [C].
- **Not documented:** how many objects a cat knocks off, chase speed, give-up distance, stun length [?].

---

# Principles for Azeroll's Goldshire

Units are yards, matching `level.rs`: the ball starts at r=0.25, an object is edible when its pickup size
`p` ≤ 0.65·r (`COLLECT_RATIO`), Hogger has p=4 so the ball needs r ≈ 6.15, and the target curve is
r ≈ 0.5 / 1.2 / 2.5 / 4.5 at 60 / 180 / 300 / 420 s with Hogger edible by 480-540 s (ARCHITECTURE.md). All
numbers below are starting values to tune, **[I]**. Edible-at radius is `p / 0.65`.

**Tier ladder.**

| rung | p (yd) | edible at r | Goldshire examples |
|---|---|---|---|
| 1 | 0.10-0.16 | 0.15-0.25 | coins, mugs, spilled bread, apples, forks, candles |
| 2 | 0.20-0.30 | 0.30-0.46 | plates, wheels of cheese, cabbages, bottles, bundled herbs |
| 3 | 0.40-0.60 | 0.60-0.92 | barrels, crates, chickens, fox, pumpkins, stools |
| 4 | 0.85 | 1.3 | sheep, hay bales, small sacks, benches |
| 5 | 1.2 | 1.85 | cows, wheelbarrows, stall tables |
| 6 | 1.8 | 2.8 | horses, handcarts, scarecrows, stall frames |
| 7 | 2.6 | 4.0 | bears, wagons, haystacks, fence segments |
| 8 | 4.0 | 6.15 | Hogger |

Successive rungs are about 1.4-1.5× apart, so the ball spends 15-25 s of rolling per rung when the curve
holds. Rungs 7-8 need extra items between them: huts, large trees and the gnoll palisade (§3) fill 3.0-4.0.

1. **Design for the growth schedule, not the scenery.** Fix the radius-at-time curve first (0.5 / 1.2 / 2.5 /
   4.5 at 60/180/300/420 s), derive which rung is "meat" at each time, then place that rung where the player
   will be. The developers planned map and object placement together for the same reason (§1).
2. **Make the Lion's Pride Inn the nursery: 0-25 yd of START, rungs 1-3 only, no hostile creature within 30 yd.**
   Spilled food and tableware (mugs, loaves, plates, cheese) on tables, steps and the road. The first 30 s
   needs about 60-80 rung-1/2 items in reach and nothing the ball cannot bump through. Goal: r 0.25 → 0.5 by 60 s.
3. **Composition for every area's design radius `r_a`: ≈30% trivial (p ≤ 0.25·r_a), 35% meat (0.25-0.5·r_a), 10%
   "just edible" (0.5-0.65·r_a), 20% blockers (0.65-1.3·r_a), 5% landmarks (> 2·r_a).** Blockers are the next tier
   in plain view; landmarks are what you will graduate through later. This makes ~75% of what you see edible
   and ~25% promising.
4. **Cluster into scenes of 5-12 items with one anchor.** Anchors are 1-2 rungs above the meat; items are
   arranged as a story. Example: an inn table (anchor, rung 5-6) + 4 stools (rung 3) + 4 mugs + 4 plates + 2
   loaves + a stew pot (rung 2-3). Place scenes 8-15 yd apart in the nursery, 15-30 yd on the road, 30-60 yd on
   farms. Between scenes, 15-25% of area is scatter, never zero.
5. **Keep three adjacent rungs within 20 yd of the player at all times.** One edible now, one edible in ~20-30 s,
   one that bumps you. Test with the autopilot log: a window of 10 s with fewer than 6 edible pickups is a
   gap to fill.
6. **Lay trails of 8-15 same-rung items spaced 2-3× the item's `p` apart, ending in a scene.** Spilled grain and
   bread crumbs from the inn door to the coop (≈ 60 yd); apples shed from a cart along the road toward the
   market stalls; coins from a tipped merchant sack along the road toward the forest edge. A trail's rung
   should be the meat rung of the area it leads to.
7. **Radial areas (all from START, along the Goldshire → Hogger heading, ≈ 250 yd):**
   - 0-25 yd inn yard and road: rungs 1-3 (nursery).
   - 20-70 yd market stalls, merchant goods, crates, barrels and the well: rungs 2-5.
   - 60-130 yd chicken coop, pigs, a fenced farm, vegetable rows: rungs 3-5.
   - 100-190 yd pastures and farmsteads: sheep, cows, wagons, haystacks: rungs 4-7.
   - 170-250 yd forest edge: trees, rocks, bears, deer, gnoll huts: rungs 6-8.
   - 230-270 yd Hogger's camp: huts, barrels, crates, cooking fire, gnolls, then Hogger.
   Edges overlap by ≥ 20 yd so the player is always in two bands.
8. **Gate with objects, never invisible walls. Four gate types:** (a) fence rings around the coop and farm
   built of rung-4/5 segments (p 0.85-1.2, edible at r 1.3-1.85), so a small ball sees the yard through the
   rails but cannot enter; (b) a wagon or stall line across the road (rung 6-7) between market and farm; (c)
   large animals (cow, horse, bear) idling in the pasture approach; (d) a dense tree/bush wall at the forest
   edge (p ≥ 2.6). Each gate sits where the ball is one rung too small and shows food on the far side.
9. **Give each gate a bypass that only costs time.** A 2-yd gap at the end of a fence ring, a gap behind the
   market stalls, a ditch path. Small balls reach the next area with a detour of 15-20 s; a fast route opens
   when the gate becomes edible. This echoes MaS2's 10 cm fences and tunnels (§3), and keeps wedging out of
   the pacing runs (ARCHITECTURE.md §Outstanding).
10. **Keep a graduation set in sight in every area.** At least 3 landmark-sized objects (chicken coop, cow,
    the Inn's sign, stall tables) the player has seen and bounced on that become edible about 90-120 s later.
    Mark each landmark with a distinct silhouette so the player remembers it. Announce the crossing with a
    banner or sound ("Critters" etc. in `TIERS` are the hook).
11. **Ratio of edible to blocking within sight changes by phase:** 4:1 in the nursery, 3:1 mid-run, 2:1 in the
    forest and camp. Late-run the player is happy to bounce because the prize is visible.
12. **Don't leave a size valley between rungs 5 and 7.** The existing passive bands (sheep 35-130 yd, cows
    50-160, horses 70-200, bears 90-240) already overlap. Add wagons and haystacks at 100-200 yd, and hay
    bales and stalls at 40-130 yd, so the 1.8-2.6 range has non-animal food. Animals move; a gap in that tier
    is fatal if they flee out of reach (see 16).
13. **Vertical layers within the town.** Floor/road → tables and stalls (0.8-1.2 yd) → stacked crates, barrel
    towers and wagon beds (1.5-2.5 yd) → porch roofs, hay lofts and cart-tops (accessible by ramps made of
    planks/hay ramps). The ball should reach the 2nd layer around r ≈ 1.3-1.8, and see the 3rd layer's prizes
    (a stacked tower of barrels, a tray of tankards on a roof ledge) earlier.
14. **Guidance by sightline, trail and motion, not arrows.** Always a bright landmark visible at each radial
    boundary (the smoke from Hogger's fire, a tall watch fire or a banner). One moving set piece per band: a
    cart rolling along the road, a chicken chase, a farmhand herding sheep. Use the camera zoom-out tiers
    (1.2 / 3 / 7 / 13 / 20 yd diameter) to open up a new view at each crossing.
15. **Scenes with a joke, each carrying a name in the collection log:**
    - the inn's spilled feast, with a drunk patron asleep in the soup ("Unconscious Patron");
    - the coop breakout, with chickens fleeing a spilled grain sack;
    - the merchant's goods stall overturned, with the merchant shouting;
    - a farmer's scarecrow wearing a tabard;
    - a cart of cheese guarded by a bored guard.
    Collection names: "Suspiciously Large Cheese", "Half-Eaten Stew (Barkeep's)", "Hogger's Lunch". Keep each 2-5
    words, one joke, WoW-flavoured. Show 2-3 recently collected names on the HUD (the "Readable doodad names"
    task in ARCHITECTURE.md).
16. **Hazards are a window, not a wall: harassers (dogs, boars, wolves) appear when the ball is at
    0.3-0.8× their `p` and stop being threats at r ≥ p/0.65.** Chase speed ≤ 1.1× ball speed; give up after
    6-8 s or 25 yd; each hit sheds 2-4 recent objects (≈ 5-10% of volume) and grants 1.5 s immunity; they
    never shrink the ball below its previous tier floor. When edible, switch to flee with a 1 s "notice" beat
    and a visible reaction, so graduation reads. Keep the first hazard ≥ 30 yd from START (one dog by the
    coop, about 60-90 s into the run).
17. **People are mostly scenery that panics, not enemies.** NPCs flee or cower when the ball is bigger than
    half their `p`; they are edible at rung 5-6 (r ≈ 1.85-2.8). Guards at the road are an exception: a 3-guard
    post across the road is a size gate at rung 7 (r 4.0).
18. **Pace for three phases, targets on the existing curve:** (a) 0-120 s: nursery and market, r 0.25 → 0.9,
    many small pickups (~150) for the pickup-feel; (b) 120-360 s: farms and animals, r 0.9 → 3.2, the ladder
    of rungs 3-6, fewer pickups (~100) with bigger visible jumps; (c) 360-540 s: forest edge to Hogger's
    camp, r 3.2 → 6.15, the largest chunks (≈ 40-60 pickups). A doubling should take ≈ 90-110 s while food
    lasts; a 10-minute limit leaves ≈ 60-120 s slack for detours and bumps.
19. **Hogger's camp is the climax: the last 40 yd contains the highest-yield food of the level.** Huts, barrels,
    campfire, gnoll guards and logs (rung 7, p 2.6-3.5), plus trees (p ≥ 4) as a final graduation wall. The
    player should cross r ≈ 5 on arrival, bounce off Hogger's guards once, then roll them (r ≥ 4/0.65=6.15 for
    Hogger himself). Hogger flees when edible, then is caught; the camera zooms out on the roll-up.
20. **After the win, keep rolling.** Give an "Eternal" free-roam state with no timer inside the 520 yd leash,
    where trees, wagons and Goldshire buildings (D5 extension in ARCHITECTURE.md) are the new tiers. It also
    doubles as a playground for re-checking the gating (§3 revisit).

---

## Sources

- Game Developer: [Postmortem: The singular design of Namco's Katamari Damacy (2004)](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)
- Game Developer: [Takahashi: The Complete GDC Lecture (2009)](https://www.gamedeveloper.com/game-platforms/gdc-keita-takahashi---the-complete-gdc-lecture)
- Katamaripedia: [Katamari Damacy](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy), [MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1), [MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2), [MaS3](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_3), [MaS4](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_4), [Make the Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon), [stage list](https://katamaripedia.miraheze.org/wiki/List_of_Katamari_Damacy_Stages)
- The Gamer: [All Royal Presents in REROLL](https://www.thegamer.com/katamari-damacy-reroll-all-kings-presents-locations/)
- Ross Churchley: [Exponential growth in Katamari Damacy](https://rosschurchley.com/2007/exponential-growth-in-katamari-damacy/)
- Ethan Hein: [Katamari](https://ethanhein.com/wp/2009/katamari/)
- Affectionate Discourse: [Katamari Damacy: The Definitive Review](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)
- Diabolical Plots: [Niche Games: Katamari Damacy](https://www.diabolicalplots.com/tag/katamari-damacy/)
- Wikipedia: [Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy), [We Love Katamari](https://en.wikipedia.org/wiki/We_Love_Katamari)
- Gamerevolution: [Beautiful Katamari FAQ](https://www.gamerevolution.com/?p=41286) (a different game; used only for district/size-schedule patterns)
- Search-snippet only (page blocked): [speedrun.com tips thread](https://www.speedrun.com/kd/forums/v69f4), [PS Trophies guide](https://www.playstationtrophies.org/game/katamari-damacy-reroll/guide/), [Steam guide](https://steamcommunity.com/sharedfiles/filedetails/?id=3409633275), [Worth Playing REROLL review](https://worthplaying.com/article/2021/2/10/reviews/124080-ps4-review-katamari-damacy-reroll/), [Unwinnable](https://unwinnable.com/2019/01/23/katamri-damacy-keep-going/)

**Not reached:** StrategyWiki walkthroughs (403), the speedrun size-bottleneck document, GameFAQs, Takahashi's
GDC 2005 slides, and L. E. Hall's Boss Fight Books monograph, which draws on new interviews with Takahashi
and is the likeliest place for statements about how areas and clusters were composed.
