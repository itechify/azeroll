# Azeroll → Katamari Damacy REROLL: gap audit 2 (the campaign)

Audit date: 2026-11-02. Scope: the three-level campaign as it stands in PROGRESS.md "State (2026-11-03)":
L1 Lion's Pride v6, L2 Market Day, L3 Stormwind, the planet hub and the verdict. This audit replaces
[katamari-gap-audit.md](katamari-gap-audit.md) (Goldshire → Hogger), most of whose batches have landed.
Its question is narrower than that audit's: **what would most make a first-time player feel they are
playing Katamari, in Azeroth?** It favours feel, fun and polish over feature count.

Method: read-only. I read the decisions (D3–D29), PROGRESS, the four research notes, the azeroll
modules where a claim needed checking, and the captures under `/home/jjd/az-patches/{l1r,l2,hub2,l3,l3b,climb}`.
I built nothing and ran nothing. Captures come from different revisions and fixtures, so I treat
their clocks as illustrative, never as pacing evidence. Evidence grades are as in
[katamari-reference.md](katamari-reference.md): **[C]** a retrieved source, **[S]** a search
snippet only, **[F]** a fan claim, **[I]** my inference.

Effort: **XS** under an hour · **S** half a day · **M** one to two days · **L** three days or more.
★ = a cheap win (high impact for XS/S effort). Agent: **Sol** = Sol 6.1 High (engineering),
**Son** = Sonnet 5.5 (implementation), **Fab** = Fable 5.1 (design/UX/writing), **Hai** = Haiku 5.5 (small).

## Ranked table (impact ÷ effort)

| # | Gap | Impact | Effort | Agent | Write set |
|---:|---|---|---|---|---|
| 1 | ★ The run ends on the goal; Katamari keeps rolling until the clock runs out | High | S | Fab (beat, lines) → Son | `level.rs`, `progress.rs`, `select/verdict.rs` |
| 2 | ★ Play-screen text is too big and too long (tier banners, finale title, intro wrap) | High | XS–S | Son (Hai can do it) | `hud.rs`, `level.rs` |
| 3 | ★ Dash is a key tap, not a wind-up (the Prince never runs on the spot) | Med-High | S | Son | `ball.rs` |
| 4 | ★ L2's limit breaks the user's ≤ 40 % rule (≈ 300 s of 620 s = 48 %) | Med-High | XS | Hai | `goldshire.toml` |
| 5 | ★ Visible data errors in the finale and results | Medium | S | Hai (verify) → Son | `progress.rs`, `select/verdict.rs`, `hud.rs` |
| 6 | No send-off: the clump never goes to the sky, and the results screen hides it | High | M | Fab (staging) → Son | `progress.rs`, `finale.rs`, `select/verdict.rs`, `camera.rs` |
| 7 | The commissioner is silent in play (the King talks; Fizzwick only talks in the hub) | Med-High | S–M | Fab (lines) → Son | `hud_models.rs`, `hud.rs`, `sfx.rs` |
| 8 | Props and trophies inside the inn render overbright (L1, the first impression) | High | M (diagnosis) | Sol | lighting / `wow_model.wgsl` lane, `props.rs` |
| 9 | Keyboard-only, digital tank steering; Katamari is a dual-stick game | High | M | Son | `ball.rs` (+ new `input.rs`) |
| 10 | No collection log: what you rolled up is forgotten after the results | High | M | Fab (quips) + Son | `progress.rs`, `select/*`, `names.rs` |
| 11 | ★ Every level shares one playlist; Katamari gives each stage its own song | Medium | XS | Hai | the three recipes' `music`, `music.rs` |
| 12 | ★ Size tier beats are invisible (+5 % boom over 3 s) | Medium | XS–S | Fab (decide) → Son | `camera.rs`, `sfx.rs` |
| 13 | ★ Sleepers lying flat on the market road read as corpses | Low-Med | XS | Hai | `goldshire-npcs.toml` |
| 14 | Huge clumps read as a brown rock trailing hay piles (L3) | High for L3 | L | Sol (shell) + Fab (variety) | `lod.rs`, the shell, `stormwind.toml` |
| 15 | No Royal Presents or cousins: nothing hidden per level | Med-High | M | Fab (placement) → Son | recipes, `progress.rs`, `creatures.rs` |
| 16 | Lumpy rolling is too subtle to feel (the pencil arc is gone) | Medium | M | Sol | `ball.rs`, `lod.rs`, `creatures.rs` (Prince) |
| 17 | Big NPCs between the lens and the ball block the view (L1 kitchen) | Medium | S–M | Son | `cutout.rs`, `props.rs` |
| 18 | ★ Knock-offs come from the newest trophies, not the side that was hit | Low-Med | S | Son | `collect.rs` |
| 19 | No stage arrival: play starts with a banner, not with the Prince arriving | Medium | M | Fab → Son | `level.rs`, `creatures.rs`, `hud.rs` |
| 20 | Human pace is unmeasured; the autopilot is the only player | Medium | M | Son | `auto.rs` |
| 21 | The see-through window's stipple ring is visible noise | Low-Med | M | Sol | `cutout.rs`, the two shaders |

