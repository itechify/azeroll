# The ritual track: roll past the goal, the send-off, Fizzwick in play, the Catalogue

Design for gap-audit-2 items **#1, #6, #7, #10**, with **#2** (HUD text diet) and **#11** (a song per
level) as short sections at the end. Written 2026-11-03 from the audit, D13/D19/D20/D20a/D24/D26 in
[ARCHITECTURE.md](ARCHITECTURE.md), [campaign-research.md](campaign-research.md) §1 and §4, the code
as it stands (`level.rs`, `progress.rs`, `hud.rs`, `music.rs`, `select/verdict.rs`, `hud_models.rs`,
`campaign.rs`) and the captures in `/home/jjd/az-patches/{hub2,l1r,l2,city}`. Design only: nothing here
was built or run.

**Lead decisions this designs within.** Size-goal stages (L1) keep rolling after the goal and grade the
final size; single-target stages (L2's Prize Pumpkin, L3's Cathedral) still end on capture. The send-off
is short, skippable, and restores camera and time on every exit. Fizzwick gets at most three pop-ins a
run, never over the ball. The Catalogue is light. The HUD stays nearly wordless and the camera steady.

**Reading order for the implementer.** §0 (the shared vocabulary and the one new module), then §1–§4 in
order (each builds on the last), then §5–§6, then §7 (the brief) and §8 (acceptance).

---

## 0. Shared vocabulary

| term | meaning |
|---|---|
| **goal beat** | the 2 s non-blocking moment when the goal is first met: ring completes, bloom, quiet fanfare, Fizzwick's goal line |
| **the candle** | the time limit (the hub's mission card already draws it as a burning candle) |
| **call it** | the player ends a size-goal run early by holding Enter for 1 s |
| **snuff** | time-up: the candle goes out |
| **send-off** | the 3.4 s departure: Fizzwick's Reclamation Beam lifts the clump; it hovers lit behind the results |
| **departure** | leaving the results for the hub: the clump shrinks up the beam and pops into the hub's star |
| **pop-in** | Fizzwick's in-play portrait and one line, 2.6 s |
| **the Catalogue** | the persistent per-player log of every unique thing ever rolled up |
| **star** | the hub's named star for a won level (D24, `progress::star_title`) |

**One new module, `azeroll/ritual.rs`,** owns the end of a run from the moment it ends until the hub or
Ready: the snuff, the send-off, the hover, the departure, the drop back for Free Roll, and the fail
moment. Today `progress.rs::finale_fx` and `level.rs::tick` share this between them with a 3.2 s
timer; the ritual replaces `finale_fx`'s camera and slow-motion for every stage (D26's Cathedral flight
stays in `finale.rs`, and the ritual starts when it ends). `progress.rs` keeps records and judging;
`level.rs` keeps the clock and the goal test. All of Fizzwick's words live in one new file,
`azeroll/lines.rs`, so a writer can edit them without reading logic (§3.5, §4.4, §1.6 are that file's
contents).

**Timing convention.** Seconds in the timelines are *real* seconds (`Time<Real>`), as the finale's are
today, so a paused or slowed world never stretches a ritual. "Play seconds" means `run.elapsed`.

---

## 1. Roll past the goal (audit #1)

### 1.1 What changes, in one paragraph

On a size-goal stage the goal is no longer the end. When the ball first reaches the goal diameter the
**goal beat** plays for 2 s without taking control, the clock keeps running, and the run ends at the
candle (time-up) or when the player **calls it** by holding Enter. The **final diameter** is graded
against `[level] grades`; the **comet** is earned by reaching the goal under `par`. Failing (the
candle out with the ball under the goal) gets a gentle, funny try-again moment, not a punishment.
Single-target stages keep ending on capture, with the same send-off (§2) afterwards.

### 1.2 Player-facing timeline (L1, Lion's Pride, 5:00 candle, goal 2 yd 14 in)

