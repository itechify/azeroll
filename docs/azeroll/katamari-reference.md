# Katamari Damacy (REROLL) mechanics reference

Research for mimicking the katamari in Azeroll. REROLL (2018) is a Unity port of the 2004 PS2 game with the same mechanics ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)).

**Evidence grade, used on every claim.**
- **[C]** Stated by a retrieved source (link given).
- **[S]** Only seen in a search-result snippet, because the page itself blocked fetch (403/402). Treat as weaker than [C].
- **[F]** Community or fan claim, unverified.
- **[I]** My inference or design recommendation. Not sourced.
- **[?]** Looked for and not found.

**The big caveat.** No source I could reach contains the game's real constants. There is no decomp or datamine write-up for the size formula, the pickup ratio, knockback numbers or camera curves. The Noclip effort that extracted the models reportedly found an object table with "four other values" per object but published no values ([Kotaku](https://kotaku.com/modders-have-finally-cracked-katamari-damacy-wide-open-1834284840)). Everything numeric below is either a level-table fact (solid) or a fan estimate (weak). Treat all constants as things to tune by play. The developer did exactly that: "We made adjustments using a trial-and-error method" ([Game Developer postmortem, Takahashi](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)).

---

## 1. The core versus the katamari's size

- **Displayed size is the diameter, driven by accumulated volume.** Rolling things up increases the volume, but the reported size is the diameter ([Dromedary and a half](https://raphael-proust.github.io/games/katamari-damacy.html)) [F]. So `diameter = cbrt(volume)` up to a constant is the natural model [I].
- **The game uses a "linear-cube law" feel.** One analysis estimates 10 thumbtacks for 5 cm to 10 cm but about 80 for 10 cm to 15 cm ([same source](https://raphael-proust.github.io/games/katamari-damacy.html)) [F]. Pure volume summing predicts only about 27 tacks for the second step, since 15³-10³ = 2375 against 10³-5³ = 875. So either the author's counts are loose or small objects are discounted as the ball grows.
- **Small objects are discounted.** A Steam post says the game has a formula where "smaller objects not add much value to size as you get bigger" ([Steam thread](https://steamcommunity.com/app/848350/discussions/0/3774483849431082336)) [F]. Takahashi's team deliberately used a simplification rather than real physics ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **Growth is roughly exponential in time.** Radius growth rate is proportional to current radius, and four let's-plays plot as a straight line on a log chart. It sags when the level runs out of right-sized objects ([Ross Churchley](https://rosschurchley.com/2007/exponential-growth-in-katamari-damacy/)) [F].
- **Count matters less than size.** "Many small objects may not pass a star's size requirement" ([Katamaripedia: Katamari](https://katamaripedia.miraheze.org/wiki/Katamari)) [C].
- **Does the core visibly grow? Not documented.** No source discusses the core mesh. The game must scale the ball's visual and collision size to the displayed diameter, since the Prince's 5 cm ball becomes 300 m ([Katamaripedia: Make the Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon)) [I]. Whether that is a literal core scale or a hidden sphere is unknown [?].
- **Collision radius versus stuck objects.** Sources never state it. Protruding pencils change how the ball rolls ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C], and "we've ignored the rules of physics and allowed for any large object to stick out" ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. Combined, the likely design is a sphere sized to the diameter, with protruding objects handled as a special lumpy case [I].
- **Fan-clone reference model.** `r = (3V/4π)^(1/3)`, with the sphere collider radius growing as volume is added ([Klonamari](https://github.com/UnderGear/Klonamari)). This is a clone, not the original game [F].

## 2. Object attachment

- **Attach on contact.** "Objects that touch the katamari become attached and gain its adhesive ability" ([Katamaripedia: Katamari](https://katamaripedia.miraheze.org/wiki/Katamari)) [C]. Attached objects themselves become pick-up surfaces, so the clump grows irregularly [C].
- **Contact point and orientation.** Not documented [?]. Fan reconstructions record the contact point as a local offset on the ball and parent the object there, so orientation is simply preserved ([Bevy/Rapier jam write-up](https://whoisryosuke.com/blog/2023/making-katamari-for-bevy-game-jam/)) [F]. Sinking depth in the original is unknown [?]. Sinking each object in by some fraction of its own size is a sensible choice [I].
- **Rendering cost and culling.** "Rolled-up objects are absorbed into the katamari to reduce CPU load, and their memory is deleted once they're no longer visible." Re-showing them after a crumble was an unresolved problem, which is why shrinking was cut ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. The fan wiki says the series relies on low-poly models to offset pickup-count cost ([Katamaripedia: Katamari Damacy](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy)) [C].
- **Low-detail replacement.** "Low-poly models replace the katamari's appearance at set sizes (12 m, 60 m, 200 m, and higher, depending on the game) to suggest more items than are actually attached" ([Katamaripedia: Katamari](https://katamaripedia.miraheze.org/wiki/Katamari)) [C]. Exact cap on rendered stuck objects is unknown [?].
- **Core visibility when big.** Not documented [?]. Since buried objects are dropped from rendering and a low-poly shell takes over, the core is effectively never seen after the first few objects [I].
- **Stuck-object lifecycle in a clone.** Collected objects keep colliders at first and lose them once the ball is large enough ([Klonamari](https://github.com/UnderGear/Klonamari)) [F]. Fine for us as a pattern [I].

## 3. Pickup rule

- **Rule of thumb.** Smaller objects stick, larger ones are obstacles ([Wikipedia](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C]. The game "uses size, weight, and surface area" ([same](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C].
- **Size is the bounding box, empty space included.** "Objects are drawn inside a bounding box which tells the game how large the object is, including any empty space." So an open umbrella is harder to pick up than a car, and a pencil can be easier than a same-sized eraser ([Katamaripedia: Katamari Damacy](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy)) [C].
- **Ratio.** No official number found [?]. One unnamed review snippet says anything roughly half the ball's size adheres (source not identified) [S]. A fan design note suggests sticking only objects at least 40-50 percent smaller ([Construct forum](https://www.construct.net/en/forum/construct-classic/help-support-using-construct-38/making-2d-katamari-damacy-33376)) [F]. A 40-60 percent starting guess, then tune [I].
- **Per-object required size exists.** Katamaripedia lists the katamari size needed to roll up cousins: Marcy 6 cm 8 mm ([MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)), Johnson 76 cm 9 mm ([MaS4](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_4)), Dipp 1 m 37 cm 5 mm ([Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon)) [C]. That confirms the game compares the katamari's diameter against a per-object threshold, but the threshold-to-object-size ratio is not published.
- **Pickup is inconsistent at the edges.** REROLL reviewers note objects that look too big sticking while smaller ones bounce, and say this was in the original ([PlayStation LifeStyle](https://www.playstationlifestyle.net/2020/11/27/katamari-damacy-reroll-review/)) [C]. This matches a bounding-box-based threshold with some shape or weight modifier [I].
- **Long, thin objects.** Slender items like pencils "can be picked up even when wider than the katamari, and they change how it rolls until more items are added" ([Wikipedia](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C]. Pencil size in bounding-box terms would say "too big", so the size test must use some other measure such as thickness or surface area [I].
- **Too-large objects.** They bounce you, and the attempt can knock objects loose ([Katamaripedia: Katamari](https://katamaripedia.miraheze.org/wiki/Katamari)) [C]. Partial pickup is not documented [?].
- **Size gates levels.** Fences in Make a Star 2's park are 10 cm tall and gate shortcut tunnels ([MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2)) [C].

## 4. Lumpy rolling

- **Confirmed behavior.** Protruding items make the ball "roll in a large arc," which makes control unwieldy and costs time ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. The ball becomes lopsided and harder to roll until balanced by more items ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
- **Mechanism.** Not documented [?]. The developers' own phrase "ignored the rules of physics" suggests a hand-tuned effect rather than real contact simulation [I]. A fan clone flags irregular objects and makes the ball bounce unevenly, keeping their colliders on until the ball is big enough to ignore them ([Klonamari](https://github.com/UnderGear/Klonamari)) [F].
- **Reasonable mimic.** Compute an "eccentricity" from stuck objects' farthest reach beyond the sphere, then wobble the ball's roll speed/heading and bump the ground contact in proportion, fading as the ball grows [I].

## 5. Collisions and losing objects

- **What happens.** Objects fall off after "a sharp knock from a moving object," and also when trying to roll up something too large ([Katamaripedia: Katamari](https://katamaripedia.miraheze.org/wiki/Katamari)) [C]. Colliding at high speed knocks items off and slows progress ([Wikipedia](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C]. You cannot die; chunks break off and the ball shrinks ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
- **REROLL.** "Each time there's a decent collision, objects fly off the katamari and it shrinks in size" ([PlayStation LifeStyle](https://www.playstationlifestyle.net/2020/11/27/katamari-damacy-reroll-review/)) [C]. Sequels punish less ([StrategyWiki / forum summary](https://strategywiki.org/wiki/Katamari_Damacy/Gameplay)) [S].
- **Shrinking is partial by design.** The planned crumble was only "minor," cut for memory and absorbed-object reasons ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. Objects lost are therefore a visible subset, most likely detached from the impact side [I].
- **Animals.** Cats chase and knock things off a small katamari, then flee once it is big enough to roll them up ([Wikipedia](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C].
- **Numbers.** How many objects, which ones, and the speed relationship: not published [?]. The stun (a brief loss of control after a hard hit) is not described in any retrieved source [?]. A speed x relative-size scaled count is a safe design [I]. A fan clone detaches objects only above a velocity threshold and shrinks mass and radius accordingly ([Klonamari](https://github.com/UnderGear/Klonamari)) [F].

## 6. Controls and movement

- **Dual-stick tank layout.** Both sticks forward or back moves; one stick turns; opposite directions turn faster ("tank controls"); "Battlezone"-like ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari), [Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C]. Analog magnitude per stick gives soft or hard turns, and steady circling lets you drift ([Steam thread](https://steamcommunity.com/app/848350/discussions/0/4633736978453499873)) [F].
- **Forward is fastest.** Rolling forward is faster than sideways or backward ([same thread](https://steamcommunity.com/app/848350/discussions/0/4633736978453499873)) [F].
- **Dash.** Rapidly tap both sticks up-and-down together ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/), [Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C]. Speedrunners call "snaking" (dash, then alternate slight left/right turns) the key tech because it holds speed longer than holding forward, and say dashing may not help until runs are near 40 minutes ([speedrun.com forum](https://www.speedrun.com/kd/forums/v69f4)) [S].
- **Quick turn.** Press both sticks in (L3+R3), flipping the Prince 180 degrees around the ball ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari), [StrategyWiki: We Love Katamari controls](https://strategywiki.org/wiki/We_%E2%99%A5_Katamari/Controls)) [C/S]. Triggers also flip on the default REROLL pad scheme ([Steam thread](https://steamcommunity.com/app/848350/discussions/0/4633736978453499873)) [F].
- **Mass, momentum, slopes.** The ball has weight and obeys gravity; steep slopes can be impossible to climb without a dash ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C]. Climbing ability depends on size; in a fan clone, step-up height is a function of radius ([Klonamari](https://github.com/UnderGear/Klonamari)) [F].
- **Speed scaling with size.** No source states it [?]. Level pacing implies absolute speed must scale with diameter, or a 300 m ball would crawl [I].
- **PC keyboard (REROLL).**
  - Two four-key clusters stand in for the sticks, WASD and a second cluster (IJKL as best I can tell). Both are needed to roll; one alone only turns ([Steam thread](https://steamcommunity.com/app/848350/discussions/0/1742227898976824058)) [F].
  - An "easier/simple controls" option maps direct movement (W forward, D right) with the left stick steering and the right stick controlling the camera ([same](https://steamcommunity.com/app/848350/discussions/0/1742227898976824058), [Steam thread](https://steamcommunity.com/app/848350/discussions/0/4633736978453499873)) [F].
  - Quick turn is reported as left Shift plus Ctrl ([Steam thread](https://steamcommunity.com/app/848350/discussions/0/1741103267260535746)) [F].
  - Players broadly call keyboard play clunky and recommend a controller. Launch-era reports say keys can't be rebound ([Steam](https://steamcommunity.com/app/848350/discussions/0/1741103267260535746), [search summary](https://steamcommunity.com/app/848350/discussions/0/1742227898976824058)) [F/S].
  - No dash key mapping was found [?].

## 7. Camera

- **Placement.** Locked behind the ball, looking over its crest (review snippets surfaced by search, exact review not confirmed, e.g. [HonestGamers](https://www.honestgamers.com/2911/playstation-2/katamari-damacy/review.html)) [S]. Camera and steering share input, so turning is also camera control; one stick forward and the other back orbits the Prince and the view ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
- **Size response.** "We made the camera zoom out automatically when the katamari grows so big that it occupies the entire screen." Takahashi wished for a camera that conveyed scale better and reduced motion sickness, but had no time ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. Zoom-out is therefore triggered by screen-fill, with FOV values unknown [?]. A fan clone found distance snapping when radius jumped and fixed it with a lerp ([Klonamari](https://github.com/UnderGear/Klonamari)) [F].
- **Size thresholds.** Visible "zoom out" moments are not documented as fixed breakpoints [?]. Area unlocks come from the objects and 10 cm-style gates in the level design, not the camera [I].
- **Walls and occlusion.** The camera often hangs on walls and corners in urban settings; regaining it by repositioning the Prince costs time (review snippets surfaced by search, exact review not confirmed, e.g. [HonestGamers](https://www.honestgamers.com/2911/playstation-2/katamari-damacy/review.html)) [S]. The REROLL camera is criticized the same way ([Cubed3](https://www.cubed3.com/games/reviews/xbox-one/katamari-damacy-reroll), snippet only) [S]. Avoid copying this flaw [I].
- **Look-around.** L1 makes the Prince jump for a better view ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C].

## 8. The Prince

- **Role.** He pushes from behind; the sticks act as his two hands, with each stick the push on that side ([Steam thread](https://steamcommunity.com/app/848350/discussions/0/4633736978453499873)) [F]. He is 5 cm tall in the first game ([Katamaripedia: The Prince](https://katamaripedia.miraheze.org/wiki/The_Prince)) [C], which equals the starting katamari diameter in most early levels.
- **Does he scale?** No scaling mechanic is documented there [C]. In play he stays his fixed size and becomes a speck next to a large ball, so the camera zoom is what keeps him visible [I].

## 9. Pacing

Original (PS2) level table. Time limits listed are the 2004 values ([Katamaripedia: stages](https://katamaripedia.miraheze.org/wiki/List_of_Katamari_Damacy_Stages)) [C]. The ratio column is mine [I].

| Level | Start | Goal | Limit | Goal/start |
|---|---|---|---|---|
| Make a Star 1 | 5 cm | 10 cm | 3:00 | 2x |
| Make a Star 2 | 5 cm | 20 cm | 6:00 | 4x |
| Make a Star 3 | 10 cm | 50 cm | 8:00 | 5x |
| Make a Star 4 | 5 cm | 1 m | 10:00 | 20x |
| Make a Star 5 | 10 cm | 1.5 m | 12:00 | 15x |
| Make a Star 6 | 50 cm | 3 m | 10:00 | 6x |
| Make a Star 7 | 50 cm | 6 m | 10:00 | 12x |
| Make a Star 8 | 10 cm | 12 m | 18:00 | 120x |
| Make a Star 9 | 50 cm | 30 m | 18:00 | 60x |
| Make the Moon | 1 m | 300 m | 25:00 | 300x |

- **REROLL differs on some limits.** MaS1 is 4:00 on REROLL (PS2: 3:00), and on a first play it has no limit and ends at 10 cm; MaS2 is 7:00 on REROLL ([MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1), [MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2)) [C]. Another guide lists 13/11/12/18/20 minutes for MaS5-9 ([mejoress](https://www.mejoress.com/?p=47636)), which conflicts with the PS2 table; unreconciled [F].
- **Clear grades.** MaS1: OK at 12 cm, Great at 14 cm, Super at 15 cm; comet bonus time 1:00 ([MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) [C]. MaS4 grants a comet for hitting 1 m inside 6 minutes ([TASVideos](https://tasvideos.org/Forum/Posts/20447)) [F].
- **Eternal unlocks.** Roll big: MaS4 at 1.3 m, MaS8 at 25 m, Moon at 800 m ([mejoress](https://www.mejoress.com/?p=47636)) [F]. The Moon stage starts at 1 m and Takahashi describes "more than 500 m" as the ultimate goal ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **Staging of object sizes.** Map layout and object placement were planned together, and the team checked memory fit and how long reaching the goal takes ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. Speedrunners route around "size bottlenecks," objects you may be too small for if you missed too much earlier ([speedrun.com forum](https://www.speedrun.com/kd/forums/v69f4)) [S]. Levels have finite object counts, with more collectible as you grow ([TASVideos](https://tasvideos.org/Forum/Posts/20447)) [F].
- **Time limit rationale.** Takahashi: "I couldn't figure out how to make the game fun without a time limit" ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **Feedback.** Sound and controller vibration on each pickup carry much of the feel; consecutive pickups trigger combo effects (fireworks, confetti, stars) ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. There are no power-ups or enemies [C].
- **Size-jump announcements.** Not documented in any source reached [?].

---

## Implications for Azeroll

User direction overrides the earlier core-growth inference: the visible core never grows. Only
the collision radius and packed debris clump grow; trophies are absorbed when hidden beneath
the debris shell. `radius = cbrt(volume)` still governs collision and camera framing. The
research suggestions below are hypotheses, subordinate to that direction. Priorities for refinement:

1. **Keep the volume-to-diameter model and make small objects count less as the ball grows.** Both fan analyses and the thumbtack estimate point to a flattened contribution; try `dV = k * s^3 * f(s/radius)` with `f` falling for small `s/radius` instead of the plain `GROWTH_K * s^3` (confirm by play).
2. **Pickup test on bounding size, not mesh volume.** Use the M2 bounding sphere or box diameter versus ball diameter with a starting ratio of about 0.5 (range 0.4-0.6) and tune. Empty space counting is original behavior and gives the umbrella-versus-car feel for free.
3. **Add a thinness exception.** Long, thin objects (poles, pencils) should pass the size gate by their thickness, and then protrude. Compute their reach beyond the radius.
4. **Lumpy rolling from protrusion.** Derive eccentricity from the farthest stuck-object reach beyond the collision sphere. Apply a speed/heading wobble and ground bump proportional to it, decaying as the ball grows past the object. This is the "pencil makes you roll in an arc" feel.
5. **Attach by contact point, orientation preserved, sunk partly in.** Store the contact direction and sink depth as a fraction of object size; the ball transform carries them. Drop objects that fall beneath the core surface, as D3 does.
6. **Cap rendered trophies and swap to a low-detail shell.** Bound the live trophy count; at size breakpoints (original: about 12 m, 60 m, 200 m) replace the clump with a coarse shell and merged geometry. Drop memory of absorbed objects like the original.
7. **Camera: zoom out when the ball fills the screen, with smoothing.** Distance should scale with radius but be lerped, since the fan clone showed popping when radius jumps. Add wall avoidance (spring-arm raycast) and a quick re-center. The original's camera snagging on walls is a known flaw.
8. **Hits shed objects, not the whole ball.** Above a speed x relative-size threshold, detach a handful of the nearest objects (most recent or impact side), shrink the volume accordingly, and give a short stun. All unsourced numbers; expose them as constants.
9. **Controls: dual-stick scheme plus a simple mode.** Two input clusters (WASD and IJKL) as sticks with forward faster than sideways, a quick-turn chord (REROLL uses Shift+Ctrl), a dash from rapid alternation, and a simple direct-movement mode for keyboards. Ship rebinding.
10. **Scale movement speed and climb ability with radius.** Speed and step-up height proportional to diameter, so a larger ball is not slower in screen terms.
11. **Level pacing from goal/start ratios.** Early levels 2-5x with 3-8 minute limits; mid levels 6-20x; late 60-300x with limits around 18-25 minutes. Make sure every size band has right-sized objects, and gate shortcuts by size (like the 10 cm fences).
12. **Game feel.** Pickup sound/vibration, combo effects, and no power-ups or enemies beyond size-scaled animals that harass small balls and flee large ones.
13. **The Prince is a fixed-size character that stays visible.** Do not scale him; rely on the camera.

---

## HUD and audio (REROLL), and what Azeroll took from it

Evidence grade as above. The primary source is the user's own REROLL screenshot (stage "Make a Star"
type level, 11:38 into a video): [C] for everything in the first list, read straight off it. Web
search was thin: REROLL reviews, the Katamaripedia pages and StrategyWiki's gameplay page either
omit the HUD or block fetch (403/404), and none describes the pickup sounds beyond "animals make
noise when rolled up" and the postmortem's "sound and vibration on each pickup, combos trigger
effects" (already in section 9). Everything about how the combo sound escalates is [?].

**What the screenshot shows [C]:**
- **Size dial, top left.** A big flower/rosette of scalloped petal layers (orange, green, yellow, blue,
  purple centre). The current size sits across it in large white digits, units run together
  (`159m25cm0mm`: metres, centimetres, millimetres all shown). An open arc ring wraps the rosette
  from the lower left over the top, with arrowheads, and the **goal size** is printed along the ring's
  end (`300m`, tilted to follow it).
- **Clock, top right.** The remaining time as a number (`14`) beside a round pie clock: a dark disc
  with a lighter wedge and a hand. The wedge is the time still left.
- **Item bubble, bottom left.** The last object rolled up, as a lit 3D model (a building) in a
  translucent dark-blue circular glass, with a faint dotted trail to where it joined the ball, and its
  **name** under it (`Pilchard Building`).
- **Prince corner, bottom right.** The Prince, animating in place, in a round window over the sky.
- **Zoom moments / feedback.** The cut-in of the item bubble is the only on-screen "pickup" marker; the
  katamari itself sits unobstructed in the middle. [?] for the camera pull-out at size thresholds.

**What Azeroll does with it** (`azeroll/hud.rs`, `hud_models.rs`, `hud.wgsl`; palette is Warcraft
gold/crimson/blue/emerald on bronze):
- A procedural rosette (shader, 16/12/10-petal layers) with the size inside (`3yd 12in`, `23.4yd`
  from ten yards), the ring filling toward the ball size Hogger needs (linear in radius), a jewel and a
  tilted `Goal …` label at the ring's end, a swell and flash on growth, and a celebration (ring
  completes, rosette spins and sparkles) once Hogger is rolled up.
- A pie clock that drains and turns red in the last minute; Free Roll shows a slowly sweeping teal clock
  and the words `Free Roll`.
- A bubble with the last pickup's real model turning slowly, its name beneath in the item-quality
  colour, a pop on each swap and a fade after about five idle seconds.
- The Prince's real animation (run while the ball moves, stand when still) in a gold-rimmed circle.
- Pickup sounds: a pop whose rung climbs with a streak, the thing's own sound (creature voice, or a
  material kit from the prop's name), a thump for things big next to the ball, and a dull thud on a
  bump; see `azeroll/sfx.rs`. The engine's kit player has no per-play pitch, so the streak is a ladder
  of different kits, not a pitch bend.
