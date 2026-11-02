# Azeroll → Katamari Damacy REROLL: gap audit

Audit date: 2026-11-02. Scope: make Goldshire → Hogger exceptional before extending geography.

## Verdict

Azeroll has a real Katamari foundation: a fixed core carrying recognisable world objects, increasingly absurd silhouettes, authored domestic scenes, a fixed-size gnome, frightened villagers, creature voices, original music, size/pickup portraits, records and Free Roll. The best captures already make collecting Goldshire's belongings look funny. Preserve these investments.

What still most clearly separates it from Katamari is **continuity**: seeing the next meal, rolling to it without inexplicable obstruction, feeling it join the clump, and discovering that yesterday's intimidating obstacle is today's food. Dense scenes are separated by weak progression links; foliage and trophies obscure the action; the Prince follows rather than visibly pushes; scale is mostly a number and a longer boom. Hogger can consequently read as a Warcraft target requiring resource accumulation rather than the punchline of a joyful transformation of Goldshire.

The collision work is a prerequisite, not another backlog item. This audit credits its three-state contact rule, additive box-volume growth, centre-of-mass wobble, angular-speed scaling and building-floor repair as landing soon. Remaining findings are integration seams, presentation gaps or unverified outcomes after that work.

## Evidence and limits

- Inspected the mode’s Rust modules, both embedded TOML recipes, architecture decisions and relevant sections of the seven requested research/design documents. Code citations below use `crates/benilla-app/src/azeroll/` as their prefix. Line numbers describe the working tree read during this audit and may move while the other agent edits.
- Directly inspected `/tmp/az-hud3/play-003.png`, `/tmp/az-fade-r8b/play-002.png`, `/tmp/az-fade-r8b/finale-1.png`, `/tmp/az-fade-r4b/play-010.png`, `/tmp/az-npc/play-012.png`, `/tmp/az-hud2/results.png`, `/tmp/az-scenes-shots/v-inn/play-003.png`, `/tmp/az-scenes-shots/v-camp/play-003.png`, and `/tmp/az-level-final2/play-014.png`. These span different revisions and several debug-size fixtures. Their clocks and sizes **do not establish natural pacing**. Old HUDs do not establish current HUD defects.
- No game launched, binary built, code modified, debug variable added or progress save created. Existing captures were sufficient for visual findings; a competing collision edit makes a new run a poor stable baseline. Steering, sound mix, wall climbing and current performance remain code-based hypotheses until played. Historical telemetry is labelled below.
- This is the requested review sub-agent's own work; no further delegation.

### External reference key

Links are evidence, not implementation specifications. Exact REROLL physics constants were not recovered. Original-game design testimony is distinguished from REROLL observations.