| play time | what the player sees and hears |
|---|---|
| 0:00 | Ready → Playing as today. The intro line is now **one line on the ring** (§5). |
| ~1:30–2:30 | **Goal met** (a first-timer; the autopilot does it in 67–97 s). The **goal beat**, 2.0 s real: the goal ring completes and flashes gold; the rosette blooms (the existing `TierReached` bloom, all four petals); the dial's size number pops; a **gold star** appears under the size number (the first of four, §1.3); the fanfare plays quietly; the music ducks 35 % for 1.3 s; a one-line banner at the new small banner size (§5) reads the recipe's `win_title` ("LAST CALL!") for 2.0 s; **Fizzwick pops in** (§3) with his goal line. Control never leaves the player; the clock keeps running. |
| goal + 2.0 s | The banner fades. The ring's cap label changes from `Goal 2yd 14in` to the next star's size (§1.3). |
| goal + 3.0 s | The **finish hint** fades in under the clock for 6 s: `hold ⏎ to call it` (§1.4). It never returns, except on the pause screen, which always lists it. |
| goal → 0:00 | The minute after the goal is the point of the change: the room is the player's. Tier banners, combos and the bubble work as before. Stars 2–4 land at 1.2×, 1.5× and 2.0× the goal, each with its own small chime and a dial swell; the 2.0× one is also Fizzwick's "Preposterous" pop-in. |
| 1:00 left | The clock turns red (today's rule) and Fizzwick pops in with a 60-seconds line (if a slot is left, §3.3). |
| 0:10 left | The clock ticks each second (a soft UI tick); the candle on the pause screen is a stub. |
| 0:00 | **Snuff.** The clock reads 00:00 and its hand stops; the ball slows to a halt over 0.6 s (the existing slow-motion ramp, `SLOW_SPEED` 0.2 then 0, no camera move); a short bell; the Prince stops pushing and looks up. The **send-off** (§2) begins 0.4 s later. |

**Calling it** (any time after the goal): the player holds Enter. Over 1.0 s the clock's rim fills
clockwise with gold; at the end of the second the clock hand sweeps to twelve, the same bell rings, and
the snuff sequence above plays with the clock showing the time left. Releasing early drains the rim
over 0.3 s and nothing happens. (Hold, not press: Enter also starts play from Ready and is the results
screen's button, so a tap must never end a run.)

**The clock stops at the snuff, never at the goal.** `run.elapsed` at the end is the candle, or the
moment called. `run.goal_time` (new) is the play time the goal was first met; it is what the comet
and the records' "best time" use (§1.5).

### 1.3 The dial after the goal: stars, not words

The dial today: the rosette with the size inside, the goal ring around it, `Goal 2yd 14in` on the
ring's end. After the goal, the ring would sit full for minutes and say nothing. Instead the dial
**counts stars**, which ties it to the medal on the results (Scraped by has one star, Respectable two,
Splendid three, Preposterous four: the medal in `hub2/6` and `l1r/r4-results.png` already draws them).

```
   before the goal                 after the goal (one star)             after 1.2× (two stars)
   ┌──────────────┐                ┌──────────────┐                     ┌──────────────┐
   │  ╭────────╮  │                │  ╭────────╮  │                     │  ╭────────╮  │
   │ ╱ rosette  ╲ │                │ ╱ rosette  ╲ │                     │ ╱ rosette  ╲ │
   │(   18in     )│                │( 2yd 16in   )│                     │( 2yd 31in   )│
   │ ╲    ·     ╱ │                │ ╲    ★     ╱ │                     │ ╲   ★★     ╱ │
   │  ╰────────╯  │                │  ╰────────╯  │                     │  ╰────────╯  │
   │ ring: 64% ●  │                │ ring: full, gold, re-arming        │ ring: 0→100% to 3 yd 21in │
   │  Goal 2yd 14in               │  ★★ 2yd 31in  (next star's size)  │  ★★★ 3yd 21in             │
   └──────────────┘                └──────────────┘                     └──────────────┘
```

- **Stars** sit under the size number inside the rosette, 14 px each, gold, drop-shadowed; each new one
  pops in (scale 1.6 → 1 over 0.25 s) with a chime. One star at the goal, then one per `grades` boundary.
- **The ring re-arms.** After the goal the ring fills from empty toward the next star's diameter (linear
  between boundaries); at four stars it stays full and the cap label disappears. The cap label is the
  next star's count and size, `★★ 2yd 31in`, in the same small type as `Goal …` today. That is the only
  text on the dial, and it is a target, which is what the label was before.
- **Free Roll** shows no stars and no cap label (there is no verdict).
- **Single-target stages** (L2, L3): the dial is unchanged (the ring shows the ball against the size the
  target needs; the run ends on the target). Stars only appear on the results medal.

### 1.4 The finish key and its hint

- **Input:** Enter held for `CALL_HOLD` = 1.0 s, in Playing, after the goal, not in Free Roll, not during
  the send-off. Gamepad later (#9): Start held. Esc still pauses; the pause screen's text block gains the
  line `Hold Enter to call it (the goal is met)`.
- **Hint on screen:** one small line under the clock, right-aligned to the clock's edge, parchment at 60 %
  alpha, 18 px: `hold ⏎ to call it`. Fades in at goal + 3 s, holds 6 s, fades 1 s, never returns. (The
  HUD is nearly wordless; one hint, once, is the cost of a hold input nobody will guess.)
- **Feedback while holding:** the clock's rim fills gold clockwise over 1 s; a rising three-note UI tick
  (`UChatScrollButton` 1115 three times, 0.33 s apart) under it; release drains it silently.
- **Why Enter:** it is already "confirm" everywhere else (Ready, results, hub). F stays Free Roll, R
  restart, L leave. The hint's glyph `⏎` renders in the HUD font? If not, write `Enter`.

### 1.5 Grading, comet and records

| | size-goal stage (L1) | single-target stage (L2, L3) |
|---|---|---|
| run ends | snuff, or called | on capture (then the send-off) |
| grade | **size only:** `final diameter / goal` against `grades` (1.2 / 1.5 / 2.0) | as today: the better of size and pace |
| comet | `goal_time < par` | `elapsed < par` (as today) |
| best time (hub card, results) | **fastest to the goal** (`goal_time`) | fastest win (as today) |
| biggest | final diameter | ball's own diameter (never the 595 yd display, D24) |

`progress::judge_win` gains the goal kind and the goal time; `DEFAULT_PACE_GRADES` stop applying to
size goals. `LevelBest::best_time` keeps its name and serde key; its meaning becomes "fastest to the
goal" for size stages, which is what a replay wants to beat. The results' tally shows
`Goal at 01:42.3` above `Time 05:00.0` (or `Called at 03:12.0`) for size stages.

**Why size only for L1.** Pace no longer separates two wins once every uncalled run lasts the candle,
and a player who calls it early at the goal has chosen "Scraped by": that is the Katamari joke the King
makes about a ball that barely cleared the bar.

### 1.6 Failing: the candle has opinions

Time-up under the goal. No beam, no star, nothing dims. Timeline (real seconds from the snuff):

| t | what happens |
|---|---|
| 0.0 | Clock 00:00, hand stops, bell. Ball coasts to a stop over 0.6 s. The world stays lit. |
| 0.4 | Fizzwick pop-in, the **try-again** line for the level (below), with his "sigh" voice. Hold 3.0 s. |
| 0.4 | Camera eases 0.8 s to the three-quarter view of the clump (§2.3, the same move as the send-off, so the player sees what they built). |
| 1.2 | A one-line banner, the small size: `TIME'S UP` (gold → parchment). |
| 3.4 | The results panel docks (§2.5) beside the clump, headline `NOT QUITE`, the recipe's `fail` line under it, the **Roll again (R)** button first and lit. Fizzwick's window on the panel repeats the try-again line. |

**Try-again lines** (`lines.rs::TRY_AGAIN`, one picked at random per level; the generic pool is used
when a level has none left):

*Lion's Pride*
- "The candle went out. The floor is still crunchy. Farley is weeping. Again?"
- "You were THIS close. I am holding my fingers apart. You cannot see them. Again!"
- "Last call was called. We shall simply call it again. Louder."
- "The regiment left the crumbs. The crumbs are still there. This is a two-crumb problem."

*Market Day*
- "The pumpkin took first prize without you. It gloated. Pumpkins can gloat. Again!"
- "Market's shut. The stalls are counting their stock and most of it is still there."
- "The judges have gone home. The judges will come back. The judges have no choice."

*Stormwind*
- "The Cathedral did not RSVP. Neither did you, apparently. We go again."
- "Stormwind is still standing. This is not the report I wanted to file."
- "A city is a big ask. I did say it was rather large. I am saying it again."

*Generic*
- "The candle has opinions about that. Shall we try again, with feeling?" (today's `FAIL_REMARK`)
- "That was a practice run. I have decided this retroactively. Again!"
- "Nobody saw that. I saw that. Nobody else saw that. Again!"
- "The fuses survived. The schedule did not. Once more!"
- "Close! By gnomish standards. Which are small. Again!"

### 1.7 Edge cases (size-goal flow)

- **Debug starts** (`WOW_AZEROLL_DEBUG_RADIUS` above the goal): the goal test runs only after
  `run.elapsed > 0.5` play seconds and the phase is Playing, so the goal beat never fires inside Ready
  or the first frame. The autopilot's `goal edible` telemetry words are unchanged.
- **Free Roll:** the goal beat plays the banner and the bloom only (no star, no Fizzwick, no hint); no
  clock, no calling it, no results, as today.
- **Pause during the goal beat:** the beat's clock is virtual (banner and bloom already are), so pause
  holds it; Fizzwick's pop-in is real-time and finishes under the pause menu (he is small, in a corner,
  harmless). The music duck already survives pause.
- **Restart (R) after the goal, before the end:** Ready as today; `goal_time` and stars reset with
  `Run`. Nothing was saved yet (records are written on the results, as today).
- **Shrinking below the goal after it was met** (shedding from a ram): the stars stay (a star, once lit,
  is lit); the ring's fill can fall; the grade is the *final* diameter, so a bad last ten seconds can
  cost a star on the medal but never the win (`run.won` is sticky, as today).
- **The L3 terminal display size** (595 yd after the Cathedral attaches, D26): L3 is a single-target
  stage, so nothing in §1 applies; the send-off shows the display size as the headline (it is the joke)
  and the grade still uses the physical diameter (D24).
- **Time-up on the same frame as the goal:** the goal wins (`run.won` is set before the time test; then
  the snuff runs on the next frame with the goal beat cut to its first 0.4 s).

---

## 2. The send-off (audit #6)

### 2.1 The Warcraft translation

Katamari lifts the katamari into the night sky, where it becomes a star. Azeroll's equivalent is
**Fizzwick's Reclamation Beam:** a gnomish "Ultrasafe Reclamation Beam, Mk. II" comes down from the sky
(or through the inn's rafters), takes hold of the clump, lifts it clear of the ground and holds it there,
spinning slowly in the light, while the verdict is read beside it. When the player leaves for the hub
the clump shrinks up the beam and pops into the star the hub shows. The star therefore *is* the clump,
which the hub's existing "a star flies up from the beacon" celebration (D24 *News*) finishes.

Two things this deliberately avoids: a black screen (the clump the player built is the whole point) and
a long cutscene (3.4 s to the results, skippable from the first frame).

### 2.2 Timeline (real seconds from the end of the run: snuff, call, or L2/L3 capture after D26's flight)

| t | camera | world | HUD | sound |
|---|---|---|---|---|
| 0.0 | the player's view | virtual time → 0 (frozen; NPCs and the Prince hold pose) | play HUD stays, the finish hint gone | music ducks 55 % (the Hogger depth) for the whole send-off |
| 0.0–1.0 | eases to the **three-quarter view** (§2.3) | | clock and dial fade to 40 % | |
| 0.6 | | the **beam** lands: a vertical column of pale arcane light, 1.3 r wide, soft-edged, with a slow upward scroll and a gold core, from the sky (outdoors) or from the ceiling (indoors, §2.4); a ring of sparkles spreads on the ground | | beam hum starts (§2.6) |
| 1.0–3.0 | holds, slow drift | the clump **lifts** to 1.5 r above its resting point (ease-out), turning one full turn about a tilted axis (a slow majestic roll, not a spin); loose trophies stay seated; the Prince (unfrozen alone) steps back and **waves** (EmoteWave 67) | | lift whoosh at 1.0, chime at the apex (3.0) |
| 2.2 | | | **headline fades in beside the clump,** one line, never over it (§2.5): the final size large, the `win_title` small under it | |
| 3.0 | | the clump settles into a hover: a 0.15 r bob over 4 s, a turn every 14 s | | |
| 3.4 | holds | hover continues | the **results panel docks** (§2.5); Fizzwick's window and the medal stamp as today; `Phase::Results` | fanfare (`QUESTCOMPLETED`, full level) under the panel's arrival; music stays ducked at 55 % |

**Skip:** Enter, Space or Esc at any point jumps to t = 3.4 (beam on, clump hovering, panel docked).
Space and Esc are skip-only here (they do nothing on the results). Enter at the results is "back to the
hub" as today, so one extra Enter during the send-off costs the player nothing but the ritual.

**L2 and L3:** D26's 3.2 s Cathedral flight (and L2's existing slow-motion pull-out) run first and
unchanged; the send-off begins when `flow.finale` would have gone to Results. The 3.2 s + 3.4 s total is
long for L2; cut L2's pre-roll to 2.0 s (`FINALE_SECS` per goal kind: Collect 2.0, Finale 3.2, Size 0).

### 2.3 The three-quarter view

A fixed offset from the ball, chosen once at t = 0 and never moved by the world: yaw = the player's
current camera yaw + 35° (so the clump turns a new face to the lens), pitch 22°, distance such that the
clump's diameter spans 46 % of the screen height. The lens stays outside the clump and above the
ground; the D8 see-through window keeps working; **no** orbit during the results (the user asked for a
steady camera; the clump itself turns, which gives the motion). Indoors (L1) the distance is capped to the
room: the D8 exterior-lens problem is avoided by never pulling past 2.2 r, and the pitch stays at 15°.

### 2.4 Indoors and daylight

- **L1, indoors:** the beam comes through the rafters: it starts at the room's ceiling height and the
  clump lifts 1.0 r, not 1.5 r, so it hangs under the chandelier. Sparkles, same chime. No sky is needed.
- **L2, afternoon:** the beam is visible against daylight because it is additive and bright with a dark
  rim; the "star" at departure is a glint that climbs out of frame, not a night sky.
- **L3, Cathedral Square:** the beam is 1.3 r wide around a 64 yd ball, so it is a shaft the width of
  the square; the Cathedral facade trophy rides the clump as D26 attached it. The fog override (D26)
  stays as it was at the end.

### 2.5 Screen layout: results beside the lit clump

Wide (aspect ≥ 1.4):

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ dial 40%                                                      clock 40%      │
│                                                                              │
│            │ beam │                     ┌──────────────────────────────┐     │
│            │      │                     │ LAST CALL!                   │     │
│          ╭─┤ ~~~~ ├─╮                   │        (medal)               │     │
│         (  │clump │  )  ← hovering      │      PREPOSTEROUS!           │     │
│          ╰─┤      ├─╯                   │ 2.13x the request · called   │     │
│            │      │                     │ COMET! goal at 01:42 (par 2:10)│   │
│      2 yd 26 in   ← headline, one line  │ ┌──────────────────────────┐ │     │
│      LAST CALL!   ← small, under it     │ │ THE TALLY                │ │     │
│                                         │ │ Goal at 01:42.3          │ │     │
│   [Fizz] "Preposterous! Unscheduled!"   │ │ Time 05:00.0 (candle)    │ │     │
│                                         │ │ Size 2 yd 26 in  NEW BEST│ │     │
│                                         │ │ Rolled up 290    NEW BEST│ │     │
│   Prince, waving, bottom left of clump  │ │ Best find: Chair         │ │     │
│                                         │ │ A new star: The Chair    │ │     │
│                                         │ │ NEW to the Catalogue: 14 │ │     │
│                                         │ │ challenges …             │ │     │
│                                         │ │ [Hub ⏎] [Again R] [Free F]│ │     │
│                                         │ └──────────────────────────┘ │     │
│                                         └──────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────────┘
```

- The panel takes the right 46 % of the width, full height minus 6 % margins, with its own parchment at
  92 % alpha so the lit world shows faintly through it. **No full-screen dim** (today's results darken
  the whole world: `hub2/6`, `l1r/r4-results.png`). The left 54 % belongs to the clump.
- The headline (`2 yd 26 in`, 56 px, gold) sits under the clump, centred on the clump's screen x, with
  the `win_title` under it at 24 px parchment. For L3 it reads `595 yd`.
- Fizzwick's window stays on the panel as today; his remark is typed as today.
- **Compact** (aspect < 1.4, the user's captures are ~0.92): the clump hovers in the upper 55 % of the
  screen, the headline under it, and the panel docks from the bottom, 45 % tall, with the tally in two
  columns and the buttons on one row. The medal and grade go on one line (`medal  PREPOSTEROUS!`).

### 2.6 Exits (the invariant: camera and time restored on every one)

| exit | what plays | then |
|---|---|---|
| **Enter → hub** | **departure,** 0.8 s: the clump shrinks up the beam (scale → 0 over 0.6 s, climbing 3 r), a star glyph pops at the top of the beam with a chime, the beam switches off (0.2 s), the panel slides out | `Phase::Select`; the hub's star-flies-up news plays as today (it reads as the same star arriving) |
| **R → again** | beam off at once, no departure | `Phase::Ready` (`reset_run` despawns the beam with the run) |
| **F → Free Roll** | **drop,** 0.5 s: the beam cuts, the clump falls back to its resting point and bounces once (the existing thud kit), the Prince runs back to it; the camera eases 0.6 s back to `cam_before`; virtual time → 1 | `Phase::Playing`, Free Roll; Fizzwick's one line: "Fine. Keep it. For NOW." |
| **window close / debug shot** | nothing | `ritual::teardown` on `OnExit(Results)` and `OnEnter(Ready | Select)` always: time 1.0, camera override cleared, beam entity despawned, fog as the level had it |

The departure's 0.8 s runs *before* `next.set(Select)` (the loading screen would otherwise cover it);
Enter during the departure is ignored (it is under a second).

### 2.7 Audio cues (all existing kits; the implementer verifies the ids against `SoundEntries`)

| moment | candidate kit(s) | notes |
|---|---|---|
| snuff bell | `BellTollAlliance` (the Stormwind bell; the same bell on every level) | one strike, 2D |
| clock's last ten seconds | `UChatScrollButton` 1115 | one tick per second, quiet (the roll yields) |
| beam lands | a teleport or arcane cast kit: search `SoundEntries` names for `Teleport`, `Blink`, `Hearthstone`, `Levitate`, `Polymorph` (names vary by build; D19 lists what resolves) | fall back to `LEVELUPSOUND` 124, reserved and unused today |
| beam hum | a looping spell-aura kit if the kit player can loop one (`play_kit_ext` + `Latch`); else none | low level, ends at departure |
| lift whoosh | `SwingWeaponSpecialWarrior` 3091 (the dash whoosh) at 0.4 strength via `emission_distance` | |
| apex chime | `TaxiNodeDiscovered` 1519 (the tier sting, 4.9 s) | it is the game's "discovery" sound |
| star pop (departure) | `LEVELUP` 888 (reserved today as the record sound; the departure can own it, and the record cue moves to `QUESTADDED` 618) | |
| drop (Free Roll) | `DeathThudSmallWood/Stone` 911/910 by ground | |
| fanfare at the panel | `QUESTCOMPLETED` 619 | full level; the goal beat's copy is quiet (§3.6) |

Music: the send-off ducks 55 % (hold = its length, release 0.9 s); on the results the level track
**continues at 50 %** instead of switching to the title theme (change to `music.rs::pick_track`'s
Results rule), so the moment stays in the level; the hub brings the title theme back. (If the
implementer prefers the title theme on results, keep it: the duck must then clear on `OnEnter(Results)`
as it does today.)

---

## 3. Fizzwick in play (audit #7)

### 3.1 The pop-in

A small brass porthole with the commissioner stage's render inside it (the same 288×384 target as the
hub, scaled to a 104 px circle), under the clock in the upper right, and a one-line speech bubble to its
left, tail toward the portrait. It slides in from the right edge (0.3 s ease), holds 2.6 s, slides
out (0.3 s). It never covers the ball (the ball is centred and the bubble's left edge stops at 42 % of
the width), never the dial, never the item bubble.

```
┌──────────────────────────────────────────────────────────────────┐
│ dial                                   03:54  clock               │
│                 ┌──────────────────────────────────┐ ╭──────╮     │
│                 │ Preposterous! I am writing a new │◁│ Fizz │     │
│                 │ category!                        │ │ wick │     │
│                 └──────────────────────────────────┘ ╰──────╯     │
│                                                                   │
│                          (ball)                                   │
│                                                                   │
│  item bubble                                        Prince corner │
└──────────────────────────────────────────────────────────────────┘
```

- Bubble: parchment, 20 px ink, at most two lines of ~34 characters (every line in §3.5 fits), typed at
  the hub's speed (≥ 48 chars/s, so the longest finishes in 1.4 s), with the voice at the first letter.
- Portrait: he gestures as in the hub (`commissioner_acts`: TalkExclamation then Talk) and plays the
  voice emote's own animation when it has one (cheer 68, laugh 70, cry 77 as `AnimationData` ids: the
  implementer checks the ids against `Emotes.dbc` through `EmoteSounds::anim`).
- Compact aspect: the same, under the clock; the bubble wraps to two lines of ~26.
- The stage gnome is spawned in Playing too (today `stage_commissioner` only runs in Select and
  Results); the porthole is hidden until a pop-in. Cost: one more carried unit, as D20a's corner Prince.

### 3.2 The budget

- `POPS_PER_RUN` = 3, `POP_GAP` = 20 s real time between pops, none in the first 6 s of play (the intro),
  none during the goal beat's banner's first 0.5 s, none while `flow.finale` or the ritual runs, none
  in Free Roll (except the drop line, §2.6), none in Ready or Paused.
- **One slot is reserved for the goal** until the goal happens (so a run never spends all three on tiers
  and then has nothing for the moment that matters). On single-target stages the goal line plays over
  the send-off's first second instead (the run ends on capture).
- Priority when several events are pending: goal > 60 s left > Preposterous > landmark > a tier. A
  lower-priority event that misses the budget is dropped, not queued.

### 3.3 Events, per run

| event | trigger | line table | voice |
|---|---|---|---|
| **goal reached** | first `goal_met` (size) / capture (single-target) | `GOAL[level]` | cheer |
| **sixty seconds** | `time_limit - elapsed` crosses 60 | `SIXTY_BEFORE[level]` if the goal is not met, else `SIXTY_AFTER[level]` | gasp (before) / laugh (after) |
| **Preposterous** | diameter crosses 2.0× goal (size stages) or the top `grades` bound of the target's need (single-target) | `PREPOSTEROUS[level]` | laugh |
| **landmark** | the level's named big thing is rolled up (`LevelEntry::landmark`: L1 "Captain's table" (the claimed D22 table), L2 "Hogger", L3 the first whole building) | `LANDMARK[level]` | cheer |
| **tier** (filler) | `TierReached` when nothing else has used a slot in 60 s | `TIER` (generic) | talk only |

### 3.4 Voice: Fizzwick's record scratch

The King's voice is a scratch; Fizzwick's is the gnome's own emote voice. The engine already routes text
emotes to a race/sex voice kit: `EmoteSounds` (`sound/emote.rs`) wraps `EmoteSoundCatalog::voice(text_id,
race, sex)` from `EmotesTextSound.dbc`; race 7 is Gnome, sex 0 male. Resolve the kit at startup for these
text emotes (`text_id("CHEER")`, …) and play it 2D through `play_kit_ext` at the pop-in:

| feeling | text emote to resolve | fallback if unvoiced for gnome male |
|---|---|---|
| cheer | `CHEER` | `APPLAUD` |
| laugh | `LAUGH` | `CHUCKLE`, `GIGGLE` |
| gasp | `GASP` | `SURPRISED`, `OOPS` |
| sigh (fail) | `SIGH` | `CRY` |
| talk | none: the gesture only | |

The implementer verifies which of these have a gnome-male row (the 1.12 client voices roughly a dozen
text emotes per race; cheer, laugh, cry and sigh are the safe bets) and logs the resolved ids at startup
beside `azeroll commissioner: display N`. A second source of lines, if the emotes prove thin, is
`VocalUISounds.dbc` (`sound/vocal.rs`): the race's error speech ("I can't do that", "Not enough
energy" and so on), which is funny out of context; the implementer can pick two.

### 3.5 Lines (`lines.rs`; ≤ 68 characters each; one picked at random per event, never the same one twice in a session)

**GOAL — size met / target captured**

*Lion's Pride*
- "Big enough! Now, Farley would like his FLOOR back. All of it."
- "That is the size. This is not the end. Last call means LAST call."
- "Goal! Keep rolling. The regiment left more than crumbs."
- "You have reached the size. Do not stop. Nobody stops at the size."

*Market Day*
- "The Prize Pumpkin! First prize! The judges are reconsidering everything."
- "Pumpkin acquired. Ma Stonefield's face is a picture. I have the picture."
- "That is the biggest vegetable I have ever filed. And I file vegetables."

*Stormwind*
- "The Cathedral has accepted the invitation. Involuntarily. It counts."
- "A whole city! Unscheduled! The committee is going to need a bigger table."
- "Stormwind, rolled up. I shall tell the King. Eventually. By letter."

**SIXTY_BEFORE — one minute left, goal not met**

*Lion's Pride*
- "One minute! The candle is a stub! Eat the NEAREST thing!"
- "Sixty seconds! Books, boots, barrels: I am not fussy and neither are you."

*Market Day*
- "One minute to judging! The pumpkin is smug. Make it less smug."
- "Sixty seconds! Roll up something with legs, it goes faster."

*Stormwind*
- "One minute! The Cathedral is RIGHT THERE. It has not moved. Cities don't."
- "Sixty seconds. A house. Any house. Houses are very filling."

*Generic*
- "One minute left! Panic is permitted. Stopping is not."
- "Sixty seconds! I believe in you. I have also prepared a form if not."

**SIXTY_AFTER — one minute left, goal already met**

- "One minute! Everything you eat now is PURE PROFIT."
- "Sixty seconds of pure, unregulated rolling. Go."
- "One minute left and the room is yours. Be greedy. Be a gnome about it."
- "Sixty seconds! Make it preposterous or make it home. Preposterous, please."

**PREPOSTEROUS — twice the request**

*Lion's Pride*
- "Preposterous! That was a supper and now it is a WEATHER SYSTEM."
- "Twice the size! Farley has gone to lie down. On nothing. You took the beds."

*Market Day*
- "Preposterous! You have rolled up the fair AND the people judging it."
- "That is not a katamari, that is a SECOND GOLDSHIRE. Carry on."

*Stormwind*
- "Preposterous! The map is going to need redrawing. I have crayons."
- "Twice the request! Stormwind will be listed as 'formerly'."

*Generic*
- "Preposterous! Unscheduled! I am writing a new category!"
- "PREPOSTEROUS! My monocle has fallen off. I do not own a monocle."
- "That is beyond the chart. I have drawn more chart. It is beyond that too."

**LANDMARK — the level's big named thing**

*Lion's Pride (the captain's table)*
- "The captain's table! The captain is under it. The captain is next."
- "A whole table! With the supper still on it. Efficient."

*Market Day (Hogger)*
- "HOGGER! Finally! The cheese stall sends its regards."
- "You rolled up Hogger. Marshal Dughan will be furious. He wanted the credit."

*Stormwind (the first whole building)*
- "A BUILDING. You have rolled up a building. With people's things in it."
- "A house! Was anyone home? Do not answer that. Roll up the next one."

**TIER — filler**
- "Bigger! Excellent. Keep it up, I am taking notes."
- "That's a new tier of thing. There are more tiers. Tiers all the way up."
- "Growing nicely. Like a gnome. Which is to say: eventually."
- "Oho! That used to be a wall. Now it is a snack."

**RESULTS remarks** (replace the single `Grade::remark` string with a pool of three per grade; the first
is today's line)

*Scraped by*
- "It counts. Technically. Please do not tell the committee."
- "The size, and not an inch more. You have the soul of an accountant."
- "Just over the line. The line is relieved. I am relieved. We are all relieved."

*Respectable*
- "Adequate! The gears are almost impressed."
- "Respectable. I shall write it in ink. Small ink."
- "A good roll. Not a great roll. A good roll. I was there."

*Splendid*
- "Marvellous! Even the cogs are applauding."
- "Splendid! That is a word I keep for special days and this is one."
- "Oh, that is a BALL. That is a proper ball. Frame it. I cannot, it is round."

*Preposterous*
- "Preposterous! Unscheduled! I am writing a new category!"
- "Preposterous! The committee has fainted. The committee is one gnome. Me."
- "That is not a result. That is an EVENT. I shall be dining out on this."

**FREE ROLL drop** (§2.6): "Fine. Keep it. For NOW."

### 3.6 Audio for the goal beat

The fanfare `QUESTCOMPLETED` at the goal beat must be quieter than the results' copy. The kit player has
no volume (D19), so place it on the listener-to-source ray at the distance where its rolloff gives
about 50 % (`emission_distance`), as the quiet touches do. The music duck is the tier's (35 %, 1.3 s).
A single-target stage's capture keeps today's full fanfare (it is the end).

---

## 4. Fizzwick's Catalogue (audit #10)

### 4.1 What it is

A persistent log of every unique **display name** the player has ever rolled up, with the level it was
first found in and how many ever. Katamari's Collection, kept light: no models to unlock, no 100 %
completion gate, a count per level and one Fizzwick line per *category* (plus ~40 lines for signature
things). Viewable from the hub. The results say what was new.

### 4.2 Data

```rust
/// progress.rs — persisted with the save (`[catalogue]` table; absent in old saves → empty).
pub(super) struct CatalogueEntry {
    /// Level id of the first find.
    pub level: String,
    /// Ever rolled up, all runs.
    pub n: u32,
    /// The biggest `pickup` axis seen, yards (so "Chair" from L1 and L3's "House" sort by size).
    pub axis: f32,
}
pub(super) struct Progress { /* … */ pub catalogue: BTreeMap<String, CatalogueEntry> }

/// Outcome gains:
pub new_catalogue: Vec<String>,   // this run's first-ever names, in pickup order (shown: count + first three)
```

Save shape (toml): `[catalogue."Cheese Wheel"] level = "lions-pride" n = 12 axis = 0.41`. Written in
`record_results` from `Run::log` (name → count; add `Run::log_axis: BTreeMap<name, f32>` for the axis,
or widen `log`'s value to `(u32, f32)`). Free Roll runs **do** add to the Catalogue (they do not touch
records; the Catalogue is a log, not a score), so `record_results` needs a Free Roll path that writes
only the Catalogue: today Free Roll never enters Results, so add the write on `OnExit(Playing)` when
`flow.free_roll` (cheap: a map merge).

**Denominator.** Per level the card shows `found N` and, where the recipe makes it knowable, `of M`:
M = unique display names the recipe's scenes and cast and herds can produce (`Design::catalogue_names()`,
parsed once at startup for `Campaign`). Claimed map doodads and city pieces are not in M (streaming
decides what loads); they list under the level as "also found". The hub's per-level line is
`Catalogue 41 of 63 · +9 found roaming`. Never a percentage.

**Categories** (`names.rs` gains `category(name, kind, source) -> Category`; a keyword table first, then
`Kind`, then the sfx `Material` as the fallback):

| category | rule |
|---|---|
| Food & drink | Material Food/Meat; keywords: bread, cheese, pie, muffin, apple, pumpkin, mug, bottle, keg, barrel? no (barrel → Goods) |
| Furniture & household | chair, table, stool, bed, pillow, chest, bookshelf, candle, lamp, rug |
| Tools & goods | barrel, crate, sack, bucket, anvil, wagon, cart, plank, rope, wood pile, hay |
| Critters & beasts | `Kind::Creature` not in a cast (rat, chicken, sheep, cow, wolf, boar) |
| Folk | `Kind::Creature` from a cast, `Kind::Boss` (Hogger) |
| Trees & greenery | tree, pine, bush, stump, sapling, flower, grass |
| Buildings & landmarks | city pieces, whole houses, the Cathedral, bridges, fences, walls |
| Odds and ends | everything else |

### 4.3 Hub and results

**Hub.** The mission card gains one line under the records (`Catalogue 41 of 63`) and the help line
gains `C is the Catalogue`. `C` (or clicking the line) opens the Catalogue page over the card's area;
the planet stays. Esc or C closes it.

```
┌─────────────────────────────────────────────────────────────┐
│  FIZZWICK'S CATALOGUE                        Esc / C closes  │
│  ─────────────────────────────────────────────────────────  │
│  I  Last Call at the Lion's Pride     41 of 63  · 9 roaming │
│  II Market Day Mayhem                 88 of 140 · 31 roaming│
│  III Stormwind or Bust!               12 of 51  · 4 roaming │
│                                                             │
│  ▸ FOOD & DRINK (14)                          ╭──────────╮  │
│     Muffin ×212   Cheese Wheel ×9  Pie ×31    │  bubble  │  │
│     Apple ×44     Mug ×80   …                 │  model   │  │
│  ▸ FURNITURE & HOUSEHOLD (9)                  ╰──────────╯  │
│     Chair ×6 ★    Stool ×12   Bed ×2            Chair       │
│  ▸ CRITTERS & BEASTS (3)                      first: Lion's │
│     Rat ×36  Hearth Cat ×1                    Pride · ×6    │
│  ▸ FOLK (4)                                   1 yd 25 in    │
│     Innkeeper Farley ×2  Brog Hamfist ×1      "A chair. The │
│                                               regiment sat  │
│                                               on it. Now it │
│                                               sits on YOU." │
└─────────────────────────────────────────────────────────────┘
```

- Left: the three levels as headers (W/S or arrows pick a level; the list under it); categories as
  collapsible groups, names with `×n`, a ★ on the level's star item, dim grey for names in M not yet
  found (shown as `· · ·`? no: show their names; the fun of a log is knowing what is missing, and the
  names are not spoilers in a three-level game).
- Right: the item bubble renderer (`hud_models.rs`'s bubble stage, spinning for props, the sway for
  creatures as D20) with the chosen thing's model, its name, first level, count, biggest size, and
  the quip. The model needs the entry's model path: store `model: String` in `CatalogueEntry` (the
  `Collectible`'s source path; cast NPCs store their display id as `display:1234`).
- Compact aspect: the list only, the bubble above it, two columns of names.

**Results.** One line in the tally, after the star line: `NEW to the Catalogue: 14 · Chair, Pillow,
Hearth Cat …` in green, with a small `NEW!` stamp; nothing when 0. The new names' quips are not read on
the results (the pace of the screen).

### 4.4 Quips (`lines.rs::CATALOGUE`; the category line is used when the name has none)

**Per category**
- Food & drink: "Edible. Was edible. Is now structural."
- Furniture & household: "Somebody sat on this. Somebody will have to stand now."
- Tools & goods: "Honest goods, honestly rolled. The merchant disagrees."
- Critters & beasts: "It is fine. It is on a ball now. Balls are a kind of home."
- Folk: "A citizen of Azeroth, volunteered. The paperwork is retroactive."
- Trees & greenery: "Nature. Compressed. Very efficient use of nature."
- Buildings & landmarks: "Architecture is just a big object that thinks it is special."
- Odds and ends: "I do not know what this is either. It is in the log now. It counts."

**Signature things** (the name as `names.rs` / the recipes / the casts spell it)
- Muffin: "Two hundred muffins. The cook fled for a reason, and the reason was muffins."
- Cheese Wheel: "The wheel is round. The ball is round. This was always going to happen."
- Cheese: "Follow the cheese. The cheese led to rats. The rats led to more cheese."
- Bread / Loaf of Bread: "Day-old bread. Now day-old bread at speed."
- Pie: "A whole pie. Nobody had a slice. Nobody will."
- Apple: "Crisp, red, and doomed."
- Mug: "An empty mug, which is the saddest kind, until it joined something."
- Bottle: "Vintage. The year is 'this one'."
- Keg: "A keg! The regiment's. They will be along to complain. Roll them too."
- Barrel: "Barrels are the universe's way of saying 'this will roll'."
- Crate: "Contents unknown. Contents now also round."
- Sack: "A sack of something. Everything is a sack of something, if you think about it."
- Book / Open Book: "Knowledge is power. Stacked knowledge is stairs."
- Boots: "Boots in the hall. Someone is in their socks right now, and furious."
- Pillow: "The pillow fight is over. Everyone lost. The pillows most of all."
- Chair: "A chair. The regiment sat on it. Now it sits on YOU."
- Stool: "Farley's stool. Farley was fond of it. Farley is fond of many things he no longer has."
- Table: "A table! With the supper still on it. Efficient."
- Captain's Table: "The captain's own table. He was under it. He is next."
- Bed: "The honeymoon suite. The honeymoon is over."
- Candle: "A candle. Not THE candle. The candle is mine."
- Rat: "A rat. Pied piper. I am not paying you in rats."
- Hearth Cat: "The hearth cat. It was asleep. It is still asleep. On a ball."
- Chicken: "The chicken did not cross the road. The road came to it."
- Sheep: "Wool, legs and a grudge."
- Cow: "This is how Taurus happened, you know. Ask the King. Not that one."
- Pig: "Market pork. Free range. VERY free range."
- Wolf: "Elwynn's finest predator, now prey. Do not tell it."
- Hogger: "HOGGER. Marshal Dughan has a poster. The poster is wrong now."
- Innkeeper Farley: "Farley. He wanted his floor back. He has it; he is on it; it is round."
- Brog Hamfist: "The cook. He fled from the kitchen into the kitchen. Poor planning."
- Remy: "Remy, asleep under the bar. Still asleep. Impressive."
- Barkeep Dobbins: "Dobbins. The card game ended badly and this is the badly."
- Marshal Dughan: "The Marshal. Goldshire's law. Now Goldshire's ballast."
- Stormwind Guard: "A guard stood his ground. The ground was collected first."
- Ma Stonefield: "Ma Stonefield grew the pumpkin. Now she is in it, roughly."
- Prize Pumpkin: "First prize! The judges awarded it to YOU. By being on you."
- Pumpkin: "Not THE pumpkin. A pumpkin. Still orange, still round, still yours."
- Hay Pile: "Hay. Lots of hay. Katamari rule: you will roll up more hay than you want."
- Fire Wood Pile: "Firewood. It would have warmed someone. It will now warm nobody, efficiently."
- Wagon: "A wagon. The horse is wondering where it went. Keep rolling, the horse is next."
- Cart: "A broken-down cart, now a broken-down cart in orbit."
- Anvil: "An anvil. Heavy. Round now. Smith Argus has questions."
- Tree / Elwynn Pine: "A tree! A whole tree! The birds have been reassigned."
- Sapling: "A sapling from the tree fair. It will grow up round."
- House: "A house! Was anyone home? Do not answer that."
- Stormwind Gate: "The gate! Stormwind is open. Stormwind is also inside you. Both."
- Bridge: "A bridge. The canal is very confused."
- Cathedral of Light: "The Cathedral of Light. It did not RSVP. It came anyway. In you."
- Stormwind Guard Tower: "A guard tower, which saw you coming, which did not help it."
- Hearth / Fireplace: "The hearth has gone out. It has gone out, round, with you."

---

## 5. Audit #2: the text diet

Captures: `l3/route-four-minutes.png` (`Wagon train · 32.00 yd` across the dial's label),
`l2/after-finale-prize-pumpkin.png` (a four-line title over the clump), `l1r/01-nursery-start.png`
(the intro wrapping as "Grow to 2 yd 14 in / across!"), `city/city-first-win.png` (the dial 562 yd under
a 595 yd title).

| text | now | change |
|---|---|---|
| **tier banner** (`level.rs::announce`, `hud.rs` `HudText::Banner`) | 42 px at 22 % height, `Name  ·  32.00 yd`, 3.5 s | **26 px** at **14 %** height (clear of the dial's label, which ends around 26 % on the user's aspect), **the name only** (the dial already shows the size), 2.6 s, fade 0.6 s. The goal beat's banner uses the same slot. |
| **finale / win title** (`FinaleText::Title`) | 104 px (+30 pop), wraps to four lines over the ball; L3 40 px | **one line, sized to fit:** `size = clamp(width_units * 0.9 / (0.58 * chars), 28, 64)` in the 768-unit layout; placed at **12 % height**, above the ball's projected circle (the ball centre sits near 55 %), never over it; the sub line under it. L2's 28-character title lands at about 54 px. |
| **intro** (`goal_lines`, `HudText::Goal` + `GoalSub`) | two lines at the top, "Grow to 2 yd 14 in / across!" + the level title | **one line on the ring:** for a size goal the ring's cap label is the intro for `INTRO_HOLD` 4 s at 1.6× its size, `2 yd 14 in!`, then shrinks to `Goal 2yd 14in`; for a target goal one centred line at 30 px, `Roll up the Prize Pumpkin!`, 4 s then gone. The level title is dropped from play (the hub said it). |
| **size text** | `fmt_size` (results), `fmt_dial` (dial, no spaces), `{:.2} yd` (banner) | **one formatter,** `fmt_size(yards, Compact)`: inches under 10 yd, one decimal to 100 yd, whole yards above; the compact form drops the spaces. The `.2` banner goes away with the size. |
| **finale numbers (L3)** | dial 562 yd under a 595 yd title | during a finale and the send-off the dial shows **the display size** too (`run.final_diameter`), so one number is on screen; the records keep the physical size (D24). |
| **"Most rolled up"** | advertises hay ×307 | unchanged here; #14 (the L3 character pass) caps filler. |

---

## 6. Audit #11: a song per level

I could not listen to the tracks on the build host; the assignment is by title, length and the level's
character, for the user to confirm. All five are the user's own compositions (D13).

| track | length | where | why |
|---|---|---|---|
| **azeroll-title** | | the hub, Ready | as today: the planet and the briefing |
| **goldshire-mambo** | 208.6 s | **L1, Last Call at the Lion's Pride** | the recipe already names it; a mambo is the band still playing after the regiment has gone, which is the inn's joke; its 3.5 min loops once in a 5 min candle |
| **goldshire-go-go** | 179.2 s | **L2, Market Day Mayhem** | "go-go" is the fair's bustle and the chase (69 NPCs fleeing); the brightest title for the brightest level |
| **kuru-kuru-azeroth** | 178.8 s | **L3, Stormwind or Bust!** (first track) | "spinning Azeroth" is the finale's scale, and the Japanese title is the Katamari homage saved for the level that becomes a planet-sized clump |
| **morning-in-goldshire** | 179.2 s | **Free Roll on every level,** and L3's second track | calm, no candle: a morning with nothing to do but roll; L3's 30 min candle needs two tracks so the same 3 min does not loop ten times |
| **ritual** | | no track swap | the send-off ducks the level track 55 %; the results hold it at 50 %; the title theme returns at the hub (§2.7) |

Recipe changes: `music` becomes a list (`music = ["kuru-kuru-azeroth", "morning-in-goldshire"]`; a single
string still parses), L1 keeps `"goldshire-mambo"`, L2 gains `"goldshire-go-go"`, L3 changes from
`"morning-in-goldshire"` to the pair. `music.rs`: a level with a list rotates within it and **does not
advance on a retry** (R restarts the same track from the top); the global rotation only applies to a
level with no `music`. Free Roll switches to `morning-in-goldshire` with the 1 s crossfade.

---

## 7. Implementation brief (Sonnet 5.5)

Build in this order; each step leaves the game playable. Keep engine edits at zero: everything below is
in `azeroll/`.

### 7.1 Step 1: roll past the goal (`level.rs`, `progress.rs`, `hud.rs`, `sfx.rs`, `music.rs`)

- `Run` gains `goal_time: Option<f32>`, `stars: u8`, `called: bool`. `reset_run` clears them.
- `level::tick`: on first `goal_met` for `GoalKind::Size`: `run.won = true; run.goal_time =
  Some(run.elapsed); run.stars = 1;` write a new message `GoalReached { diameter, goal }`; **do not** set
  `flow.finale`; the clock keeps running (`run.elapsed` keeps accruing while `!run.ended`). The time-up
  branch and the called branch both set `flow.ending = Some(Ending::Snuff)` (new) instead of
  `next.set(Results)`; `ritual.rs` takes it from there. For `Collect`/`Finale` keep `flow.finale` as
  today and let the ritual start when it expires (replace `next.set(Results)` in the finale branch with
  `flow.ending = Some(Ending::Captured)`).
- `stars`: in `tick`, after the goal, `run.stars = 1 + grades.iter().filter(|b| d >= b * goal).count()`,
  rising only (write `StarEarned { n }` on each rise; the HUD and `sfx.rs` read it).
- `level::keys` (Playing): track `Enter` held time in a `Local<f32>` (real time); `>= CALL_HOLD` with
  `run.won && !flow.free_roll && goal.kind == Size` sets `run.called = true` and the Snuff ending; also
  publish the hold fraction in `Flow::call_hold: f32` for the clock rim. Reset on release.
- `progress::judge_win(diameter, goal_diameter, elapsed, limit, meta, kind: GoalKind, goal_time:
  Option<f32>)`: Size → size grade only, comet from `goal_time`; others as today. `record_results`:
  for Size, `best_time` takes `goal_time`. Tests: a called run at exactly the goal is Scraped; 2.1× is
  Preposterous; the comet needs `goal_time < par`; L2 unchanged.
- `hud.rs`: the star row inside the rosette (a `HudText::Stars` with `★` glyphs, or four small
  `ArtMaterial` star nodes if the font lacks the glyph: check once), the re-arming ring (`refresh_hud`
  computes fill = `(d - lower) / (upper - lower)` between boundaries after the goal), the cap label
  `★★ 2yd 31in`, the finish hint (`HudText::CallHint`, real-time fade), the clock rim fill (shader
  kind for the clock, `p.z` = hold fraction). Pause text gains the hold line.
- `sfx.rs`: `GoalReached` → quiet fanfare via `emission_distance` (50 %); `StarEarned` → chime
  (`igBonusBarOpen` 3201 pitched by the kit flag; or `PickUpRing` 1193); last-ten-seconds tick; snuff
  bell. `music.rs`: `GoalReached` → `TIER_DUCK`.
- Autopilot (`auto.rs`): after the goal keep rolling the tour (today it stops being useful at the win?
  check `auto.rs` for a `run.won` early-out); add `WOW_AZEROLL_CALL_AT=<secs>` to hold Enter at that
  play time for captures and tests.

### 7.2 Step 2: the ritual (`ritual.rs` new; `progress.rs` loses `finale_fx`'s camera; `select/verdict.rs`; `camera.rs` read-only)

```rust
pub(super) enum Ending { Snuff, Called, Captured, Failed }
pub(super) enum Stage { Settle(f32), Beam(f32), Lift(f32), Hover, Depart(f32), Drop(f32) }
#[derive(Resource, Default)]
pub(super) struct Ritual {
    pub ending: Option<Ending>,
    pub stage: Option<Stage>,
    beam: Option<Entity>,            // RunEntity child of the ball root, not of BallSpin
    cam_before: Option<(f32, f32)>,  // moves here from Flow
    rest_pos: Vec3,                  // where the clump sat; Drop returns it
    view: Option<CamOverride>,       // yaw/pitch/distance for CamRig
}
```

- Systems: `begin` (Playing, when `flow.ending` is set: freeze `Time<Virtual>` to 0, snapshot the camera,
  spawn the beam, start Settle), `advance` (real time through the stages; writes `BallState::pos`'s
  render offset: **do not** move `BallState::pos`; add a `BallState::lift: f32` the ball root adds to
  its translation and the camera ignores, so collision and the autopilot never see the hover), `skip`
  (Enter/Space/Esc in Lift/Beam → Hover and `next.set(Results)`), `exits` (in Results: Enter → Depart
  then Select; R → teardown then Ready; F → Drop then Free Roll), `teardown` on `OnExit(Results)`,
  `OnEnter(Ready)`, `OnEnter(Select)`: time 1.0, override cleared, lift 0, beam despawned.
- The camera: `CamRig` gains `override_: Option<CamOverride>`; `camera.rs` applies it after its own
  radius-driven pose (one small seam, same as the finale's zoom/tilt writes today). Indoors clamps per
  §2.3.
- The beam: a `Mesh3d` cylinder (open ends, 1.3 r × 60 yd, or 1.3 r × ceiling height indoors from
  `WorldPoint::indoors_at`'s ceiling if available, else 6 yd), an additive unlit material with a
  scrolling gradient (`hud.wgsl`-style; reuse `cutout`'s material lane if it has an additive mode), a
  sparkle ring as a particle-less textured quad ring scaling out. No new assets.
- The Prince: `creatures::prince` gets a `Ritual` read: in Lift/Hover he stands 1.2 r from the rest
  point facing the clump and plays EmoteWave (67) once, then stand; in Drop he runs back.
- Fail: `Ending::Failed` runs Settle (camera move) then straight to Hover-without-beam at 3.4 s;
  `verdict.rs` keys the `NOT QUITE` layout off it.
- `verdict.rs`: remove the full-screen dim; dock the panel (wide right 46 %, compact bottom 45 %);
  the headline text nodes beside the clump (`VerdictText::Headline`, `::Title`), positioned from the
  ball's projected screen point each frame (`Camera::world_to_viewport`); the catalogue line; the
  remark from the pool. Results' three buttons unchanged.
- `music.rs`: `RITUAL_DUCK` = (0.55, send-off length); the Results rule → level track at 0.5 gain
  (optional, §2.7).
- `FINALE_SECS` per goal kind (Collect 2.0, Finale 3.2).

### 7.3 Step 3: Fizzwick in play (`hud_models.rs`, `hud.rs`, `lines.rs` new, `sfx.rs`, `campaign.rs`)

```rust
// lines.rs: &'static str tables keyed by level index (LEVELS order) + a generic pool.
pub(super) struct Pool { pub per_level: [&'static [&'static str]; 3], pub generic: &'static [&'static str] }
pub(super) const GOAL: Pool; SIXTY_BEFORE; SIXTY_AFTER; PREPOSTEROUS; LANDMARK; TIER: &[&str];
pub(super) const TRY_AGAIN: Pool; REMARKS: [&[&str]; 4]; CATALOGUE_CATEGORY: [&str; 8];
pub(super) const CATALOGUE_ITEM: &[(&str, &str)];  // sorted by name for binary search
pub(super) fn pick(pool: &Pool, level: usize, used: &mut Vec<usize>) -> &'static str;

