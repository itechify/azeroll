# Azeroll architecture

Living record of the major decisions and the outstanding work. Research behind them:
[engine-world.md](engine-world.md) (doodads, WMOs, bounds, collision, streaming, performance).

## Decisions

**D1 — Foundation: a server-less mode on the benilla engine.** The engine is wenilla ⊃ benilla: a
from-scratch WoW 1.12.1 client in Rust + Bevy 0.18 with a wasm/WebGPU build. Azeroll runs it
server-less, an approach inspired by [Azeroth Survivors](https://github.com/outthislife/azeroth-survivors-re):
`CaptureMode` turns off FrameXML and the player controller, the net layer runs offline, the client
boots straight `InWorld`, and local units are dressed by the stock attach/animation systems. Azeroll
is `crates/benilla-app/src/azeroll/` plus the `crates/azeroll` launcher.

**D2 — Mode selection lives in `benilla-app/src/lib.rs`.** `run_azeroll` sets `azeroll::ACTIVE`;
`launch` then sets the start map, disables the connection and adds the Azeroll plugin. The wasm
entry calls `run_web`, which runs Azeroll. Native: `cargo azeroll`, which needs
`WOW_DATA=wow-data/Data`.

**D3 — The ball is three entities.** `Ball` (root, translation only) → `BallSpin` (roll rotation)
→ core mesh plus trophies. `BallState` (resource) is the authority for position, velocity, spin and
radius and tank heading. The brass core stays at its starting size. Only the collision radius and
a coarse, mottled debris shell grow. New trophies stick at contact with world orientation
preserved, sunk by 0.35 of their size. Trophies whose outer bounding sphere falls below 0.62 of
the collision radius are buried; the 500-trophy cap also absorbs deepest/oldest first. The shell
is hidden until something is buried and has radius
`cbrt(core_radius³ + 0.55 × Σ buried credited_volume)`, independent of collision radius.
Its coarse displaced mesh replaces the old fragment-covered fuzzy sphere.

**D4 — Maximum-axis gate, additive box volume.** (`collect::contact(radius, axis)` is the one
classifier: `RollUp` / `Dislodge` / `Wall`, shared by collection and the autopilot.) Roll-up requires `D >= 2.15 × A`,
where A is the longest full axis of the scaled oriented model box (creatures use
`M2Bounds::stand_extent`, including fitted species). `Collectible.pickup` stores A for the
HUD, AI and autopilot. Actual growth is bbox volume × `FILL_FACTOR = 0.5`, with no
radius-dependent credit or per-pickup cap. BallState stores sphere-normalized volume
`3V/(4π)`, starting at `START_RADIUS³`; thus `r = cbrt(stored_volume)` is exactly
`cbrt(3V/(4π))`. Each trophy remembers its exact credit, removed on shedding.
Long thin boxes give little volume but retain their full visual extent after attachment.
Attached oriented boxes remain adhesive. The collider stays spherical: trophies no longer
lever the support height up by their tips. Their volume-weighted local centres instead
produce a CoM offset; its rotating vertical component modulates speed and spin, and its
horizontal component introduces gentle heading drift. Eccentricity is capped at 0.3r,
inertia at 1.6×, speed modulation at ±7.5%; balanced pickups and buried volume dilute it.
Flat speed is `r × 12 / sqrt(1 + 0.65r)` (dash ×1.5), so angular speed falls with growth
while travel speed rises. Acceleration still takes longer at larger radii.

**D5 — The map's own doodads become collectibles through one engine seam.** The world crate can't
remove or move a streamed placement in place: static geometry goes to a retained renderer or merged
meshes, and doodad collision is welded per tile. So `benilla-world` gains a placement-claim hook:
the mode supplies a predicate, and the streamer skips the placements it claims (no render, no hull)
and announces them instead. `azeroll::props` spawns ordinary-entity copies of claimed doodads with
`spawn_model_entities` (no static diversion), sizes them from M2 bounds, and keeps a tombstone set so
collected placements stay gone across stream-out/in. Unclaimed scenery keeps the optimized path.

**D6 — Three contact states.** Engine-owned buildings/doodads use `slide_body` with a
0.85r horizontal sphere. Full-radius vertical support comes from terrain and 73 downward
collision rays across the footprint (centre and 12-ray rings at 0.3r, 0.5r, 0.7r, 0.85r, 0.95r
and 0.99r; the first version's three rings left a gap where a small ball sat against a step
edge the rays never sampled, and r=0.5 could not climb the inn's 0.29 yd step), taking
floor height plus the sphere's spherical-cap lift at each offset. Rays start only a reachable
step above the bottom to avoid snapping through ceilings to the next storey. Walkable hits
require normal.y > 0.6. Step height is `0.2 + 0.45r`; reachable support ahead raises the
ball before the horizontal sweep, whose origin otherwise stays at the real centre.
Mode-owned objects compare diameter / maximum axis: ≥2.15 absorbs; [1.5,2.15) dislodges;
smaller balls bounce off authored solids (hostiles retain their ram behaviour). Dislodgement
needs closing speed >0.5 yd/s, knocks the object forward and upward, costs 25% momentum,
and locks collection out for at least 1.5 s. Props tumble at 4 rad/s and settle on their
rotated box; creatures stay upright and stagger. Both use `Shed`; carried units skip the
engine's ground clamp during flight. Compound splitting is not implemented.

**D7 — Creatures are local units (a `NetEntity` + `ObjectStore` adapter).** Real Elwynn
creatures are spawned by display id (Hogger = display 384; rabbits and chickens are resolved from
`CreatureDisplayInfo` by model path at runtime). A collected unit is re-parented to `BallSpin`.
Because the net ground clamp and terrain conform treat a unit's local `Transform` as world space,
the app gains a generic `crate::entities::Carried` marker that those systems skip.

**The Prince pushes (Batch 2 Track C, `creatures::prince`).** Fixed scale forever: 0.25, about
0.35 yd tall (the gnome is 1.1 yd at 0.8), a little figure against the 0.5 yd start ball and a
speck against a huge one; the camera does the work. He is a `Prince` + `Push` entity, never
`Collectible`. Placement: his bearing from the ball's centre eases to the back of the heading
(`π/2 − heading`) at no more than 13 yd/s of strafe (rate clamped 1.8–9 rad/s), so a turn or a Space flip
sends him round the clump rather than teleporting. His distance is `clump_reach`: the ball and every
`Attached` trophy (bounding sphere × 0.85) are cut into discs at his hand height (0.16 yd, or the
ball's equator for tiny balls) and knee height, and the ray from the centre along his bearing
leaves the last disc + `BODY_GAP` (0.1 yd). So a log sticking out behind pushes him out, a
trophy on the crown does not, and on a big ball he stands at the foot of its bulge (chord at hand
height), not at `r`. Out at once, in slowly. Walls: one `ray_body` per frame from the ball at hand
height; every 0.2 s he searches bearings 0, the sticky detour, ±0.7, ±1.4, ±2.1, 3.0 for one with
room (a wall behind him moves him to the side). Feet: the terrain or `ball::floor_rest` probed
from a step above where he last stood (a hall floor, porch or stair holds him), whichever is
higher. Pose: `CastHold` on the unit with ReadySpellDirected (AnimationData 51): the driver
plays it full-body when he stands (hands out on the clump: the idle lean) and as a torso overlay
while he moves (legs run, arms reach). Chosen from screenshots against plain Run (arms swing like
a runner) and ReadyUnarmed 25 (raised fists, reads as boxing); `WOW_AZEROLL_PRINCE_POSE=run|<id>`
tries others. Lean: tilt about the feet (`from_rotation_x(-lean)`), 0.12 rad at rest to 0.36 at
top speed, +0.12 dash, +0.16 wind-up. Feet: run clip driven at 5.2 + 0.45 × ball speed (cap 16,
×1.3 on a dash) so legs run at small sizes and scramble at huge ones; a `Dash::WindUp`
(reserved by Track A, handled if it arrives at any moment) runs him on the spot at 12 for 0.45 s;
`BallState::dash > 0` is the truth for a dash in progress, so a missed `Started` costs nothing. A
stopped ball stops his legs and he holds the pose. If the HUD Prince bubble (`hud_models.rs`, fixed
offsets from his root) is not reframed for the 0.35 yd Prince he reads as a speck.

**D8 — Camera: fixed FOV, distance and pitch follow the radius.** The stock scoped-view system resets
FOV every frame even under `CaptureMode`, so Azeroll grows the boom distance instead of the FOV.
Terrain streaming follows the ball through `Player.pos` → `ViewFocus::body`, and
the far clip grows with the ball within the engine's 177..777 range.
The boom is at least twice the largest attached radial extent plus the collision radius;
wall shortening stops outside that extent, and growth cannot leave the lens inside a newly
attached trophy. Pitch is Katamari-low: it eases from 20° to 31° over radii 0.25–3.75 yd and on to
38° by r≈13, with the boom at 2.2 + 7r (up to +30% at huge sizes), so a big clump keeps its depth.
Occluder fading (`props::occlusion_fade`) never hides what the ball can run into, which is what is
around and ahead of it. Per part: contact-height geometry (underside below the ball's top) stays
opaque, except what lies *between the lens and the ball* (`sight_ghost`): a part whose bounds meet the
eye→look sightline (a ball-radius-thick capsule) and end wholly short of the ball's centre by 1.25 radii
along the view axis, while staying 1.5 radii (+0.15 yd) from that centre, ghosts to 35%. Anything beside,
ahead of or behind the ball, within that margin, or around it (a box the ball sits in) stays solid. Geometry
above the ball's top on the sightline (canopies) ghosts to 40%, parts around the lens to 40% (never invisible),
and trophies to 45% only when large, protruding and on the camera's side of the clump. A part already
ghosted keeps its fade through 0.2 radii more margin and a 1.3x thicker sightline, and eases back in
rather than snapping. The boom still stops at a prop's ball-side face, so the lens is usually in front of a post
and the ghost shows only for what stands within the no-shorten zone (1.8r + 0.3 yd from the look point).
`WOW_AZEROLL_CAM_SCENE=post|lens` (`debug_fill.rs`) stages posts around the first camera for a check.
W/S roll along the heading; A/D turn it, Space flips it, Shift dashes for
0.65 seconds with 1.8 seconds between starts. The camera follows heading with a temporary mouse
or Q/E orbit. The fixed-scale gnome Prince stands behind the clump, using an NPC-appearance
male gnome display resolved from the creature DBC catalog.
`WOW_AZEROLL_DEBUG_RADIUS` starts are dressed by `debug_fill.rs`: it sets `BallState::buried_volume`
to 0.65 r³ (the shell's volume) and attaches up to 200 copies of nearby small props (non-flora, 4%–93% of r
across) to `BallSpin` as `Attached`, seated like `collect` does (centre at `r − 0.35·size`), so debug captures look like a
clump of that size.

*Scale grammar (Batch 2, Track B).* A tier crossing is punctuated by `camera::scale_beats`, which
reads `TierReached`/`GrowthBeat` (so it inherits `level`'s ratchets: `Tiers.announced` and
`GrowthSteps` only rise, and the camera keeps its own tier high-water, so a shed across a line
never repeats a beat) and writes `CamRig::{beat_boom, beat_pitch, beat_yaw, beat_look}`, which
`place_camera` applies on top of the smoothed boom/pitch/look. It is non-blocking: no input, ball or
time is touched. FOV and slow-mo are deliberately unused (the stock scoped-view system resets FOV).
* Standing frame per tier reached: boom +5%, elevation +1° (so +25% / +5° at Hogger's tier).
* Pull-back overshoot on top (tier strengths 0.7/0.9/1/1/1): up to +16% boom, +4° elevation, rising
  over 1.2 s, held 0.5 s, settling onto the standing frame over 3 s. A retrigger continues from the
  current level, never snapping.
* Glance (reveal): from 0.25 s after the beat, ease in 0.3 s, hold 0.3 s, ease out 0.45 s (1.3 s
  window; the view is off the heading for ~0.7 s). Turns the view up to 23° toward the target and
  slides the look point 0.06 boom (bounded by 0.5 r + 0.15 yd) toward it, so the ball stays in frame; no glance when the target is
  nearly behind. Q/E or a camera drag cancels it; so does the finale. Targets: `level::TIER_REVEALS`
  (nearest listed area that is open ground and not underfoot): tier 1 Market / Northern goods /
  Smithy; 2 Smithy / Broken cart / Roadside camp / Southwest road; 3 Roadside camp / Southwest road
  / the harvests; 4 Riverpaw camp / the harvests; 5 Riverpaw camp. A test pins the names to
  `goldshire.toml`.
* Far clip/fog: +40 yd per tier reached, eased over 1.6 s on top of `drive_ball`'s own
  `350 + 25 r` (fog ends at `min(zone fog, farclip)`), applied after `drive_ball` each frame.
* A `GrowthBeat` every third step (x1.95 diameter) breathes the boom at strength 0.3, never within
  4 s of a tier beat. Beats within 2 s of the run start (debug start sizes) set the framing only.
* Debug: `WOW_AZEROLL_BEAT_TRACE=1` logs the offsets every 0.2 s for 8 s after a beat;
  `WOW_AZEROLL_BEAT_SHOTS=<dir>` screenshots 0/0.4/0.8/1.2/1.8/3/6 s after each tier beat.

*Foliage window (large-ball tree visibility).* `cutout.rs` publishes a disabled-by-default
`lighting::FoliageCutout` resource after camera placement. Two vec4 rows in the existing shared
scene-light buffer carry the physical-pixel centre, radius (1.5× the ball's projected sphere
radius), eye-depth limit, contact-height floor and enable flag; no per-frame material mutation
or new GPU binding is needed. Both `wow_model.wgsl` (entity lane, marker bit 14) and
`static_gx.wgsl` (retained scenery, vertex word bit 30) discard foliage within that circle,
using stable screen-space dithering across its outer 28%. Only fragments nearer than the
ball centre minus 0.3r and above centre + 0.6r are eligible, so the contact band and scenery
beyond the clump remain readable. Eligibility requires an M2 alpha-test batch and a texture
path containing tree/bush/leaf/leaves/foliage/canopy, excluding bark/trunk/branch; opaque
wood and all WMO batches are excluded. Pathless textures conservatively stay solid.
The previous whole-part overhead/lens 40% alpha ghost is replaced; contact-height
`sight_ghost` (35%) and trophy ghosts (45%) remain. Foliage itself never takes a whole-part
sightline ghost. Other modes leave the resource at zero, preserving ordinary rendering.
The probe/palette offsets derive from the enlarged prefix; off-world light blobs allocate
the full layout with zeroed cutout rows.

**D9 — Upstream hygiene.** Blizzard data stays out of git (`/wow-data`, fetched from archive.org per
MPQ). Changes to upstream files are few and generic: the claim seam, `Carried`, and the mode hooks.
Engine updates come from `wenilla` and `benilla` as real merges, so later syncs keep their
ancestry.

**D10 — No traps.** Enclosed areas validate entry at doorway clearance plus every
contained object's additive box volume, independent of order. Exact scaled bbox volumes
are used for props; unknown creature boxes conservatively use a cube of the maximum axis.
Wall squish is horizontal only: ground/floor support always uses full r, while the horizontal
sweep reaches 0.85r above the centre; total required doorway height is therefore 1.85r.
There is no unconditional lifted sweep. Unstuck rechecks current-radius room and uses the
same WMO-aware floor support before accepting a remembered position. Backspace and wedge
recovery remain fallbacks. Recovery requires at least max(12 yd, 4r) of separation;
a three-yard rewind repeatedly returned the autopilot to the same doorway approach. `WOW_AZEROLL_FLOOR_TEST=1` drives six 18-second entrance probes
(inn then smithy at r=0.5/1.0/1.8, or `WOW_AZEROLL_FLOOR_RADII=a,b,c`); combine with AUTO=108
(56 for the inn only) and SHOT_EVERY=6. Each stage logs `centre_floor` and `bottom`; a resting
ball has them equal (verified to 5 mm at r=0.25, 0.3, 0.36, 0.5, 1.0, 1.8). With the test on,
the log also carries a once-a-second `slide from/to ... wall=... ahead_rest=...` line.

**D11 — Hogger is the stage goal (camp and foreshadow script implemented).** Growth is the experience; Hogger is its
culmination. He is foreshadowed while the ball is still too small to take him: an early pass
near the village that rams a small katamari, then retreats. Multiple routes lead to his camp
at the forest's edge, the richest area in the level. The finale is a celebration, then
results. Target run: 8–10 min. Replay value comes from saved best times, light optional
challenges, and an untimed Free Roll after the win.

*Staging (Batch 2 Track C, `creatures::hogger_pass`, `Stage` on Hogger).* Hogger spawns at the
camp, ~250 yd from the start, and the creature AI does not tick that far out, so the pass does
not walk him there. **The pass** (once per run): from `max(patrol_at, 15)` s to `patrol_until + 30`
s, while the ball is within 150 yd of the recipe's `patrol` point, outdoors (no ceiling within
14 yd), small enough that he is a Wall to it, and the camera yaw (`CamRig::yaw` + heading) has
been steady (< 0.35 rad/s for 0.4 s), he is set down 28–34 yd off at a bearing of ±0.15–0.9 rad
off the view with a clear ray from the ball. He *stalks* in at 3.2 yd/s (Walk) to 16 yd, stops and
roars for 1 s (EmoteRoar 74), then charges at 5.8 (Run) for at most 14 s in total, aiming 0.6 s
ahead of a moving ball; within `r + 0.6 × size` he writes one `Ram` (the knock, the thud and the
aggro growl through `sfx`'s existing ram path), then lumbers home at 2.8 yd/s, giving ground and
skirting the ball on the camp's side (`lumber_dir`), and is put back on his camp loop once he is
55 yd off and has walked 12 s (or at 110 yd). A ball that outruns him just watches him give up.
Measured 2026-11-02, autopilot: pass begins t=15–17 s, ram 5–10 s later. Never forced; `Collected` is untouched, so the
win flow is. **At the camp** he walks a 6-point loop of radius 8 yd round the camp at 1.9 yd/s,
senses the ball at `12 + 3r` yd (+6 hysteresis once he has taken a stand, so he does not flicker),
and `react` decides: `Wall` charges and rams as before; `Dislodge` roars (EmoteRoar, again at most
every 5 s), stands his ground a second, then backs away facing the ball at 2.4 yd/s
(WalkBackwards); `RollUp` flees at `min(5.8, 0.7 × top speed)` with a zigzag across the line of
flight, a 0.6 s stumble every 3.4 s and EmoteChicken flapping every 2.6 s, sliding round the
camp's leash when cornered, so the ball always catches him. Fixture:
`WOW_AZEROLL_CAMP_TEST=<yd>` (with `WOW_AZEROLL_DEBUG_RADIUS`) pins the ball that far from the
camp on the approach, facing it, and walks it in at 1 yd/s down to 8, logging Hogger's range and
bearing. Unverified: the roar's and flapping's clips exist on his model (no log either way), and an audible roar needs a hook in `sfx` beyond `Ram`.

**D12 — Creature sizes come from the Stand pose; hostiles ram, they do not wall.** An M2's header
sphere and box cover every animation (a rat's sphere is 1.3 yd, a frog's 1.34; the Stand box is
0.4), so sizing creatures from them made critters edible only at diameters of 4+ yd, long after
the "Critters" banner. `M2Bounds::stand_extent` (the Stand box, a one-field addition in
`benilla-formats`) gives `size = half its diagonal × scale` and a feet-based centre. Fitted
species now scale their Stand box to the fitted maximum full axis; their gate and rendered
geometry therefore agree.
`WOW_AZEROLL_DUMP=1` logs each species' scale, size and header sphere at spawn. A hostile has
`solid = 0` and acts in bursts (`creatures::creature_ai`): charge for at most 4.5 s while within
its aggro range and 70 yd of home, on contact send one `collect::Ram` (knock 7 yd/s, hop, 0.35 s
stun, a bump that sheds as a hard one does), back away for 2.2 s, then ignore the ball for
6–10 s. An edible hostile still flees.

**D13 — Music is embedded, and the engine's mixer plays it.** `assets/music/azeroll-title.mp3`
(ready and results screens) and the Goldshire playlist (`morning-in-goldshire.mp3`,
`goldshire-mambo.mp3` and `goldshire-go-go.mp3`; ducked to 0.3 while paused) are `include_bytes!`'d in `azeroll/music.rs`,
which sets the `sound::ModeMusic` resource from the phase. The playlist rotates through the level
tracks: the next one fades in a second before the current one would loop, using their lengths as
measured with `ffprobe`. Each run opens one track further on than the previous run. All four tracks are
the user's own compositions. `sound/mode_music.rs` loops the track on the glue-music path (`mixer::play_stream`:
decode-streaming natively, browser `decodeAudioData` via `web_load::begin_bytes` on wasm), crossfades
over 1 s, follows the Music slider, and holds WoW's zone music down through the cinematic's
`music_suppressed` flag. Ambience and SFX are untouched. The browser build is compile-checked
only; the audio itself was verified natively.

Batch 2 adds one mechanic on top, and no new track or playlist change: `music.rs` ducks
`ModeMusic::gain` under the biggest moments, by 35% for 1.3 s under a `TierReached` and by 55%
for 2.4 s under Hogger's `Collected` (the finale's 3.2 s), easing in over 0.12 s and out over
0.9 s on real time. The dip multiplies the same gain the pause menu uses (0.3), so a paused finale
still sounds, it always ends (the hold counts down and the release reaches exactly zero), and
`OnEnter(Ready)` and `OnEnter(Results)` clear it. A tier that lands with the win leaves the
dip to the Hogger rule. Each duck and each release is logged at info (`azeroll music:`).

**D16: Traversal contract and handling envelope (Batch 1, Track A).** Thin long claimed props
(fences, rails) collide as oriented boxes rather than discs. Discs made gaps between fence posts
impassable and held the ball beside edible items. Unstuck uses the same wall-only clearance
geometry as movement. Fixture routes (`WOW_AZEROLL_STALL_DEBUG`, scripted paths) hold any stall
under 0.1 s while the ball can physically pass. Long stalls happen only where the ball is
genuinely too big for an entrance, which is correct: that entrance is a wall.

Measured envelope for a balanced clump:

| r (yd) | top speed (yd/s) | to 90% speed | reversal to 90% | turn radius (yd) |
|---:|---:|---:|---:|---:|
| 0.3 | 3.29 | 0.41 s | 0.74 s | 1.50 |
| 1 | 9.34 | 0.50 s | 0.89 s | 3.69 |
| 3 | 20.96 | 0.77 s | 1.30 s | 6.08 |
| 8 | 38.55 | 1.44 s | 2.23 s | 7.87 |

The full report is in `/tmp/az-track-a-report.txt` (ephemeral). Open: integrating autopilot
detours, clean transit through the camp under gnoll rams, and a human steering check.

D16 follow-up (playtest collision pass): standing tree walls use the M2 hull vertices in
its first 1.5 world yards above the placement foot (buried roots excluded), capped at
4% of the rendered full horizontal footprint, with a 0.35 yd minimum cap for small trees.
Sparse hulls without basal vertices use that footprint approximation. Their full rendered
box still controls pickup, dislodging and credited volume; a canopy's offset box centre cannot
move the trunk wall. Bushes, grass, ferns and flowers have no wall disc. Movement and Unstuck
share `collect::solid_push`, including the trunk origin. Synthetic regression coverage includes
an offset canopy, canopy-height hull vertices, scaling, and r = 0.3/1/3 trunk clearance.
The native baseline at the village canopy (-9421.20,58.49) held r = 1 roughly 15 yd from the
fixture goal against an approximately 8 yd disc (`/home/jjd/az-fix2-evidence/tree-before.log`).
The inn's invisible cell-edge wall was terrain-only grounding: over an ADT hole, the initial
footing guard held the ball, or final settlement discarded its valid WMO-supported movement.
A side-passage baseline stopped exactly at y = 33.333744, with floor support 57.26606, no sweep
hits, and zero velocity. `footing` now accepts a loaded WMO floor when terrain is absent;
Unstuck accepts the same support. Full-radius support and the 0.85r doorway sweep are unchanged.
The initial straight inn fixture instead hit the real full-height vestibule wall at y ≈ 34;
that route must turn through the side opening and is not evidence of a stair defect.
Matched dining fixtures (`/home/jjd/az-fix2-evidence/dining-before.log`, `dining-after.log`):
r = 0.3 stopped 17.33 yd short with a 12.06 s stall before; after, it reached within 0.11 yd,
maximum held stall 0.05 s. The same dining leg at r = 0.25/0.3/0.6 reached within
0.08/0.12/0.06 yd, with maximum stalls 0.05/0.05/0.03 s (`dining-small.log`).
Tree approaches from the south at r = 0.3/1/3 finished within 0.15/0.15/0.18 yd; the last goal
is on the round trunk clearance boundary, so continued input there correctly stops. The huge
canopy at (-9421.20,58.49) now has a 1.931 yd trunk wall instead of the old ~8.25 yd disc.
A basal-only prototype was rejected: its lowest vertices were buried roots, still giving 7.177 yd.
Native build and wasm check passed. The Azeroll tests passed 98/99, including all three new
collision regressions; the remaining failure is `design::progression::tests::routes_run_from_the_start_area_to_the_camp`
("the tour passes everything the primary route does"), outside this collision pass. Clippy
completed, with existing Bevy-system argument-count warnings. The D16 matrix rerun uses a
temporary recipe with Hogger's count set to zero: otherwise his early pass is collected by the
r = 5 leg and ends the fixture on the victory screen. All other recipe content is unchanged.
D16 matrix completed all 20 eight-second legs (`matrix.log`): yard/farm all radii reach within
0.14 yd with maximum stalls ≤ 0.07 s; inn/smithy r = 0.3/1 reach within 0.14 yd with stalls
≤ 0.05 s, while r = 2.5/5 remain blocked at actual building restrictions. Camp r = 0.3/1/2.5/5
has maximum stalls 0/0.04/0.04/0.05 s; small balls remain mobile under gnoll rams (final goal
distances 3.11/4.60 yd), larger balls reach within 0.12/0.04 yd. No automatic recovery.
Dining screenshots: `dining-small/play-002.png` (r = 0.25) and `play-005.png` (r = 0.3).
Tree pickup: `tree-south-after/play-006.png` shows the crate bubble under the canopy; foliage
still obscures some shots, a separate visibility issue. All paths are under
`/home/jjd/az-fix2-evidence/`. Progress-save cleanup is performed after fixtures.

**D19 — Audio grammar: one tactile vocabulary with a voice budget (Batch 2, Track F).** `sfx.rs`
turns the Batch 2 messages into sound with kits that already ship (no new audio files), and every
decision is an info line `azeroll sfx:` because the build host cannot be listened to. The engine's
kit player takes no volume, pitch or stop for a one-shot, so two levers stand in: a quiet sound is
put on the listener-to-source ray at the distance where the kit's 3D rolloff
(`min / (min + 4 (d - min))`) gives the wanted level (`emission_distance`, which keeps its stereo
position and works whatever the camera distance), and cues are 2D at the kit's own level.

| event | kit (SoundEntries id) | level and gates |
|---|---|---|
| pickup pop (existing) | ladder PickupGems 1221, PickUpRing 1193, LOOTWINDOWCOINSOUND 895, PickUpWand 1196 | by streak; always plays |
| pickup voice, material, thump (existing) | creature voice columns, `PickUp*`, `DeathThud*` | now admitted through the budget |
| `Touched::Dislodge`, closing >= 0.6 yd/s | `PutDown*` of the thing's material (wood 1217/1216, metal 1213/1207, cloth 1202/1200, food 1203, meat 1208, glass = gems 1204, paper 1209/1201, plant 1205, stone 1211, chain 1212/1206); living things use cloth | amp 0.22 + 0.045 strength, clamped 0.22..0.6; entity gap 0.9 s |
| `Touched::Wall`, closing 1.0..2.0 yd/s | `DeathThudSmall{Wood,Stone,Grass}` 911/910/908 | amp 0.2 + 0.05 strength, max 0.45; entity gap 1.2 s; skipped when the same hit is a `Bumped` (that path keeps its own thud) |
| `Touched::Wall` with ball diameter / axis >= 1.2 ("almost" knock-loose at 1.5) | the lean thud, then two `PutDown*` knocks of its material 0.11 s and 0.22 s later, the second at 60% | wobble |
| `Dash::Started` | SwingWeaponSpecialWarrior 3091 (five 0.6..0.9 s whooshes) | 2D; roll quiet for 0.6 s |
| `Dash::WindUp` (reserved, not emitted) | Whirlwind600% 5674 | wired, never heard yet |
| `Dash::Ready` | UChatScrollButton 1115 (0.14 s tick) | 2D; skipped within 0.4 s of a pickup |
| `GrowthBeat` | igBonusBarOpen 3201 (quest-objective tick, 1.6 s) | 2D; covered by a tier in the same frame |
| `TierReached` | TaxiNodeDiscovered 1519 (discovery sting, 4.9 s) | 2D; plays under the banner's `igQuestListOpen` 875 |
| roll | footstep kits by size band and ground, below | amp 0.09..0.26 (x1.1 per band); never heard under a louder sound |

Reserved and never used for cues: `QUESTCOMPLETED` 619 (finale fanfare), `LEVELUP` 888 (record),
`igQuestListOpen` 875 (tier banner), `LEVELUPSOUND` 124, `QUESTADDED` 618/890. Growth, tier and
dash cues are held back once `Run::won` or `Flow::finale` is set, and when the phase is not
Playing, so nothing sounds over the fanfare.

*The roll.* One tick per turn of the ball (`2 pi r / speed`, clamped 0.32..1.1 s), only while
grounded and above 20% of `ball::top_speed`, so a big ball ticks slower and a lumpy clump reads as
bumps rather than a hum. The size band comes from the HUD's tiers where the world changes scale
(diameter 1.04, 4.33 and 8.6 yd): CharacterSmall*, CharacterMediumLarge*, FootStepsHuge* and
CharacterGiant* kits, each in Dirt, Grass, Stone, Wood and Snow (20 kits, pitch-varied by the kit
flag). The ground is read every 0.35 s from the engine's own footstep chain
(`WorldPoint::terrain_type` for the player's point, `FootstepCatalog::resolve_terrain` with class 7,
the character class, mapped back to its surface), and any building floor
(`WorldPoint::indoors_at`) is wood; unknown ground stays what it was, dirt at the start.

*The budget.* The kit player has 12 software channels and lets a louder newcomer steal the
quietest voice. `sfx.rs` estimates live voices from each play's length and admits a play only
under its priority's ceiling: pickups and cues 10, touches 6, the roll 3 (the pop and cues are
always played, the rest of the pickup layer goes through the 10). The systems run in the order
pickup, touch, cue, roll in one chain, so the higher priority claims its voices first. The roll
also stands aside for 0.2..1.0 s after anything louder, and a touch window admits at most two
touches, the hardest first. Drops are counted and logged, and a run summary line is written on
the results screen. The startup kit check covers every kit above (and the real-data test
`every_kit_resolves_against_the_game_data` runs it with `WOW_DATA`).

Not decided here, and needing a listening test (`katamari-gap-audit.md` #22 says not to infer a mix from
logs): whether footstep ticks read as a roll or as walking, whether the roll is audible but
unobtrusive under each of the three level tracks, the levels of the touch and wobble sounds, the
tier flourish's length against the banner's `igQuestListOpen`, and the duck depths. All of them
are constants at the top of `sfx.rs` and `music.rs`.

**D17: Packed attachment and touch (Batch 2, Track A).** New trophies are seated along their
contact direction against the fixed core, the buried-volume debris shell and the existing oriented
trophy boxes. Five samples along the longest axis find support with controlled overlap, and a
contact that would push an outward branch is compressed into the collision-radius growth
envelope. Long models lie tangent to the surface with a small random twist. Newly collected
trophies get a 0.6 s grace period before they can be buried; the 500-trophy cap still holds.
Pickup squash, the contact bulge and touch rocking all restore their exact rest transforms before
physics, contact and centre-of-mass evaluation. Visual effects therefore never change credited
volume, support or traversal bounds. `Touched` is emitted with a 350 ms per-entity cooldown. The
dash keeps the Shift press; `Dash::WindUp` is reserved and not emitted. Full report:
`/tmp/az-b2a-report.txt` (ephemeral).

## Module map (`crates/benilla-app/src/azeroll/`)

| file | owns |
|---|---|
| `mod.rs` | plugin, `Phase`, shared components, resources and messages, reset |
| `level.rs` | map / collection tuning, tier announcements and phase flow |
| `design.rs`, `goldshire.toml` | embedded recipe, native restart reload, scenes / trails / herds, validation |
| `unstuck.rs` | safe history, current-size clearance checks, Backspace / wedge recovery |
| `ball.rs` | ball entities, input → rolling physics, grounding and sliding, focus publication |
| `camera.rs` | adaptive chase camera |
| `collect.rs` | overlap tests, roll-up and attachment, growth, bumps and shedding, burying |
| `props.rs` | the claimed-doodad lifecycle: copies, sizes, tombstones |
| `creatures.rs` | creature spawning, wandering, fleeing and aggro, `Collectible` sizing |
| `units.rs` | local unit spawn adapter, coordinate helpers |
| `npcs.rs`, `goldshire-npcs.toml` | 33-person authored cast, ambient scenes, size reactions, voices and carried animation budget |
| `hud.rs` | size, timer and count HUD, pickup toasts, banners, results screen |
| `sfx.rs` | the audio grammar: pickup pops, voices and thumps; touches, cues and the roll; one voice budget |
| `music.rs` | embedded tracks, phase → `ModeMusic`, the duck under big moments |

## Batch 2 contract

Shared types and message registration are frozen in `azeroll/mod.rs` for the batch.
The seven tracks keep their audit write sets. Prince work stays in `creatures.rs`, whose
plugin is already registered; no new module or stub is required.

| Message | Fields / semantics | Producer | Consumers |
|---|---|---|---|
| `Collected` | `entity`, `name`, `kind`, `source`, `size` (visual sphere **radius**, yd), `pickup` (scaled full maximum box axis, yd) | A: `collect` | Existing tombstones, creature motion, level win flow, HUD/model bubble, audio; B may add camera response |
| `GrowthBeat` | `step` (one-based), `threshold`, actual `diameter` (yd); 25% diameter steps relative to recipe start, at most the highest newly crossed step per frame | B: `level` (thin emitter installed) | B camera; E HUD; F audio |
| `TierReached` | `tier` (zero-based TierTable row), `name`, row `threshold`, actual `diameter` (yd); each newly attained row in order, once per run | B: `level::announce` (emitter installed alongside unchanged banner) | B camera; D social escalation; E HUD; F audio |
| `Dash` | `Started`, `Ended`, `Ready` on existing timer transitions; `WindUp` reserved for an actual future wind-up, currently not emitted | A: `ball` (timer transition emitters installed) | C Prince effort; E HUD; F audio |
| `Touched` | `entity`, `state: TouchState::{Dislodge, Wall}`, nonnegative pre-response closing `strength` (yd/s), `point` on ball in Bevy world yards | A: `collect` (existing overlap/response paths emit) | A bounded wobble/contact effects; D local alarm; F audio; B camera if needed |
| `Bumped` | Existing `entity: Option<Entity>`, `name`, closing `strength`; hard wall response or ram, not every touch | A: `ball` / `collect` | Existing bump log/audio; presentation must avoid double-counting `Touched` / `Ram` |
| `Ram` | Existing `entity`, `name`, ground-plane unit `dir` away from attacker; once per charge | C creatures / D NPCs | A collection knock/shedding; existing progress ram count and audio |

`Contact::{RollUp, Dislodge, Wall}` and `Ram` now live in `mod.rs`; `collect` re-exports
both so existing imports and the sole `collect::contact(radius, pickup)` classifier remain
valid. There was no standalone dislodge message: `Shed` remains the ballistic/lockout
component. `Touched::state` describes eligibility, **not** proof of launch: Dislodge contacts
launch only above 0.5 yd/s. Touches may repeat every simulation frame, including gentle taps;
consumers must bound/rate-limit reactions. Non-solid, ineligible creatures can touch without
blocking the ball. Engine-owned walls continue to report only `Bumped`, without a collectible
entity. No new physics response is introduced.

Existing resources remain authoritative: `BallState` supplies continuous diameter, position,
velocity, grounded state and dash/cooldown seconds; `level::TierTable` supplies the resolved
representative pickup thresholds; `Run` supplies elapsed time and legacy banners; `progress::Flow`
supplies finale/free-roll state. Growth has only a private high-water cursor in `level`, reset
on Ready. Growth/tier messages run after collection while `playing`; shedding/regrowth does
not replay them. Tier thresholds are snapshots of the live derived table. A large size jump
emits all new tiers but one growth beat, so consumers should coalesce their own presentation.
Paused time does not advance dash timers; Ready/reset silently resets them without lifecycle
messages. A cooldown crossing emits Ready even if the player starts another dash that frame.

Eligibility/capacity comparisons in HUD colours and audio fill now use `pickup`; visual fitting,
attachment reach, pickup selection by extent, and sound material ranking retain sphere `size`.
The roll-up rule is diameter >= 2.15 × pickup (via the classifier), not size versus radius.
Presentation messages are transient Bevy messages; continuous displays read resources, and
consumers should gate on phase rather than treating messages as persistent run history.

## Running and testing

```bash
# Toolchain: rustup with the pinned 1.98.1 (rust-toolchain.toml); use ~/.cargo/bin's cargo.
WOW_DATA=$PWD/wow-data/Data cargo azeroll            # play (native, `play` profile)
cargo build --profile play -p azeroll                # build only (~1–3 min incremental)
# Hands-off run: autopilot for N seconds of play, a log line every 2 s (fps, radius, count),
# screenshots every K seconds, exit at the end or at the results screen.
WOW_NOVSYNC=1 WOW_BG=1 WOW_DATA=$PWD/wow-data/Data WOW_AZEROLL_AUTO=120 \
  WOW_AZEROLL_SHOTS=/tmp/azshots WOW_AZEROLL_SHOT_EVERY=10 target/play/azeroll
```

### Autopilot, telemetry and the level report (`auto.rs`, `design.rs`)

The autopilot is a fair test player, not a good one: its results should reflect the level, not its
own stupidity. It follows the level's progression, clears the area it is in, walks the authored
trail to the next area (eating what it passes), and moves on when the area holds only crumbs
(< 3 % of the ball's volume), is cleared, has not grown 1.5 % in the last 20 s, or has taken 75 s.
It will not go for food it cannot roll to (above the ball's reach, e.g. on a table) and, since
fleeing creatures outrun a small ball, rates prey it is not close to at a quarter and drops it
after 8 s. A target that has come no closer for 3.5 s is given up on together with its
neighbours; repeated failures in one place write off a disc of ground (up to 14 yd) until the ball
is 40 % bigger. After a ram it runs from the nearest thing too big to eat for 2.5 s. It scans the
level 8 times a second, not every frame, so its own cost stays out of the frame times it
measures. Every minute it logs `azeroll auto census`: the edible things still uneaten by area and
name, and how many of them sit at a height the ball can reach, which is how food on tables shows up.

| variable | meaning |
|---|---|
| `WOW_AZEROLL_AUTO=<s>` | run the autopilot for `<s>` seconds of play, then exit (also exits at the results) |
| `WOW_AZEROLL_AUTO_ROUTE=primary\|tour\|free` | `primary` (default): the shortest trail walk from the start area to the camp. `tour`: every open area on the trail graph, in the cheapest order, then the camp. `free`: the old value-for-distance hunt with no route |
| `WOW_AZEROLL_TELEMETRY=<path>` | one CSV row per second of play, plus `<path>.summary.txt` at the end |
| `WOW_AZEROLL_REPORT=<path>` | the level's volume report (written once the level settles, rewritten if far areas stream in later) |
| `WOW_AZEROLL_SHOTS`, `_SHOT_EVERY`, `_AUTO_FREE`, `_AUTO_TITLE` | as before |

**Telemetry CSV columns:** `t` (run seconds), `wall` (real seconds of play), `phase` (play / finale /
free), `mode` (target, route, hunt, escape, boss, camp, wander), `step` (the route step), `area`,
`radius`, `diameter`, `count`, `trophies` (live attached), `npcs` (carried), `x,y,z` (WoW), `speed`,
`edible` / `collectibles` (counts the ball can eat / all loose), `nearest_edible` (flat yards, -1 when
none), `fps`, `ft_p50_ms`/`ft_p95_ms`/`ft_p99_ms`/`ft_max_ms` (frame time over the last 10 s of
play), cumulative `stuck` (autopilot saw the ball not moving), `unstuck` (`unstuck.rs` teleported it;
detected as a jump far above top speed), `escapes` (autopilot recoveries), `rams` (cumulative), `target` (name@yards), and `events` (steps,
skips, stuck notes, "hogger edible", "hogger rolled up").

**Summary** (also logged as `azeroll auto summary:`): binary path, size and mtime (what ties a run to
a build), recipe hash, time to each tier, to Hogger edible and to Hogger, frame-time percentiles
overall and per tier, stuck/unstuck/escape counts, the longest wait for a pickup and the longest
radius plateau (< 2 % growth), steps entered, and seconds and volume by area.

**Level report** (also logged as `azeroll REPORT`): for every area and trail, count/volume of what is
in it per size band (the ball radius needed to roll it up, from the live `collect::contact`; volume
by the live `Collectible::volume`, in ball-volume units, r³), then a greedy growth curve for the
primary and tour routes: arriving radius, what is edible on arrival, what is eaten, leaving radius,
what is left and the radius needed for it, a travel-only clock, and `STARVE` / `thin` flags where
the next stop has (almost) nothing edible at the radius you arrive with. Limits: claimed map doodads
count only where the terrain has streamed in, creatures are counted by the cube bound of their
authored size, and the clock excludes time spent eating inside an area.

```bash
mkdir -p /tmp/az-tel && flock /tmp/azeroll-run.lock env WOW_NOVSYNC=1 WOW_BG=1 \
  WOW_DATA=$PWD/wow-data/Data WOW_AZEROLL_AUTO=620 WOW_AZEROLL_TELEMETRY=/tmp/az-tel/run1.csv \
  WOW_AZEROLL_REPORT=/tmp/az-tel/report.txt WOW_AZEROLL_SHOTS=/tmp/az-tel/shots \
  WOW_AZEROLL_SHOT_EVERY=30 timeout 1200 target/play/azeroll > /tmp/az-tel/run1.log 2>&1
rm -f benilla-config/azeroll-progress.toml   # a finished run writes a best time
```

- **`WOW_NOVSYNC=1` is required for unattended runs.** On Wayland (Hyprland), a window on a hidden
  workspace gets no frame callbacks, so FIFO presentation blocks for ~1 s per frame and the
  screenshots come out black.
- The world takes about 2 minutes from launch to playable on a cold start (asset loads and pipeline
  warm-up).

## Browser build

Verified 2026-11-01 (Chromium 155, RTX 2070, Hyprland). Azeroll is server-less, so the host needs no
realm: `wenilla-host`'s `--upstream` only matters when the page opens `/ws/3724` or `/ws/8085`,
which Azeroll never does. Game data stays in `wow-data/` and is read by the host, never copied.

```bash
export PATH=$HOME/.cargo/bin:$PATH
export CARGO_TARGET_DIR=$PWD/target-wasm        # keeps the wasm build off the native build's cargo lock
scripts/web-setup.sh                            # once: wasm32 target, wasm-bindgen-cli 0.2.122, tools/wasi-sdk, tools/binaryen
scripts/web-build.sh                            # -> web/dist (~3 min cold; wasm 78 MB, 16 MB brotli)
cargo build --release -p wenilla-host
target-wasm/release/wenilla-host --bind 127.0.0.1:8090 --www web/dist --data wow-data/Data
# Remote play over the tailnet (HTTPS gives the page the secure context WebGPU needs). Use a
# non-root port: the root `/` on this host belongs to another service.
tailscale serve --bg --https=8443 http://127.0.0.1:8090
node scripts/web-smoke.mjs                      # drives Chromium over CDP, output in /tmp/azeroll-web/
```

- Open `http://127.0.0.1:8090/`. The page needs WebGPU.
- `web-setup.sh` installs `wasm-bindgen-cli` into `~/.cargo/bin`, replacing any other version there.
- **Chromium flags on this NVIDIA + Wayland box** (set as the defaults in `scripts/web-smoke.mjs`):
  `--enable-unsafe-webgpu --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan
  --use-angle=vulkan --disable-gpu-vsync --disable-frame-rate-limit`, plus `--ozone-platform=wayland`
  (or `x11`). Without the Vulkan/ANGLE trio the canvas stays black ("Error creating wgpu::Texture");
  without the vsync flags Chromium delivers about one requestAnimationFrame per second to *any*
  WebGPU page and the game looks frozen. Headless loads the world but cannot present it. Launch
  `/usr/lib/chromium/chromium` directly: the omarchy `chromium` wrapper appends its own ozone flag.
- Cold load to playable is about 1 minute with the data warm in the host.
- Measured with the native build and a full `cargo` run competing for the CPU: 47 fps idle, 39 fps
  rolling, in a 937x322 window.

## Windows build

`scripts/win-build.sh` cross-compiles the player build for `x86_64-pc-windows-msvc` with
cargo-xwin and a static CRT, then packages `~/azeroll-win.zip` (exe, launcher bat, README, empty
`Data/`). Test builds go to the user's Windows PC by Taildrop:
`tailscale file cp ~/azeroll-win.zip jjd-pc:`. The game data is sent separately, once.

## Collision and collection rework: findings (2026-11-02)

- **Floors.** `WOW_AZEROLL_FLOOR_TEST=1` (+ `WOW_AZEROLL_FLOOR_RADII`) drives the inn and smithy
  entrances. Result at r = 0.25, 0.3, 0.36, 0.5, 1.0 and 1.8: `bottom == centre_floor` to 5 mm at
  every rest point, on the steps and the hall floor (`/tmp/az-floor3`, `/tmp/az-floor4`). Before the
  dense support rings an r=0.5 ball stopped on the lowest step (sweep touching the 0.29 yd riser
  edge with a tilted normal while no ray sampled that edge); the slide diagnostic that found it is
  `WOW_AZEROLL_SLIDE_DEBUG=1` (or the floor test).
- **Three states are logged** (`azeroll dislodge: ...`, `azeroll bump: ...` for walls,
  `azeroll: rolled up ...` for creatures). One 620 s run logged 243 wall bumps (fences, canopy
  trees, graves), 168 dislodges (loaves, sacks, tombstones, dummies) and 300+ roll-ups.
  `collect::contact` has a unit test on the 2.15 / 1.5 edges.
- **Pacing is still not closed.** Valid growth samples are from the free-hunt autopilot before the
  route rewrite, `FILL_FACTOR = 0.5`: run A (`/tmp/az-pace-a.log`) r = 0.56 / 1.43 / 1.82 / 2.13 /
  2.14 at 60 / 180 / 300 / 420 / 480 s, no Hogger in 620 s (150 s wedged in the fenced graveyard);
  run B (escape heads for the next open area, since removed with the autopilot rewrite) r = 0.37 /
  1.51 / 3.54 / ~4.3 and **Hogger rolled up at 380.2 s**, r = 4.72. That is too fast for the 480 s
  target, and `FILL_FACTOR = 0.4` is the first thing to try; it has no finished winning run (run C,
  `/tmp/az-pace-c`: walked into the camp at r 1.5, was rammed and shed for 300 s). Every run on the
  current level (`/tmp/az-pace-e|f|g`, free and primary routes) parked in the inn yard at r = 0.34–0.36
  for 500+ s with edible items 1 yd away; `/tmp/az-slide2` escaped the same yard by t = 60 s, so it
  is a dithering trap rather than geometry (terrain and floor support agree there). Fix the yard
  start before judging constants.
- Level size budget at t = 6 s (axis band: count, credited volume): <0.5 371/0.8; 0.5-1 591/23.7;
  1-1.5 372/36.8; 1.5-2 339/81; 2-3 482/251; 3-4 432/1023; Hogger needs V = 79.5 (r = 4.3).

## Historical findings from play (before the max-axis/box-volume rework)

- Native build passes. `cargo clippy -p benilla-app --profile play 2>&1 | grep azeroll`
  produces no output; unrelated existing warnings remain elsewhere.
- Hogger spawns at his camp with pickup size exactly 4 yd. Autopilot prioritizes an edible Boss
  regardless of distance or skip list; if not spawned, it drives toward HOGGER_AT to stream terrain.
  It measures position progress for escape detection rather than reported velocity.
- **Pacing is not closed.** Completed 600-second runs include `/tmp/az-pace3.log`,
  `/tmp/az-final1.log`, and `/tmp/az-final2.log`. Runs can wedge against world geometry for
  minutes. The last two completed samples (same roster and camera; before the final growth taper
  and walkable-slope adjustment) were:

  | elapsed, s | target r | final2 r | final3 r |
  |---|---|---|---|
  | 60 | 0.5 | 0.64 | 0.83 |
  | 180 | 1.2 | 1.09 | 1.67 |
  | 300 | 2.5 | 1.15 | victory already reached |
  | 420 | 4.5 | 1.18 | victory already reached |
  | 480 | 6.15 edible soon | 1.18 | victory already reached |
  | 540 | 6.15 edible | 1.18 | victory already reached |
  | 600 | victory | 1.18, timeout | victory at 260.2 s |

- `final3` rolled up Hogger at approximately 258.6 s; results at 260.2 s. This proves spawning,
  reachability on that route, collection, win state and results UI, but is much too fast.
  Its growth K rose from 0.32 to 0.5; **that is not the current formula**.
- Current formula instead tapers 0.32 → 0.14. Its only run, `/tmp/az-tuned1.log`, stopped
  during turn interruption at 324.3 s: r=0.89 at 60 s, 1.47 at 180 s, 1.61 at 300 s.
  No current-formula full-run or win claim is supported. Two full runs of it are still needed.
- Added passive bridge bands: 24 chickens at size 0.6 (20–100 yd ring), 24 sheep at 0.85
  (35–130), 20 cows at 1.2 (50–160), 16 horses at 1.8 (70–200), 12 bears at 2.6 (90–240).
  Scatter stays 320 models within 55 yd; scattered tabletop props are non-solid so they cannot
  cage the starting ball. Claimed prop bump discs are capped at 0.15 of pickup thickness;
  these remain an approximation needing collision review.
- Inspected small/medium/large screenshots: the clump is dominated by actual trophies and the
  old dark sphere is gone. Existing tree trunks/foliage still obscure the camera at some angles.
  Evidence: `/tmp/az-final3/play-002.png`, `play-006.png`, `play-008.png`, `results.png`.
- Sequential runs are required on this machine: starting a second client killed the first
  (exit 137) under memory pressure. An early pre-contact baseline ended at 494 s and is not
  counted as a full run.

## Outstanding

- [ ] Finish pacing: two finished winning runs; target Hogger rolled up around 480 s (run B
      won at 380 s with `FILL_FACTOR` 0.5). Fix the inn-yard start stall first.
- [ ] Resolve persistent geometry wedges before judging growth constants. Position-based escape
      and walkable-slope adjustment are insufficiently verified; avoid tuning around stuck runs.
- [ ] Keep world foliage/trunks out of the view; clump extent clearance alone does not solve this.
- [ ] Measure fps and trophy counts on native and wasm; cap or bake trophies if needed.
- [ ] Readable doodad names (`azeroll/names.rs`, in progress).
- [ ] Ball look: the core is an unlit `StandardMaterial`, so it doesn't match world lighting.
- [ ] Camera inside buildings and doorways; the ball wedges in doorways at ~5 yd.
- [ ] WMO-internal props (inn mugs, candles): claim by `(MODF uid, MODD index)`.
- [ ] Rolling up WMO buildings (D5 extension), sound feedback, collection log UI.
- [x] wasm build and browser test (see "Browser build"). Still open: a clean fps number on an idle machine.

## Authored level system

**D14 — Embedded recipes, restart-time overrides.** `design.rs` deserializes `goldshire.toml`
with serde/TOML (already dependencies). Native `WOW_AZEROLL_LEVEL=/absolute/recipe.toml`
is read on Ready / R; wasm uses `include_str!`. Parse/read failures log and use the embedded
recipe. Coordinates are WoW yards, yaw is radians. Scenes carry explicit model paths, counts,
arrangements, spacing, scale and deterministic scale jitter. Creature scene items reference
species defined in herds. Herds replace the old constant random-ring roster. Trails sample
segments at a configured spacing. Terrain is sampled per spawn; indoor props use a downward
collision ray from four yards above the authored floor hint, below the inn's upper floor.
Map-owned claimed collectibles retain their lifecycle and tombstones.

New modules: `design.rs` owns recipes, authored prop planning/spawning and load validation;
`goldshire.toml` owns the level; `unstuck.rs` owns safe-position history and manual/automatic
recovery. `level.rs` retains collection tuning, tiers and phase flow. `props.rs` no longer
scatters random models. `creatures.rs` consumes the recipe roster and scripts Hogger's pass.

D10 implementation: building sweep uses 0.85r horizontally; full-radius floor support is
shared with Unstuck. Hold Backspace for one second, or hold movement while making <0.12 yd
progress per half-second for four seconds, to search remembered positions. Candidates are
rechecked at current size using floor collision, wall rays, overhead clearance and prop solids.
The room check sums exact filled bbox volumes (conservative max-axis cubes for unknown creatures),
so collection order has no effect. Exit capacity is `2*clearance/1.85`.


D11 layout: a **relocated** Riverpaw camp at the surveyed (-9600,260,48.15) forest pocket.
Hogger's early pass is scripted near the ball, not at `patrol` (see D11 staging); he uses the
existing Ram message on contact, then returns to camp. No collection lock or timer growth grant is used.
The farms require claim radius 650 / leash 680. They are optional long detours, while the
woodcutters supply a compact village-to-forest route. The real Riverpaw patrol, outside the
survey, is not claimed to be reproduced here.


Recipe excerpt (the full recipe is `crates/benilla-app/src/azeroll/goldshire.toml`):

```toml
start = [-9450, 60, 57]
start_radius = 0.25
time_limit = 620
goal = "Hogger"

[[scenes]]
name = 'Inn supper spill'
anchor = [-9460, 32, 56.97]
indoors = true
[[scenes.items]]
model = 'World\Generic\Human\Passive Doodads\Food\Pie01.m2'
count = 14
arrangement = 'table-set'
spacing = 0.5
```

Scene arrangements are `pile`, `ring`, `line`, `grid`, `table-set`, `scatter`. Yaw is radians
about Bevy up; offsets are Bevy X/Z (east/south), anchors remain WoW coordinates. `scale`
defaults to 1, `jitter` to 0; deterministic golden-angle layouts make edits reproducible.
`solid = true` makes a soft prop barrier; farm barriers are open-ended, with bypasses on both
sides. Creature items may use `species = 'Chicken'` instead of `model`, referencing a herd
species template. Herd `at`/`radius` places a group with fitted pickup thickness and passive,
skittish or hostile behaviour. A trail has `from`, `to`, `spacing`, `model`, optional `scale`.
Areas are named circles with a floor hint; `enclosed = true` additionally needs `clearance`
and `doors`. This is a compact recipe format, not an editor or navigation mesh.

Validation counts bands by **pickup thickness** [0,.16), [.16,.32), [.32,.65), [.65,1.3),
[1.3,2.6), [2.6,4), [4,infinity). Recipe models load eagerly for their actual M2 box sizes;
herds use their exact fitted sizes. Claimed map items are added from the streamed initial
inventory, and retained so collection cannot make a later report look empty. Unstreamed map
placements and WMO-internal furniture are not claimed as part of that initial inventory.
Room checks include recipe items on the room's floor and claimed collectibles in its circle.
Unknown model sizes are reported as pending, never certified as zero.

Verification logs: `/tmp/az-build-final.log`, `/tmp/az-test-final.log`,
`/tmp/az-clippy-final.log`, `/tmp/az-wasm-final.log`. Native play build and D10 test pass;
clippy has no Azeroll diagnostics; wasm check passes (unrelated existing warnings remain).
The first layout calibration (`/tmp/az-level1.log`) was deliberately stopped around 310 s,
so is not a full-run sample. It exposed overly fast opening growth (r=1.54 at 60 s) and
proved recovery executed at r=2.77 and 3.40. Opening K is now 0.10, interpolating to 0.14;
this supersedes the historical growth formula and pacing notes above. No time-based growth
or collection gate was added.

### Final authored-level samples (2026-11-01)

The final two runs use the same recipe, growth law and wall-aware test player. Final2 reads
that recipe through the native override; final3 uses the embedded copy. Both were configured
for 620 seconds; the successful one exits through the existing victory/results flow.

| seconds | target radius | final2 radius | final3 radius |
|---|---:|---:|---:|
| 60 | 0.50 | 0.67 | 0.68 |
| 180 | 1.20 | 1.76 | 1.17 |
| 300 | 2.50 | 3.44 | 1.50 |
| 420 | 4.50 | 4.99 | 1.50 |
| 480 | 6.15 edible | 7.54 | 1.93 |
| 540 | | won | 2.94 |
| 620 | | won | 4.05, timeout |

Nearest two-second telemetry samples, not interpolation. Logs: `/tmp/az-level-final2.log`
and `/tmp/az-level-final3.log`. Final2 collected Hogger at **488.5 s**, reached results at
490.1 s, r=7.99. He became edible about 466 s. Final3 timed out at 620 s, r=4.05. It spent
roughly 250–420 s circling the smithy; recovery repeatedly found clear nearby positions,
but the test player reapproached the same building. A complete earlier baseline,
`/tmp/az-level-final1.log`, timed out at r=1.34 before adding wall-ray target filtering.
**Pacing is not closed:** an 8:08 win is demonstrated, but route reliability is not.
The wall-aware test player also rejects vertically unreachable pickups. No movement speed,
collection rule or timer reward was changed to make it pass.

Resolved validation (all recipe models resolved; claimed map counts are the initial streamed
inventory, not a promise that every placement in the 650-yard circle was resident):

| area | <.16 | .16–.32 | .32–.65 | .65–1.3 | 1.3–2.6 | 2.6–4 | >=4 |
|---|---:|---:|---:|---:|---:|---:|---:|
| Inn ground floor | 14 | 8 | 0 | 0 | 0 | 0 | 0 |
| Inn yard | 50 | 82 | 39 | 2 | 1 | 0 | 1 |
| Market | 3 | 11 | 87 | 10 | 5 | 3 | 4 |
| Northern goods | 0 | 1 | 57 | 13 | 2 | 1 | 2 |
| Southwest road | 0 | 0 | 10 | 80 | 15 | 11 | 7 |
| Stonefield harvest | 1 | 5 | 14 | 97 | 41 | 19 | 5 |
| Maclure harvest | 4 | 0 | 33 | 32 | 53 | 24 | 8 |
| Riverpaw camp | 6 | 0 | 4 | 49 | 41 | 73 | 5 |

No global tier-band gaps logged. D10: `PASS Inn ground floor entry=1.89 upper=1.8924358
exit=2.0432432 doors=2`; missing models=0, unresolved spawned props=0. This is a conservative
content-volume certificate for the authored area, not a swept-trophy doorway clearance proof.
Safe-position recovery samples wall/head clearance with rays; it is not a continuous sphere
volume test. Enclosed-area shapes are circles with a three-yard floor-height tolerance.

Inspected images: `/tmp/az-level-final2/play-000.png` (inn spill), `play-005.png` (merchant
barrel line and supplies), `play-008.png` (road / fence scale), `play-014.png` and
`play-016.png` (late forest and camp: substantial foliage obstruction), `results.png`
(victory). Final3's `play-010.png` records the smithy stall. FPS was mostly 60; the rich camp
with ~175–190 visible trophies fell to ~43–49 fps. No browser runtime performance claim.

Remaining authored-level work:
- [ ] Resolve the test player's repeated smithy approaches; demonstrate two reliable wins
      before calling the growth schedule finished. Do not tune growth around blocked routes.
- [ ] Camera foliage occlusion is severe in late forest/camp captures. Improve visibility before
      calling the camp visually polished; the current victory proves reachability, not readability.
- [ ] Dedicated small/large-ball farm-gate bypass playtests and indoor start/exit camera checks.
      The final two routes did not visit the distant farm harvests.
- [ ] Visually verify Hogger's foreshadow contact on a village route. The scripted pass and Ram
      path are implemented, but neither final capture proves an early ram occurred.
- [ ] Harden authoring diagnostics for invalid numeric fields, and replace circular room masks
      with polygons if adding content near neighbouring rooms/floors.

Final D10 review accounts for floor support: the centre sits r above the floor and the
squishy sweep reaches another 0.85r upward, so the conservative exit capacity is
`2*door_clearance/1.85`, not `door_clearance/0.85`. `/tmp/az-d10-smoke.log` confirms the
corrected **2.0432432 yd** capacity and the same 1.8924358 yd growth upper bound. This is
validation-only; neither final run's gameplay changed. Latest verification logs are
`/tmp/az-build-d10.log`, `/tmp/az-test-d10.log`, `/tmp/az-clippy-d10.log`,
`/tmp/az-wasm-d10.log`, and `/tmp/az-d10-smoke.log`. The radius comparison is
`/tmp/az-level-growth.png` (derived from the two-second telemetry).

**D18 — Goldshire's authored NPC cast is non-solid life.** `npcs.rs` and the separately embedded
`goldshire-npcs.toml` add 33 local units: the crossroads guard post (including Dughan), two
explicitly invented road patrols, the inn's staff and upstairs trainers, the smithy merchants,
Kira and seven children. Display IDs, spawn positions and Bo's waypoint loop derive from the
GPL-3.0 CMaNGOS research dataset in `data/goldshire-npcs.json`; the recipe records that provenance.
Kira's route uses the first six real points and returns along them. Native
`WOW_AZEROLL_NPCS=/absolute/goldshire-npcs.toml` reloads on Ready/R alongside the level recipe;
wasm embeds it. Failed overrides fall back to the embedded cast.

NPC maximum axis, contact sphere and growth volume come from the scaled Stand box, matching D4.
They have `solid=0`, so they cannot form walls in narrow rooms. Guards only bump a very small
ball on contact, through `Ram`, with a five-second cooldown. Movement checks a forward body ray,
rejects floor changes above 0.6 yd and avoids the level's authored doorway points by 1.8 yd. Paired horizontal rays also reject
passages narrower than 3.6 yd (including smithy openings), so an NPC stops and begs rather
than entering a pinch. Nearby stationary villagers in the same scene face their conversation partner.
Inn NPCs are included in the same cached D10 inventory as props; validation stays unsettled while
NPC spawns remain pending. Hogger and the creature roster are unchanged.

**Social escalation (Batch 2, Track D; supersedes the old ratio thresholds, which had villagers
running at D = 1.4 × axis, below the 1.5 dislodge ratio).** Fear is no longer a private ratio:
it is `collect::contact(radius, NPC pickup axis)`, the classifier the ball itself uses, plus
a documented comic margin, awareness `12 + ball radius` yd with the old vertical gate.

| `contact` | villager | kid | guard |
|---|---|---|---|
| `Wall` (D < 1.5 a) | calm: glances, chats; backs off if the village is nervous | calm: runs up and cheers | holds; bumps a ball in reach (`Ram`, 5 s cooldown) |
| `Dislodge` | notice; flees from D ≥ 1.96 a (radius margin 0.10) | notice; flees from D ≥ 1.72 a (0.25) | braces in a line; one brave guard (Dughan) charges, bounces off, flees |
| `RollUp` (D ≥ 2.15 a) | flee | flee | flee |

Heard alarms (`panic`, 0..1) add up to 0.30 to the margin (villager flees from 1.54 a when fully
infected) but never pull flight out of the `Dislodge` band: no one runs from a wall. Guards
ignore contagion and flee exactly at `RollUp`.

*Mood ladder* (`npcs::Village.mood`, from `TierReached`, monotone within a run): `Curious` at
tier 0 (glance, point, chat; kids cheer), `Nervous` at tiers 1-2 Critters/Creatures (calm NPCs
within 7 yd + r watch warily and back off; guards ready weapons), `Panic` at tier 3 NPCs and horses
(adults edible: freeze, bolt, drop what they carry), `Chaos` at tiers 4+ (a third of runners circle
their post, a third hide behind buildings out of the ball's line of sight, the rest scatter).
`GrowthBeat` steps raise a mild gasp ripple and the census `heat`.

*Contagion*: `Alarm` ripples (screams when an NPC is rolled up, shouts when one bolts or is
knocked loose, gasps at growth beats and eaten creatures, a tier ripple at every `TierReached`)
travel at 16 yd/s; each NPC hears one once, turns toward it, points or shouts, gains `panic`,
and, after a loud one, freezes for 0.35-1.15 s before bolting. A bolting NPC raises the next
shout, so alarm moves person to person. Budgets: voices from a token bucket (0.8/s, burst 2, three
priorities) plus a per-NPC cooldown; alarms from their own bucket, 10 live ripples, 7 witnesses per
ripple, 2.5 s per-NPC reaction cooldown. Flight is fanned by a per-NPC angle (±63°), zigzags, and
is catchable: panic raises the nominal burst by up to 20 %, but it is always capped at 0.5 × the
ball's undashed top speed at the NPC's own edible size (a unit test sweeps this), with a half-second
stumble every four seconds (every other one a Knockdown trip), a 35 yd leash, and NPCs
sheepishly walking back to their post afterwards.

*Funny beats* (data in `goldshire-npcs.toml`): `carry` (a mug, bread, apple, hammer, cleaver,
bottle or jug is dropped as a tossed prop collectible the first time the NPC bolts, 24 per run),
`partner` (Corina/Kurran Steele, Lyria/Wilhelm, five child pairs flee holding hands with a shared
heading, pace and stumbles, and the survivor mourns when the other is rolled up), `quirk`:
`polish` (Innkeeper Farley never reacts until rolled up), `sleeper` (Remy sleeps until the ball is
3 yd away), `brave` (Marshal Dughan charges only a `Dislodge` ball, at most every 30 s, is thrown
back by it and flees shaken). Kids run up to a small ball and hop and cheer. Doorway exclusion is
kept (an NPC may leave a doorway circle, never enter one).

*Verified (2026-11-02, native play build, logs `/tmp/az-b2d-*.log`, shots `/tmp/az-b2d-*/`)*: the
ladder tour (`ladder`, `ladder-side`, `ladder-focus`; 6 rungs, 95 s) logged every mood step
(Curious → Nervous at tier 1 D=1.40 → Nervous at tier 2 → Panic at tier 3 D=4.60 → Chaos at tier 4
D=6.80), the brave guard charging only at `Dislodge` (D/axis 1.59), being thrown back and fleeing,
freeze-then-bolt, drops (apple, bread, hammers, mug), hiding behind cover (Argus, Wilhelm, Lyria,
Zaldimar) and screams on every rolled-up NPC. Debug radius 1.5 and 3 starts (70 s each) and the
200 s autopilot run exercised the same paths with a real clump. Median fps: 60 (ladder, AUTO=200
runs), 26-40 in the debug-radius runs while other agents' builds loaded the machine (load average
6-8; fps returned to 50-60 within the same runs), so these are not an NPC benchmark. Seen but not
verified: carried-NPC cries only reached `cries=4` in the r=3 run; wail/cheer at a tier step on a
carried NPC and partner grief were not hit by any run; the voices were verified in the log (budget
0.4-0.6 voices/s), never by ear. The default chase camera sits inside the clump at these radii
(camera track), so crowd screenshots come from `ladder-focus`.

*Carried NPCs (#22)*: one real scream at capture, then a muffled `HELPME` every 8-16 s per exposed
NPC (low priority, positioned behind the ball from the camera, so the existing distance
attenuation muffles it), and at every tier step each exposed carried NPC cheers (45 %) or wails.
The 8-animated cap and `park_carried` are unchanged.

Ambient adults play Talk/Laugh/Cheer/Point, smith/cook work clips, eating or sleep, while path
actors walk their authored loops. Adult carried motion requests Fall using FALLING/FALLING_FAR;
children lack that clip, so use Run plus an absolute, non-accumulating rotation wiggle. The
newest eight exposed carried NPCs animate; older/buried ones receive `AnimParked` after the
engine visibility LOD and before pose evaluation. No death state is authored. FLEE and HELPME
voice kits resolve through `EmoteSounds`/`EmotesTextSound`, human race/sex for adults and the
explicit high-pitched gnome-female substitute for children. The existing positional sound
message path enforces mixer settings; this module adds one global bark per second and 6–11 s
per NPC. Attached voices use the ball's world position, never a child's local trophy transform.
The current shared sound-message path does not provide a muffled voice filter.

`WOW_AZEROLL_NPC_TEST=ladder` instead rolls the ball (r = 0.3, 0.7, 1.6, 2.3, 3.4, 4.2, 14 s per
rung, 1.6 yd/s north from the guard post's south side) through the crossroads crowd: one rung per
mood, with the game's own camera; the clump is not dressed, so the ball itself looks small.
`WOW_AZEROLL_NPC_TEST=1` adds a deterministic capture tour (20-second stages: guards, stable
yard, defiance, market panic, collection) when paired with the ordinary autopilot and screenshot
switches. It is a test fixture, and overrides ball position/radius only when explicitly enabled.
Visual flail choice, full-cast spawn coverage, D10 and performance results must be checked from
the capture run before considering these behaviour settings final.

NPC integration verification (native shared collection run `/tmp/az-coll2.log`): 32 of 33 authored
units spawned (Melika subsequently switched to the indoor floor probe); Stand maximum axes were 1.93–2.01 yd for adults and 1.16–1.29 yd for children.
D10 included the NPCs and passed: upper radius 2.006924 yd, exit limit 2.043243 yd. Collected
Kira, both crossroads guards, Dughan, Erma, Wilhelm and Lyria appear in the pickup audio log.
`/tmp/az-coll2/play-010.png` shows attached human bodies and outstretched limbs among the props.
The 16 Azeroll unit tests (including cast/routes, reaction profiles and catchability) passed in
`/tmp/az-tests-final.log`; the native play build and wasm check passed. The focused capture tour
is the source for the final visual/performance assessment below.


NPC focused play findings (`/tmp/az-npc.log`, completed 300 s): seven NPC pickups are logged
(Corina, Lyria, Wilhelm, a guard, Andrew, Kurran and Argus). Fall was retained: the spread limbs
and bent legs remain readable on the rotating clump (`play-012.png` and `play-013.png`), without
using a death clip. Ambient samples t=4..38 had median 60 fps (59–61); reaction/streaming samples
t=40..80 had median 30 (23–55); carried-stage samples t=100..300 had median 59 (20–60), with up
to 162 total trophies. These are whole-village figures, with concurrent compilation during part
of the run, not an isolated NPC benchmark. Seven attached NPCs were exercised; the >8 park
boundary still needs a dedicated stress capture.

The clearer NPC-only camera aim and Melika's `indoors=true` floor-probe correction are
implemented and compile-checked, but unverified in play. The queued second capture was cancelled
at the lead's request so combined integration can verify them. First-run guard Alarm and villager
Flee are confirmed in the log; existing scene shots are poorly framed and are not accepted as
clear guard/alarm visual proof. Remaining polish: muffled carried voices (the sound message path
has no per-play filter), a clean stress benchmark, and the combined scene/camera/Melika capture.
Final native play build: `/tmp/az-npc-complete-build.log`; final wasm check:
`/tmp/az-npc-wasm-complete.log`; final clippy: `/tmp/az-npc-clippy-complete.log` (no Azeroll
warnings). The earlier 16-test pass is `/tmp/az-tests-final.log`; the redundant final test-profile
rebuild was stopped on handback to free shared build resources. No commits were made.

**D20 — The play HUD is nearly wordless; the world carries the joke (Batch 2, Track E; gap #15 and
the HUD side of #16).** Write set: `hud.rs`, `hud_models.rs`, `hud.wgsl`. `progress.rs` still owns
title, pause, finale, results and Free Roll flow; those screens keep working and are only read.

- **On screen in play:** the size dial (rosette, goal ring, "Goal 8yd 22in" on the ring's end), the
  clock with its number, the item bubble, the Prince corner. The always-on goal title and
  requirement sentence, the "Rolled up: N" count and the four-line toast stack are gone. The
  goal ("Roll up Hogger!" and "Grow to 8 yd 22 in across"; Free Roll: "Free Roll") shows for
  `INTRO_HOLD` = 4 s of *play* time (`run.elapsed`, so pause holds it), then fades over 1.2 s and
  does not return. The requirement, the count, the size and the time moved to the pause screen
  (`PauseText`); the pickup log was already on the results screen. Tier banners (`run.banner`,
  `level`'s) are unchanged and transient.
- **Cues** (`Cues` resource, reset on `OnEnter(Ready)`): `GrowthBeat` → a pulse on the dial and a
  bright "tick" running along the filled ring (shader kind 0, `p.z`); `TierReached` (and Hogger
  coming ready) → a rosette bloom: four petal rings burst outward in gold, emerald, blue and
  crimson over ~1.2 s with a swell of the dial (`p.w`). The continuous per-growth pulse is kept.
- **Guidance, never a nag** (`guidance`, `refresh_guide`; one arrow widget, shader kind 3, drawn
  procedurally, rotated by its `UiTransform`):
  - *Hogger ready:* the first time `contact(radius, hogger.pickup) == RollUp` in a run (once per
    run; shedding and regrowth never re-announce) → "Hogger is ready!" under the banner for
    ~3.4 s, the banner sound, a bloom. While he is edible and **off screen** an orange arrow rides
    the screen edge toward him (the camera's local x/y direction; directly behind → points down)
    and fades out when he is on screen. No arrow for a Hogger already visible.
  - *Plateau:* no pickup for `PLATEAU_SECS` = 20 s of play (not wall-clock, not paused) and Hogger
    not edible → one pale arrow to the nearest meal cluster for `HINT_SECS` = 7 s: off screen on
    the edge, on screen bobbing above the cluster pointing down. At most `HINT_MAX` = 3 per run, at
    least 60 s apart, cancelled by any pickup, and silent when nothing qualifies (re-looked-for
    after 8 s, not per frame). A meal is something the ball can take now (`contact == RollUp`),
    not a crumb (< 20% of what it could take), within 110 yd; clusters are counted on a 6 yd grid
    with 3×3 neighbours, three or more beat any lone thing, the nearest rich cell wins
    (`find_cluster`, unit-tested). No hints during the finale or after the win.
  - `WOW_AZEROLL_HINT_TEST=1` (debug): plateau after 4 s, pickups neither reset nor cancel it, so
    the autopilot (which never stalls) can show the hint.
- **Item bubble:** the name shows 1.5 s after a pickup then fades (the bubble itself 3.2 s + 0.8 s);
  every `Collected` links a combo chain (gap ≤ 1.4 s, counted before the 0.14 s swap throttle) and
  from the second link a "×N" badge on the bubble pops on each link, warms from gold to orange
  with length and fades 1.2 s after the chain ends; the rim and glass flash the quality colour of
  the thing (grey/white/green/blue/purple, Hogger orange), harder the bigger the mouthful was for
  the ball (`fill`). Shader kind 2: flash strength `kt.w`, colour `p.yzw`.
- **The bind-pose creature:** *not* animated and *not* pose-frozen. The HUD clones only the bind
  twin (`PortraitPart::static_mesh`); the live skinned mesh cannot be moved to the stage (the
  palette rows are rig-relative and add the live rig origin back), and the only animating path,
  `portrait::booth::spawn_booth_model`, is `pub(super)` to the portrait module (outside this
  track's write set). Instead a creature or Hogger no longer spins: it sways in three-quarter
  view (±0.5 rad), breathes (squash and stretch) and bobs, with a stretch on the pop. Props still
  spin. Humanoid NPCs read well this way (Bo, `/tmp/az-b2e-a/play-050.png`); a creature with an
  extreme bind pose is unverified. The real fix is one line: make `spawn_booth_model` (and its
  `BoothPart`, `BoothMotion` types) `pub(crate)` and bake a `Loop` booth into `ITEM_LAYER`.
- **Prince corner** (`prince_reactions` in `hud_models.rs`, effects in `hud.rs`): `ball.dash > 0`
  eases a lean-in (camera pushes closer and lower, the corner scales 6%); `Dash::Started` a gold
  rim flash, `Dash::Ready` a pale blue one; a hard `Bumped` (≥ 2.0 yd/s) or `Touched::Wall`
  (≥ 3.5 yd/s), at most every 0.45 s, a red flash and a jolt (camera shake plus a rattle of the
  corner), scaled by strength. Reads the Batch 2 contract messages; no new timers. His own
  animation is still `creatures::prince`'s (Track C).
- **Text shadows fade with their text.** A fading `TextShadow` left at full alpha ghosted the
  faded goal title as dark lettering over the finale; every fading HUD text now fades its shadow.
- **Verification:** native play build clean; 4 new `hud` unit tests (cluster choice, crumbs and
  giants and range excluded, a lone meal, `fill`) and the other azeroll tests pass except
  `design::progression::tests::routes_run_from_the_start_area_to_the_camp` ("the tour passes
  everything the primary route does"), a route assertion over `goldshire.toml` that another track
  is editing; no azeroll clippy warnings; wasm check passes (four unrelated warnings in
  `entities.rs`, `loading_screen.rs`, `net.rs`). Captures (1000×1018-ish window, native, NOT
  browser): `/tmp/az-b2e/` (debug radius 7: Hogger ready, edge arrow, wince flash, results,
  finale) and `/tmp/az-b2e-a/` (default start: title, combos, colour flashes, plateau arrow over a
  cluster). Not verified: browser/WebGPU HUD, a plateau on a human-controlled stall, narrow
  windows (the layout is the old 768-unit scale; the goal title keeps its gutters), the Prince
  dash lean as a still (the Prince wince is in `play-005.png` of the first run).

**D20a — The Prince corner is its own gnome on a private stage (HUD rework, supersedes the Prince
bullet of D20 where they differ).** Write set: `hud_models.rs` (`hud.rs`/`hud.wgsl` unchanged; the
bubble, rim, flashes and the corner's UI lean/rattle are as before). Feedback: the corner Prince
wandered and was too small, because its camera rode the world Prince, who circles the clump, turns
and (now) is 0.25-scale. Katamari's corner Prince stands in one spot, large, running on the spot.

- **A second gnome** ([`StagePrince`]), spawned by `stage_prince` through `units::spawn_unit` (so the
  stock attach and animation build and drive him) with the world Prince's display id (read from his
  `NetEntity`, cached), at `PRINCE_STAGE` = (400, 3000, 0), scale `PRINCE_SCALE` = 1.0 (about 1 yd
  tall; the world Prince is 0.25), yawed −0.45 rad toward screen left. One fixed camera (layer 41,
  `PRINCE_DIST` 2.3, eye 0.32, looking at 0.5 of his height, FOV 0.5) frames him full body, a little
  low. The camera and his transform are never written again. The world Prince's parts are no
  longer on layer 41 (they are not touched at all); the stage gnome's parts are on layer 41 only,
  so the world camera never draws him.
- **Isolation.** `Carried` (every net motion, facing, spline, ground-seating, scale-ease and
  attach-transform system skips a `Carried` transform); `RunEntity` removed (he survives restarts;
  `stage_prince` respawns him if he is ever missing); **`WorldUnit` removed each frame**
  (`publish_world_units` re-adds it whenever his model resolves): its reader `exterior_cull`
  elects every net body by the *world camera's frustum* and would set a unit 3000 yd up
  `Visibility::Hidden` every frame, blanking the corner. He has no `Collectible`, `Prince`
  or `Push`, so no azeroll system (AI, bumps, pickup, the world Prince's stagecraft) queries him;
  `ScriptedFacing` (from `spawn_unit`) keeps the facing pump off him. He is `Visibility::Visible`.
- **Animation, in place** (`prince_motion`, `prince_reactions`): only his `MovementState` and one-shot
  `EmoteAnim`s are written. Ball ground speed > 0.25 yd/s → `FORWARD` at a run-clip speed of
  `5 + 8 × (speed / top_speed(radius))` (smoothed, rate 8/s), ×1.3 while `ball.dash > 0`; stopped →
  flags 0, the stock Stand idle. A hard `Bumped`/wall `Touched` (same thresholds and 0.45 s gap as
  the rim wince) plays StandWound (8); `TierReached` plays EmoteCheer (68). The dash's lean-in and
  gold flash are the corner's UI scale and rim as before; the old camera push-in and camera shake
  are gone (the camera is fixed; the corner's UI rattle remains).
- **Not verified:** the hit and cheer clips as stills; a sustained idle in a screenshot (the
  autopilot never stops long enough: the stand pose is `play-001.png`, before the first roll).
  Footfall/creature sounds the stage gnome's animation might raise at 3000 yd are not checked.
  Captures: `/tmp/az-prince/` (native, 1000×1018-ish, default start).
