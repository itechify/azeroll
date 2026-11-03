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

**D8 — Camera: fixed FOV, distance and pitch follow the radius, the world never moves it.** The stock
scoped-view system resets FOV every frame even under `CaptureMode`, so Azeroll grows the boom distance
instead of the FOV. Terrain streaming follows the ball through `Player.pos` → `ViewFocus::body`, and
the far clip grows with the ball within the engine's 177..777 range.
The camera is Katamari-style: boom and pitch are a smooth function of the radius (plus the player's
zoom/orbit and the yaw's auto-align behind the heading) and **no obstacle alters them**: no wall or prop
boom shortening, no side-bearing or pitch-lift search, no glance. The lens may sit behind or inside
trees, walls and roofs; the cutout (below) makes what lies between it and the ball see-through. The
one world input is the terrain floor: if the ground rises above the lens it is lifted (boom length
kept; fast up, 1.5/s back down, hard floor underneath), so it never goes under the ground.
Pitch eases from 20° to 31° over radii 0.25–3.75 yd and on to 38° by r≈13, with the boom at
2.2 + 7r (up to +30% at huge sizes). The boom is at least twice the largest attached radial extent
plus the radius, but that extent is smoothed (eases out at 1/s, back in at 0.25/s), so a long item
attaching glides the lens out and shedding it never pulls it in fast; boom shrink after shedding
radius glides at 1.5/s.
Whole-part ghosting (`props::occlusion_fade`) is left only for trophies: 45% when large, protruding
and on the camera's side of the clump, or around the lens. A ghosted part eases back, not snaps.
`WOW_AZEROLL_CAM_SCENE=post|lens` (`debug_fill.rs`) stages posts around the first camera for a check.
W/S roll along the heading; A/D turn it, Space flips it, Shift dashes for
0.65 seconds with 1.8 seconds between starts. The camera follows heading with a temporary mouse
or Q/E orbit. The fixed-scale gnome Prince stands behind the clump, using an NPC-appearance
male gnome display resolved from the creature DBC catalog.
`WOW_AZEROLL_DEBUG_RADIUS` starts are dressed by `debug_fill.rs`: it sets `BallState::buried_volume`
to 0.65 r³ (the shell's volume) and attaches up to 200 copies of nearby small props (non-flora, 4%–93% of r
across) to `BallSpin` as `Attached`, seated like `collect` does (centre at `r − 0.35·size`), so debug captures look like a
clump of that size.

*Scale grammar.* `camera::scale_beats` reads `TierReached` (so it inherits `level`'s ratchets: the
camera keeps its own tier high-water, so a shed across a line never repeats a beat) and writes
`CamRig::{beat_boom, beat_pitch}`, which `place_camera` applies on the smoothed boom/pitch. It is
non-blocking: no input, ball or time is touched. FOV and slow-mo are deliberately unused.
* Standing frame per tier reached: boom +5%, elevation +1° (so +25% / +5° at Hogger's tier), a
  3 s smoothstep zoom-out that only ever widens. There is no overshoot-and-return, no `GrowthBeat`
  breath and no glance toward an area (the old yaw/look reveal was removed: the view never moves on
  its own). The glance's data (`TIER_REVEALS`, recipe `reveals`) is removed.
* Far clip/fog: +40 yd per tier reached, eased over 1.6 s on top of `drive_ball`'s own
  `350 + 25 r` (fog ends at `min(zone fog, farclip)`), applied after `drive_ball` each frame.
  Beats within 2 s of the run start (debug start sizes) set the framing only.
* Debug: `WOW_AZEROLL_BEAT_TRACE=1` logs the offsets every 0.2 s for 8 s after a beat;
  `WOW_AZEROLL_BEAT_SHOTS=<dir>` screenshots 0/0.4/0.8/1.2/1.8/3/6 s after each tier beat.

*See-through window (cutout).* `cutout.rs` publishes a disabled-by-default
`lighting::FoliageCutout` resource (name kept from the foliage-only version) after camera
placement. Two vec4 rows in the existing shared scene-light buffer carry the physical-pixel centre,
radius, planar eye-depth limit, height floor and enable flag; no per-frame material mutation or new
GPU binding. `wow_model.wgsl` (entity lane: M2 props, trees, WMO parts, interior props) and
`static_gx.wgsl` (retained scenery: doodads, WMO walls, ceilings, roofs) discard **any world
fragment** inside the circle that is nearer the eye than the limit and above the floor, with stable
screen-space dithering across the circle's outer 28%. Limit = ball-centre depth − 1.1 × the clump
extent (largest attached `radial + size`) − 0.7 yd, so attached trophies and the Prince (who stands
at the clump's back, on the lens side; units are not told apart from scenery in the shader) are never
cut, and anything at, beside or beyond the ball stays solid. The ordinary floor plane is the ball's
centre − 0.5 r. A second foreground plane clears low chair rails/legs when nearer than ball-centre
depth − max(1.5 r + 0.20, clump extent + 0.22) yd; fragments below centre − 0.98 r remain solid,
preserving the supporting floor. Circle radius = 1.55× (2.1× when `indoors_at` the
ball, e.g. the Lion's Pride Inn, where the lens is often outside the walls or above the ceiling) the
clump's projected radius, with a 0.25 yd indoor minimum to accommodate the fixed-size Prince.
Indoor finales keep the gameplay boom/pitch and enlarge the opening by 30%; the ordinary outdoor
pull-out remains unchanged. UI model tiles are excluded as before. The old foliage-only markers (M2
bit 14, static word bit 30) are no longer consulted. Other modes leave the resource at zero,
preserving ordinary rendering. The probe/palette offsets derive from the enlarged prefix; off-world
light blobs allocate the full layout with zeroed cutout rows.
`WOW_AZEROLL_CAM_TRACE=1` logs the framing and window once a second; `WOW_AZEROLL_NO_CUTOUT=1`
switches the window off for an A/B.

*Interior rooms seen from outside.* The engine floods WMO groups from the camera's room (and draws
exterior content only through portal windows), so a lens outside the inn's walls would draw no rooms.
`wmo_portal::WmoRevealPoint` (a resource, `None` by default) is a small seam: the building whose
collision bounds contain the point draws every group. `cutout.rs` sets it to the ball each frame; the
camera's own claim (fog, windows) is untouched, so the lens outside still gets the outdoor scene.

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
recovery remain fallbacks. Recovery requires at least max(12 yd, min(4r, 24 yd + r)) of separation;
a three-yard rewind repeatedly returned the autopilot to the same doorway approach (D34 capped the 4r,
and the automatic wedge rewind now needs a ball boxed in on every side). `WOW_AZEROLL_FLOOR_TEST=1` drives six 18-second entrance probes
(inn then smithy at r=0.5/1.0/1.8, or `WOW_AZEROLL_FLOOR_RADII=a,b,c`); combine with AUTO=108
(56 for the inn only) and SHOT_EVERY=6. Each stage logs `centre_floor` and `bottom`; a resting
ball has them equal (verified to 5 mm at r=0.25, 0.3, 0.36, 0.5, 1.0, 1.8). With the test on,
the log also carries a once-a-second `slide from/to ... wall=... ahead_rest=...` line.

**D11 — Hogger (camp and foreshadow script implemented). Update 2026-11-02: he is no longer the goal (D28).** In
Market Day Mayhem the goal is the Prize Pumpkin; Hogger keeps his pass, camp and staging (the herd flag `boss = true`
makes him a `Kind::Boss` for staging, the thump and the music duck) and rolling him up is the optional "hogger"
challenge. The text below describes him as the goal, which was true until then: Growth is the experience; Hogger is its
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
`goldshire-mambo.mp3`, `goldshire-go-go.mp3` and `kuru-kuru-azeroth.mp3` (くるくるアゼロス, 178.8 s); ducked to 0.3 while paused) are `include_bytes!`'d in `azeroll/music.rs`,
which sets the `sound::ModeMusic` resource from the phase. The playlist rotates through the level
tracks: the next one fades in a second before the current one would loop, using their lengths as
measured with `ffprobe`. Each run opens one track further on than the previous run. All five tracks are
the user's own compositions. `sound/mode_music.rs` loops the track on the glue-music path (`mixer::play_stream`:
decode-streaming natively, browser `decodeAudioData` via `web_load::begin_bytes` on wasm), crossfades
over 1 s, follows the Music slider, and holds WoW's zone music down through the cinematic's
`music_suppressed` flag. Ambience and SFX are untouched. The browser build is compile-checked
only; the audio itself was verified natively.

*Update 2026-11-03 (user's request):* the hub, ready and plain results screens play the user's
"A Wonderful Walk Through Azeroth" (`a-wonderful-walk-through-azeroth.mp3`); the old title theme
`azeroll-title.mp3` joined the level playlist and each level's song list in `campaign.rs`. Nothing
plays under the boot loading screen (`music::Boot`: silent until the first cover rises and drops,
or 20 s if none ever does). The loading screen shows the user's painting
(`assets/art/azeroll-loading.png`) through the engine's default-absent
`loading_screen::LoadingArtOverride`, inserted at Azeroll's startup.

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

**D22 — WMO furniture ownership and collision.** Selected MODD records can be claimed
before all engine prop work using `(map, MODF uid, absolute MODD index)` keys and a
predicate snapshot latched at registration. Azeroll's embedded inn allowlist excludes
light/effect and structural fixtures; mode-owned frozen M2 copies keep per-run
collection tombstones and separate residency, preserving carried roots on release.
Tables/chairs/benches use corner legs plus an upper slab; other furniture uses oriented
boxes. Legs yield once growth approaches their narrow gap; the seat/top stays solid, so
a ball that entered small can still leave. Movement, Unstuck and autopilot share those
radius-dependent boxes, Wall-only under D6. D29 supersedes the original lack of mode-solid floor support. See [wmo-collectibles.md](wmo-collectibles.md) for the
selected inventory, limitations and remaining L1 playability work.

**D23 — Scale-relative retirement and bounded pickup search.** Collection now rejects loose
candidates outside the conservative envelope of all trophy OBBs before doing adhesive tests;
inside it D4/D6 are unchanged. D17 seating data is cloned only on pickup. `lod.rs` caches local
CoM moment/extent until pose, bounds or membership changes. Every 0.25 s, hidden trophies and
those with bounding radius < r/24 transfer their exact credit to the D3 shell; ball volume is
unchanged. The safety budget is 160 roots (brief overshoot between batches), deepest/oldest first;
only over-budget batches sort. All retired entities are excluded from same-frame shedding.
The 0.6 s pickup grace also uses first-seen virtual time, because Free Roll freezes Run.elapsed.
Small/hidden carried rigs use `AnimParked`, after the NPC budget; shedding releases our park.

World cleaning uses strict power-of-two maximum-axis tiers. Once D eclipses a tier ceiling by
24×, claimed scenery farther than max(60 yd, 8r) permanently loses render/animation descendants.
A minimal collectible root preserves exact food credit, report, census and autopilot semantics;
no fake pickup/score is awarded. The release hook retains this ledger across stream-out/in.
Authored props (including supper), gates, units and goals are excluded. Ready deletes cleaned
roots through `RunEntity`, restores complete props and clears retirement IDs. A ten-second
warmup also advances in Free Roll. `WOW_AZEROLL_PROFILE=1` (`?azeroll_profile=1` on web) logs
5-second elapsed system mean/p50/p95; disabled probes allocate/read no timers and use platform Instant.

Native evidence, 2026-11-02: i7-10750H / RTX 2070 Vulkan, `play`, NOVSYNC/BG, 45 s autopilot
per dressed debug start. Both builds use app codegen-units=255 to isolate shared-target
fingerprints. Frame percentiles cover play; collection means/trophy ranges cover seconds 10–40.
Starts grow, RNG/routes vary, and other builds competed for CPU. After r=8 includes the final
Free Roll clock refinement; smaller after rows precede that refinement.

| start r | live trophies before → after | collect mean ms before → after | frame p50/p95 ms before → after |
|---:|---|---|---|
| 0.5 | 69–138 → 68–165 | 1.257 → 0.382 | 16.6/18.6 → 16.6/19.1 |
| 2 | 149–302 → 118–163 | 7.517 → 0.353 | 19.4/49.7 → 16.6/19.2 |
| 5 | 144–229 → 160–162 | 11.351 → 0.274 | 24.6/28.4 → 16.6/18.8 |
| 8 | 188–297 → 131–180 | 18.418 → 0.288 | 32.7/38.2 → 16.6/19.0 |

Measured cause: uncullable loose × trophy tests dominate. At r=8, physics mean is
0.174 → 0.190 ms and occlusion 0.178 → 0.175 ms, while collection falls ~64×.
The r=5 trophy counts overlap, yet collection falls ~41×. Cache rebuilds average
0.003–0.011 ms; world cleanup 0.16–0.23 ms once/second. Only 3/27 world visuals retire at
r=5/8: lifecycle groundwork, not the main measured win. Small-ball frame p95 does not improve.

Repeated 30 s r=8 A/B/B/A runs: collection means A=17.56/24.56 ms, B=0.75/0.31 ms.
Frame p50/p95: A=30.6/42.3 and 39.2/43.2; B=41.2/55.1 and 16.6/19.5 ms.
The slow B run's other system timings also rose under shared load; this is CPU scaling
evidence, not an isolated FPS guarantee. A 400 s-budget normal run won at 240.4 s:
617 pickups, r=5.65, max 167 live trophies, frame p50/p95 16.8/38.1 ms. Final Free Roll
(runtime score clock zero) also retired 27 world visuals and stayed at max 166 live roots.

Validation: native `play` build, 103 Azeroll tests and wasm/WebGPU check pass. New tests cover
OBB-envelope contacts, cache invalidation, volume/scene protection, Ready reset and Free Roll
aging/warmup. An r=8 screenshot confirms the packed clump. Evidence: `/home/jjd/az-patches/perf-evidence/`.
Residual limits: food metadata still requires a linear loose scan; roots are budgeted, not batches
or GPU memory. Full-detail WMO trophies still need a separate representation/batch budget.

## Module map (`crates/benilla-app/src/azeroll/`)

| file | owns |
|---|---|
| `mod.rs` | plugin, `Phase`, shared components, resources and messages, reset |
| `campaign.rs` | the level registry (`LEVELS`), `CurrentLevel`, boot level and phase, `Campaign` (every recipe's `[level]` data, unlock rule), `SelectState` |
| `select.rs` | the level-select hub: keys, the `CardView` view model, the replaceable widgets |
| `level.rs` | global collection tuning, the tier table derived from the recipe, goal evaluation, key flow |
| `design.rs`, `goldshire.toml` (Market Day), `lions-pride.toml`, `stormwind.toml` | recipes: parse, native restart reload, scenes / trails / herds, barriers, validation |
| `unstuck.rs` | safe history, current-size clearance checks, Backspace / wedge recovery |
| `ball.rs` | ball entities, input → rolling physics, edge-refined support (`ring_support`), grounding and sliding, the render-only vertical blend (`Shown`, D34), focus publication |
| `glitch.rs` | the per-frame movement trace and frame-sequence captures behind D34 (`WOW_AZEROLL_GLITCH`, off by default) |
| `camera.rs` | adaptive chase camera |
| `pad.rs` | the gamepad (twin-stick maths, dash gesture), the dash wind-up state machine, pad menu keys, the fake-pad lever (D32) |
| `collect.rs` | overlap tests, roll-up and attachment, growth, bumps and shedding, burying |
| `props.rs` | the claimed-doodad lifecycle: copies, sizes, tombstones |
| `creatures.rs` | creature spawning, wandering, fleeing and aggro, `Collectible` sizing |
| `units.rs` | local unit spawn adapter, coordinate helpers |
| `npcs.rs`, `goldshire-npcs.toml` | 33-person authored cast (Market Day's; a level's cast is its registry entry's), ambient scenes, size reactions, voices and carried animation budget |
| `hud.rs` | size, timer and count HUD, pickup toasts, banners, results screen |
| `sfx.rs` | the audio grammar: pickup pops, voices and thumps; touches, cues and the roll; one voice budget |
| `music.rs` | embedded tracks, phase → `ModeMusic`, the duck under big moments |
| `replay.rs` | the replay layer (D33): the `Wardrobe` the Prince's display is resolved from, `ParcelFound` / `CousinCaught`, the cousin's size window, the cousin's caption, the capture levers |
| `lod.rs`, `profile.rs` | clump geometry cache, visual retirement, carried rig parking, opt-in CPU probes |

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
| `WOW_AZEROLL_LEVEL_ID=<id>` | skip the level-select hub and boot into that level (`lions-pride`, `market-day`, `stormwind`); with `WOW_AZEROLL_AUTO` alone the level is `market-day` (D21) |
| `WOW_AZEROLL_AUTO=<s>` | run the autopilot for `<s>` seconds of play, then exit (also exits at the results) |
| `WOW_AZEROLL_AUTO_ROUTE=primary\|tour\|free` | `primary` (default): the shortest trail walk from the start area to the camp. `tour`: every open area on the trail graph, in the cheapest order, then the camp. `free`: the old value-for-distance hunt with no route |
| `WOW_AZEROLL_TELEMETRY=<path>` | one CSV row per second of play, plus `<path>.summary.txt` at the end |
| `WOW_AZEROLL_GLITCH=<path>` | one CSV row per played frame: the ball after each stage that may move it, support source, step lift, wall closing, camera, frame time, render blend (D34, `glitch.rs`) |
| `WOW_AZEROLL_GLITCH_SHOTS=<dir>:<t0>:<t1>:<every>` | a screenshot every `<every>` run seconds between `t0` and `t1`, for frame sequences of one spot (works without the autopilot) |
| `WOW_AZEROLL_REPORT=<path>` | the level's volume report (written once the level settles, rewritten if far areas stream in later) |
| `WOW_AZEROLL_PLACEMENT=1\|<path>.txt` | the placement validator (D35): once the recipe has settled, check every authored prop and ramp on the live world and write `/tmp/azeroll-placement-<level>.txt` (or `<path>.txt`); `_PLACEMENT_EXIT=1` quits after it, `_PLACEMENT_GRID="x0 y0 x1 y1 ztop[;…]"` adds a 0.25 yd top-surface height grid of each rectangle |
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

[goal]                      # D21: kind = "collect" (a herd species) or "size" (a diameter)
kind = "collect"
target = "Hogger"

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
ball on contact, through `Ram` (fair-play rules in *Fair rams* below). Movement checks a forward body ray,
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
| `Wall` (D < 1.5 a) | calm: glances, chats; backs off if the village is nervous | calm: runs up and cheers | holds; bumps a ball in reach (`Ram`, 6 s cooldown, see *Fair rams*) |
| `Dislodge` | notice; flees from D ≥ 1.96 a (radius margin 0.10) | notice; flees from D ≥ 1.72 a (0.25) | braces in a line; one brave guard (Dughan) charges, bounces off, flees |
| `RollUp` (D ≥ 2.15 a) | flee | flee | flee |

*Fair rams (2026-11-03, `ram.rs`).* Every `Ram` (guards, creatures, Hogger's pass) goes through
`ram::Rammer`. A global grace (1.75 s) means no second attacker's ram lands right after the last; a guard
bumps no more often than every 6 s and then backs off from the ball for 3 s (and does not brace forward
while on cooldown), so he is not standing on the ball's way out; the knock direction is turned aside (steps
of 40° up to 120°) until a ray from the ball finds 3 yd of room, and with no room at all (< 1.2 yd) the
ram is not thrown. The guards' direction was also backwards before: `dir` is *away from the attacker* and
the guard bump sent `-toward` (into the guard). A run in which Dughan and a Stormwind Guard bumped a 1 yd
ball at the post and the player needed Unstuck was the trigger; the ball's pin that time was the pocket of
two solid inn tables the knock flung it toward, so the knock check is a ray, not a guarantee against pockets.

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
  `NetEntity`, cached), at `PRINCE_STAGE` = (400, 3000, 0), scale `PRINCE_SCALE` = the world Prince's
  0.25, yawed −0.45 rad toward screen left. One fixed camera (layer 41, `PRINCE_DIST` 2.75, eye
  0.32, looking at 0.5 of a scale-1 gnome's height, all × `PRINCE_SCALE`; FOV 0.5) frames him full
  body, a little low. The same scale matters: the driver divides the run clip's rate by the model
  scale and picks Run/Sprint from the raw speed. The camera is never written again; his transform keeps its place and yaw (only the lean below turns it). The world Prince's parts are no
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
- **Animation mirrors the world Prince, in place** (`prince_motion`, `prince_reactions`).
  `prince_motion` copies the world Prince's `MovementState` (his stride speed and forward flags, so
  the Run/Sprint pick, the speed-up, the dash scramble and the wind-up all come with it), his
  `CastHold` (ReadySpellDirected 51: full-body when standing, torso overlay while moving) and the
  pitch of his root (the lean) × `LEAN_SHOWN` 0.6 (the full lean pushes the head out of the frame).
  Nothing else of the transform is written. At the same scale and speed his gait and cadence are the
  world Prince's. A hard `Bumped`/wall
  `Touched` (same thresholds and 0.45 s gap as the rim wince) plays StandWound (8); `TierReached`
  plays EmoteCheer (68). The dash's lean-in and gold flash are the corner's UI scale and rim.
- **Why he was frozen (fixed).** The driver was running him (Run/Sprint armed, `AnimationPlayer`
  playing) but the corner showed one crouched pose: `creature_anim::lod::gate_rig_animation` parks
  pose evaluation (`AnimParked`) for every rig outside the *world camera's* frustum for 0.5 s, and a
  stage 3000 yd up always is. He now carries `portrait::StageRig`, the marker the gate skips for
  booth stages. Same family as the `WorldUnit` cull above: any world-view gate that reads the world
  camera will treat the stage as off screen.
- **Not verified:** the cheer clip as a still, and a sustained idle (the autopilot never stops
  long enough; the stand pose is the first shot, before the first roll). Run poses: a 12-shot
  montage of the corner, `/tmp/az-prince2/fixed2-montage.png`.
  Footfall/creature sounds the stage gnome's animation might raise at 3000 yd are not checked.
  Captures: `/tmp/az-prince/` (native, 1000×1018-ish, default start).

**D21 — The campaign foundation: levels are recipes (decided 2026-11-02, built by the campaign
track).** Azeroll is a three-level campaign on one world (Eastern Kingdoms, `MAP = 0`) with a
level-select hub: `lions-pride` ("Last Call at the Lion's Pride": inside the inn, tiny start, reach
1.5 yd, the doors closed), `market-day` ("Market Day Mayhem": exactly the former Goldshire level,
Hogger the goal) and `stormwind` ("Stormwind or Bust!": the Elwynn road, grow toward the city;
cargo route and whole-house pilot; staged Cathedral finale (D26)). Only the embedding is Rust; the rest is recipe data.

*What lives where.* `campaign.rs::LEVELS` is the ordered registry,
`&[LevelEntry { id, recipe: include_str!(..), cast: Option<include_str!(..)> }]`. The recipes are
`lions-pride.toml`, `goldshire.toml` (kept under that name for churn; it is `market-day`) and
`stormwind.toml`; Market Day's cast is `goldshire-npcs.toml`, the others have none. Adding a level
means a recipe, an entry, and an `unlock_after`. `design.rs` parses a recipe into the `Design`
resource, which is reparsed from the current level's recipe on every `OnEnter(Ready)` (so R still
reloads a native `WOW_AZEROLL_LEVEL=/abs/recipe.toml` override, now applied to *the current level*).
`Campaign` (resource) holds every level's `[level]` metadata and claim zones, parsed once.

*Recipe schema additions* (everything else in D14 is unchanged; top-level keys first, then tables):

| key | meaning |
|---|---|
| `[level]` | `id` (= the registry's), `title`, `subtitle` (tagline), `briefing`, `objective` (one line), `unlock_after` (level id, optional), `music` (opening track name from `music.rs`, optional), `cast` (informational) |
| `start_heading` | the ball's tank heading at the start, radians (0 faces WoW north) |
| `bounds = { centre?, radius }` | the soft leash; centre defaults to `start`, radius to 680 |
| `claim = [{ centre, radius, to? }]` | doodad claim zones: a disc, or a capsule when `to` is given (flat distance; Stormwind uses one, the city will need more). Default: one 650 disc at `start` |
| `[goal]` | `kind = "collect"` (`target` = a herd species, which becomes `Kind::Boss`) or `"size"` (`diameter`); `area` = the area the goal lies in (the autopilot's primary route ends there); optional `title` (win headline) and `fail` (results line) |
| `[[tiers]]` | `name`, `at` (default diameter), `reps = ["species:Chicken", "adults"]` (the tier opens when every representative is edible). The first row is the start (`at = 0`); `TierTable` is derived per recipe, with `Rep` parsed from the strings |
| `[[challenges]]` | `id` (stable, stored in saves), `title`, `kind`: `collect-all` (`species`, or `model`/`name` of a scene prop), `time` (`secs`), `scene` (every prop of the named scene; `radius`), `unrammed`, `size` (`diameter`) |
| `[[barriers]]` | `from`, `to` (WoW points), `height` 6, `thickness` 2, `extend` 0.5 (jamb overhang) |
| `camp`, `patrol`, `patrol_at`, `patrol_until` | now optional: without a `camp` there is no Hogger staging (no camp loop, no early pass) |

*Global, not per level:* `COLLECT_RATIO`, `DISLODGE_RATIO`, `FILL_FACTOR`, `ADULT_MIN_AXIS`, the
growth law, the finale. The ball's core radius is `0.85 × start_radius` (it was a constant), so a
0.13 yd start gets a 0.11 yd core; `BallState::start_radius` carries the level's value to the
ball, the shell, the camera's pitch ramp, shedding's floor and the turn rate.

*Claims.* `props::install_claims` takes the **union** of every registered level's claim zones,
once at startup (the streamer latches a placement's claim when its tile loads and cannot
re-evaluate), so switching levels never re-claims. Tombstones still reset on each Ready. The cost
of the union is that Goldshire's 650 yd of claimed doodads exist whichever level is played; for a
level that cannot leave the inn they are unreachable and free.

*Barriers* are invisible, mode-owned solids that block at **every** ball size and cannot be rolled
up or knocked loose. A barrier is a hidden entity with `Collectible { pickup: 1e6, solid: 1, half
extents of the box }`, `Gate` and `BarrierWall`, so it rides the existing oriented-box path
(`collect::solid_push`): `contact(radius, 1e6)` is `Wall` for any radius, so the roll-up and
dislodge branches never fire (unlike an authored `solid = true` Gate prop, which is a normal
collectible and stops blocking once the ball outgrows it: those are not containment). Because Unstuck's
`room()` and the autopilot's blockers use the same solids, remembered safe positions never lie in a
barrier and the autopilot steers round them. The box is 2 yd thick (a dash at 0.05 s per frame
never crosses a box expanded by the ball's radius: unit-tested to r = 4), starts a yard under the
lower endpoint (floor overlap) and runs a half yard past each end (jambs). Lions-pride closes the
inn's two vestibule portals (`survey/inn-portals.json` portals 0 and 1) and also leashes the ball
to a 17 yd circle as the backstop for any opening that was missed.

*Phase flow.* `Phase::Select` is the boot phase:

```
Select --Enter/F--> Ready (CurrentLevel set, recipe reparsed, world loads, ball at start)
Ready --first input--> Playing <--Esc--> Paused --L--> Select
Playing --goal or time--> Results --Enter--> Select | --R--> Ready | --F (won)--> Playing (Free Roll)
Paused/Playing --R--> Ready
```

`WOW_AZEROLL_LEVEL_ID=<id>` skips the hub and boots into that level's Ready (ignoring locks: it is
a tooling switch); a bare `WOW_AZEROLL_AUTO` plays `market-day`, so existing tooling and the pacing
numbers stay valid. Entering the hub despawns the last run's entities, resets the run and ball (at
`campaign::HUB_VIEW`, the inn yard, which is always loaded) and pauses nothing; the camera takes
the results screen's slow orbit. The terrain streams around that point as usual.

*Records and unlocks.* `Progress` keys everything by level id (`levels: BTreeMap<id, LevelBest>`).
A level is unlocked when it has no `unlock_after`, or its gate level has `wins > 0`, **or it has
wins of its own** (a pre-campaign save that beat Hogger keeps Market Day). The save is
backward compatible: a `goldshire` key is read as `market-day` and written back under the new name.
A win that opens a level shows "New level unlocked: <title>" on the results screen. Free Roll is
offered from the hub (`F`) for any level with a win; it sets `SelectState::free_pending`, and
`level::keys` starts it when the chosen level's Ready has loaded.

*The hub* (`select.rs`) is functional, not pretty: a full-screen panel of one card per level (title,
tagline, locked or won state, objective, briefing, records), W/S or the arrows to move, Enter to
play, F for Free Roll. The state is `campaign::SelectState`; `select::view` turns the registry,
records and state into plain `CardView`s (unit-tested); only `select::build` and `select::refresh`
touch bevy_ui, so a nicer presentation replaces those two and nothing else.

*The win flow* is generalised. `level::goal_met` evaluates both kinds (a rolled-up boss; a diameter)
in `level::tick`; the finale (slow motion, camera pull) plays for both, and its headline, the
results title and line, the pause goal text, the dial's ring and the intro text all read the
recipe (`Design::goal_diameter`, `win_title`, `fail_line`). The Hogger-only guidance (the "ready"
moment, the edge arrow, the boss bubble colour) needs a `Kind::Boss`, which only a `collect` goal
creates. The autopilot's telemetry words follow the goal ("hogger edible" for Hogger, "goal
edible" for a size) and its tier summary reads the recipe.

*Music:* a level's `music` names the track it opens on (else the rotation continues as before);
the hub plays the title theme. *Autopilot routes:* the primary route ends at the goal's `area` (or the
camp); a size goal with no area walks the tour.

*Indoor starts.* A recipe with `start_indoors = true` (lions-pride) holds the authored start height
in Ready until the building's floor has loaded (`BallState::floor_ready`; `level::begin` and the
autopilot wait on it). The support rays also hit the terrain under an inn, and the WMO streams in
after the terrain, so without the gate the ball settled on the terrain beneath the floor on the first
frame and could never reach the floor again (its support rays start a step above its bottom).

*Inn floor survey correction.* The original foundation interpreted the common-room height
as missing collision. Raw group 5 has coplanar render (`MOPY 0x24`) and walking (`0x08`)
triangles at **z=56.3391**; group 1's landing is **z=56.95–56.97**. The 0.624 yd difference
is a raised landing, not a hole. The engine's 1.12 reject-DETAIL rule is correct. The L1
content pass starts on the lower boards and removes the false south "pit" barrier and
6.5 yd leash. `furniture::tests::inn_common_room_and_landing_have_matching_visible_and_walk_floors`
pins both heights against installed data; see "L1 — Lion's Pride" below for current play evidence.

*Evidence (2026-11-02, native play build, logs in `/tmp/az-found/`, ephemeral).* Market Day
autopilot (`WOW_AZEROLL_AUTO=620`, no level id): tiers at 53.7 / 96.1 / 138.2 / 161.2 / 236.5 s,
Hogger edible 236.5 s, **rolled up 241.8 s**, r = 5.76, 706 things, pass begins t = 17 s and rams
the ball; results show "Hogger, rolled up!", the "speed" challenge, and "New level unlocked: Stormwind
or Bust!". Lions-pride boots at z = 57.1 on the floor with both doorways and the south edge logged
as barriers (`azeroll barrier N`), and the ball stays inside for 240 s. Stormwind boots on the road,
grows from 0.6 to 0.66 yd and walks the trail. The hub lists all three levels with their lock states.

*Open for the content tracks:* L1 and L3 are placeholders (their item sizes were set by eye and
run-checked once; see `lions-pride.toml`); the Cathedral of Light finale; the hub's art and a mouse
interface; barriers have no debug draw (they are logged at spawn as `azeroll barrier N`); L3's
corridor capsules; per-level NPC casts beyond Market Day's.

Environment: `WOW_AZEROLL_LEVEL_ID=<id>` (boot into a level), `WOW_AZEROLL_LEVEL=/abs/recipe.toml`
(replaces the current level's recipe on Ready / R), `WOW_AZEROLL_AUTO=<s>` (defaults the level to
`market-day`). Tests: `cargo test --profile play -p benilla-app --lib azeroll`.

**D24 — The hub is the planet Azeroth and a win is a graded verdict (UI/presentation track; replaces
the functional list of D21).** First built as a brass orrery; after the first playtest ("it is all
on a single planet… you are selecting the level from the single planet") it became the planet.
Write set: `select.rs` + `select/{hub,verdict,maps,mapart}.rs`, `select/art.wgsl`, `hud_models.rs`
(the commissioner), `hud.rs` (the results panel is gone; the capture-aspect UI scale), `progress.rs`
(grades, records), `campaign.rs` (per-level map pin and quip), `design.rs` (`[level]` grade keys),
`web/boot-manifest.json` (the map tiles, below).

- **The maps** (`select/maps.rs`, pure pixel work; `mapart.rs` feeds it): once the archives are open
  (`WorldAssets`, the first frame in the hub, ~260 ms behind the loading screen) the tiles under
  `Interface\WorldMap\{Azeroth,Kalimdor,Elwynn}\<Zone>1..12.blp` are decoded (`blp_to_rgba`) and
  composed. *The globe texture* (2048x1024 equirectangular, `Rgba8Unorm`, repeating in longitude) is
  a painted ocean (slow swells) with the two continents cut out of their parchment sheets: land is
  found by smoothed saturation (the sea is a pale blue-grey wash, land saturated yellow/orange/green
  with brown outlines), the crop's edges feather into the sea, and a soft sea-green shelf rings each
  coast; the lettering ("KALIMDOR", "EASTERN KINGDOMS") and region borders come along. *The Elwynn
  map* is the sheet plus every `WorldMapOverlay.dbc` overlay of the zone (the fully explored map:
  Stormwind, Goldshire, roads, lakes), cropped to the 1002x668 of art. Where things are comes from
  `WorldMapArea.dbc` (`maps::Rect::uv`: `(left - y)/(left - right)`, `(top - x)/(top - bottom)`):
  Goldshire on the Eastern Kingdoms sheet gives the globe's aim (longitude/latitude), each level's
  `LevelEntry::pin` (WoW x, y: the inn; the middle of the fair's ground; the Cathedral) its pin on
  the Elwynn sheet. All 54 tiles are in `web/boot-manifest.json` so the page's boot prefetch warms
  the cache for the synchronous reads. Without data `MapArt` stays empty and the shader paints a
  plain sea and parchment. `AZEROLL_DUMP_MAPS=<dir> WOW_DATA=...` with the `dump_the_real_maps_for_a_look`
  test writes the textures as PNGs.
- **The hub** (`select/hub.rs` + `art.wgsl`, one `UiMaterial`, `ArtMaterial`; kinds: scene, plaque,
  scale ring, candle, medal, portrait frame, inset). The scene is the night sky and the planet, a
  ray-cast sphere sampling the globe texture (4 taps), lit from the upper left, with a glint on the
  sea, drifting clouds (sums of sines: no seam, no blocks) and an atmosphere rim and halo. It spins
  in from 1.25 turns away on arrival and settles, aimed 0.3 rad west of Goldshire so Kalimdor shows
  on the limb, with a slow sway. A gold **beacon** with a dashed lens ring and a pulse sits over
  Elwynn, and a translucent dashed **beam** fans from it to the **Elwynn inset** beside the planet:
  the map under a brass rim, zoomed 1.4x (it was 1.55, which cut Stormwind's pin off the top when Goldshire was chosen), with a **pin** per level (a map pin with the level's
  numeral; shut ones grey with a padlock; open ones gold with a pulsing ring; won ones green with a
  gold star badge on the shoulder, not a pennant, which read as a second "I" beside the numeral, and, if
  the best time beat par, a spark). Pins that stand within `hub::PIN_SEP` (36 x 42 UI units) of each
  other on the inset (the Lion's Pride and Market Day are both at Goldshire) are fanned apart by
  `hub::fan_out` and each joined to its map point by a leader line and ring (the shader reads the
  offset in `lv[i*3+1].xy`); the chosen pin's name goes where `hub::name_slot` finds room (under,
  over, then beside the pin) so it never covers a neighbour. The chosen pin grows, glows and bounces and its
  name shows under it; the inset's window eases toward it (`inset_target`: toward the chosen pin, but
  every pin, with room for its head, stays in view) and pops when the choice changes. Won levels
  light **named stars** in the sky around the planet (an arc over its top; the star's name is the
  best find, `progress::star_title`), joined by a dashed thread. Tags under the planet (numeral,
  title, state, grade), the commissioner's window, the speech bubble and the mission card are as
  before (below). `layout(aspect, n)` places everything in 768-tall UI units: wide (aspect >= 1.4) is
  planet and inset left, commissioner and card right; compact stacks (planet with the inset beside
  it, tags, portrait + bubble + a trimmed card: no tagline, no challenge list, no help line). Empty
  lines take no room.
- **The speech bubble** types the level's one-line **quip** (`LevelEntry::quip` in `campaign.rs`,
  e.g. "A mouse ate my fuses! Be the mouse. Be BETTER than the mouse!", at least 48 chars/s but
  never over 2.2 s), then fades in the recipe's `briefing` under it in small type.
- **Mission card:** title, tagline, status, the objective as a **scale ring** (start ball inside the
  goal ring, log-compressed) and the time limit as a **burning candle**, the records (best time,
  biggest, most rolled up, wins), the best verdict (+ comet), the challenges with titles (met/total),
  the refusal note and two buttons ("Free Roll: no candle" once the level is won).
- **Input.** W/A/Up/Left and S/D/Down/Right move (wrapping), Enter/Space roll, F Free Roll (only
  once won). The pointer: hovering a tag or a pin on the inset (when the mouse moves) chooses it,
  clicking the chosen one rolls it, the card's buttons roll and free-roll (`SelectState::request`,
  acted on by `select::keys`). Locked and not-yet-won refusals show a note.