// hud.rs
#[derive(Message)] pub(crate) struct FizzSays { pub line: &'static str, pub voice: Voice }
pub(crate) enum Voice { Cheer, Laugh, Gasp, Sigh, None }
#[derive(Resource, Default)] struct Pops { used: u8, goal_reserved: bool, last_at: f32, queue: Option<FizzSays> }
```

- A `pops` system in `hud.rs` reads `GoalReached`, `StarEarned{4}` (Preposterous), `TierReached`, the
  clock crossing 60 and `Collected` with `name == LEVELS[i].landmark`, applies the budget (§3.2) and
  writes `FizzSays`. `LevelEntry` gains `landmark: &'static str`.
- The porthole widget (`FizzPop` node: the commissioner's image handle from `HudModels::commissioner`,
  a bubble text, real-time slide/typewriter); `stage_commissioner` runs in Playing as well; a
  `commissioner_acts` read of `FizzSays` for the gesture and the emote anim.
- `sfx.rs`: resolve the voice kits at startup through `EmoteSounds` (race 7, sex 0) for the text emotes
  in §3.4, log them, play 2D on `FizzSays`. If none resolve, the gesture plays silent; never block.
- `WOW_AZEROLL_FIZZ_TEST=1`: no budget, no gaps, the autopilot shows every event.