**Do first:** 1–5 together. They take about two days for one Sonnet and one Haiku, and they change how
every run feels and ends. Then 6–7 as one "ritual" track, 8 in parallel on Sol, then 9–10. Item 14 is
part of PROGRESS "Next" #2 (the L3 character pass), so fold it in there rather than starting it twice.

**Keep these.** The item bubble with its model, name and ×N combo; the corner Prince; the rosette;
the planet hub with its stars and pins; the graded verdict with a comet; NPC panic ripples and
carried NPCs that still animate (`npcs.rs`, the newest eight); L1's book stairs and food lines;
L2's Prize Pumpkin, foreshadowed from the first frame (D28). The captures show these working.

---

## Findings

### 1. ★ End on the candle, not on the goal
- **Katamari.** MaS1 is documented to end automatically at its goal only on the *first* play; replays
  run to the timer and grade the final size as Minimum 10 / OK 12 / Great 14 / Super 15 cm
  ([Katamaripedia MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) [C].
  The King criticises a ball that barely cleared the goal and praises a big margin
  ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C]. Guides and reviews say that
  after the goal you keep rolling until time runs out, for a bigger ball (search summary of
  StrategyWiki and reviews) [S]. Note: [campaign-research.md](campaign-research.md) §4 says "the stage
  ends on the goal". That is true of the first MaS1 play, but the grade bands above contradict it.
- **Azeroll.** "A run ends on its goal" (D21, D24 *Verdict*). Because of that, the size grade only
  measures overshoot, so pace decides the grade. L1's autopilot wins in 67 s of 300 and the player
  never sees the room become small.
- **Impact.** The best part of Katamari, the minute after you are big enough when the room is all yours,
  is cut off. A human who wins also loses the chance to recover a poor grade.