- **News.** `HubMemory` remembers each level's wins and openings at the last visit; on arrival
  `Celebration` lists what is new. A first win's star flies up from the beacon over 2.2 s (after
  0.7 s); a newly open level's pin shakes, its padlock hops off and a spark burst rings it (after
  2.8 s, with the quest-complete chime) and, if the player has not moved the highlight, the choice
  moves to it and the commissioner cheers. Nothing plays at boot, nothing while the loading screen
  covers the hub.
- **Commissioner.** "Grand Tinker Fizzwick" (invented; the client DBC has no names, so no
  Mekkatorque display id can be looked up): a male-gnome NPC display with a helm and chest piece
  (`commissioner_display`, the second helmed id; `WOW_AZEROLL_COMMISSIONER=<display id>`
  overrides), on his own stage (layer 42, (800, 3000, 0), 288x384 target, scale 1) built exactly
  like the corner Prince (D20a: `Carried`, `StageRig`, no `RunEntity`/`WorldUnit`), spawned only
  in Select and Results. He gestures: TalkExclamation then Talk every 1.7 s while a line types,
  cheers on news, and reacts to a verdict (Preposterous laughs). `Commissioner` (resource) is the
  hub's handle on him.
- **Verdict.** `progress::judge_win` (a win only): the size grade is `ball diameter / goal diameter`
  against `[level] grades` (default 1.2 / 1.5 / 2.0 for Respectable / Splendid / Preposterous);
  the pace grade is the share of the time limit used against `[level] pace` (default 0.8 / 0.6 /
  0.4); the grade is the better of the two. **Why both:** a run ends on its goal (D21), so the size
  multiple is only above 1 for overshoot (a `collect` goal that needed less than the ball had);
  pace is what separates wins of a size goal. Under `[level] par` (seconds; default half the
  limit) a win earns the **comet**. The ball's own size is judged, never `Run::final_diameter` (a
  595 yd terminal trophy must not make every win Preposterous). Records: `LevelBest` gains
  `grade` (best, as `Grade` index), `find` and `find_size` (the best-finding win's biggest thing);
  all `#[serde(default)]`, so old saves read. `Outcome` gains the verdict, `new_grade`, the find
  and `new_star`.
- **Results** (`select/verdict.rs`; `hud.rs` no longer has a results panel). The title, a rosette
  **medal** stamped in (rays, the grade's stars, the comet streak), the grade's name, "N x the
  request, P% of the candle left", the comet line, the commissioner's remark; the tally (time,
  size, rolled up with NEW BEST/best-to-beat), the best find and its star, the three most rolled
  up, the challenges, the pulsing **New level unlocked** line and three buttons (Enter back to the
  hub, R, F; the keys are `level::keys`'s). All beats land within 0.75 s (the autopilot shoots
  the results at 0.5 s). A fading line fades its `TextShadow` too (a faded line otherwise leaves a
  black ghost). A loss shows "NOT QUITE" and the retry.
- **Uniform and shader must agree.** `Data` (`kt`, `p`, `lv[24]`: 3 rows per level up to 6 levels,
  then the globe's aim, the beam's anchors and the inset's size) is 2 + 24 `vec4`s = 416 bytes and
  `art.wgsl`'s `array<vec4<f32>, 24>` must match (`GLOBE = 18` in both). A binary built before a
  layout change and run beside the new shader panics in pipeline creation ("buffer size 416 >
  min_binding_size 320"): rebuild, don't mix.
- **Music** is unchanged: Select, Ready and Results already play the title theme (`music.rs`).
- **Captures** (env): `WOW_AZEROLL_HUB_SHOT=<png>` (+ `_AT=<secs>`; `WOW_AZEROLL_HUB_KEYS=` w/a/s/d
  to move, `e` to roll, `f` to free roll), `WOW_AZEROLL_HUB_CELEBRATE=<level id>` (play that level's
  star and the unlock on arrival), `WOW_AZEROLL_RESULT_SHOT=<png>` (+ `_AT`),
  `WOW_AZEROLL_RESULT_DEMO=<secs>` (+ `_LOSS=1`: from the hub, fake a finished run and open the
  results), `WOW_AZEROLL_LOOP_SHOT=<png>` with `HUB_KEYS=e` (the whole loop: the hub rolls the level
  for real, four seconds into it a win is faked, the results show, Enter is pressed, the hub's
  news plays and is shot), and `WOW_AZEROLL_UI_ASPECT=1.78` (lay the screens out for that aspect at
  the matching UI scale, in the top left of a window the compositor will not make wide). Seed
  saves with `BENILLA_HOME=<dir>`.
- **Tests.** The view model (locks, star, grade, comet, records), news, navigation and decisions,
  the layout (every piece on screen and apart at 8 aspects; the planet clear of the inset), the
  star arc, the inset window (every pin always in view, the choice nudging it), the globe's
  spin-in, the typewriter, the map composition (tile placement, overlays, alpha, the projection,
  land and sea, the atlas), pins on the sheet, the grade bounds, the verdict's two measures, star
  naming, and the save round trip.
- **Open.** No Prince standing on the planet (the stage gnome's camera only runs in play; stars and
  a gnome on the same arc need a layout of their own); the briefings are paragraphs (small type
  under the quip); one globe texture is built per run (260 ms), not cached to disk.

**D25 — Whole-placement retirement and reduced building trophies.** Optional generic
`RetireWmo { map, uid }` requests run at the head of the Stream chain, before pending
colliders finish. A suppressed MODF retains its identity and tile reference count, but
owns no engine entities, pending props, weld/merge accumulators, or portal instance.
The portal instance's removal reaps retained WMO and prop regions before extraction;
interior residency explicitly invalidates even when another instance shares the asset.
Suppression tombstones survive last-reference release/re-registration. `RestoreWmos`
clears them and queues resident placements for respawn on Ready. Snapshot publication
is default-off in the engine. No consumer means normal engine placement behavior.

`buildings.rs` opts in by recipe `buildings = [MODF uid, ...]`. It uses the full render
vertex box in model space and nearest OBB-face contact, with D4 eligibility and no
Dislodge path. Before requesting retirement it prepares an exterior-group-only,
entity-hosted trophy with no physics, portals, liquids, lights, or MODDs. It refuses
pickup over missing terrain or when the terrain support differs by more than
`2 + 0.5r` yd; accepted pickups settle the ball onto that validated terrain support. Only an accepted retirement acknowledgement adds full box-volume credit and
reveals the reduced (28%) visual under BallSpin, preserving placement orientation.
The two building trophies have no ordinary `Collectible` component: they cannot be
shed/recollected for reduced-model credit. Ready removes their RunEntity roots.
This is a pilot seam; retained on/off, streaming reload, and browser acceptance gates
must be verified before widening the allowlist to complex buildings.

### L3 — Stormwind or Bust!

**Character pass and polish pass (2026-11-03).** The level is seven acts, designed in
[katamari-room-design.md §7](katamari-room-design.md) and generated by
[data/stormwind-gen.py](data/stormwind-gen.py) (recipe `stormwind.toml`, cast `stormwind-npcs.toml`, floors
`data/stormwind-floors.json`). The D27 engine, piece manifest, claim list, navigation capsules and the finale
flight are unchanged; what changed is the content, the route and the finale's physical size.

*Structure.* Start r 1.25 in the lumber camp north of Goldshire (the woodpile nursery, the sawyers' lunch,
the lumber wagon and the first signpost), the harvest road (hay and stooks in a double line, Hobbs's tipped
cart, the scarecrow), the hill road carrying the saplings the Royal Sapling Convoy dropped, the quartermaster's
dump, the convoy halted on the meadow before the Valley (four wagons, the escort in two lines, loose horses),
the toll gate, the Valley of Heroes (ten guards, a line of hay and barrels to the 13 yd gate, a brave captain
and six cast guards in a line across it), the Trade District on market day (three stalls and two awnings as
solid promises, the Auction House's lots in size order, the bank's gold spilled down its steps, Grand-Uncle
Thrum haggling, a brewer's wagon parked across the 5 yd dead-end street, the porters' drops round the
Auction House and up the south-west ramp), the canals (the towpath's line, the fishmongers, the map's lions,
drops over the crossing to the 7 yd Cathedral bridge), and Cathedral Square dressed for the Festival of the
Light: offerings between the planter rows, the tithe heaped at the stair foot in ever bigger piles, the
Guard's cordon and salute battery, the Cathedral's new statuary of the heroes of the Second War delivered in
every size the masons' yard had, the orphans' lantern ring, the bishop's procession; then the hunt: the
Square's houses, the oversize statuary on the canal plazas and in the Trade District (D 115–123), and the
Cathedral from the Square's east at D 128 with 28 pieces. No trees are authored in the city (the user's note:
"the random trees through the city are kinda strange"); the Square's own planter trees remain. 32 areas in one
chain, 31 trails (five carry the road's drops), 77 scenes and about 1040 props, 20 herds, 39 cast NPCs.

*Wayfinding (G5).* An unbroken line of food on every road and towpath; a Stormwind signpost (`blocker`) at
the camp gate, the hay line's end, the hill crest, the quartermaster's dump and the toll gate; the convoy,
the gate committee and the crowds as beacons; the brewer's wagon closing the dead-end street; the porters'
line round the Auction House and up the ramp (each drop on its measured floor); the offerings line between
the Square's planters. No invisible walls.

*Budget.* Everything before the Square is capped: a probe with the city agent's canal fixture crosses both
canal bridges at r 4.0 (0 stuck, 0 Unstuck) and wedges on the 7 yd Cathedral bridge at r 4.5; the street
north of the Auction House (5 yd between its buttress and the city's planter at raw (−179, 4)) stops a r 3.5
ball, so the route leaves the plaza by its 20 yd south-west ramp and the street is closed by a parked wagon.
Authored food before the Square is 45 r³ (r 3.6 if all of it is eaten; about 3.9 with the city's own barrels
and hay the autopilot takes); a human who also eats every claimed barrel in the Trade District could reach
r 4.5 and fail the bridge, a documented hazard of the funnel. The Square's ladder is built by `front()`:
each row's count opens the next step with a 1.08 radius reserve, over row slots read off the placement
validator's collision grid (`WOW_AZEROLL_PLACEMENT_GRID`, parsed into the city frame by the pass's
`gridmap.py`: the lane south of the east strip's raised planter, the nursery and the open middle south of it,
the south field, the south flat either side of its kerb, the north flat and field, the east field and the
planters' north lane; never the stair or the apron), each row inset by its piece's quarter-box; the fronts
run pumpkins and the tithe's gold (D 6–37), the Guard's cordon of lions and gold (D 38–55), the new statues
(D 56–67) and the tithe's Khadgars and house-sized gold (D 67–76) with the festival fountain on its own in
the east field, and stop there because the Square's ground is used up (trees gave 26,000 r³ each; a gold pile 44, a lion 136, Khadgar 741). The finale is D 128 (r³ 262,000)
instead of 160, carried by the pieces at half credit (the bank excluded), the Square's corner giants
(D 71–118), the canal plazas' statuary (D 107–112) and the Trade District's (D 115–123): ×1.19 at the
thinnest band (D 110) and ×1.38 at 128. The generator's ledger asserts ≥ 1.15× at every one of its gates and that the authored food before the
Square stays under 46 r³.

*Placement rules learned.* A city scene prop spawns only if the WMO floor ray from its anchor (+4, down 8)
hits; the Square's terrace wings (z 6.0), the orphanage's block, the Cathedral apron (25 × 23 yd, z 4.6, with
a hole at x −433..−437), a pit by the bank at raw (−105, −30), a monument at (−118, −24) and holes on the
plaza's north-east all swallowed props until the installed-data test
`installed_stormwind_city_scenes_can_spawn_every_authored_prop` (every `indoors` scene, against the whole
city's collision) listed them; breadcrumbs on ramps are one scene per drop. A herd animal spawns only if its
floor (or, failing a WMO floor, the terrain) is within 0.7 yd of the authored z. Solid props wedge a
street-sized ball when two of them, or one and a wall or fence, leave a gap narrower than the ball (run 1 sat
on the auction lots against the Auction House wall, run 2 balanced on a 0.8 yd podium top, run 3 wedged
between two stalls and an awning, run 6 in the pocket between a stall and the plaza's south wall); every
solid on the plaza is now 14 yd clear of any other, off the gate-to-canal line and never between a wall and
open ground, food stands a ball's radius plus two yards off a wall, the convoy halts on the open meadow, the
harvest road has no solids near the farm, and the hill road claims nothing so farmhouse furniture is never
bait. A row that is still a wall must never stand between the ball and its food (the first Square probe:
6 Unstucks among rooted rows and the Square's stone planters); the Square is dense fronts eaten from the
small end along the ball's path, with no waypoint on the stair or the terrace (a ball sent up the stair
wedged at its foot in runs 7 and 9 of the first pass, a r 15 ball wedged on it in probe 3).

*Runs (second pass, normal start, autopilot, the embedded recipe unless noted).*

| run | recipe / binary | win | stuck | Unstuck | note |
|---|---|---:|---:|---:|---|
| Square probe 7 | fixture start on the canal east bank at r 3.6 | 2:00 | 1 | 0 | the city-content ladder: fair gate at 26 s, finale at 118 s; one escape at the fair gate's planter tree at r 12 |
| 8a | build 8 (festival ladder, toll-gate signpost in the wagon pocket) | 8:50 | 9 | 0 | 30 s in the pocket between convoy wagons 3–4 and the signpost; 60 s plateau from the bridge foot to the inner street; 10 s pinned on the pumpkin cart |
| 10a | build 10 (signpost moved, ramp line off the planter, cart off the road line, sheep off the dock) | 9:12 | 10 | 0 | 76 s plateau through the gate (waypoint 16 yd before the sill, flanking bays); 4 s on a Valley plinth |
| 10b | build 10 | 7:58 | 10 | 0 | the detector logged one 12 yd "jump" after the win, on the Cathedral-attach frame hitch with the clock frozen; no unstuck.rs event |
| 12a | build 12 (gate waypoint on the sill, hay in the passage, guns across the stair foot) | time up at D 24 | 84 | 18 | the guns reached into the nursery rows and pinned a D 24 ball; the first Unstuck threw it 464 yd to the Valley. Gate crossed in 20 s |
| 13a | build 13 = the shipped recipe (guns removed) | 12:27 | 31 | 0 | a stuck-escape at the gate court's south pillar drove the ball down the moat stair at raw (−6, −45); 4½ minutes in the moat and up its grass bank; the rest of the run as 10b |
| 13b | build 13 | 8:54 | 14 | 0 | 72 s plateau at Square south (r 12) between the fair gate's planter trees (Elwynn Canopy Tree, edible at D 26) before the festival ground opened; gate crossed without a stall |
| 15a | build 15 = the shipped recipe (placement pass: the Square's rows on the validator's grid, the plaza's market, lots and bank on flat cells, the convoy on the meadow's clear line) | 7:59 | 10 | 0 | no plateau over 32 s; the Square eaten in 30 s from the fair gate |
| 15b | build 15 | 11:18 | 7 | 0 | 186 s plateau at D 106 riding the Square's wing roofs (raw (−424, −50) then (−431, 61), 20–30 yd above the paving) before leaving for the canal statuary (D 107+) |

The dock of the farm at (−9313, 287) is claimed by the Goldshire levels' 650 yd disc (the claims are a
campaign-wide union), so its jars are bait and its dock a pocket: the autopilot loses 20–40 s there in every
run, without an Unstuck; a human has no reason to go. The gate passage was empty and its waypoint sat 16 yd
before the sill, so runs 8a and 10a wandered the gatehouse's flanking bays for 30–76 s; the waypoint is on the
sill (raw x 24) and a hay line runs the passage, and run 12a crossed from the toll gate to the inner street in
20 s. A line of the Guard's big guns across the stair foot (run 12a) was the one blocker experiment that
failed: lying along the Square's axis they reached into the nursery rows, pinned a D 24 ball between the guns
and the rooted lions, and the game's Unstuck threw it 464 yd back to the Valley; the stair is left open and
off the route.

*Placement validator.* The L1/L2 placement pass's validator (`placement.rs`, `WOW_AZEROLL_PLACEMENT`) listed 279
failures on the first polish recipe; its collision grid (`WOW_AZEROLL_PLACEMENT_GRID`) was read into the city
frame and used to re-lay the Square's row slots and giants, the plaza's stalls, lots and bank, the convoy and
the gate scenes, and the shipped recipe has 152: 71 are the Square's heaps and statues straddling the
paving's 0.3 yd kerbs (rooted piles 7–29 yd wide on a terraced square; eaten from above, never a wedge),
10 the Trade District's oversize statuary (D 115–123) cutting the plaza railings, 11 on the canal walks
and bridge landings, and the rest single props on the plaza's steps; the report is in
`/home/jjd/az-patches/l3p/placement-validator-after.txt`.

The previous outside-gate D64 route, the city agent's single-ladder recipe (wins at 5:38–5:52) and the first
character pass's tree-fair recipe (run 8: 6:48 clean; run 9: 9:10 with four Unstucks) are historical; see D27
for the engine acceptance they validated.

## L1 — Lion's Pride

**Redesigned 2026-11-03 after the first playtest** ("lacking character … sprawled out everywhere",
"unable to get to … the fireplace … no ramps", "more of the rooms should have items", "no invisible
walls INSIDE the inn"). The design grammar and the room-by-room plan are in
[katamari-room-design.md](katamari-room-design.md) (sections 2 and 5); this section keeps the facts a
maintainer needs and the acceptance evidence.

`lions-pride.toml` is **generated** from a room plan (positions are computed from the inn's WMO
geometry; the generator is `docs/azeroll/data/lions-pride-gen.py`, documented in katamari-room-design.md §5 and the recipe's own
comments). Start r=0.2 beside the captain's table in the sunken common room, five-minute clock,
2.4 yd size goal, par 150 s. Cast `lions-pride-npcs.toml`: Farley polishing behind the bar, Remy asleep
under it, Brog at the kitchen counter, Dobbins asleep at the card table, William upstairs.

**Geometry that drives the design** (WoW yards; from `goldshireinn.wmo` MODF 71414, yaw 83°, and the
probe survey). The common room is a **0.62 yd pit** (boards 56.339; everything around it 56.95):
x −9458.5..−9479, y 19.5..28 (to 30 west of x −9470). It has three step-rims of two 0.31 risers (west:
x −9464.5..−9469.5 at y 28.5; east: x −9466..−9470.5 at y 19; south-east: x −9474..−9479 at y 19.5), and
the main staircase (0.25 risers, 0.5 treads: lower flight x −9476..−9479, y 22 → 16; landing 60.2;
upper flight x −9470..−9474, y 13 → 20; upstairs 63.82). The fireplace is in the pit's **north** wall:
firebox floor 57.0 at x −9458.5..−9456, y 20..24.5, 57.5 side stones, 58.2 flanking counters
(the survey: the 58.2 "counter" along y 18..18.75 is a 0.75 yd rail between the pit and the bar floor, and the counter east of the chimney, 58.05 at x −9454..−9456.5, y 15..18, has a panel front no ball can top; the hearthstone beside the firebox, 57.5 at x −9458.25..−9459.25, y 18.5..20.75, is the reachable ledge), chimney top 61.3. The "61 yd block" the probes show over
the pit is the hanging chandelier at (−9471, 25). Bar top 58.13 at x −9458.8..−9465.7, y 10.4..12.4 (placement survey 2026-11-03; the first plan's y 11.5..13.5 put the book stair 1.7 yd short). Kitchen
through portal 9 (2.63 × 3.77): prep counter 58.3 at x −9462.5..−9465.5, y 0.5..−3 (bigger than the
1 yd probe grid suggests: props placed over it snap onto it; the 0.25 yd survey: x −9462.5..−9465.3, y 1.5..−2.25,
its top mostly held by claimed meat and bowls), oven block 60.5 at x −9467..−9471, y −5.25..−6.25 (survey), a
hole to the cellar at x −9463..−9466.5, y −7.75..−9, stove platform 57.2–57.4 west and south of it. Cellar stairwell x −9459.5..−9463,
y 2.4 → −7.7 (3.4 wide), cellar floor 49.80, room x −9459..−9480, y −13..9. Upstairs doors 2.67–3.9 wide.

**No barriers.** The recipe has zero `[[barriers]]`. The only containment is scene "The door": a
`blocker = true` `G_Keg` at scale 1.6 in the inner vestibule portal (4.4 × 3.9), inedible at every size
(D29); `bounds` (29 yd) is the emergency backstop. `design.rs` tests assert this.

**Ramps (D29; placement pass 2026-11-03, D35).** Book staircases (`BookSmallOpen02`, 0.12–0.15 rise,
0.3 run) onto the captain's table (claimed D22 table, slab top 57.09; the meal is lifted 0.76 in two rows
between the chairs), into the firebox (on the y 23.5 line: at 23.1 a claimed chair pushed the ball off the
first book), onto the hearthstone (57.5, four books from the firebox floor) and onto the bar top (58.13; its
front edge is y 12.4 on the stair's line, not 13.5); a scale-0.22 `RedRidgeDocksPlank01` at pitch 0.29 up
the SOUTH side of the kitchen prep counter (5.4 yd long at 0.25, never edible here). The first version's
bar stair ended 1.7 yd short of the counter (the playtest's "books don't lead up onto the bar"), its
hearth-counter stair ran along the pit rail (a 0.75 yd strip at 58.2 along y 18, the "beam") toward a
panel-fronted counter no ball can top, and its plank leaned east over a second counter with its foot
over the hole to the cellar. Every ramp sits on one base floor and stops short of the surface it serves,
because each prop snaps to the highest WMO surface under its origin (+4 yd); its last step stands at
least 0.05 over that surface (D35). All five pass the placement validator and a live scripted climb at
r 0.2 (`/home/jjd/az-patches/placement/`).

**Areas.** Common room (goal start), Bar, North hall, Kitchen (enclosed, clearance 2.63), Cellar
(enclosed, 3.4), Upstairs (**enclosed** so its 20 yd circle, which covers the whole inn in plan, only
claims points within 3 yd of 63.82; without that the autopilot believed it was "clearing Upstairs" while
on the ground floor). Five `navigation_only` trails join them; the goal `area` is the Kitchen so the
primary route crosses the bar; `tour` adds the cellar, north hall and upstairs. Herds: 12 rats
(`Creature\\Rat\\Rat.m2`, fit 0.4) among the cellar cheese at (−9467, −6, 49.8), radius 4, and the
hearth cat. With `wmo_floors = true`, a small probe around authored/current feet selects the nearest
walkable floor; terrain above a cellar cannot replace it. Indoor herd wandering/fleeing retains the
spawn storey. No species-specific placement rule is involved. Captures in
`/home/jjd/az-patches/l1fix/rats-diagonal/` show the rats moving below the cellar stairs; placement and
flee telemetry remain at feet 49.79.

**The replay layer's placements (D33, 2026-11-03).** *The parcel in the fireplace* (scene of its own, anchor
the cold firebox 1 yd in from the lip at (−9457.5, 22.3, 57.0), one `XmasGift02` at scale 0.238, so it opens
at D 0.45): small, climb the hearth's book stair while the books are still steps (they are edible from D 0.67
and vanish under a ball that size); big, from D ~2.0 the ball steps the 0.66 yd lip; between the two the
fireplace is out of reach on purpose (the goal is 2.4, so every win can come back for it). It unlocks the
**Monocle** (display 5377, `id = "monocle"`). *Pippa Fizzwick* (`lions-pride-npcs.toml`, `role = "cousin"`,
display 3121, scale 0.45, `catch = [1.33, 2.0]`) counts the rats among the cellar cheese at
(−9466.5, −4.0, 49.8) and circles the Colossal Swiss in her window. The challenges are now `rats` (a
`before`: every cellar rat before the first Cheese Wheel), `pillows` (the upstairs lounge's pillows),
`floor` (Farley's stools) and `big`; `speed` is the comet's job and `hearth` is gone because the parcel lives
there. Generator fixtures: `LP_FIX=parcel|lip|cellar LP_R=<radius> [LP_LIMIT=<secs>] python3
docs/azeroll/data/lions-pride-gen.py` writes a start-moved copy under `/tmp/az-replay/` for `WOW_AZEROLL_LEVEL`.

**Indoor starts and camera (2026-11-03).** Two independent loading stalls were fixed. The report's
expensive per-food route accounting was gated behind `WOW_AZEROLL_REPORT` and capped. The remaining
Ready hang came from seeding the ball centre with authored feet: at r ≥ 0.5 its support ray began
below the WMO floor and `floor_ready` could never become true. Ready now seeds feet + radius before
probing. Both cellar and upstairs fixtures begin Playing promptly; the normal pit start is unchanged.
Ball support and Unstuck ignore terrain above the body when the WMO supplies its actual floor.
The indoor camera likewise does not lift the lens to that overlying terrain; the normal radius-driven
boom and eased lift remain, with the existing cutout clearing intervening walls/ceilings.

**D10 validator output** (`azeroll D10 …` at level load): Kitchen PASS (upper 2.77 < exit 2.84). Cellar lost one giant cheese wheel during authoring; returning the rats to the cheese hoard leaves its
conservative bound a hair over again (3.6771 vs 3.6757). Upstairs FAILS (upper 3.30 > exit 2.89)
because of the claimed bookstacks, pillows and bottles already in the bedrooms, not the authored scenes;
the timed size goal is 2.4 yd and can be called before that door limit. Continued rolling after the
goal and Free Roll can exceed it, so the content warning remains. A FAIL does not block play.
**Autopilot stairs (2026-11-03).** Indoor route graph nodes now include enclosed rooms, and room/trail
arrival checks require the correct floor as well as XZ proximity. Tour takes starter food until r=0.3
(the r=0.2 ball cannot climb the inn risers), visits remote storeys before clearing the main floor, and
walks navigation-only connections without foraging detours. Body-swept A* plans through the WMO stairs with the mover's 0.85r horizontal
clearance; room waypoints allow a nearby clear stance on the authored floor (the kitchen endpoint is
under a counter). Small-ball waypoint tolerance is smaller than one stair riser. A 42 yd leash replaces
the old 29 yd circle, which clipped the cellar stair foot; the visible giant keg still contains the
exit. Installed-data tests traverse cellar↔kitchen and ground↔upstairs on the actual inn mesh.
Ready/new-solid placement and Unstuck teleports resolve initial overlaps against the mode's actual solid
shapes before sweeping movement, logging any correction. Nearest-face pushes handle an embedded
centre; a bounded adjacent-box pass chooses a clear outer face if neighbouring boxes conflict. A
ball already supported on a solid's top remains there. Permanent blocker footprints also mask covered
bait from target scoring, the census and claimed-food inventory. The target's own rotated extents form
the overlap margin, so a tall harness/post protruding above the replacement cannot become fresh bait
with growth. Separate storeys, food above upward walkable faces, and distant food behind a blocker
remain eligible.
Each search is bounded by nodes and a one-second deadline. Census reach estimates use the report's
support-patch flood instead of launching a full A* for every food in one frame. Indoor room scans use
the authored circle and exclude food whose box bottom is below the room floor; a raised hall must not
lure the ball back into the pit. Escape invalidates cached room paths, and an unreachable area approach
has a bounded timeout rather than holding the route indefinitely. Live item routes still
use collision-aware A*.

**Acceptance (2026-11-03).** Normal shipped recipe, cellar rats and 42 yd bounds: primary won at
65.6 s with zero AI stalls/escapes and zero Unstuck; tour won at 217.3 s with three AI escapes and zero
Unstuck. Tour sampled the upstairs floor at 28.1–40.0 s and the cellar at 86.5–132.0 s, completing both
stair round trips. `WOW_AZEROLL_CALL_AT=0` calls the finish action once the size goal is eligible.
Cellar/upstairs r=.5 fixtures begin play; rat spawn/flee telemetry stays at feet 49.79. Logs, fixtures
and selected captures: `/home/jjd/az-patches/l1fix/README.md`. The final covered-bait footprint refinement
follows these L1 runs; its unit cases include tall protruding models, extent margins, other storeys
and walkable tops. Final native, wasm and installed-data suite pass (216 tests, including the city
track's new gate execution regression).

**Pacing law for authors.** An item rolls up once the ball's **diameter** reaches **2.15 × its
longest scaled extent** (`level.rs`: needed radius = extent / `COLLECT_RATIO`, 2/2.15), and adds
0.119 × (box volume x·y·z) to r³ (`growth`, `FILL_FACTOR` 0.5). So the claimed inn chairs (1.33) and
kegs (1.35–1.48) are furniture below D 2.9–3.2, and the 2.4 yd goal is reached on barrels (0.99),
bottles, bowls, steins, bookstacks, sacks, tiny tables and loose scaled stools. The goal is r³ 1.73:
twelve barrel-sized things (≈ 0.1 r³ each) or four scaled crates. Items worth eating carry about
0.0003 r³ at the start size and the autopilot picks up roughly one thing a second, so each band must
offer the next band within a few yards; the generator's `at = D` sizes every item to the diameter it
should open at (the first draft used 1.075 × extent and starved the ball at D 0.39).

**Evidence.** Run 1 (tour, first version, pre-D29-planner binary): the pit and table vignettes worked
(book stairs climbed at r 0.2–0.27), but the pilot wedged at the pit's north-east corner between the
claimed kegs and the hearth stairs for 80 s, then sat on the kitchen counter; D 0.54 at 300 s, 18 stuck.
Fixes: hearth stairs moved west of the kegs, no crumbs in the firebox, Upstairs enclosed, a mid-band
"empties" vignette inside the pit, the kitchen plank. Run 2 (tour, binary with the D29 planner band but
before the full-3D waypoint fix): the pilot wedged under a claimed chair chasing coins placed under the
table, wrote the nursery off (its 14 yd failure disc) at D 0.33 and wandered to the kitchen, where nothing
was edible below D 0.39; 21 pickups, 27 stuck, no growth after 31 s. Lessons applied: the start is 2 yd
clear of the book stairs, no bait under furniture, and **every room carries a tiny-food band** (cheese and
bread at scale ~3, edible from D 0.26) so a small ball that strays, or a pilot that has written a room
off, can always recover. Run 3 (same binary generation) still starved at D 0.39, which exposed the real
cause: the recipe had been sized for roll-up at 1.075 × extent when the rule is 2.15 × extent, so most
"food" was furniture. The generator was rewritten with `at = D` sizing and the start raised to r 0.2.

**Run 4 (2026-11-03, tour route, binary 21:15 with D29 + the full-3D waypoint fix, recipe FNV
`9a82eda3163dd431`): WIN at 66.8 s** (22 % of the 300 s clock), 282 pickups, tiers at 5.0 / 12.4 / 26.8 /
38.2 / 49.5 / 65.0 s, **2 stuck events, 0 game teleports**, longest pickup wait 4 s, longest plateau
8 s. The ball never left the pit and the bar: the common room alone holds ≈ 3.8 r³ of food below the
goal against the 1.73 needed, so the kitchen, cellar, hall and upstairs are the expert content (the
3 yd "Preposterous" challenge and the size grades). The goal stays at 2.4 because the kitchen door is
2.63 yd: any larger goal lets a ball grow past the door inside the kitchen (the pumpkin and cauldron
alone add 2 r³). Evidence: `/home/jjd/az-patches/l1r/r4-*`.

**Run 5 (primary route, same binary; speed challenge 120 s, par 130; recipe FNV `411f5a81000556d3`): WIN
at 97.1 s** (32 % of the clock), 336 pickups, **3 stuck, 0 teleports**, longest pickup wait 6 s, longest
plateau 12 s. Two wins by two routes in 67–97 s put a first-time human (2.5–3× slower) at roughly
170–290 s on the 300 s clock. Evidence: `/home/jjd/az-patches/l1r/r5-*`; room captures in the same folder.

**D26 — Terminal assemblies and scale presentation (original outside-gate version; D27 extends this).** `finale.rs` prepares a render-only
Cathedral assembly from UID 10047 before allowing the finale goal to succeed. Manifest v2
hides 95,109,134..151,173..183 but flies only exterior facade/spires group109;
crypt/interior groups are deliberately omitted from the trophy to avoid black structural boxes; structural guards require the installed 306-group city
and the Cathedral interior range. The read-only audit records root SHA-256
`e621547f50bc9cd0c3b7b1f21fbb1d342d5852d3c772742bb64075fe97763ceb`.
Group 112 (`holy01`) contains 22,093 vertices / 18,748 triangles over a roughly
196×184×132 yd box: it remains excluded because the broad shared district geometry
has not been separated into an independently authored Cathedral exterior.
Selected actual render vertices determine the trophy pivot and reduced scale.

`WmoRenderMask` is default-empty, per resident instance, applied after portal flooding;
entity and retained render lanes consume the resulting visibility. During the shot, cutout
publication moves `WmoRevealPoint` from the body to the Cathedral pivot before PVS. This
reveals the surrounding city/square under the flying assembly instead of leaving it in sky;
the selected-group mask still wins after the reveal. Camera override precedes CameraPoseSet
and cutout publication, which precedes WmoPvsSet. It never changes model
assets, city collision, portal topology or retirement tombstones. During the 3.2 s real-time
finale, the camera follows the lifting assembly along an arc toward the stationary ball;
the reduced trophy attaches to BallSpin. Existing Flow slow motion and music duck apply.
`Run.final_diameter` records 595 yd only after attachment and feeds the dial and persistent
size record; the collision radius and volume never jump. Interruption clears the mask;
Results restores the original city while retaining the trophy for the results shot. Ready
removes the run root and resets assembly state; Free Roll hides the terminal trophy and
clears the displayed size. The original r40 cap is superseded by D27's authored city cap. Enter skips to attachment/results; R
and Escape retain restart/pause behavior. Missing/unready city assets cannot win.

The optional generic `ModeFogOverride` changes outdoor fog distances only when supplied by
a mode. Azeroll starts fog beyond the eye-to-ball distance plus two radii at r>=8, clamped
to leave a ramp before the 777 yd far clip. Stock modes retain the original zone atmosphere.
The camera boom caps at 480 yd, keeping diagnostic huge radii inside the engine envelope.
D23 still caps carried geometry at 160 roots. At r>=8, substantial recent cargo (60 s) and
objects of bounding radius >=r/5 redistribute once around a stable spherical surface,
then ride near 0.98r minus a small embedding share, preserving recognizable surface objects
as the shell grows. No decorative credit or moving colliders are added. D27 adds a separately capped visual building skin with shared meshes.



L3b first normal-start acceptance run (pre-pacing stops): win at **284.8 s**, physical
D=64.01 held throughout the terminal shot, final display/record **595 yd**, 890 pickups.
This is a successful finale/route baseline, **not** the requested 8–10 minute pacing pass.
The expanded route splits existing roads through three additional forage stops; its timed
acceptance is tracked in `/home/jjd/az-patches/l3b/`.
Native `WOW_AZEROLL_BUILDING_RESET_TEST=1` requests Ready once after the first accepted
building retirement, then permits an ordinary second pickup of the restored UID; it is an
unattended lifecycle acceptance probe, never enabled in regular/native or browser play.


L3b expanded-road normal-start run: win at **552.9 s / 9:12.9**, physical D≈64.17,
final displayed/recorded 595 yd, 1,073 pickups, median 60 FPS. It exposed a **75 s**
no-pickup interval in the loose final wagon band (215 AI rescue attempts). The final
recipe therefore compacts that band from 120 wagons / 65 yd to 168 / 42 yd and moves
the trigger into the same safe clearing (55 yd radius); this revision needs its own pass.
All 143 Azeroll tests, native play build and WebGPU wasm check pass after the city-reveal
adjustment. Logs and before/after flight/attachment frames: `/home/jjd/az-patches/l3b/`.


D26 final validation (2026-11-02): the compact final band and safe clearing trigger won
a full normal-start run in **587.4 s / 9:47.4**, physical D≈64.15, displayed/recorded
595 yd, 1,157 pickups, median 60 FPS. Longest no-pickup interval fell from 75 s to
28.2 s and AI rescue attempts from 215 to 8; ordinary scenery detours still occur.
UID125746 was retired, Ready was entered, and the restored UID was retired again
with fresh acknowledgements (685.07 credit each, no retained suppression).

The finale also opts the city instance into default-off `WmoExteriorView`: camera
window/fog admission treats this one placement as exterior, so the portal claim
cannot hide outdoor terrain beneath otherwise-visible mode-owned trees. No portal
graph or collision data changes. Ready, Select, Results and interruption clear the
override. City reveal publication only runs for the active Playing finale; pause
returns to the body's reveal point. Final native frames show city, square and terrain
under the lifting facade, without black crypt boxes. Reduced house trophies also
ride the growing shell without changing credit; the surface test covers that lane.

Latest verification: **144 Azeroll tests pass**, native play build and WebGPU wasm
check pass. Three normal-start wins are archived: 284.8 s baseline, 552.9 s expanded
route, 587.4 s final recipe. Free Roll restores the city and the ordinary physical
size; its r=40 cap remains in level flow. Cold browser/runtime and renderer-off/
merge-off/weld-off matrices remain untested; r≈300 is a terminal display scale,
not a supported live-traversal promise. No Blizzard data or commits were added.

Final clean native capture (23:21Z) confirms the city, terrain and Cathedral facade
render together without crypt boxes or a tier-banner overlap. Five seconds of Free
Roll completed at r=32.74 after the terminal595yd display cleared. Capture evidence:
`/home/jjd/az-patches/l3b/final-scene-shots/`; full handoff: `l3b/report.txt`.

**D27 — Collectible Stormwind city pieces (2026-11-03).**
The mode embeds a versioned `stormwind-pieces.toml` manifest for placement10047:37
parts, comprising36 ordinary structures and the terminal Cathedral. The immutable source
has306 groups and6,026 MODDs; its root FNV-1a fingerprint guards the extraction. Raw-WMO
triangle-centroid box selectors separate buildings in mixed groups, including112, while
whole-group selectors cover standalone hulls. Every selected face has one owner. Streets,
canal beds, bridge decks, low floors and foundation covers remain permanent.

The implementation uses a private derived model inside the ordinary placement streamer,
rather than a second mode-owned city streamer. This keeps tile references, loading, form
residency, retained rendering, cancellation and Ready restoration under one owner. Original
permanent groups precede synthetic part groups. Each part owns walking/camera colliders;
opted-in structures promote camera-only DETAIL faces to walking faces. Closed interior
render batches are omitted from this outdoor representation, but their physical walls and
floors remain until their owning structure is eaten. The derived city has no portal/interior
ownership graph. Stock worlds retain their source asset and normal portal walk.

`RetireWmoPart` removes entity products, retained static_gx regions, collision faces and
owned/pending MODDs together in Stream. Claimed MODDs receive releases; edible products
cannot merge or weld across ownership boundaries. Run tombstones survive stream-out/in.
Before requesting retirement, `city.rs` validates support across the sphere footprint with
all retiring colliders excluded. Only an accepted acknowledgement awards the D4 OBB volume,
settles the ball onto validated support and flies the exterior trophy onto BallSpin. The
visual reduction is45%; the eligibility and credit use the original building's maximum
axis and box. Zero-volume `CityTarget` proxies serve the planner, never ordinary collection.

Ready cancels pending merge/weld products and rebuilds each changed placement once. Whole
suppression rejects subsequent partial retirement; whole restoration exclusively rebuilds
a suppressed root. Ungated map/world teardown releases derived strong handles, instance
maps and placement/part snapshots, preserving authored plans and run tombstones. Part
snapshots index collider products once and publish on residency/collider changes. Combined
whole/part/restore, scope teardown and settled-publication regression tests cover these seams.

The separate UID10047 claim entry lists522 reachable loose MODDs, including69 trees;
91 shallow planters remain scenery. TreeFacades293–304 are distant trunkless canopy cards,
omitted when the city becomes edible; real MODD trees remain. Authored market cargo,
723 tree-fair props and104 guards/citizens supplement the live city. Scene props and crowds
opt into the same WMO-floor rays through `wmo_floors`; other recipes keep their defaults.
The reproducible route generator is [data/stormwind-gen.py](data/stormwind-gen.py). Its
Canopy04 ladder checks an8% radius reserve before every new pickup-size band using audited
D4 dimensions, independently of incidental city food.
The fair opts into `rooted = true`: its basal trunk remains a wall through the intermediate
Dislodge band, while normal pickup eligibility/credit are unchanged. This prevents future
growth fuel being flung away before it becomes edible. Other scene items retain their default.


The road arrives at the arch near r3. Measured waypoints follow the curved canal bridge to
the open Square; navigation-only trails use a3yd arrival radius at narrow connections.
Collision-aware navigation ends at the Square boundary. The planner uses the mover's
0.85r horizontal sphere; a full-radius planner incorrectly rejected the gate. An installed
regression executes the real mover at r2.3/r2.96 over all three gate segments. At large
sizes, the planner prefers eligible structures and the remaining food around the terminal
district rather than leaving through the gate to forage in Elwynn.

Permanent collider IDs are exposed separately from edible hulls. The city deck guard checks
these floors beneath the ball's footprint, from below skyline roofs, and corrects growth or
a genuine centre-below-deck fall. Approaching terraces remains the mover's job. Small clumps
recover to remembered clear positions; validated rooftop recovery begins at r12. The
movement regression bypasses this guard so recovery cannot conceal a movement failure.

Some retained walking faces are collision-only. Empty-by-default `support_surfaces` gives
upward permanent faces a visible skin0.035yd below unchanged collision. Six-plane clipping
keeps every generated vertex inside authored walled-city bounds. Foundations choose an
opaque upward source floor batch and derive its affine world-plane UV mapping, preserving
street texture density/alignment. The Cathedral retains its own bounded foundation when it
lifts. Bounds/conservation tests verify that skinning leaves walking faces unchanged.

The orange plane in the initial aerial finale was independently isolated: it remains with
skins, liquids, WDL, fog, sky, M2s or WMO rendering disabled; disabling detailed ADT terrain
removes it. At the exact same fixed lens, the untouched source WMO shows the identical plane.
Entity-lane census: stock306 resident groups,23 drawn/209 batches; derived138 resident
groups,25 drawn/203 batches. The stock-only backdrop groups are TreeFacades295/304. Thus
this view exposes stock base terrain beyond the Cathedral's rear city boundary; it is not
an omitted route district. The finale now establishes the Cathedral from its rear quarter,
looking inward across Stormwind. Terrain rendering is unchanged. Native source-isolation
levers are `WOW_AZEROLL_STOCK_CITY`, `WOW_AZEROLL_VISUAL_AB` and `WOW_WMO_DRAW_AUDIT`;
use `WOW_STATIC_GX=0 WOW_STATIC_MERGE=0` for an entity draw census.

A separately capped building skin keeps up to64 shared-mesh exterior instances from the
kinds actually collected. Each eligible house/tower supplies three copies; a FIFO replaces
old kinds. Slots reflow over both hemispheres as the pool fills, roofs point outward, and
scale follows the shell up to original building scale. These copies have no pickup, credit,
support or collision role. Ordinary trophies still use D23. Ready destroys the run roots.

The Cathedral requires physicalD160 and24 ordinary structures at the Square trigger.
Only its synthetic group is hidden, leaving shared Square group112 live. The trophy stays
attached through Results, and the source remains hidden while that trophy is displayed;
Ready, Select and Free Roll restore it. The displayed595yd final size does not change the
physical radius. The authored clock is1800s, par900s, speed challenge660s and Free Roll capr110.

Acceptance: normal-start run14 won in351.7s (5:51.7), physicalD210.84 at the Cathedral
trigger, displayed595yd,24 required structures, zero game Unstuck and five planner escapes.
Native frame p50/p95/p99 were16.7/17.8/19.8ms. Free Roll continued for15s with the terminal
mask/display cleared. An actual Ready-after-three test restored workshop43, east bell
tower49 and orphanage block40; each was collected again for identical normalized credits
4093.30/493.79/4046.47 with fresh collider entities. Its one telemetry relocation is the
expected Ready reset, not recovery. The independent embedded-recipe run15 won in337.5s (5:37.5), physicalD209.26 at the trigger,
zero game Unstuck and six planner escapes. Both used recipe FNV e9297de5287c97e5; run15 used
the shipped registry without a recipe override. Its frame p50/p95/p99 were16.7/18.6/33.3ms.
Earlier city wins8/12/13 remain historical tuning evidence. Captures, full telemetry and
`ready-final-verified.json` live in `/home/jjd/az-patches/city/`.

Two installed-data regressions protect the final authoring: every planned tree-fair prop
must find geometry in its actual placement ray (the terrace rises above the old flat anchor),
and five gate radii across five lane offsets must remain below the overhead entrance deck.
Recovery rays stay below that deck until rawX=-90. Canal navigation capsules are12yd wide,
covering the detour that previously left the6yd strip and drove beneath the bridge. They
still end before the nursery. The tree ladder's measured floor centres and planted state
keep every future growth band available.

Validation: native play build, full app without data (4472 passed,10 ignored), installed-data
Azeroll suite (246 passed), terrain-stream suite (59 passed), WebGPU wasm check and diff
whitespace check pass. Three legacy full-app installed-data tests assume enGB localization
and fail against the supplied enUS data; game/engine checks above are green. WebGPU gameplay
performance is not measured. The current automated route is about6min against a30min human
clock; the requested10–12min pacing and deliberate district vignettes remain a character pass.
In particular, the densely packed fair causes a very rapid but fully D4-credited growth chain.

**D28 — L2, Market Day Mayhem: the Prize Pumpkin and a living harvest fair (2026-11-02, polish pass 2026-11-03).** The former
Goldshire level (`goldshire.toml`, id `market-day`, file names kept) is now Goldshire's harvest-fair
market. Hogger is no longer the goal (see the D11 note).

*Objective.* The market's centrepiece, **Ma Stonefield's Prize Pumpkin**, on a judging stage in the
village square, 30 yd south of the start (the start, at (-9458,60), faces it). It is `G_Pumpkin_02`
(not uncarved after all: see D28b, "Uncarved pumpkins") at scale 3.7: pickup axis 8.62, so it needs a ball
18.54 yd across (r 9.3, volume about 800), twice Hogger's tier; the last tier row is `at = 18.54`,
which must be at or above `2.15 x axis = 18.533` or `contact` calls it Dislodge. It stands 3.2 yd up on a
two-bale hay plinth (bales of axis 9.4+, so they outlast the pumpkin), inside a ring of hay, flagpoles
and banners with a 22 yd contest flag over it, and is solid (a `Gate`) until it can be rolled up.

*Foreshadowing.* The small-ball camera sees only 8-10 yd ahead and looks down 20 degrees, so nothing 30 yd
away can be seen at r 0.3-1 whatever its height. The recipe therefore leads the eye with props the camera
does see: the goal ring and intro text from the first frame; *last year's champion* (a plain 3.5 yd
pumpkin with banners, 7 yd ahead of the start, edible at r 3.8) filling the frame at r 0.5-0.6; and a
*prize vegetable trail* of apples and pears, then melons, then small watermelons and pumpkins, running
south from the start to the stage, each leg bigger than the last. The real Prize Pumpkin is first seen
from the market at r about 1.5-2.

*Generalised goal (recipe + code).* `[goal] kind = "collect"` accepts a `target` that is not a herd
species: exactly one scene item with `goal = true` and `[goal] axis = <pickup axis after scale>` (needed
before the model loads, for the tier table and the autopilot). `design::GoalProp` marks the entity;
`props::assemble` makes it a `Kind::Boss` named `target`. Win checks (`level::tick`), the HUD guide
(`hud.rs`) and the autopilot's boss pick (`auto.rs`) match the boss by the goal's name, so a herd with
`boss = true` (Hogger) keeps his staging, thump, music duck and orange quality without ending the level.
The finale, results and `win_title`/`fail` text are unchanged.

*Hogger as a bonus.* Challenge `hogger` (`collect-all`, species Hogger). His pass, camp loop and flight
are untouched; the primary route still ends its outbound leg at his camp.

*Route and layout.* Areas: Beer garden (start, the inn yard), Market (stalls), **Judging stage** (the goal
area, radius 9, so only the new trail enters it), Northern goods (farm wagons), Smithy, Broken cart,
Southwest road (drovers' pens), Roadside camp, Riverpaw camp (the raiders' loot), and the two farms
(tour only). The primary route is Beer garden, Market, Smithy, Broken cart, Southwest road, Roadside camp,
Riverpaw camp, then *Parade home* (a new 230 yd trail of big leftovers) to the Judging stage: it leaves
town and comes back, because the stage is a leaf of the route graph reachable only from the camp.
Stalls: cheese and bread (a toppled table: Hogger's last raid), greengrocer, potter, weaver, fishmonger,
drovers' pens (chickens and 10 `Piglet`s), beer-garden kegs, and a ring of awnings, catapult trunks, cages
and chests ("Stallholders' big goods") for the ball of r 4.3-5.2. The inn interior left this level
(Level 1 owns it). Start radius 0.32 (Level 1 ends at 1.25). Tiers: Crumbs and coppers, Critters 1.04,
Livestock 2.58, Shoppers and horses 4.33, Forest giants 6.02, Hogger 8.6, The Prize Pumpkin 18.54.

*Cast* (`goldshire-npcs.toml`, 33 to 69): twelve at the stage (judges, applauding gawkers, a bard and two
dancers, kids), twelve stallholders, shoppers and adventurers, a beer-garden crew, a fishwife and an
angler, drovers and farmers. The smithy crew (Argus, Andrew, Corina, Kurran, Lyria, Wilhelm, Bo) now
work the yard: inside the forge they lured the ball against the building's wall. The existing escalation
(D18) does the rest. New activities: `bark`, `dance`, `applaud`, `fish`; new `carry` drops: pumpkin,
watermelon, fish, cheese.

*No-trap work (polish pass).* Nine full autopilot runs had 0-3 game Unstuck teleports each. The wedges, from
the telemetry (position at each stuck/unstuck event, ball radius) and `WOW_AZEROLL_STALL_DEBUG` /
`SLIDE_DEBUG` solid-contact logs of run 11:

| pocket (WoW x,y) | ball r | what wedges it | fix |
|---|---|---|---|
| (-9513,107) | 3.9-4.8 | the Elwynn trunks at (-9514,103) and (-9518,114) are 7.2 yd apart between surfaces | invisible barrier from trunk to trunk |
| (-9499,87), the Gypsy Wagon's west flank | 4.3 | a 5 yd lane between the wagon box (11.5 x 7.1 yd) and the bare trees at (-9498,92), (-9499,95), holding a post, harness, rope and bucket the ball cannot reach but keeps trying for | barriers capping both ends of the lane |
| (-9484,45), east of the stage | 3.9 | tree trunks at (-9483,46), (-9485,45); stage rings and gawkers lured the ball in | rings pulled in, crowd moved west |
| (-9463,109), the smithy's west wall | 4.9-5.4 | an engine wall at y 105; the smithy NPCs inside the forge lured the ball against it | the crew moved to the yard |
| (-9466,79) by the guard post | 4.8 | the Market is exhausted at r 4.7, so the bot hunts distant fences through trunk gaps | "Stallholders' big goods" (about 45 volume at axis 4-4.8) |
| (-9598,95), deep forest | 8.4 | the bot hunting far bushes at r 7-8.4 between trunks 15 yd apart | "Hay for the pens": haystacks and hay wagons in a clearing on the road (axis 5.9 and 7.7) |

Moving a trail endpoint into another area's circle collapses the route graph (run 8: a one-step hunt, 3
teleports); the unit test `market_day_goal_is_the_prize_pumpkin` asserts the primary route runs through
Riverpaw camp and ends at the Judging stage.

*Pacing.* The claimed map doodads (bushes, fences, trunks) dominate what the ball can eat near town:
the first pumpkin (axis 4.5) was won in 181 s without leaving the market, so it was sized up until the camp
was needed. Autopilot (`WOW_AZEROLL_AUTO=620`):

| run | binary | win | Hogger tier / pumpkin edible | final r | stuck | unstuck |
|---|---|---:|---|---:|---:|---:|
| 1 | pumpkin axis 4.5 | 180.7 s | tiers 54 / 96 / 138 / 161 / 237 s | 5.2 | 2 | 0 |
| 4 | axis 8.63 (carved `G_Pumpkin_01`) | 364.3 s | 142 s / 354 s | 11.2 | 6 | 0 |
| 5, 6, 7 | same | 301.2, 357.4, 372.1 s | | 10.1-10.3 | 5, 7, 8 | 1, 0, 2 |
| 9, 10 | same | 349.2, 356.9 s | | 10.4, 11.1 | 8, 8 | 1, 3 |
| 11 | uncarved, plinth, start moved, trunk barrier (solid-contact logging on) | 337.7 s | 157 s / 331 s | 9.8 | 5 | 0 |
| 12 | + smithy crew in the yard | 316.6 s | 136 s / 310 s | 9.8 | 2 | 1 (deep forest, r 8.4) |
| 13 | + haystack clearing | 367.9 s | 136 s / 356 s | 11.6 | 7 | 0 |
| 14 | same | 336.5 s | 156 s / 330 s | 10.0 | 6 | 1 (wagon lane, r 4.3) |
| 15 | + wagon-lane end caps | 338.3 s | 147 s / 330 s | 10.1 | 6 | 0 |
| 16 | same | 334.8 s | 140 s / 329 s | 9.9 | 7 | 2 (guard post, r 4.8) |
| 17 | + stallholders' big goods | 333.0 s | 149 s / 327 s | 10.1 | 7 | 0 |
| 18 | same | 365.5 s | 142 s / 354 s | 11.6 | 8 | 2 (wagon box, r 1.8; tree pocket, r 5.8) |
| 19 | same | 343.2 s | 120 s / 337 s | 9.7 | 10 | 2 (guard post, r 4.4; signpost row, r 5.1) |
| **20** | **final: + wagon-lane west cap, trunk barrier** | **324.5 s** | 161 s / 317 s | 9.7 | **6** | **0** |
| **21** | final | **296.1 s** | 137 s / 291 s | 9.6 | **4** | **0** |
| **22** | final | **296.7 s** | 123 s / 292 s | 9.7 | **6** | **0** |

Three consecutive wins on the final recipe with no Unstuck (runs 20-22; the same binary, recipe hash
unchanged). Before it, the same design alternated: runs 15 and 17 were clean, 16, 18 and 19 each had two
teleports, so the margin is not large: an r 4-5 ball in the village's cluster of posts, fences, lamps and
trunks is the remaining hazard, and a fourth, fifth run could still hit one. The recipe cannot clear the
claimed doodads; the barriers and the lures are what it controls.

The hay clearing means the bot now reaches the pumpkin's size on the road (r 9.0 at 288 s in run 22)
and walks home without visiting Riverpaw camp or Hogger; a human exploring will still find the camp the
richest area.

Autopilot times of 5-6 min correspond to roughly twice that for a human in earlier Goldshire
measurements, so the 6-8 min brief is met by a human, not the bot. Stuck events (the autopilot's own
"ball not moving" detection, recovered by its escape) stay around 6-8 per run: most are a tiny ball
brushing the porch fence at (-9451,52) in the first 20 s and an r 8-9 ball between forest trunks.

*Evidence.* Before/after captures in `/home/jjd/az-patches/l2/`: `before-*` (carved pumpkin, 38 yd start),
`after-last-years-champion-plain-r0.6.png`, `after-prize-vegetable-trail-start.png`,
`after-prize-pumpkin-on-hay-plinth-r5.png` (the uncarved Prize Pumpkin on its plinth between banners, from r 5),
`after-finale-prize-pumpkin.png`. The barriers are invisible; the pockets are documented in the table above
and their positions in `goldshire.toml`.

*Open.* The cast reuses existing human displays. Not verified by eye: the piglets' look (display 503), the
dancers' and barkers' animations, whole-village performance with 69 units, and any small-ball view of the
real Prize Pumpkin (the autopilot only visits the stage at the end).

**D28a — Market Day rebuilt as a Katamari town stage (2026-11-03, after the first playtest).** The user's
L1 verdict (sprawl, surfaces without ramps, invisible walls) applies to every level, so L2 was redesigned
to the grammar in [katamari-room-design.md](katamari-room-design.md) §6 (the town-stage additions T1–T4 and
the district plan). `goldshire.toml` is now **generated** by `docs/azeroll/data/market-day-gen.py` (the far
farms and the northern household are appended verbatim from `market-day-outskirts.toml`); edit the
generator, not the recipe. The generator sizes every item with the L1 law (`at = D`: a prop opens at ball
diameter 2.15 × its longest scaled extent, boxes from `props.csv`), validates its own TOML with tomllib and a
recipe-shape check, and prints a growth ledger (`--ledger`). `MD_CAPTURE=camp|fair|stage` writes a start
variant to `/tmp/az-l2r/` for screenshots of vignettes the primary route never shows.

*What changed.* The goal is the Prize Pumpkin at **scale 2.4 (axis 5.59, D 12.03, r³ 216)**, down from
18.5 yd, so the level is won inside the village and the livestock fair instead of 250 yd into the forest;
Hogger's camp moved to the forest shelf past the fair (`camp` (−9570, 194), 8 gnolls, 3 bears beyond) and
his early pass comes to the cheese stall (`patrol` (−9470, 66) at 12 s). The village is seven districts:
Beer garden (start, r 0.32), Market, Judging stage (leaf), South junction (waypoint), South gate,
Livestock fair, Raiders' camp (tour), Smithy yard (tour), plus the kept outskirts. Every stall is a vignette
with a reason, a landmark and a size band; the runners-up stand in a line of increasing size down the
street to the stage; every table with food has a ramp (bale, sack and crate `stairs`, a dock plank; tables
are `solid = true` since an unflagged authored prop has no collision at all). **Zero `[[barriers]]`**: seven
`blocker` props stand where the five barriers were (a hay wagon in the trunk pocket, the weaver's tent in
the bare-tree gap, three stumps and a haystack round the gypsy wagon's lane and fence corner, an awning under the square's tree clump), and
`design.rs` asserts it. Tiers: Crumbs 0 · Critters 1.04 · Livestock 2.58 · Shoppers and horses 4.33 ·
Raiders at the gate 6.02 · Hogger 8.6 · The Prize Pumpkin 12.03 (`MARKET_TIERS` in `level.rs` updated); the
challenge ids are unchanged (speed is now 5:00). Cast: the same 69 (11 kids), everyone outdoors in the
vignette they belong to (the inn is shut: "gone to the fair").

*What play taught (runs in `/home/jjd/az-patches/l2r/`, binary `target/play` of the day, recipe override
via `WOW_AZEROLL_LEVEL`).* The first draft won in 346 s. Every stall or Unstuck after that was **bait beside
something the ball cannot pass**, never a size gap: two solid tables 3 yd apart with the stairs between them
(run 3); the autopilot's trail from the square to the south gate through the 4.5 yd gap between the map's
roadside fence (x −9499, y 63–76) and the gypsy wagon (5 Unstuck in run 4: the South junction waypoint now
takes every trail round the fence's east end); the potter's tables against that fence, then against the
stumps; the weaver's goods in the pocket between the tent and a stump; a banner stake beside a lamppost; the
cheese tower and the toppled table in the tree clump at (−9473, 76); the runners-up overlapping the plinth;
the champion's cart behind the tree clump east of the stage; and the map's own harness, rope, bucket, post,
crate and barrels under the wagon, which the autopilot kept driving at from outside the blockers because
`look()` only hid enclosed targets indoors (fixed by the physics track on 2026-11-03: enclosed targets are
masked outdoors too). Rule for authors: a blocker must cover the bait behind it, and nothing edible goes
within one ball radius (of the size that wants it) of a trunk, fence end, lamppost or solid.

| run | recipe / binary | win | stuck | Unstuck | note |
|---|---|---:|---:|---:|---|
| 3 | first draft | 346 s (56 %) | 9 | 0 | garden wedge 45 s, fair thin at D 8, 65 s bush hunt at the end |
| 4 | fair pumpkins, trail props | 378 s | 18 | 5 | all at the fence–wagon gap on the square→gate trail |
| 5 | South junction waypoint, stage moved | 257 s (41 %) | 8 | 1 | map crate/barrels in the fence corner |
| 6 | corner stump, weaver moved | 242 s (39 %) | 4 | 1 | harness through the wagon's porous footprint |
| 7 | lane haystack | 252 s (41 %) | 6 | 1 | ball parked on the 8 bale steps at r 2.6 |
| 8 | 5 bale steps, corner stump over the barrels | 254 s (41 %) | 4 | 0 | potter pinched between stumps |
| A1 | potter and pumpkin wagon moved | 268 s (43 %) | 8 | 1 | runner-up pumpkin overlapping the plinth |
| A2 | parade end, champion moved (final layout) | 235 s (38 %) | 3 | 0 | |
| A3 | same | 238 s (38 %) | 4 | 1 | cheese tower at the tree clump |
| A4 | same, binary with blocker masking (08:53Z) | 249 s (40 %) | 4 | 0 | no harness stalls |
| B1 | cheese tower and apple heap moved, junction waypoint (−9505, 52) | 267 s (43 %) | 8 | 0 | |
| B2 | same | 248 s (40 %) | 6 | 1 | the square's tree clump (two trunks 2.9 yd apart) at r 2.3 |
| B3 | same | 263 s (42 %) | 6 | 0 | |
| C1 | awning blocker on the tree clump | 228 s (37 %) | 3 | 1 | pocket between the awning and the fence rail at (−9475, 80) |
| **C2** | **awning sealed to the rail, fishmonger moved west (shipped recipe, fnv 31a9fe92c70d2602); binary 10:03Z with the footprint mask** | **228 s (37 %)** | 3 | **0** | |
| **C3** | same | **240 s (39 %)** | 3 | **0** | |
| **C4** | same, binary 10:14Z | **231 s (37 %)** | 4 | **0** | tiers at 12 / 98 / 118 / 185 / 214 s, pumpkin edible 224 s |

Three consecutive wins with no Unstuck on the shipped recipe (C2–C4), 37–39 % of the 620 s clock, 808–834
pickups each; the remaining 3–4 stalls per run are the autopilot's escapes at the map's own bushes and
lamp posts at r 1–3 (the beer-garden kegs corner, the watermelon heap by the square's tree clump, the
junction lamppost), each recovered in under 10 s.

Tour route (A-layout, old binary): 361 s, 2 Unstuck, both in the kept legacy Northern household scenes and
at the junction corner at r 5.5; the tour reached the pumpkin's size before the raiders' camp, so Hogger
stays a human's detour. A human at 2–2.5× the autopilot's pace lands at 8–10 minutes on the 10:20 clock.

*Open.* The outskirts (Northern household, the farms) are the unrevised first-level scenes and still
sprawl: a free-roll pass should give them the same vignette treatment or drop them. Hogger's camp is
authored but only verified by the capture variant, not by an autopilot visit. The 4–6 stalls per run that
remain are the map's own bushes, lampposts and tree clumps at r 2.5–3 in the square, which the recipe
cannot move. The judges' table and the beer-garden tables are climbed by the autopilot only when their food
is the best value nearby; a human test of the ramps is the next acceptance item.

**D28b — The outskirts and Hogger's camp (2026-11-03, second pass).** The first version's far farms
(Stonefield at 505 yd, Maclure at 460 yd) and the northern household are gone: they were the sprawl the
user described, the tour run's two Unstucks were in them, and a Katamari town stage has no second town.
The claim radius is unchanged (the map's doodads out there stay Free Roll food). In their place:

- *The scarecrow contest* (`[[areas]]` Scarecrow field, (−9393, 60) r 14), the fair's other contest in
  the meadow east of the north road: five scarecrows on their straw in order of size (D 5 → 10) round the
  champion scarecrow on a bale (D 11), corn stooks, the judges' lunch, the map's own barrels and crate,
  giant crates, 8 rabbits, and Melika keeping score. It is the tour's first branch and Free Roll's second
  size ladder. The lane to it (`North lane` waypoint (−9420, 46)) passes south of the thicket at
  (−9420, 55) instead of through it.
- *Hogger's camp on the way home.* `camp` is (−9568, 180), 60 yd past the fair on the same flat shelf;
  the *Gnoll outskirts* (skull stakes, two gnoll banners, emptied barrels, the start of the loot trail)
  stand at the fair's south-west corner in sight of the sale ring, and the loot trail runs from there to
  the camp and back. The trail graph is fair → camp → *Fair gate* ((−9540, 100), Stonefield's entries) →
  parade home → stage, so the camp is on the primary route and a ball that is not yet pumpkin-size walks
  through it at D 9–11 and takes Hogger; a ball that reaches 12 yd at the fair (the autopilot usually does)
  goes straight home, which is what a human in a hurry would do too.

*Runs (recipe fnv e91bf69778c501cc, binary of 10:14Z, summaries in `/home/jjd/az-patches/l2r2/`).*

| run | route | win | stuck | Unstuck | note |
|---|---|---:|---:|---:|---|
| P1 | primary | 235 s (38 %) | 2 | 0 | pumpkin size reached at the fair; home without the camp |
| P2 | primary | 237 s (38 %) | 3 | 0 | |
| P3 | primary | 229 s (37 %) | 3 | 0 | |
| tour | garden → scarecrow field → garden → market → junction → gate → fair → … | 331 s (53 %) | 8 | 0 | 40 s in the field (r 1.06 → 1.18); pumpkin size at the fair again, so the smithy and the camp were skipped |
| camp visit | `MD_CAPTURE=campvisit`: the goal area set to the camp, so the route ends there | 231 s (37 %) | 4 | 1 | pumpkin size at the fair again before the camp leg; the Unstuck was Marshal Dughan's brave charge pinning a r 1.05 ball at the guard post (two rams), not geometry |
| camp (first pass) | `MD_CAPTURE=camp`: start r 2.4 beside the camp | won at 51 s | 0 | 0 | 16 s in and around the camp at r 2.4–3.6, then home |
| Hogger | `MD_CAPTURE=hogger`: start r 4.4, 18 yd from the camp, a shot a second | Hogger rolled up at 1.6 s | 0 | 0 | captures 13, 14, 14a |

The stalls are the same map clutter as before (the beer-garden kegs corner at r 1, the square's tree
clump, the junction lamppost). The autopilot never walks the camp leg because it is pumpkin-size by the
fair; the camp is verified trap-free by the two capture starts (0 stalls, 0 Unstuck in and around it) and
the loot trail, not by a route walk. Hogger stays the human's optional detour, foreshadowed from the fair
by the gnoll gate at its corner. Open: the HUD overlap of the tier label "The Prize Pumpkin" with the "Prize
Pumpkin is ready!" banner (capture 06 of the first pass) is `hud.rs`, reported to the lead, not touched.


*Uncarved pumpkins and fair rams (2026-11-03, polish).* All three `World\Goober\G_Pumpkin_01/02/03` models wear
`mm_JackOLantern_01` (a carved, candle-lit face), so the earlier note "02 is uncarved" was only true from the back; the
HUD icon and any pumpkin turned its face to the camera read as Halloween. The recipe now uses the one plain
pumpkin in the data, Duskwood's `Bush\Pumpkin01.m2` (a gourd with a vine). Its header box is 3.44 yd long
because the vine is in it, so at the same opening D the gourd is about 40 % smaller than the jack and a single
one gives 0.31 of the growth; the generator keeps the law (`at = D`; mixes scale from their other members; the
Prize Pumpkin is scale 1.625 = axis 5.59 = D 12.03) and makes each runner-up at D 6.4 or more a litter of three
(0.92 of the old budget at D 12; ledger 361 against 217 needed). The parade-home trail dropped its pumpkins
(its shared scale would have opened them 1.56 x later) and the stallholders' carried pumpkin is the same model.
`L1`'s one pumpkin (`lions-pride.toml`, G_Pumpkin_03) is untouched. `MD_CAPTURE=guardpost|heap|pumpkins`
(`MD_AUDIT=<paths>`) are new tooling variants. Runs of the final binary (summaries in
`/home/jjd/az-patches/l2fix/`): 243 s (39 %), 247 s (40 %), 247 s (40 %), 0 Unstuck, 0-2 Hogger/guard rams each;
the guard-post fixture (r 1.05, 0.7 yd from the Stormwind Guard) is rammed twice by two guards 3.6 s apart,
knocked clear each time, and has 0 stuck events. The first draft (single pumpkins at the same D) won in 261-297 s
(42-48 %) with one pin: the pocket of two solid inn tables west of the guard post, 10 yd from where Dughan's
knock left the ball, found by the autopilot rather than thrown by a guard.


**D29 — Climbable mode solids, ramps and visible blockers (2026-11-02).**
Loose solids in Wall or Dislodge state now supply full-radius support from upward walkable oriented-box
faces (including a side face that becomes the top when a book topples). The mover uses the same 73 footprint rays, spherical-cap lift, reachable-step
origin and normal.y > 0.6 rule as WMO floors. It raises before horizontal motion and settles
afterwards; sides do not push a sphere already supported by the top/edge cap. Furniture
keeps D22's radius-dependent open legs and slab. The slab supplies support; leg tops
are internal faces beneath it and cannot suspend a ball inside the seat. Tree canopies remain excluded (basal trunks
are not platforms). RollUp objects supply no support, and Attached/Shed objects are excluded:
eating/dislodging a platform releases the ball to gravity and the remaining floor next frame.
Recovery validates remembered tops at the current radius. The progression report estimates ramp
access by flooding touching support patches; it does not run an expensive full A* for every food. Indoor A* samples mode box tops
as floor states as well as WMO floors, using the same step allowance and support edge exemption. Indoor
WMO stances use the mover's full-radius footprint too. Drop edges check head clearance, room routes
follow individual risers, and sweeps reserve the mover's contact margin; speculative step lifts require
rising support. Waypoint arrival permits one step of cap lift but cannot skip a storey. Installed-data
tests actually drive both inn staircases up/down at r=.3 and .5, rather than only testing path existence.
The live search retains its one-second cancellation deadline and node cap. Search errors distinguish
deadline expiry, node exhaustion and no route. Installed-WMO geometry tests retain the same node cap
and mover assertions but inject a non-expiring clock, so competing app tests cannot turn CPU scheduling
into a false geometry failure; a separately injected expired clock checks deadline cancellation.
Downward motion remains allowed: platforms have no retaining wall above their top.

`collect::Planted` is an opt-in marker for rooted scene props. Collection converts only their
Dislodge contact to Wall and uses the ordinary basal trunk/box correction; RollUp thresholds and
D4 volume credit stay unchanged. Recipe `rooted = true` attaches the marker (default false).
L3's authored fair trees opt in; live MODD trees and other levels retain their existing behavior.
This prevents future planted growth bands being launched away before they become edible.

Recipe contract for the L1 designer (available now):
```toml
[[scenes.items]]
model = "world\\generic\\human\\passive doodads\\books\\book01.m2" # use an audited model path
count = 8
arrangement = "stairs"
offset = [0.0, 0.0] # first step, relative to the scene
spacing = 0.35     # run per prop, yards
step = 0.12        # rise per prop, yards; keep below 0.2 + 0.45r
heading = 0.0      # east=0, south=pi/2, relative to scene yaw
scale = 0.5
yaw = 0.0
# stairs are automatically solid; lift offsets the entire run

[[scenes.items]]
model = "<audited plank model>"
count = 1
arrangement = "line"
solid = true      # any prop: solid + walkable until edible
pitch = 0.25      # fixed radians about local X AFTER yaw; positive rises toward local -Z
yaw = 0.0         # use yaw to aim the slope; roll rotates about local Z; tilt is RANDOM lean
lift = 0.5        # model origin above the recipe's underlying WMO/terrain floor

[[scenes.items]]
model = "<audited keg or crate model>"
count = 1
arrangement = "line"
scale = 5.0
blocker = true    # visible, solid, permanently inedible at EVERY size
```
Use overlapping steps/planks whose low end meets the floor and whose high end overlaps the
target table/hearth; model bounds determine actual top height. Choose long ramp pieces that
remain too large to eat at the intended radius; edible steps disappear by design. `pitch` is
fixed, `tilt` remains seeded random lean. `[[barriers]]` remains backward compatible; L1 can
replace it with visible blocker props. Native acceptance evidence is recorded separately in
`/home/jjd/az-patches/climb/`; no runtime acceptance is implied by this implementation note.


D29 acceptance: the scripted native r=.2 fixture climbed nine open books (0.30 yd run,
0.125 yd rise) onto `InnTableSmall` and stood at Y=57.67135 (floor56.339 +top1.132 +r.2).
It also climbed a scale .25, pitch .25 dock plank (lift .65); longest scripted stall .05s.
Best captures: `/home/jjd/az-patches/climb/ball-on-table.png`, `ball-on-ramp.png`.
The unscripted autopilot fixture initially collected **0/5** table muffins: XZ-only waypoint
arrival/progress marked raised stances complete while the ball was beneath them. Height-aware waypoint
arrival and remaining distance fixed it: **5/5**, max centre Y=57.7, zero stuck/Unstuck in40s.
A regression test pins under-waypoint arrival. The planner's aggregate height band is12yd
(to consider actual stairs to other storeys); every individual edge still obeys the mover's
step and collision rules. Ray origins never move above reachable foot height.

Market Day native regression won at342.9s, D19.28,1012 pickups, median60FPS once concurrent
compilation finished. Nine AI stalls and one Unstuck occurred; the rewind at(-9508,108),
r6.51, names the pre-existing invisible barrier/tree pocket, not a newly climbable crate.
The old porch fence stall remained; no barrel/crate contact was identified as a new wedge.
This is **not** a zero-Unstuck outdoor guarantee. Results then hit a concurrently changed hub
shader binding-size mismatch (416 vs320); gameplay had already recorded `won=true`.
The hub owner was notified. Logs, recipes and CSVs live in`/home/jjd/az-patches/climb/`.
The final authored L1 hearth/mantel placement and pacing belong to the recipe's own acceptance;
this fixture verifies the climb/support mechanics in the common room by the hearth.

D29 final verification: **170 Azeroll tests pass**, native `play` build and WebGPU wasm
check pass. Temporary navigation tracing was removed. No commits were made. Full handoff
and retained command logs: `/home/jjd/az-patches/climb/report.txt`.

**D30 — Copies retain interior light; trophies share the ball's light node.** Frozen copies
previously passed no probe to `spawn_model_entities` and had no `InteriorLit` membership, so
recipe food and claimed MODDs always used the exterior scene SH. The stock MODD path instead
folds its `DoodadBase::Interior` ambient/diffuse words and world-space MOLR lights into a probe;
WMO walls separately use their authored MOCV/group material lane. `lighting::resolve` resolves
the camera's scene/fog, and cannot substitute for an object's interior classification (the
D8 camera can be outside while the ball and food are inside).

The generic, opt-in `frozen_interior_materials` seam supplies bake/fade variants for ordinary
entity-hosted M2 batches. Azeroll registers copies with the existing `InteriorLit` classifier:
authored copies use the engine's containment/MOCV footprint and group MOLR fold, invalidated
by movement or WMO residency. Claim messages carry the original `PropLight` provider;
stationary furniture retains that exact MODD fold, even if its centre's containment ray misses.
On pickup its provider/probe is released and its batches join the ball's `ClassifiedBy` anchor
(at the ball centre), so spinning trophies share one moving floor resolve and probe. Shedding
returns batches to their own anchor. Exterior material handles stay the original ones;
trophy alpha composes with the current interior/exterior material and preserves probe bits.
Anything that clones a part off-world (the HUD item bubble) must take
`InteriorLit::exterior_material()` and a neutral tag: the bake material plus tag name a world
probe slot that pickup releases, which rendered the bubble black.
The engine seam does nothing without explicit membership/provider installation (D9).
Per-frame cost: the classifier re-resolves a copy only when it moves or residency changes, and
the whole clump costs one resolve; L1 autopilot frame time is unchanged (p50 16.6 ms either way).

Capture switches: `WOW_AZEROLL_NO_PROP_INTERIOR=1` restores the old copy lane;
`WOW_AZEROLL_STOCK_FURNITURE=1` leaves MODDs engine-owned for same-placement comparisons.
Evidence and measurement notes: `/home/jjd/az-patches/light/` on the investigation machine.

**D31 — The ritual: roll past the goal, the send-off, Fizzwick in play, the Catalogue (2026-11-03;
design in [ritual-design.md](ritual-design.md), audit-2 #1 #2 #6 #7 #10 #11; amends D20, D24, D26).**
Write set: `ritual.rs`, `fizz.rs`, `lines.rs` (new), `select/catalogue.rs` (new), `level.rs`,
`progress.rs`, `hud.rs`, `hud.wgsl`, `hud_models.rs`, `select/verdict.rs`, `music.rs`, `campaign.rs`,
`names.rs`, small seams in `sfx.rs`, `cutout.rs` (`publish` is `pub(super)` so the ritual's lens can
be ordered before it) and the results branch of `auto.rs`.

*Roll past the goal (a size level).* Meeting the goal is a beat, not the end: `level::tick` stamps
`Run::goal_time`, lights the first of up to four dial stars (`Run::stars`; one at the goal, one per
`[level] grades` boundary, 1.2x / 1.5x / 2x by default; a star once lit stays lit), sends
`GoalReached` (the rosette blooms, a half-level fanfare, the music ducks like a tier, Fizzwick's goal
line) and keeps the clock running. The run ends at the candle (`Ending::Snuff`), when the player
holds Enter for `level::CALL_HOLD` = 1 s (`Ending::Called`, the clock's bezel fills while held; the
hint is now the persistent Finish button, below), or, under the goal at the candle,
`Ending::Failed`. A single-target goal (L2 pumpkin, L3 Cathedral) still ends on capture: its
pre-roll is `progress::COLLECT_FINALE_SECS` (2 s) or `FINALE_SECS` (3.2 s, the D26 flight), then
`Ending::Captured`. The goal test waits for 0.5 play seconds so a debug start above the goal does
not end the run in Ready. Grades: a size goal is graded on the final size only; the comet is
`goal_time < par`; `LevelBest::best_time` is the fastest *to the goal*; time challenges read
`goal_time` too (`progress::judge_win`). Single-target levels judge as before (D24).

*The send-off.* The phase goes to `Results` at the ending (not at the dock; so every gameplay system
that is gated on `playing` stops for free) and `ritual::begin` pauses virtual time. Everything in
`ritual.rs` is a pure function of the real seconds since the ending (`Ritual::t`), so Enter, Space
or Esc skip by jumping to `DOCK_AT` = 3.4 s. 0-1 s: the lens eases to a three-quarter view
(`three_quarter`: +35 degrees round, 22 degrees up, 15 indoors; the clump fills 46 % of the height
in the left 54 % when wide, 30 % in the top half when the aspect is under 1.4, which the panel
and the headline (`Composition`) both read). 0.6 s the beam lands (open additive cylinder, ground
ring), 1-3 s the clump lifts 1.5 radii (1.0 under a roof) with one tilted-axis turn, then hovers
(0.15 r bob). The lift and the turn are render offsets applied to the ball root and `BallSpin` after
the ball's own write-back (`BallState` is untouched, so nothing in physics or the autopilot sees the
hover). The lens overrides `CamRig::eye/look` after `place_camera` and `finale::animate`, before
`cutout::publish`, so the see-through window works. `OnExit(Results)`, `OnEnter(Ready)` and
`OnEnter(Select)` run `ritual::teardown` (time, lens, lift, scale, beam). Exits are `Wish`
messages (`Hub`, `Again`, `Free`) from the keys and the buttons: Hub plays a 0.8 s departure (the
clump shrinks up the beam into a glint) before `Select`; Free Roll a 0.5 s drop and a 0.6 s lens
return before `Playing`; Again goes straight to `Ready`. A lost run has no beam and no lift: the
clump just stops, Fizzwick sighs a level-specific try-again line at 0.4 s, `TIME'S UP` shows at
1.2 s, `NOT QUITE` docks at 3.4 s with Roll Again first and lit. The play HUD is hidden during the
send-off (the spec dimmed it to 40 %; the headline and panel carry the numbers). The results panel
is one docked plaque (no full-screen dim): the medal beside the grade, Fizzwick's window, the tally
(`Goal at` / `Time (the candle)` or `Called at`, size, count, best find, star, `NEW to the
Catalogue`, challenges), the three buttons.

*Music (audit #11).* `campaign::LevelEntry::songs`: L1 `goldshire-mambo`, L2 `goldshire-go-go`,
L3 `kuru-kuru-azeroth` then `morning-in-goldshire`; a retry restarts the first, Free Roll plays
`morning-in-goldshire` on any level, the send-off ducks 55 % for 3.4 s and the results hold the
level track at 50 % (the title theme returns at the hub). The recipe's `music` key is no longer
read. The assignment is by title, length and mood; it is data, easy to change.

*Fizzwick in play.* `fizz.rs` owns the budget (`POPS_PER_RUN` 3, `POP_GAP` 20 s, nothing in the
first 6 play seconds, one slot held back for the goal, none in Free Roll, priority goal > sixty >
preposterous > landmark > tier, a refused event is dropped and logged, `WOW_AZEROLL_FIZZ_TEST=1`
lifts the gaps and the quiet start) and sends `FizzSays`; `hud.rs` draws the porthole (`hud.wgsl` kind
4: the commissioner stage's render cropped to a brass-rimmed round window) and a typed bubble
under the clock, on real time, sliding in from the right; `hud_models.rs` keeps the commissioner
stage alive in play (`Commissioner::pop` wakes his camera). His voice is the gnome-male text-emote
voice kit (race 7, sex 0) resolved through `EmoteSounds` for CHEER, LAUGH, GASP and SIGH (fallbacks
in `sfx::FEELINGS`; the resolved kits are logged at the first line as `azeroll fizz voice:`); a
feeling with no kit is a silent gesture. All his words are in `lines.rs`.

*The Catalogue.* `progress::CatalogueEntry { level, n, axis, cat }` per display name in the save's
`[catalogue."Name"]` table (absent in old saves). `Run::finds` (axis, category, pickup order) is
filled from `Collected`; `Progress::merge_catalogue` adds only the pickups it has not been told of
(`Run::merged`), so it runs idempotently on pause, restart, leaving for the hub and the results;
Free Roll therefore fills it too, and a name found in two levels keeps its first. The recipe's
denominator is `campaign::catalogue_names` (scenes, herds, cast; map doodads and city pieces are
"found roaming"). The hub shows a foot line and `C` opens `select/catalogue.rs`: three levels with
`N of M`, categories (`names::category`) and a detail pane with Fizzwick's quip. Deviation from the
design: no 3D model in the detail pane (the item bubble clones a live entity's meshes; a model by
name needs a loader this track did not add).

*Text diet (audit #2).* The tier banner is 26 px at 14 % height, the name only, 2.6 s; a size goal
is said once on the dial's ring (`2yd 14in!` at 1.6x, then `Goal 2yd 14in`), a single-target goal as
one 30 px line; the finale title is one line sized by `hud::fit_title` at 12 % height. Amends D20.

*Numbers (audit #5).* During an L3 finale the dial and the finale's subtitle read the D26 display
size (`hud::FINALE_DISPLAY`, 595 yd) from the trigger, so one number is on screen; records keep the
physical size (D24). The results `Size` line already read `Run::final_diameter`.

*Follow-up polish (same day).*
- **Indoor send-off.** The indoor lens tries twelve views in order (`ritual::clear_view`: the three-quarter view, the other side, further round, low and high), keeps the first whose boom the world's colliders leave free (`WorldCollision::cast_camera` from the clump) and otherwise the freest with the boom cut to what it can reach. The lift indoors is 0.6 radii (was 1.0), a warm additive halo (`BeamPart::Halo`) sits behind the clump, and `cutout::publish` centres its see-through window on the *lifted* clump (x1.25 during the ritual) so walls and the ceiling never hide it.
- **Huge clumps (L3).** The lens is framed on the clump itself (`reach = extent + 0.15 lift + crown`), follows 70 % of the rise (`FOLLOW_LIFT`), is capped at 6.5 radii (the finale's own distance: farther and the world's distance fade drops the Cathedral clone) and frames the terminal trophy on top (`crown`). Outdoors the beam and halo are dimmer so pale daylit trophies are not glared out. The Cathedral, the real one below included, is visible in the hover; probes use the city agent's `visual-ab.toml`.
- **Empty stage frames.** The cause of the empty portrait was the cutout, not warm-up: `foliage_window` discarded the stage gnomes' fragments (they are 3000 yd up and their pixels fell inside the window's screen circle). `wow_model.wgsl` now never cuts anything above 2000 yd. Fizzwick's camera also stays awake through play, and a line waits (up to 3 s) until his stage has rendered (`who.alpha >= 0.95`) before the porthole slides in.
- **Banners.** "<Boss> is ready!" moved to 38 % of the height, below the tier banner (14 %), the dial's label and the porthole, so a tier opening and the ready moment never overlap; the goal and tier banners share one slot (the later replaces the earlier).
- `WOW_AZEROLL_RITUAL_TRACE=1` logs what rides the clump and whether it is visible.

*The Finish button (follow-up to the first playtest: "I got to a large ball size early in the inn level
and then I couldn't move anywhere else so just had to wait until the time ran out").* The once-only
"hold Enter to call it" hint is gone. From the goal of a size level to the end of the run
(`hud/finish.rs`; write set `hud.rs`, `hud/finish.rs`, `lines.rs`, `pad.rs`) a brass **Call it!** button sits
under the clock (above Fizzwick's porthole, clear of the dial, the goal title and the banners), with
"hold Enter or click" or, when the last input was a pad, "hold Y" (`pad::LastDevice`, set by real keys
and mouse buttons or by pad activity). A click (press and release over it) ends the run exactly as the
hold does (`finish::call_it`, `Ending::Called`); the Enter hold still works and fills the button as it
fills the clock's bezel. The pause menu shows a **Finish run** button (and Enter finishes) once the goal
is met; Free Roll and single-target goals have neither. *The nudge:* `finish::nudge` counts play seconds
since the goal or the last pickup; at 15 s with no meal cluster in reach (`hud::cluster_of`, or 45 s with
meals in reach) Fizzwick pops in once with a `lines::NUDGE` line pointing at the button (outside the
three-a-run budget, waits for a quiet porthole). Levers: `WOW_AZEROLL_FINISH_NUDGE=<secs after the goal>`
(nudges whatever the player has rolled), `WOW_AZEROLL_FINISH_CLICK=<secs after the button is in>` (moves the
cursor to the button and clicks it through bevy_ui's focus), `WOW_AZEROLL_FINISH_PROMPT=pad|keys` (pins the
prompt).

*Capture and test levers.* `WOW_AZEROLL_CALL_AT=<play secs>` holds the finish key; `WOW_AZEROLL_FIZZ_TEST=1`;
`WOW_AZEROLL_RITUAL_SHOT=<png>` with `WOW_AZEROLL_RITUAL_SHOT_AT=<real secs from the ending>`;
`WOW_AZEROLL_CATALOGUE_SHOT=<png>` (`_LEVEL`, `_ROW`); the hub/result aids of D24 still work
(`RESULT_DEMO` now plays the whole send-off first). The autopilot waits for the dock before its
results screenshot and exit.


**D32 — The gamepad and the dash wind-up (2026-11-03; audit-2 #3 and #9; amends D3, D17).**
Write set: `pad.rs` (new), the input layer of `ball.rs` and `camera.rs`, the Prince's wind-up in
`creatures.rs`, two lines in `mod.rs`.

*Pad.* `pad::read_pad` runs in `PreUpdate`, after `InputSystems` folds Bevy's gamepads in, and
writes `PadInput` (read by `ball::drive_ball` and `camera::place_camera` beside the keyboard, so no new
physics) and the key-equivalents for the menus (it presses the same `ButtonInput<KeyCode>` keys the
keyboard would, so `select`, `level` and `ritual` know nothing of a pad). With several pads connected
the busiest one is read. Twin-stick, as Katamari: each stick is a tank track after a radial 0.18
deadzone; roll is the mean of the two Y axes (so analog throw is speed), turn is half their difference
(0.12 deadzone, so two level sticks roll straight), and both sticks sideways to the same side strafe
(`min` of the two, 0.3 deadzone) at 55 % of top speed (`pad::STRAFE_SCALE`; the ball's wish vector is
`ahead·roll + right·strafe·0.55`, clamped to length 1). Keyboard and pad inputs add, then clamp.

| | |
|---|---|
| both sticks up / down | roll forward / back, speed by throw |
| one up, one down | spin on the spot (left up, right down = right) |
| both sticks sideways | strafe roll |
| sticks swapped up/down 3 times in 1 s | dash gesture: wind-up, then burst; the sticks leave the ball while it lasts |
| `A` (hold, release) | dash; the `A` that resumed from pause or started the run is ignored until let go |
| `X` or click both sticks | quick turn (Space) |
| `L1` / `R1`, d-pad up / down, `L2` / `R2` | camera swing (Q / E), zoom, peek (the camera tilts up and over while held) |
| `Start`, `Y` (held), `Select` (held) | pause (Esc), call a won size run / skip the finale (Enter), unstick (Backspace) |
| menus | d-pad or left stick move (arrows), `A`/`Start` Enter, `B` Esc, `X` R (restart; `C` on the hub opens the Catalogue), `Y` F (Free Roll) or L (leave, on the pause screen) |

Simple mode (`Select` on the pause screen, or `WOW_AZEROLL_PAD_MODE=simple`; not saved): the left
stick steers like a car and the right stick works the camera (X swings, Y tilts). A pad touched while
Ready starts the run, as W does (an Enter pulse).

*Dash wind-up (all inputs).* `pad::DashCharge` is the state machine; `ball.rs` feeds it
`Shift || pad.dash_held` and acts on what it returns. A press arms it (cooldown over, a direction
held or a pad dash). Released within `TAP_SECS` = 0.12 s it is the dash it always was (0.65 s, no
wind-up cue). Held longer, the *wind-up proper* begins: `Dash::WindUp` is sent once (the Prince
scrambles on the spot with the lean, the corner Prince copies him by D20a, the Whirlwind cue plays),
and the ball's top speed falls to 25 % (`WINDUP_SLOW`; braking and building use the unheld pull). Release,
or 0.45 s (`FULL_SECS`), bursts: `Dash::Started`, the dash lasts 0.65 s × (1 + 0.45·charge) and pulls
1 + 0.6·charge harder, charge running 0 at the tap to 1 at full wind-up. The 1.8 s cooldown starts at
the burst. A burst with no direction held (the gesture, `A`) rolls straight ahead. A held input that
has burst must be let go to arm again; leaving play cancels a wind-up. `Dash::Started` ends the
Prince's scramble.

*Dash gesture.* `pad::Wag`: a stick pair counts as pushed at ±0.5 (one up, one down); each change of
side is a push; 3 pushes within 1 s engage it, and it releases 0.45 s after the last push.

*Levers.* `WOW_AZEROLL_FAKE_PAD=<script>` feeds `pad::derive` a scripted pad from the start of the run
(segments `secs:lx,ly,rx,ry[:a+x+...]` joined by `;`, y up; `wag<n>@<period>` is `n` alternating
pushes); `WOW_AZEROLL_PAD_SHOT=<png>` saves a frame once the wind-up is under way;
`WOW_AZEROLL_PAD_EXIT=<play secs>`; each dash message is logged as `azeroll pad:`. Not tested on
hardware: the host has no pad, so the Bevy-to-`Frame` read (button names, axis signs) is the one
unexercised link.

**D33 — The replay layer: parcels, cousins, outfits, new challenges (2026-11-03; audit-2 #15; design
`docs/azeroll/replay-design.md`, built by a Sonnet pass).** Every level hides one **parcel** (a Winter
Veil gift box, `XmasGift02.m2`) and one **cousin** of Grand Tinker Fizzwick. A parcel unlocks an
**outfit** for the Prince, a cousin an alternate **pusher**; both are worn from the Catalogue. All
cosmetic: nothing changes physics, scale or the Prince's animation.

*Data.* `design::Design::present: Option<PresentSpec { id, title, display }>` (`[present]`) and
`Item::present` / `Item::name`: `parse` rejects a recipe whose `[present]` and `present = true` items
do not pair up one-to-one. `ChallengeKind::{Before, Within}` and `ChallengeSpec::{before_species,
before_name, names, species_list, prefix}`. The cast gains `role = "cousin"`, `cousin` (the wardrobe
key), `scale` (multiplies the display's own), `catch = [opens, swallowed-above]` and `caption`.
`campaign::LevelInfo::{present, cousin}` are built from the recipe and the cast (`replay::cousin_in_cast`
reads the cast as plain TOML), so the hub, the Catalogue and the wardrobe resolve keys without parsing a
recipe. The Catalogue name of every parcel is "Fizzwick's Parcel" (`replay::PARCEL_NAME`).

*Save* (`progress.rs`, all `serde(default)`; an old save reads unchanged). `LevelBest::{parcel, cousin,
cousin_fair}` (skipped when false), and a top-level `[wardrobe] outfit / pusher` (keys; skipped when
empty). A key no recipe knows resolves to nothing: the Prince is the default, no error. `Run` gains
`parcel_found`, `cousin: Option<(key, fair)>`, the three `*_merged / *_new` flags and `picks` (every
pickup, in order, with its play time). `Progress::merge_replay` is the Catalogue merge's twin
(idempotent, on pause, restart, leaving and the results), so a find survives whichever way a run ends;
`record_results` fills `Outcome::{parcel, cousin}` from it. A debug start (`persist = false`) sets the
in-memory flags and writes nothing.

*The moment* (`replay::moment`). `Collected` of an entity with `design::Present` (the first time this
run; a shed parcel stays found and re-rolling it is silent) or of a cousin NPC sets the run's flag, the
banner (`PARCEL!` / `COUSIN!`, 2.6 s, the existing banner slot), and writes `ParcelFound` /
`CousinCaught`. `sfx` plays the discovery sting (`TaxiNodeDiscovered`) at full level even after a
size goal is met; `fizz` queues the bonus pop-in (`Bonus::{Parcel, CousinFair, CousinSwallowed}`,
one of each per run, **outside** the three-a-run budget and Free Roll's silence; it waits for a porthole
quiet for `BONUS_GAP` = 4 s and then sets `last_at`, so the 20 s gap still separates the next budgeted
line). The cousin's caption (`CousinSays`) is a gold line projected above their head for 2 s
(`replay::caption`; the design said an NPC caption style existed, it did not). `HumanOnly` is on the parcel
and every cousin and `auto::Loose` excludes it: **the autopilot never targets or counts either**, so
the pacing numbers do not move.

*Cousins* (`npcs.rs`, `Role::Cousin`). The window is `replay::window(D, catch)`: below `catch[0]` the
cousin is `Ambient` and ignores panic contagion; inside, within 6 yd + r they `Notice` (EmoteWave 67 and
the caption), 2 s later `Flee` with `Style::Circle` at 1.3x the villager speed (still below the ball's
top speed at the size they open at), the chase held while the ball is within 10 yd + r and never a
Freeze or a "keep your distance" in a panicking village; above `catch[1]` they `Cower` on sight. The
catch is the engine's own roll-up rule (`Collectible::pickup` is their scaled height), so "too small" is
simply un-rollable. `fair = window(D, catch) != Above` at `Collected`. A carried cousin cheers (CHEER
voice, emote 68) instead of crying.

*The wardrobe* (`replay::Wardrobe { outfit, pusher }`, displays). Rebuilt every `PreUpdate` from
`Progress::wardrobe` and the campaign (`WOW_AZEROLL_WARDROBE=<outfit>,<pusher>` overrides it for
captures). `creatures::resolve_roster` re-resolves the Prince when it changes: **capture override
(`WOW_AZEROLL_PRINCE_DISPLAY`) > pusher > outfit > the lowest male gnome**, each falling through if the
catalog does not know the display; it logs `azeroll prince: display N (override|pusher|outfit|default)`.
`stage_prince` rebuilds the corner gnome whenever the world Prince's display differs from the one it
was built with, so the two never disagree and the corner still mirrors his motion (D20a).

*Challenges* (`progress.rs`). `before_met` (set complete, and the gate's first pickup after the set's
last, or the gate never rolled) and `within_met` (any window of `secs` seconds holding every wanted
name's count) are pure over `Run::picks`; `scene` gains `prefix` (the union of scenes) and `collect-all`
gains `names` (display names; a city piece counts once, read from `stormwind-pieces.toml`) and
`species_list`.

*UI.* The hub card gets a finds line under the verdict (`Parcel [X]   Cousin [X]`, the font has no check glyph; a not-yet
find is a dim `·` and no word; `select::hub::finds_line`). The results tally gets
`PARCEL FOUND: the Monocle   NEW!` and `COUSIN CAUGHT / SWALLOWED WHOLE: Pippa Fizzwick   NEW!`
(`verdict::parcel_line / cousin_line`). Hub news: `Celebration::{parcels, cousins, all_parcels,
all_cousins}`; Fizzwick types the matching `lines::NEWS_*` line in his bubble until the highlight moves.
The Catalogue (`select/catalogue.rs`) has a family group above the level list that never scrolls:
the Prince, an outfit per parcel and a cousin per cast row, dim `· · · (numeral)` until found.
`Enter` wears an outfit (and sends the pusher home), picks a pusher, or on the Prince restores
everything; Enter on what is already on takes it off; Fizzwick's reply (`lines::equip_line`) types in
the detail pane (the Catalogue covers his hub window). Trinkets: the **Royal Fireworks** (a gated
burst block in the hub scene shader, `lv[GLOBE + 3].x` = seconds, on every hub arrival once all parcels
are found; `WOW_AZEROLL_FIREWORKS=1` forces it) and the **Family Portrait** (`hud_models::stage_family`:
the cousins on Fizzwick's stage, once all are found; `WOW_AZEROLL_FAMILY_PORTRAIT=1` forces it).

*Levers.* `WOW_AZEROLL_PRINCE_DISPLAY=<id>`, `WOW_AZEROLL_WARDROBE=<outfit>,<pusher>`,
`WOW_AZEROLL_FIND_TEST=parcel|cousin` (after the autopilot steers, aim at the parcel or cousin; the
autopilot still starts, shoots and exits), `WOW_AZEROLL_SHOTS` also saves `moment-*.png` 0.7 s after a
find, `WOW_AZEROLL_FIREWORKS`, `WOW_AZEROLL_FAMILY_PORTRAIT`; the L1 generator's `LP_FIX=parcel|lip|cellar`
`LP_R=<radius>` writes a start-moved copy to `/tmp/az-replay/` for `WOW_AZEROLL_LEVEL`.

*Placements.* L1: see its section ("The parcel in the fireplace", Pippa). **L2 (Market Day):** the parcel
(Mirela's Bandana, display 4873) lies at the back of a crate pocket in *The coop by the signpost*: eight
`ReplaceCrate01` crates (`solid`, opening at D 1.9, so food from then on): two side walls, a back wall and two
jambs 1.5 yd apart. Measured, not designed: with the jambs 1.84 apart and no back a D 1.24 ball got in, because
a crate's collision is narrower than its box; at 1.5 apart and closed, D 0.94 enters and D 1.2 is refused
(`/home/jjd/az-patches/replay/`). Twinkle Fizzwick (display 5378, scale 0.5, real axis 0.58 yd, `catch = [1.25,
1.88]`) bobs for apples beside the barrel at (−9472.2, 82.9) (the barrel is at (−9473, 84.5), not where the design
guessed). Challenges `hogger`, `chickens`, `garden` (`within`, 60 s) and `parade`; `speed`, `supper`,
`unrammed` dropped. Primary autopilot run on the refreshed recipe: **won at 223.4 s of the 620 s clock (36.0 %),
0 Unstuck**, so the clock stays at 620. The autopilot's path swallowed Twinkle and found the parcel
incidentally (both are `HumanOnly`: never targeted, but nothing stops a roll-up on the way). **L3 is not placed**
in this build: its snippets are in `docs/azeroll/replay-placements.md`.

*Capture-found fixes (all in the tree).* The hub news line latches the highlight on the first frame
(`Anim::news_at`; the hub picks its level after `arrive`); the finds line uses `[X]` (the WoW font has no
check glyph); the capture harness's `WOW_AZEROLL_HUB_KEYS` only fire with `WOW_AZEROLL_HUB_SHOT` set;
`WOW_AZEROLL_FAKE_FINDS=parcel,cousin[,swallowed]` fakes the finds of a `RESULT_DEMO` / `LOOP_SHOT` run.
Known: the family portrait shows one cousin peeking in Fizzwick's arch (the window is narrow); the fireworks
are a few small bursts. Not exercised live: Enter in the Catalogue (unit-tested only; the harness presses no
keys there) and a real Thrum (L3).

**D34 — Rolling polish: edge-following support, a render-only vertical blend, no wall-lean teleports (2026-11-03;
the playtest's "glitchy rolling around Stormwind"; amends D6, D10, D27).**
*Instrument.* `glitch.rs` (`WOW_AZEROLL_GLITCH=<csv>`, off by default) writes one row per played frame: the
centre after the mover, collection's push-outs, Unstuck and the city deck guard (the post-Stream building
settles show as the next frame's start), velocity, the support that settled the ball (terrain, an engine floor
classified as a permanent deck / a live city piece / other WMO, or a prop top), the step lift, wall closing,
the camera and the real frame time. `/home/jjd/az-patches/glitch/analyze.py` classifies discontinuities in units
of r (the camera frames the ball at a near-fixed size, so r is what reads as "a lot"): a drawn vertical jerk
over 0.04 r, a non-mover correction over 0.03 r, a speed collapse, a frame over 50 / 100 ms.
*What read as glitchy* (baseline, the run-8 recipe pinned: a normal-start autopilot win in 8:04, scripted
`WOW_AZEROLL_FAKE_PAD` drives through the gate and Trade District at r 3 and across the Square at r 25, and the
autopilot from those starts), ranked:
1. **Staircase climbs.** The 73 support rays sit on fixed rings (0, 0.3 … 0.99 r), so any curb, deck seam,
   bridge lip or roof edge was climbed in jumps whenever the next ring crossed it (0.1–0.2 r a frame, then
   nothing), not along the sphere's contact circle: 352 drawn pops in the normal run (85 over 0.15 r), 119 in
   80 s of pad driving, on every surface type (deck→deck curbs at the gate, the canal-ramp walls at raw
   (−160, 10), city pieces at r 25+). Prop tops (D29 `box_rest`) used the same rings.
2. **Wall-lean teleports.** Unstuck's wedge timer fired after 4 s of input without progress, so holding forward
   against a wall rewound the ball: 6 Unstucks in each 80 s pad drive, and in the normal run an r 64 ball
   leaning on the canal wall was thrown 271 yd back out to the outer gate (the 4 r separation) and spent 90 s
   hopping on the gate ramparts before it found the city again.
3. **Snaps the mover cannot avoid.** First contact with a high ledge at city scale (step height eases to
   0.95 r), a surface rising under a ball that was airborne for a few frames off a roof edge (14 yd at r 54),
   growth revealing a roof inside the step window, the building pickup's settle onto its validated floor (up
   to 2 + 0.5 r down, D27) and the rare deck-guard lift.
4. **A dead stop on every building eaten** (`city.rs` zeroed the velocity at prepare and acknowledge).
5. **A buzz leaning on a wall below a ledge:** the pre-sweep lift raised the ball toward the ledge, the wall
   kept it there, the settle dropped it: 0.036 yd every other frame.
6. **Hitches are the autopilot's, not the game's.** The pad drive through the same Trade District streets never
   exceeded 28 ms; the autopilot there had 13–15 frames over 100 ms (its synchronous indoor A*, a 1 s deadline
   per candidate). Two hitches followed Unstuck teleports (a new area streams in).
*Fixes.* `ball::ring_support` (shared by `floor_support` and `furniture::box_rest`) keeps the rings but, along
each spoke, where a sample stands higher than the plane of the sample inside it predicts (an edge, not a
slope), four bisection samples find the edge, so the centre follows the contact circle (unit test: within
0.03 r of the true sphere-on-ledge height at r 0.5 / 3.4 / 25; a plane costs no extra samples). Edges that cannot
raise the support are skipped, so the result is exact; on the installed gate geometry the call costs 0.021 /
0.025 / 0.055 ms at r 1 / 3 / 12 against the fixed rings' 0.021 / 0.026 / 0.048
(`city_support::tests::installed_gate_support_cost`, `--ignored`).
`ball::Shown` is a render-only blend: the drawn root and the camera's look point follow the physical centre, but
a grounded pop beyond 0.02 r + 0.01 yd a frame, or an airborne move faster than both its own fall and its
ballistic `vy·dt`, is taken out and eased back at 12/s (95 % in a quarter second); falls, landings, bounces and
steady slopes are drawn as they are, a teleport resets it. Collection, support and every planner keep reading
`BallState::pos`. Unstuck's automatic rewind now needs the ball boxed in (the mover's 0.85 r sphere cannot
travel 0.4 r in any of 8 directions into room clear of the mode's solids); a ball leaning on a wall it could
turn from logs "free to turn; no rewind". Backspace is unchanged. The autopilot, which cannot always find the
way it could turn (L1: 110 s leaning in the inn), gets the Backspace a frustrated player would press after
12 s of leaning; it is still a counted Unstuck. The rewind separation is
max(12, min(4r, 24 + r)) yd. Eating a city building keeps the horizontal momentum. (Undoing a small step
lift whose sweep made no headway was tried for the buzz in item 5 and did not change it; it is not in the tree.)
*Results* (`/home/jjd/az-patches/glitch/`, `compare.txt`; drawn = what is rendered):

| run (final binary) | drawn pops (>0.15 r), before → after | Unstuck | frames >100 ms | result |
|---|---|---|---|---|
| normal start | 352 (85) → 17 (1) | 1 → 0 | 42 → 15 | won 8:04 → won 6:22; frame p50/p95/p99 16.7/22.9/43.4 → 16.6/18.9/24.8 ms |
| pad drive, gate → Trade District, r 3 | 119 (32) → 4 (1) | 6 → 1 | 0 → 0 | the one left is a real pocket (below) |
| pad drive, Square, r 25 | 91 (32) → 6 (0) | 6 → 0 | 1 → 4 | (an earlier fix binary) |
| autopilot, Square r 25 → finale | 79 (37) → 19 (0) | 0 → 1 | 0 → 0 | won 46 s → won 78 s (routes vary from this dressed start) |
| L1 / L2 | 32 (3, two are Unstuck cuts) / 1 (0) | 2 / 0 | 137 / 0 | both won; L1's two 13–15 yd rewinds are the autopilot's leaning fallback after the goal |

L1's hitches are the autopilot's indoor A* in the inn (an earlier pass's L1 tour logged 36 frames over 100 ms,
max 9.4 s); the before binary of this pass cannot boot L1 (its embedded Stormwind recipe was mid-edit), so the
support cost was compared head to head instead (above). `analyze.py`, `compare.txt`, `drawn-vertical.png` and the
per-frame traces are in `/home/jjd/az-patches/glitch/`.
Known: a real pocket at raw (−164, 19), WoW (−8791.6, 626.3), floor z ≈ 96, holds an r 3 ball still (every
direction blocked; Unstuck rescues it); the autopilot's indoor A* still blocks frames (L1's inn especially);
Space's quick turn still cuts the camera 180° in one frame (D8 yaw follows the heading); the buzz of item 5
(about 0.036 yd at r 3, 1.2 % r) remains; a boxed-in r 27.7 pocket at WoW (−8572.9, 756.2) on the north flat
cost the r 25 autopilot one rewind.

**D35 — The placement validator and the placement pass (2026-11-03; after the playtest "the books don't
lead up properly onto the bar").** `placement.rs`, enabled by `WOW_AZEROLL_PLACEMENT` (table above),
waits until the recipe has settled (`design::Pending::settled`) plus 240 frames for claimed furniture,
then checks every spawned recipe prop through the game's own spawn result (`design::Authored`, the
component the spawn now attaches: scene/item/index, the authored hint height, the surface the spawn found,
the lift) against the live world:

- *(a) surface*: indoors, the spawn ray's hit (terrain, WMO or doodad hull) against the intended floor (the
  scene anchor's z, or the item's new `on = <z>` for a group meant for a counter or a bar top); over 0.3 yd
  is a warning, over 1.5 (a beam, another storey) a failure. Outdoors (terrain-sampled), any surface
  above the terrain under the prop: a roof or a hull it was dropped into.
- *(b) collision*: the prop's box, clipped to above the surface it stands on, against the body world at
  half (failure) and 0.8 (warning) of its width; terrain hits are ignored (a sloped prop always touches its
  uphill edge; a WMO walk collider is told from a terrain tile by its non-default `CollisionLayers`).
  Blockers are exempt (the door keg is wedged on purpose). Also: the centre inside another solid prop's box
  (furniture by its slab only).
- *(c) support*: the bottom against what is under it (the world from the prop's own top down, or another
  solid's top): floating more than 0.15 yd beyond its authored layer/topple lift, or sunk by more than
  30 % of its height (60 % fails).
- *(d) ramps*: every `stairs` item and pitched solid plank. Static checks (first step on the floor at its
  base, every riser within `ball::step_height` of the item's `climb` radius, default the start radius,
  no step edible at that radius), then a walk: a ball of that radius rolls from the floor before the foot to
  past the high end with the mover's rules (`ball::floor_rest` plus every solid's `collect::solid_rest`,
  the step-up limit, the 0.85 r sphere sweep against the world, and solid props it cannot step onto
  pushing it back). It must arrive on a surface higher than the foot; if it falls off the end, the report
  names how far past the high edge the nearest surface at its height begins.

Two mover facts the live scripted climbs exposed, now encoded: a target surface standing just above the
last step (within 0.2 r) stalls the ball at its edge (the 0.85 r sweep meets a bevelled edge before any
step-up; the captain's table at 57.09 over a 57.07 step), so **a ramp's last step stands at least 0.05
over the surface it serves**; and a solid beside a stair's foot (a chair) pushes a small ball off the
first step. Two spawn changes fix whole classes of the failures: `seat` lifts a small (< 2 yd), loose,
non-solid, non-rooted recipe prop whose box hangs below its origin onto its floor (the gold helm's origin
is the head it was worn on, the half loaves' their middle: they sank 50–90 % into every floor), and an
item's new `level = true` samples the ground once at the group's offset instead of under each prop, so a
table-top group on sloping ground stands on the table like the table itself (L2's generator `top()` uses
it with a `table-set` square inside the table's edges; clusters dropped stragglers off the tables).

Results (reports in `/home/jjd/az-patches/placement/`): L1 from 352 failures (first, uncalibrated run)
to 0, all 5 ramps climbed live at r 0.2; L2 from 61 (calibrated) to 2 explained (a 5 yd cage on a slope,
a trail helm inside a map haystack), all 7 ramps pass the walk; L3 is not edited in this pass (its own
polish agent): 279 failures listed in `l3-failures.md`, mostly large props spanning the Cathedral
stair's risers. A height-grid probe (`WOW_AZEROLL_PLACEMENT_GRID`) surveyed the real geometry for the
fixes; the first L1 recipe's survey numbers (bar edge, the "hearth counter", the kitchen) were off by up
to 1.7 yd.