### 7.4 Step 4: the Catalogue (`progress.rs`, `names.rs`, `select/catalogue.rs` new, `select/hub.rs`, `verdict.rs`)

- `Progress::catalogue`, `CatalogueEntry` (§4.2), `SaveFile` round trip test with an old save (no
  table) and a new one.
- `Run::log` widens to `(count, axis, model)`; `record_results` merges into the catalogue and fills
  `Outcome::new_catalogue`; the Free Roll write on `OnExit(Playing)`.
- `names::category`; `Design::catalogue_names()` (scenes' display names via `names::display_name`,
  the cast's names, herd species' names).
- `select/catalogue.rs`: the page (§4.3), opened by `C` or the card's line; reuses `hub.rs`'s
  `ArtMaterial` plaque kind and the bubble stage from `hud_models.rs`. `SelectState` gains
  `catalogue: Option<CatalogueView { level, category, row }>`; `select::keys` routes W/S/Enter/Esc to
  it while open.
- `verdict.rs`: the `NEW to the Catalogue` line.

### 7.5 Edge-case checklist (every one needs a test or a capture)

- [ ] Free Roll: no stars, no hint, no call, no ritual; the goal beat is banner + bloom; the drop exit
      restores `cam_before` and time 1.0; the Catalogue still fills.