- **Change.** When the goal is first met, play a 2 s non-blocking beat: the ring completes, the
  commissioner's line, and the fanfare at half level. Keep the clock running. The run ends when the
  candle burns out, or on a held "Call it" key (Enter, 1 s) for players who want out. The verdict
  grades `final diameter / goal` against the existing `[level] grades`. The comet keeps its meaning:
  the goal reached under par (record `goal_time`). Terminal finales (L3's Cathedral) still end at
  once, because there is nothing left to eat. L2's Pumpkin becomes the goal-reached beat, and rolling
  up the rest of Goldshire afterwards is exactly the Katamari joke.

### 2. ★ A text diet for the play screen
- **Katamari.** In play the only words are the item name in the bubble, plus the goal on the dial
  (user's REROLL screenshot, katamari-reference.md "HUD") [C].
- **Azeroll.** Captures show `Wagon train · 32.00 yd` across the screen, overlapping the dial's
  `Goal 64.0yd` label (`l3/route-four-minutes.png`; banner built at `level.rs:534` with `.2`
  decimals). `PRIZE PUMPKIN, ALL ROLLED UP!` fills four lines over the clump at the moment of
  triumph (`l2/after-finale-prize-pumpkin.png`). The L1 intro wraps as "Grow to 2 yd 14 in /
  across!" across the top (`l1r/01-nursery-start.png`).
- **Change.** Tier banners at about 60 % of their current size in the upper third, the tier name only
  (the dial already shows the size), never covering the dial. Finale titles go on one line, sized to
  fit, and never cover the ball's projected circle. Make the intro one line ("2 yd 14 in!") on the
  goal ring itself. Size text uses one formatter everywhere: the banner, the dial and the results.

### 3. ★ A real dash wind-up
- **Katamari.** Dash is charged: you waggle both sticks while the Prince runs on the spot, then the
  ball bursts forward ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari),
  [Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
- **Azeroll.** Shift starts a 0.65 s dash immediately (`ball.rs:812`). `Dash::WindUp` is reserved and
  never emitted (D17), although the Prince's run-on-the-spot (D7) and the Whirlwind whoosh (D19) are
  already wired to it.
- **Change.** Holding Shift charges for 0.45 s. The ball slows and the Prince scrambles, emitting
  `WindUp`. Releasing it dashes, with the burst scaled by charge. A quick tap stays a weak dash, so
  keyboard players lose nothing. On a gamepad (#9), alternating the sticks charges it.

### 4. ★ L2's time limit
- **User rule.** The autopilot should win in at most ~40 % of the limit (PROGRESS "Direction"). The
  user could not beat old L1.
- **Azeroll.** `goldshire.toml` `time_limit = 620`; the autopilot wins in about 5 min (PROGRESS) = 48 %.
  L1 (22–32 %) and L3 (8–10 min of 1800 s, ~30 %) pass.
- **Change.** Set `time_limit = 780` and move `par` with it, or re-measure after the character pass and
  then set it. With #1 in place, a longer limit becomes the time a player spends being huge, not
  slack.

### 5. ★ Visible data errors
- `hub2/6-results-verdict-wide.png`: "Best find: Prize Pumpkin (1 yd 29 in)". The pumpkin's pickup axis
  is 8.62 yd and needs an 18.5 yd ball (D28), so `find_size` is in another unit or comes from the wrong
  box. Check `progress.rs` `find_size` against `Collectible.pickup`.
- `l3b/big-clump-after.png`: the dial reads 553 yd under a title reading 595 yd. This is deliberate
  (D24: the ball's own size, not the terminal trophy), but on screen it looks like a bug. Show one
  number during the finale.
- `l3b/cathedral-exterior-*.png`: the corner Prince window shows the city, or a dark ghost of the
  Prince. This may be a fixture artefact; verify it on a normal-start finale.
- `l3b/normal-win-9m47.png`: "Most rolled up: Shadowfang Hay Pile ×307 · Fire Wood Pile ×305".
  This is accurate, but it advertises filler (see #14).

### 6. The send-off: the clump goes to the sky, and the results show it
- **Katamari.** "At the end of each stage, the katamari is lifted into the night sky and becomes a
  constellation or a star" ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C];
  "it will turn into a star that colors the night sky" (Nintendo eShop listing, snippet) [S]. The
  final size picks one of four ending dialogues (MaS1) [C]. The rainbow, cosmic opening imagery is
  documented by [Reale](https://epub.uni-bayreuth.de/321/1/ACT2011_02_Reale.pdf) [C]; the King's
  rainbow as the exit is [?].
- **Azeroll.** The finale is 3.2 s of slow motion and a pull-out (none indoors: `finale_fx`,
  `progress.rs:431`). It cuts to a results panel over a darkened, empty world (`hub2/6`,
  `l1r/r4-results.png`). The star first appears on the hub on the *next* screen. The clump the player
  built is never shown off. Old audit #20 raised this and it is still open.
- **Change.** Between the finale and the results, add a 4–5 s skippable send-off. Time freezes. The
  camera eases to a three-quarter view of the clump. A prismatic beam lands on it (for Warcraft
  identity: Fizzwick's gnomish "Ultrasafe Transporter", a ribbon of arcane light), the clump lifts,
  spins, shrinks and pops into a star with the best-find name. Then dock the results panel to one side
  and keep the world camera slowly orbiting the *lit* clump behind it, without dimming it. Indoors
  (L1), play the beam inside the room, which avoids the D8 exterior-lens problem.

### 7. The commissioner's voice in play
- **Katamari.** The King is the game's personality: he briefs, judges and needles the Prince
  ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C]. His voice is a record scratch
  rather than speech [F].
- **Azeroll.** Grand Tinker Fizzwick speaks only in the hub and on the results screen (D24); in play
  the corner holds only the Prince (D20a). The `sfx.rs` cues are generic UI sounds.
- **Change.** Give Fizzwick a small pop-in portrait for three moments per run: the goal reached (#1),
  60 s left, and a "Preposterous" size. He gets one typed line and a gnome emote voice from the game's
  own character sound kits (cheer, laugh, "oops") as his "record scratch". Hold each line to 2.5 s.
  Fab writes about 30 lines (per level, per grade); Son reuses the commissioner stage from
  `hud_models.rs` in play.

### 8. Overbright props in the inn
- **Evidence.** In `l1r/01`, `04` and `05` the muffins, cheese, books and bread on the floor and on the
  clump glow pale yellow-white next to the warm, darker WMO. The loose table in
  `climb/ball-on-table.png` is blown out. Outdoor L2 props look correctly lit. The likely cause is that
  mode-spawned entities indoors get exterior sun or ambient light, not the interior lighting the
  WMO's own doodads get. This is unverified.
- **Impact.** It is the first thing a new player sees. The clump reads as a glowing blob rather than
  "the supper", so the joke of recognisable objects is lost.
- **Change.** Sol: compare an inn-claimed doodad (D22) with a recipe prop of the same model in one
  frame, and route mode props through the same interior light resolve (`lighting/resolve.rs`,
  `indoors_at`). Keep the change generic per AGENTS.md.

### 9. Dual-stick controls
- **Katamari.** Both sticks forward rolls; one forward and one back spins; both sideways rolls
  sideways, more slowly. Clicking both sticks quick-turns, and REROLL also offers a simple mode
  ([Wikipedia](https://en.wikipedia.org/wiki/Katamari), [Steam](https://steamcommunity.com/app/848350/discussions/0/4633736978453499873)) [C/F].
- **Azeroll.** W/S/A/D digital tank controls only, with a constant turn rate (`ball.rs:654–667`), no
  sideways roll and no gamepad. `bevy_gilrs` is already in `Cargo.lock` (Bevy's `default_platform`),
  so gamepad input is available but nothing reads it.
- **Change.** Add a `Gamepad` reader that maps two analog sticks to (forward, turn, strafe) with
  classic tank mixing. Add a sideways roll at about 55 % of top speed, quick-turn on L3+R3 (or both
  triggers), and a simple mode (left stick moves, right stick turns). Keyboard: keep WASD and add IJKL
  as the second "stick" for purists. The user tests on a Windows PC, where a pad is likely.

### 10. A collection log ("Fizzwick's Catalogue")
- **Katamari.** Every object ever rolled up goes into the Collection, with a line of text. Cousins
  count toward 100 % ([Katamari wiki: Cousins](https://katamari.fandom.com/wiki/Cousins),
  [Wikipedia](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C/F]. The log adds "lighthearted
  learning" about its objects (campaign-research.md §1a) [C].
- **Azeroll.** `Run::log` exists per run, but only "most rolled up" and the best find survive into the
  records (D24).
- **Change.** Persist a set of unique names per level (and the first level each was found in) in
  `Progress`. Add a hub card listing `N / total` per level, with the model in the bubble renderer and
  a one-line Fizzwick quip (Fab writes about 80 lines for the signature items; the rest get a generic
  line per category). Show "NEW to the Catalogue: N" on the results screen.

### 11. ★ A signature song per level
- **Katamari.** Each stage has its own song; the music carries the identity (Reale, K6 in the old
  audit) [C].
- **Azeroll.** One four-track playlist rotates through every level; a recipe `music` key exists
  (`music.rs:182`).
- **Change.** Set `music` per recipe: for example, L1 "morning-in-goldshire", L2 "goldshire-mambo",
  L3 "kuru-kuru-azeroth". Ask the user which they intend. Don't advance the rotation on a retry.

### 12. ★ A tier beat you can feel
- **Katamari.** The camera zooms out when the ball fills the screen ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
  The pull-back is the felt moment of "the world just got smaller" [I].
- **Azeroll.** +5 % boom and +1° per tier over 3 s (D8 *Scale grammar*), which is imperceptible. The
  user asked that the view never move by itself, so yaw and glances were removed deliberately.
- **Change.** Keep it zoom-only and widen-only, but make it a single 0.6 s ease of +12 % paired with a
  whoosh and the rosette bloom. Fab confirms with the user before it ships.

### 13. ★ Sleepers in the market
`l2/before-market-potter-r1.7.png`: two villagers lying flat in the road read as dead bodies. Move
`sleeper` quirks onto hay bales, benches or the beer garden, or swap them to the sit-sleep pose.

### 14. Readable huge clumps (L3)
- **Katamari.** At set sizes (12 m, 60 m, 200 m) low-poly stand-ins "suggest more items than are
  actually attached" ([Katamaripedia: Katamari](https://katamaripedia.miraheze.org/wiki/Katamari)) [C].
- **Azeroll.** At 18–35 yd the shell is a brown cracked-stone sphere with orange hay piles stuck on
  (`l3b/run3-shots/play-002.png`, `l3/route-four-minutes.png`). At 553 yd it is a trail of identical
  hay piles (`l3b/big-clump-after.png`). The results confirm hay ×307 and firewood ×305.
- **Change.** (a) Fab, in the L3 character pass: cap any one model at about 40 per area and mix the
  sets. (b) Sol: tint the D3 shell from the average colours of the buried trophies, give it an
  item-lump displacement rather than rock cracks, and always keep the 20–30 largest recent trophies
  seated proud of the shell so the silhouette changes with what was eaten.

### 15. Royal Presents and cousins
- **Katamari.** Each stage hides a Royal Present (a wearable for the Prince) and a cousin
  ([Wikipedia](https://en.wikipedia.org/wiki/Katamari_Damacy); Marcy under the MaS1 table at 6 cm 8 mm,
  [MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) [C].
- **Azeroll.** Nothing is hidden. The challenges are the only optional goals.
- **Change.** Hide one gift box per level in a set piece (a "Winter Veil" style present model if 1.12
  has one) that unlocks a Prince costume, which is a display-id swap like `WOW_AZEROLL_COMMISSIONER`.
  Hide one "cousin" per level (a named gnome or goblin, e.g. under the captain's table in L1) that
  unlocks as an alternative pusher. Show both on the hub card. This builds on #10.

### 16. Lumpy rolling you can feel
- **Katamari.** A pencil makes the ball "roll in a large arc"
  ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **Azeroll.** Tips deliberately no longer lever the collider; the CoM wobble is capped at ±7.5 %
  speed (D4) to protect D6/D16 traversal.
- **Change.** Keep the collider as it is. Add a render-only bob of `BallSpin` when a protrusion longer
  than 0.6r sweeps through the ground (a damped spring, a thud, a Prince stumble). While a single
  dominant long trophy outweighs the rest, raise the heading drift. Fade both as the ball grows past it.

### 17. Units in the lens corridor
`l1r/07-kitchen-cheese-trail-brog.png`: an NPC between lens and ball fills half the screen; the
cutout only stipples part of his torso. Fade non-Prince units within the lens-to-ball corridor to
about 30 %, the way `props::occlusion_fade` does for trophies.

### 18. ★ Shed from the side that was hit
`collect.rs` `shed` sorts by `at` (newest first). Katamari loses things on impact [C], and a reader
expects them to come off where the hit was [I]. Sort by `dot(trophy direction, impact normal)`, keep
the size cap, and play each piece's material `PutDown*` sound as it lands.

### 19. A stage arrival
Katamari drops the Prince into the stage after the King's briefing (campaign-research §1a) [C/I].
Azeroll opens on a banner. Add a 2.5 s skippable drop-in: the ball and Prince fall from above with the
goal ring filling and Fizzwick's one-liner (#7), and input unlocks on landing.

### 20. Human pace
The ≤ 40 % rule is measured with an autopilot that never hesitates. Add a "human proxy" mode in
`auto.rs` (70 % throttle, 0.4 s reaction delay, a random detour every 20 s, no teleports) and report its
win time beside the autopilot's. Most important of all: Taildrop a build to the user after items 1–5
land. A human playtest outranks every number here.

### 21. The see-through window's edge
`l1r/04` and `09` show a coarse stipple ring and jagged cut edges on the bar counter. Narrow the dither
band and use a blue-noise threshold, or fade the band over two frames. This is low priority; the
window works.

---

## Sources
- Takahashi postmortem (2004): https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-
- Wikipedia, *Katamari* and *Katamari Damacy*: https://en.wikipedia.org/wiki/Katamari · https://en.wikipedia.org/wiki/Katamari_Damacy
- Katamaripedia, Make a Star 1 (grades, first-play end, Marcy): https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1
- Katamaripedia, Katamari (low-poly stand-ins): https://katamaripedia.miraheze.org/wiki/Katamari
- Katamari wiki, Cousins: https://katamari.fandom.com/wiki/Cousins
- Diabolical Plots (controls, dash): https://www.diabolicalplots.com/tag/katamari-damacy/
- Steam REROLL control thread: https://steamcommunity.com/app/848350/discussions/0/4633736978453499873
- Reale, *Chaos in the Cosmos*: https://epub.uni-bayreuth.de/321/1/ACT2011_02_Reale.pdf
- In-repo: katamari-reference.md, campaign-research.md, level-design-research.md, katamari-room-design.md.