- **K1 — creator testimony:** [Keita Takahashi's 2004 postmortem, republished in full](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-). Scale research, simple rolling, presentation, audio, absorption and camera rationale. Per-entry summaries are deliberately brief; proposed constants here are ours.
- **K2 — REROLL play review:** [PC Gamer](https://www.pcgamer.com/katamari-damacy-reroll-review/). Evaluates the remaster's controls, growth and personality. A review is first-hand play testimony, not a physics specification.
- **K3 — REROLL play review:** [Twinfinite](https://twinfinite.net/reviews/katamari-damacy-reroll-review/). Search excerpts describe eccentric controls and getting stuck; full-page retrieval timed out. Treat this as snippet-level support only. Fidelity does not require reproducing those frustrations.
- **K4 — original-game controls and route guide, hosted under REROLL:** [aechris, GameFAQs](https://gamefaqs.gamespot.com/switch/248069-katamari-damacy-reroll/faqs/29492). Forward pressure for wall traversal and size-dependent routes. The guide predates the remaster: useful for inherited mechanics, not proof of REROLL-specific timings or bindings.
- **K5 — official REROLL page:** [Bandai Namco](https://www.bandainamcoent.com/games/katamari-damacy-reroll/). Confirms Prince Dash and the Switch-specific gyro variant; it does not specify acceleration curves.
- **K6 — first-hand musical/visual analysis:** [Steven Reale, *Chaos in the Cosmos*](https://epub.uni-bayreuth.de/321/1/ACT2011_02_Reale.pdf). Playful music and the opening's dancing, rainbow and cosmic imagery; also describes frightened humans and resisting police. Original-game evidence.
- **K7 — explicit weakness in evidence:** [player discussion of near-threshold wobble](https://www.reddit.com/r/katamari/comments/1ohmcli), [object knock-up sound recollection](https://www.reddit.com/r/katamari/comments/oeaw0f). Community recollection across games, not verified frame analysis of REROLL.
- **V1 — video index, not watched footage:** [GameCorner's REROLL full playthrough](https://www.youtube.com/watch?v=tgUJSKCln4w), search description indexes Make a Star 4 at **00:39:59**. [Second playthrough](https://www.youtube.com/watch?v=XpXizjq_O0I) indexes it at **35:43**. Direct video retrieval failed and no transcript was obtained. Neither is used to assert a frame-specific animation or Royal Rainbow timing. These are targeted follow-up viewing points.

A literal Royal Rainbow reproduction is unnecessary. The transferable target is a recognisable arrival/departure ritual and a larger-than-life commissioner reacting to the player's creation. Exact transition duration, object-wobble amplitude and camera breakpoints remain observational questions, not facts supplied by this audit.

## Ranked gaps

Rank is impact on this level's player experience, not implementation order. **S** ≈ a focused task, **M** ≈ several connected changes, **L** ≈ a subsystem/engine seam with substantial verification. Estimates assume existing assets. Each acceptance condition is a proposed test, not a result.

### 1. Traversal still needs a no-stall contract

- **Katamari:** forward pressure can carry the ball over eligible ledges (K4); getting trapped is also a known flaw (K3), not a goal.
- **Azeroll:** `ball.rs:331–358,551–620` samples support and sweeps a squashed sphere; `unstuck.rs:79–110` waits four seconds and teleports to distant safe history. Architecture's final2/final3 samples document a win versus a long smithy stall; these predate the new floor work.
- **Why:** repeatedly bonking and rewinding destroys flow and invalidates growth tuning.
- **Minimal fix:** after the floor patch lands, record inn/vestibule/smithy approaches at r=0.5/1/1.8, diagonal contacts and reverse escape. Repair residual support/sweep disagreements in `ball.rs`; make `unstuck.rs` prefer an escape heading and suppress immediate repeated recovery to the same approach. Keep manual recovery as fallback. Do not raise growth to compensate.
- **Size/deps:** M; collision landing. Accept: human traversal without automatic recovery on these fixtures, with full-radius floor support retained.

### 2. The playable ball disappears behind foliage or inside the camera's obstruction compromise

- **Katamari:** the behind-ball camera is described in the [Cubed3 REROLL review](https://www.cubed3.com/games/reviews/xbox-one/katamari-damacy-reroll) (search excerpt only). Readable route framing is our design criterion, not a claim that its camera is flawless.
- **Azeroll:** `camera.rs:181–226` protects the largest trophy extent by imposing a minimum boom even when a wall is nearer; terrain-only eye lift follows at 234–236. `props.rs:57–69` conservatively ghosts obstructions. `/tmp/az-fade-r4b/play-010.png` is almost entirely an orange surface; `/tmp/az-fade-r8b/play-002.png` still hides much of the ball among trunks/leaves. Older `/tmp/az-level-final2/play-014.png` establishes the original severe canopy case.
- **Why:** invisible contact geometry turns steering into guessing.
- **Minimal fix:** in `camera.rs`, distinguish desired silhouette clearance from a physically valid lens position; test several boom directions/height candidates before accepting a blocked minimum. In `props.rs`, fade the nearest blocking render parts with hysteresis while preserving ground-contact cues. Audit WMO occluders separately: claimed-prop fading cannot promise full building coverage.
- **Size/deps:** M; stable trophy extents. Accept matched before/after inn, smithy and camp angles at small/medium/large sizes.

### 3. Growth pacing is unvalidated after the volume-law change

- **Katamari:** map and object placement are developed together (K1).
- **Azeroll:** `collect.rs:216–231` now credits actual filled boxes; `design.rs:764–829` checks inventory bands, not route-accessible growth per second. `goldshire.toml:4` gives 620 seconds. Architecture's 488.5-second win and 620-second timeout used superseded growth; they are not current balance evidence.
- **Why:** a recipe balanced for a different growth law can produce either immediate domination or starvation.
- **Minimal fix:** extend `auto.rs` telemetry with current position, diameter, volume gained by area, seconds since meaningful pickup and recovery count. Use `design.rs` to report available volume near route checkpoints at the current gate. Rebalance **counts and object mixes in `goldshire.toml`**, leaving the physical law stable.
- **Size/deps:** M; #1 and collision. Accept two different complete routes with no long involuntary growth plateau; target Hogger around 8–10 minutes, then confirm with a human novice rather than autopilot alone.

### 4. Size bands exist globally but not necessarily along the next reachable path

- **Katamari:** routes become useful as size changes (K4).
- **Azeroll:** `goldshire.toml:866–927` trails use 2–10-yard spacing and reusable model mixtures; the validator totals area inventories. An edible trail can become negligible food long before a large ball reaches its next scene. Far farms are hundreds of yards from the hub.
- **Why:** collecting becomes commuting through Elwynn rather than a continuous chain of tempting decisions.
- **Minimal fix:** author one compact primary loop: porch → merchant → woodcutters → camp, with a visible alternative at each junction. Put the next band's meals in sight before the current band's food runs out. Replace long low-value breadcrumbs with size-appropriate clusters; keep farms optional until their travel pays off. Only `goldshire.toml` needs initial changes.
- **Size/deps:** M; #3's measurements. Accept: on each intended transition, a useful next pickup is visible within roughly one camera view; measure real empty travel separately from intentional scenic pauses.

### 5. Visible size, animal reactions and tier promises disagree

- **Katamari:** evaluating edible versus dangerous objects is central to routing (K2); edge ambiguity is a known series frustration.
- **Azeroll:** `collect.rs:108–115` gates on `pickup`; `creatures.rs:371–372` distinguishes sphere `size` from maximum-axis `pickup`, but AI uses `col.size <= reach` at 486–517. `level.rs:40–46` announces “NPCs and wagons” at D=3.44; documented adult axes 1.93–2.01 imply D≈4.15–4.32. `Collected.size` and audio fill also remain sphere-based (`mod.rs:245–251`, `sfx.rs:313–329`).
- **Why:** fleeing and banners can falsely teach that something is edible.
- **Minimal fix:** unify **eligibility** on `collect::contact`/`pickup` in `creatures.rs`; retain sphere size only for spatial reach. Recheck tier thresholds/names in `level.rs` against resolved representative models. Later separate visual extent from pickup threshold in `Collected` consumers rather than silently changing `size` everywhere.
- **Size/deps:** S for AI/tier correction, M for message cleanup; collision landing. Accept boundary checks at dislodge and roll-up thresholds for rabbit, cow, adult and Hogger.

### 6. Steering has momentum, but its handling envelope is not established

- **Katamari:** paired-stick pushing and deliberate directional control are distinctive (K1/K2); a keyboard equivalent need not copy every binding.
- **Azeroll:** `ball.rs:446–509,527–549` has digital tank input, constant heading turn, symmetric forward/reverse target speed, turn drag, coasting and slope pull. New ω·r scaling is already credited here. No current hands-on comparison validates turn radius or reversal.
- **Why:** weight should yield controlled arcs, not sluggish correction or a vehicle-like spin.
- **Minimal fix:** tune `ball.rs` using identical slalom, S-turn, coast, slope and reverse tests at three radii. Separate pivot-from-rest from steering while moving; try slower reverse acceleration/speed and preserve lateral momentum on ordinary turns. Keep Space flip and keyboard usability. Add optional analog dual-hand input only after the envelope feels good.
- **Size/deps:** M; #1 and stable collision. Accept recognisable small/large handling with no abrupt speed collapse on gentle bends; proposed tuning, not a claim about original constants.

### 7. No clearly authored moment of “the village is small now”

- **Katamari:** growth changes the perceived scale of familiar surroundings (K2).
- **Azeroll:** `camera.rs:177–200` continuously eases distance/pitch; `level.rs:259–265` supplies text tiers. These work, but there is no coordinated scale reveal. Large debug captures show camera retreat without a clear before/after landmark.
- **Why:** diameter gains need perceptual punctuation.
- **Minimal fix:** in `camera.rs` and `level.rs`, add two hysteretic, nonblocking framing beats at validated tier crossings: briefly hold familiar framing, ease wider, reveal an available meal and retain input. Compose the route so the same wagon/guard/tree is first intimidating and later collectible. Fixed FOV is fine; no engine camera rewrite needed.
- **Size/deps:** M; #2–5. Accept no repeat beat after shedding, and no interruption of player motion.

### 8. Attachment can read as a loose orbital scaffold rather than a packed clump

- **Katamari:** protruding objects create an irregular silhouette (K1).
- **Azeroll:** `collect.rs:203–224` immediately places object centres on a radial shell while preserving orientation; burying at 0.62r and the 500 cap govern lifecycle. `/tmp/az-npc/play-012.png` shows a readable carried human, but also widely separated logs/weapons around an exposed brass core. `/tmp/az-fade-r8b/play-002.png` has a scattered-looking fence silhouette. These are examples, not proof every clump is sparse.
- **Why:** growth should feel like accumulation with weight, and the ball's contact surface should remain legible.
- **Minimal fix:** in `collect.rs`, retain physical contact orientation but ease the last small sink into the final pose; tune depth by box shape instead of one sphere-size fraction. In `ball.rs`, bridge visible gaps with the existing buried-volume shell without inflating it into a smooth featureless sphere. Protect prominent new trophies from immediate absorption.
- **Size/deps:** M; collision and #2. Accept dense food, elongated-prop and human clumps through a full rotation, without changing credited volume or trophy reach unexpectedly.

### 9. Huge scenery cannot complete the reversal of power

- **Katamari:** buildings join the ball (K1’s illustrated house example).
- **Azeroll:** `props.rs:164–176` consumes `DoodadClaimed`; WMO buildings remain engine-owned. Architecture explicitly lists WMO-internal props and building pickup as outstanding. The claim circle is not equivalent to “everything is collectible.”
- **Why:** getting huge while familiar architecture stays permanently invulnerable limits the fantasy.
- **Minimal fix:** first make a few existing M2 wagons/trees/fences conspicuous late rewards in `goldshire.toml`. Then extend the placement-claim seam for **one surveyed, isolated WMO**, with render proxy, collision removal, streaming tombstone and a cheap carried silhouette. Touch `props.rs`, a new `structures.rs`, `mod.rs`, and the world placement/collision owners found through D5. Do not pretend duplicating a building mesh removes its original collision.
- **Size/deps:** M for recipe payoff; L for real WMO support; #1–5, #17. A building may be a Free Roll stretch reward after Hogger; the main goal remains intact.

### 10. Hogger is a destination, but his reveal and reversal are not reliably staged

- **Katamari:** the creator explicitly avoided making growth merely preparation for one special target (K1). Hogger is an intentional adaptation, so protect the growth experience around him.
- **Azeroll:** `creatures.rs:491–505` sends him on a time-window patrol; `hud.rs:798–801` only says he is far southwest when edible. Architecture says clear foreshadow contact is not established by the final level captures.
- **Why:** missing the early encounter weakens the late joke, while searching for the boss after reaching size wastes the climax.
- **Minimal fix:** in `creatures.rs`, stage a safe, proximity-triggered pass on the primary route, allow escape and return him visibly to camp. In `goldshire.toml`, frame the camp approach through an open sightline and an escalating feast. In `hud.rs`, provide a short contextual bearing/distance only once edible; avoid permanent quest navigation.
- **Size/deps:** M; #2–5. Accept early encounter and late approach on two routes without forced player damage or mandatory chasing.

### 11. The Prince follows the clump instead of visibly pushing it

- **Katamari:** the Prince's two-handed effort is part of its control identity (K2/K4).
- **Azeroll:** `creatures.rs:637–678` directly relocates a fixed-size gnome behind the radius and plays ordinary movement. His height uses terrain, not the indoor support surface, and the trailing offset can intersect scenery.
- **Why:** the main character currently explains little about acceleration, effort or contact.
- **Minimal fix:** `creatures.rs`: select a suitable forward-lean/emote pose, vary lean/stride with acceleration and dash, use floor-aware feet placement and a short obstacle check. Animate the Space flip around/over the ball instead of teleporting his facing. Preserve fixed world scale; don't enlarge the gnome to keep him visible.
- **Size/deps:** M; #1, #6; asset clip inspection. Accept small-ball pushing indoors/outdoors and a readable, unobtrusive flip.

### 12. Objects yield mechanically but lack a near-threshold expressive response

- **Katamari:** player recollections describe near-edible wobble/knock-up feedback (K7); exact REROLL behavior still needs targeted footage confirmation.
- **Azeroll:** `collect.rs:257–278` launches intermediate objects through `Shed`, then locks collection for 1.5s. This is already a substantial improvement, not absent physics. Wall contacts mainly move the ball (`279–320`).
- **Why:** a visual shiver says “almost” without numbers and makes the environment feel touchable.
- **Minimal fix:** add a bounded, damped render-only rock at contact for eligible props in `collect.rs`, plus a distinct short cue through `sfx.rs`. Restore the exact authored transform; never accumulate rotation or move a gate's collision accidentally. Keep meaningful ballistic dislodgement separate and check that lockout does not turn a nearly successful sweep into repeated misses.
- **Size/deps:** M; collision landing, #5. Accept gentle tap, intermediate knock and successful pickup that are visibly distinguishable.

### 13. NPC chaos is individual fear, not yet a village-scale comic escalation

- **Katamari:** humans flee and police resist (K6).
- **Azeroll:** `npcs.rs:319–428` already has notice/alarm/flee, guard defiance, zigzags, stumbles and a 35-yard home leash. `goldshire-npcs.toml` supplies the authored 33-person cast. Do not replace this with another generic flee system.
- **Why:** Goldshire should change from domestic bustle to collective disbelief, not merely reveal independent trigger radii.
- **Minimal fix:** in `npcs.rs` and `goldshire-npcs.toml`, author three local beats: guards form a brief defiant line; market partners interrupt their activity together; camp witnesses react when Hogger becomes vulnerable. Spread alarm to nearby scene peers with cooldowns, not globally. Keep actors catchable and out of doorways; no combat health or quest UI.
- **Size/deps:** M; #5, #10; #17 budget. Accept a clear escalation viewed during an ordinary run, not just NPC debug teleports.

### 14. Creatures can move through geometry even though villagers avoid it

- **Katamari:** moving subjects provide amusing pursuit and reversal within the playable space (K2/K6).
- **Azeroll:** `creatures.rs:607–615` advances animals/Hogger using terrain height only; `npcs.rs:430` onward performs body/path clearance for villagers. An animal can choose a visually impossible path through buildings or trunks.
- **Why:** ghosting breaks the world's toy-like physical logic and can make collection unfair.
- **Minimal fix:** reuse the villagers' lightweight forward clearance/floor-delta approach in `creatures.rs`; turn tangentially at obstructions, pause or visibly cower when blocked. Check vertical separation before rams. Bound flee speed using the **actual pickup threshold** so newly edible targets are catchable.
- **Size/deps:** M; #5; verify Hogger's revised route from #10. No navigation mesh required.

### 15. HUD has Katamari-shaped widgets but too much persistent Warcraft text

- **Katamari:** size and object recognition support play while the world carries the joke (K2); the Prince corner is expressive (K1).
- **Azeroll:** current `/tmp/az-hud3/play-003.png` has a promising size medallion and two model bubbles, but also a large goal title, requirement sentence, timer plus clock, count and four “Rolled up:” lines. `hud.rs:27–43,754–829` owns these. Colour quality is inferred from current relative size rather than a discovered object's identity.
- **Why:** the interface competes with a busy scene and reinforces a quest-target reading.
- **Minimal fix:** `hud.rs`/`hud.wgsl`: retain diameter, clock and one last-pickup presentation; move detailed count/log to pause/results, abbreviate requirement to an icon/size, show direction contextually, and make toast text legible over bright terrain. Preserve joyful medallion motion and avoid a conventional MMO action bar. Check both narrow and wide windows.
- **Size/deps:** M; #5, #10; presentation changes can begin before physics tuning. Accept that the next ground-level meal remains visible at the inspected 936×1018 layout.

### 16. Pickup and dash feedback are not yet a single tactile vocabulary

- **Katamari:** sound and vibration reinforce rolling (K1); Prince Dash is an explicit action (K5).
- **Azeroll:** `sfx.rs:299–351,380–443` already batches pickups, climbs a combo ladder and plays material/creature cues. `ball.rs:500–509` gives dash duration/cooldown without a dedicated shared dash event. `ball.rs:85–88` pops the small core, increasingly hidden inside trophies.
- **Why:** important impacts/growth may sound busy but look weak, while dash readiness is hard to infer.
- **Minimal fix:** use `Collected`/`Bumped` for a bounded flash, contact puff and subtle camera impulse; scale it by relative pickup significance. In `ball.rs` expose dash start/readiness to presentation rather than duplicating timers; add a short windup/effort cue and clean ready cue in `sfx.rs`. Optional gamepad rumble can follow. Never freeze input for every pickup.
- **Size/deps:** M; #5, #6, #8; small serial message contract in `mod.rs`. Accept dense pickups without flash/shake overload and an unmistakable dash start.

### 17. Late-game performance has caps, but no trustworthy frame-time budget

- **Katamari:** buried objects are discarded to control cost (K1).
- **Azeroll:** `collect.rs:193–198` may compare each edible loose object against every trophy; visible trophies sort each frame (335–339). `ball.rs:340–343` currently performs 73 collision support rays per call, potentially ahead and at rest. `props.rs:304–309` removes frustum culling for copied parts; `npcs.rs:562–584` parks all but eight carried NPCs. Architecture reports older camp 43–49fps and NPC transition 23–55fps, with compilation confounds.
- **Why:** hitches sabotage steering and reward exactly when the fantasy should peak.
- **Minimal fix:** instrument first, in `auto.rs` plus a native/browser capture harness: p50/p95/p99 frame time, load stalls, loose/attached/part counts, ray and attachment candidate counts. Then spatially bucket collection candidates in `collect.rs`, budget support probes without breaking thin ledges in `ball.rs`, and park/cull by projected visibility. Treat HUD extra cameras as a measured cost, not an assumed culprit.
- **Size/deps:** M to measure/target hotspots; #1/#8 correctness before optimization. Accept 60fps/16.7ms as an aspirational target on a named native machine; publish actual browser data separately. No current performance guarantee.

### 18. Tabletop and vertical routes are attractive but incompletely connected

- **Katamari:** eligible walls/steps can become routes (K4).
- **Azeroll:** `design.rs` supports tables/heaps and elevated props; `ball.rs:95–98,551–563` only offers a radius-scaled automatic step. The inn's real vestibule and upper storey impose clearance/floor constraints documented in the survey. No pressure-climb state is present.
- **Why:** seeing unreachable meals feels like broken collision unless the route or future size requirement is clear.
- **Minimal fix:** first author a low stair of sacks/benches or a spill ramp to one outdoor tabletop in `goldshire.toml`; avoid forcing an engine rewrite to prove the experience. If testing warrants it, implement bounded forward-pressure climbing in `ball.rs`, constrained by edge height, headroom and contact normal. Never use a lifted sweep that pulls the ball through ceilings.
- **Size/deps:** M; #1, #3, #4. Accept one obvious vertical reward loop and an intelligible failed climb, with doors still escapable.

### 19. Personality has jokes but no commissioning character or arrival ritual

- **Katamari:** the King's absurd demands and cosmic framing distinguish routine collection (K2); opening imagery is exuberant (K6).
- **Azeroll:** `level.rs:194–215` starts on first movement; `hud.rs:1026–1050` mainly offers records and Free Roll. Results' “Everything is Bind on Pickup” is a good existing joke (`/tmp/az-hud2/results.png`), not an absent voice.
- **Why:** the premise needs to teach players how to interpret what follows as joyful nonsense.
- **Minimal fix:** use `progress.rs`, `hud.rs` and `level.rs` for a skippable five-second gnomish commissioner vignette: oversized portrait, malfunctioning reclamation device, one absurd request and a diegetic “roll tiny things first” hint. Reappear briefly at size milestones without pausing. Invent an Azeroth voice rather than copying the King verbatim.
- **Size/deps:** M; #15; use existing models/music. Accept instant replay skip and first-run understanding without a wall of instructions.

### 20. The finale hides the achievement instead of exhibiting the creation

- **Katamari:** the accumulated ball becomes a celestial object (K1); reframing the player’s collection is the design lesson here, rather than a claim about an unviewed REROLL cutscene.
- **Azeroll:** `progress.rs:26–38,256–265` already supplies 3.2 seconds of slow motion and camera pullback; `hud.rs:976–1023` flashes and scales giant gold text. `/tmp/az-fade-r8b/finale-1.png` puts that text over a still-foliage-obstructed scene; Hogger's actual attachment is not the focus.
- **Why:** rolling Hogger should be a remembered physical gag and the player's own clump deserves a curtain call.
- **Minimal fix:** `progress.rs`/`hud.rs`/`level.rs`: first clearly show Hogger on the surface, then an occlusion-safe trophy orbit, then a brief prismatic reclamation beam and commissioner verdict keyed to this run's largest/most absurd finds. Keep skip/retry immediate and restore camera/time on every exit. A Royal Rainbow-inspired ritual is a design recommendation, not an asserted exact REROLL animation.
- **Size/deps:** M; #2, #8, #10, #19. Accept readable trophy and unobscured ball before results text appears.

### 21. Authored abundance can still look like repeated asset placement

- **Katamari:** memorable scenes juxtapose recognisable everyday things (K2/K3).
- **Azeroll:** mixed-model clusters, heaps and punchline singles already exist (`scene-format.md`; `design.rs:396–568`). However, appended bakery groups at `goldshire.toml:1030–1049` add 90 and 75 food props, with the latter at scale 4.0. `/tmp/az-hud3/play-003.png` reads as a large food carpet behind a fence.
- **Why:** numerical density alone does not supply surprise or a coherent scale ladder.
- **Minimal fix:** replace filler with three directed tableaux in `goldshire.toml`: a spilled supper leading to its guilty chicken, guards protecting a ridiculous picnic, and Hogger's stolen village belongings. Give each a dominant silhouette, mixed smaller items and one identifiable punchline; use enlarged food sparingly and visibly justify it with the device. Capture each from the actual approach, not an editor overview.
- **Size/deps:** M; #3/#4. Preserve room budgets and the named supper challenge's scene/food semantics (`scene-format.md`).

### 22. Music is strong; the remaining sound gap is world-to-clump continuity

- **Katamari:** K6 analyses the soundtrack's playful contradictions. Azeroll's original tracks already serve this aim; replacing them is not a priority.
- **Azeroll:** `music.rs` rotates three level songs; `sfx.rs` supplies collectible voices and material cues; `npcs.rs` rate-limits barks and animates carried bodies. Architecture acknowledges no muffled carried-voice filter. There is no continuous rolling contact layer in `sfx.rs`'s event-only gather/flush path.
- **Why:** the clump can look alive while its ongoing movement and carried occupants sound disconnected.
- **Minimal fix:** in `sfx.rs`, add a restrained speed/surface rolling layer and a sparse exposed-trophy rattle; let the music remain dominant. In `npcs.rs`, reduce carried chatter with burial/visibility and vary callouts. Add a per-play filter only if the existing sound-message seam can carry it cleanly; volume/distance shaping is the smaller first step. Verify by listening, including paused/resumed playback.
- **Size/deps:** M; #8/#13/#16, sound mixer capability. Do not infer mix quality from logs or screenshots.

## Next three batches

These are ownership proposals, not delegations already started. Tasks within each batch have **disjoint write sets**. Reading another owner's files is allowed. If an unexpected shared-file edit is required, defer it to integration rather than silently expanding ownership. Each batch ends with a serial integration/verification pass. Sol is for hard engineering and diagnosis; Sonnet for implementation against a fixed contract; Haiku for bounded inventory/copy/documentation work.

### Batch 1 — establish a reliable, readable growth loop

| Track | Tier | Exclusive write set | Scope / gaps |
|---|---|---|---|
| A: movement/contact verification | **Sol** | `ball.rs`, `collect.rs`, `unstuck.rs` | After in-flight handoff, #1/#6; preserve new growth/contact law, investigate missed/surprising contacts rather than redesigning it. |
| B: visibility | **Sol** | `camera.rs`, `props.rs` | #2, robust boom placement and local occlusion; no scale-beat API yet. |
| C: eligibility consistency | **Sonnet** | `creatures.rs`, `level.rs` | #5, initial #14; proper pickup gate, truthful tier claims, geometry-aware animals. |
| D: progression evidence | **Sonnet** | `auto.rs`, `design.rs` | #3/#17 telemetry and per-route volume diagnostics. No physics tuning. |
| E: compact content pass | **Sonnet**, with optional **Haiku** inventory preparation in a separate note | `goldshire.toml` only | #4/#21 and first recipe-only part of #18; draft against new gates, final counts depend on D output. Keep optional inventory note out of other tracks' files. |

**Integration gate:** record exact binary/build identity and recipe; run inn/smithy escape fixtures plus two complete route samples under the shared lock, sequentially. Perform a human slalom and sound check. Native play build, focused Azeroll tests, wasm check for changed logic. Log current D10 budgets including NPCs; no empty-band warnings, no new floor snaps, no repeated automatic recovery. Content rebalance after telemetry is a serial final adjustment by E, not a concurrent rewrite.

### Batch 2 — make growth expressive and the village alive

Before parallel work, a **small serial Sol contract pass in `mod.rs`** defines only needed presentation data/events: meaningful growth/tier, dash, contact response and pickup threshold distinct from extent. Register new modules here if needed. Freeze that contract for the batch; do not let every task edit the plugin.

| Track | Tier | Exclusive write set | Scope / gaps |
|---|---|---|---|
| A: attachment and touch | **Sol** | `collect.rs`, `ball.rs` | #8/#12 physical-to-visual response; #16 dash event production and visible pickup pop. |
| B: scale grammar | **Sonnet** | `camera.rs`, `level.rs` | #7 milestone beats without pausing, truthful announcements and hysteresis. |
| C: Prince + Hogger staging | **Sonnet** | `creatures.rs` | #11 and creature-side #10; keep Batch 1 eligibility/clearance fixes. |
| D: social escalation | **Sonnet** | `npcs.rs`, `goldshire-npcs.toml` | #13, exposed/carried activity from #22; budget reactions and preserve catchability. |
| E: play-facing HUD | **Sonnet** | `hud.rs`, `hud_models.rs`, `hud.wgsl` | #15, contextual Hogger guidance, #16 restrained combo/portrait response. |
| F: audio grammar | **Sonnet** | `sfx.rs`, `music.rs` | #16/#22 cues and rolling layer; music changes only when a listening test supports them. |
| G: encounter composition | **Sonnet** | `goldshire.toml` | #10/#18/#21 content follow-through using agreed Hogger route; no NPC cast edits. |

**Integration gate:** real-time small/medium/large capture and listening review, indoor Prince footing, full clump rotations, continuous input during reveals, catchable frightened subjects, legible camp approach. Stress >8 carried NPCs and check capped actors recover when shed. Compare frame-time percentiles to Batch 1, separately on native and browser. No automatic expansion to another zone.

### Batch 3 — deliver the Goldshire payoff and close polish risks

First freeze one isolated WMO target and the desired end sequence. The integration owner alone changes `mod.rs` for structure registration, before the parallel window. The WMO task explicitly excludes new interior loose-prop claims in this batch: that would expand the collection/inventory contract.

| Track | Tier | Exclusive write set | Scope / gaps |
|---|---|---|---|
| A: landmark pickup pilot | **Sol** | `props.rs`, new `structures.rs`, necessary `benilla-world` placement/render/collision seam files | #9 one real building/structure with original collision removed and tombstoned; no edits to collection internals. Scope engine carries to the fork's required seam/new-file rules. |
| B: intro and departure | **Sonnet** | `progress.rs`, `hud.rs`, `level.rs` | #19/#20: short commissioner intro, visible Hogger capture, prismatic departure, personal results. All phase/UI changes share this owner. |
| C: measured performance | **Sol** | `ball.rs`, `collect.rs`, `npcs.rs`, `hud_models.rs` | #17 measured hotspots only; maintain Batch 2 behavior. Coordinate read-only diagnostics through D. |
| D: final evidence/reproduction | **Sonnet** for harness, **Haiku** for a separate evidence index | `auto.rs`, audit-specific `scripts/azeroll-audit-*`, separate `docs/azeroll/audit-evidence.md` | Sequential lock-respecting runs, configuration hashes, frame-time reports and reproducible routes. Evidence writer owns only the index; no shared-file writes. |
| E: final progression/content | **Sonnet** | `goldshire.toml`, `design.rs` | #3/#4/#9/#21: make landmark rewards optional until ready, final accessible-volume and doorway budgets, no altered physics law. |

**Integration gate:** full native and browser play routes; a fresh-player attempt; repeatable win in the intended time range without recovery dependence; alternate route and optional farm traversal; pause/restart/time restoration; streaming out/in does not respawn a collected structure or leave invisible walls; no save contamination from debug fixtures. Use the provided `flock /tmp/azeroll-run.lock` command and required environment for captures, remove audit-generated `benilla-config/azeroll-progress.toml` afterwards. Save measured limitations rather than marking unplayed cases passed.

If the WMO pilot exceeds one structure or threatens contact correctness, finish the remaining Goldshire improvements and keep it an explicit open #9. Do not substitute a decorative duplicate and declare the scale problem solved.

## Audit cautions for the implementation lead

- Historical `ARCHITECTURE.md` pacing targets, older tier comments, `prop-catalog.md`'s thickness rule and `scene-format.md`'s size terminology conflict with current maximum-axis/additive-box code. Reconcile after the in-flight handoff; the executable rule is the authority for measurement. Do not bulk-update this evidence during another agent's active edit.
- Separate a generated scene shot, a debug-size fixture, an autopilot route and a human play result in all completion claims.
- The audit intentionally does not request another zone, combat systems, power-ups or a wholesale physics engine replacement. Its first release criterion is a readable, uninterrupted and funny Goldshire growth loop ending in a clear Hogger payoff.