- [ ] R during Settle/Beam/Lift/Hover/Depart: Ready with time 1.0, no beam, lift 0, no override.
- [ ] Esc during the send-off skips (never pauses); Esc on the results does nothing (as today).
- [ ] Pause during the goal beat holds the banner and bloom; the pop-in finishes under the menu.
- [ ] `WOW_AZEROLL_DEBUG_RADIUS` above the goal: no beat before 0.5 play seconds; stars appear
      correctly on the first growth tick after.
- [ ] L3: the headline reads 595 yd, the dial reads 595 yd during the send-off, the records keep the
      physical size, the Cathedral trophy rides the hover; the city reveal and fog are unchanged
      through Hover and restored on every exit (D26's "interruption clears the mask" stays).
- [ ] L1 indoors: the beam starts at the ceiling, the lift is 1.0 r, the lens never leaves the room.
- [ ] Time-up and goal on the same frame → a win.
- [ ] Shedding below the goal after the goal: stars stay, the grade uses the final size, `won` stays.
- [ ] Three pops max; the goal always gets one; none in the first 6 s; none in Ready/Paused/finale.
- [ ] Unvoiced emote for gnome male → silent gesture, logged once, no panic.
- [ ] An old save without `[catalogue]` loads; a new save round-trips; a name found in two levels keeps
      its first level.
- [ ] Narrow aspect (0.92, the user's captures) and 1.78: the panel docks without covering the clump;
      the headline is one line; the pop-in bubble is two lines at most.
- [ ] wasm: no `std::time`; `web-time`/`bevy::platform` only; `cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu`.

---

## 8. Acceptance (captures and tests)

Captures go under `/home/jjd/az-patches/ritual/`, native, the user's ~0.92 aspect and `WOW_AZEROLL_UI_ASPECT=1.78`.
Existing switches: `WOW_AZEROLL_RESULT_SHOT`, `_RESULT_DEMO`, `_LOOP_SHOT`, `_HUB_SHOT`, `_DEBUG_RADIUS`,
`_AUTO`. New: `WOW_AZEROLL_CALL_AT=<secs>`, `WOW_AZEROLL_RITUAL_SHOT=<png>` + `_AT=<real secs from the
ending>`, `WOW_AZEROLL_FIZZ_TEST=1`, `WOW_AZEROLL_CATALOGUE_SHOT=<png>`.

| # | capture | what must be true |
|---|---|---|
| 1 | L1 autopilot, the frame of the goal beat | ring full and gold, one star under the size, the small one-line `LAST CALL!` banner at 14 %, Fizzwick's porthole under the clock with a GOAL line, clock still running (compare two frames 1 s apart) |
| 2 | L1, goal + 4 s | the hint `hold ⏎ to call it` under the clock; the cap label reads `★★ …` |
| 3 | L1, `CALL_AT` = goal + 20 | the clock rim filling; the next frame after 1 s: 00:00, bell logged, Settle |
| 4 | L1 send-off at 0.8, 2.0, 3.4 s | the beam from the rafters, the clump lifting then hovering, the headline one line beside it, the panel docked right (wide) / bottom (compact), no full-screen dim, the Prince waving |
| 5 | L1 results → Enter | the departure frame at 0.5 s (clump shrinking, star glyph), then the hub with the star news |
| 6 | L1 results → F | the clump back on the floor, the beam gone, the player's camera, the play HUD (no stars) |
| 7 | L1 fail (`AUTO` with a tiny throttle, or `DEBUG_RADIUS` below the goal and no movement) | `TIME'S UP`, the clump lit on the ground, Fizzwick's try-again line, `NOT QUITE` panel with Roll again first |
| 8 | L2 capture | the pre-roll 2.0 s then the send-off outdoors; the title `PRIZE PUMPKIN, ALL ROLLED UP!` on one line at 12 % |
| 9 | L3 capture (`city` fixture) | the Cathedral riding the hover, headline `595 yd`, dial `595yd`, the city visible under the beam |
| 10 | `FIZZ_TEST` run | three porthole frames (goal, sixty, Preposterous); the fourth event logged as dropped |
| 11 | Catalogue page, wide and compact | three levels with `N of M · k roaming`, a category open, a model in the bubble, a quip |
| 12 | results after a run with new names | the `NEW to the Catalogue` line |
| 13 | tier banner frame (any level) | 26 px, name only, not touching the dial's label |
| 14 | L1 intro frame | one line on the ring, `2 yd 14 in!`, no title at the top |
| 15 | the `azeroll music:` log of an L3 run | kuru-kuru first, morning-in-goldshire at the handoff, a retry starting kuru-kuru again |

Tests (`cargo test -p benilla-app --lib azeroll`): the verdict table (§1.5), star counting, the pop
budget (goal reserved, gaps, priority, Free Roll none), `lines::pick` never repeating within a session
and never panicking on an empty per-level table, `names::category` on the signature names, the
catalogue save round trip, the ritual's stage machine (every exit ends with time 1.0 and no override),
`fmt_size` for the three bands, the finale title's fit function on the three titles.

Not verifiable on the build host, for the user's Windows playtest: whether the quiet fanfare reads as
"goal, keep going" rather than "the end"; whether the gnome voice kits are funny or grating at three per
run; whether the hover's bob is steady enough for the user's camera taste; the song assignment.
