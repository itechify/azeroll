# Engine integration for an Azeroll sibling mode

Source-only research of this checkout. No Cargo, downloads, or runtime validation. References are repository-relative `path:line` (ranges identify contiguous implementations). Recommendations and unknowns are explicitly marked. Orientation: `CLAUDE.md:1` delegates to AGENTS; `docs/METHOD.md:9–15` places non-reference gameplay in a fork/layer; `docs/MAP.md:1–4` is a generated map, not an authoritative API specification. Deep doodad/WMO/M2 implementation is outside this note.

## 1. Boot path and integration hooks

`crates/benilla-app/src/lib.rs:230–233` is the entire mode entry:

```rust
pub fn run_survivors(build: BuildId) -> AppExit {
    survivors::ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    launch(build, None)
}
```

`launch` sets the launcher's project folder, installs native crash reporting, creates `App`, inserts BuildId and static-transform optimization, and configures Update/PostUpdate executors (`lib.rs:263–299`). It registers the MPQ source before DefaultPlugins (`lib.rs:325–338`), adds tuned DefaultPlugins with the mode window title (`lib.rs:343–414`), WorldPlugins, dev tools and FPS journal (`lib.rs:415–432`), then GamePlugins, the mode plugin, WebBridgePlugin and asset loaders (`lib.rs:435–450`). Optional extension and probes precede `app.run()` (`lib.rs:477–488`). This is the complete client/plugin stack with selected authorities disabled, not a small standalone Bevy world.

Activation is **presence**, not a parsed boolean (`crates/benilla-app/src/survivors/mod.rs:26–34`):

```rust
pub(crate) fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
        || crate::webenv::var("WOW_SURVIVORS").is_some()
}
```

Native `webenv::var` is `std::env::var(name).ok()` (`webenv.rs:14–19`). On wasm it strips `WOW_`, lowercases, and reads snapshotted `window.__wenilla_env` first, query string second (`webenv.rs:21–44,53–69,75–101`): `WOW_SURVIVORS` → `survivors`, so `?survivors=1` works (and `?survivors=0` also counts as present). **The current wasm launcher already calls `run_survivors` unconditionally**, so no query flag is needed there (`crates/wenilla/src/lib.rs:21–39`). Native launcher does likewise (`crates/warcraft-survivors/src/main.rs:5–16`).

The mode inserts `CaptureMode` during plugin build, before initial world entry (`survivors/mod.rs:38–42`). Its effects are distinct from network configuration:

| Authority / behavior | Owning implementation |
|---|---|
| FrameXML world UI unwanted unless a dev capture explicitly opts in | `ui_script/lifecycle.rs:123–128` (`ui_wanted`); opt-in defaults false in a non-dev build: `run_mode.rs:29–40` |
| Stock player `control`, server-driven self ride | `player.rs:265–293` |
| Camera shake application and emission | `player.rs:318–324`; `camera_shake.rs:158–166` |
| Stock world right-click action, self-model fading | `player.rs:340–369` |
| Saved camera-view requests | `player/camera_view.rs:307–318` |
| Additional capture-specific UI extraction behavior, if UI exists | `ui_script/mod.rs:363–367`; `ui_script/extract/mod.rs:686,748,824,909,969,979` |
| **Network off separately**, not by CaptureMode | `lib.rs:435–442`: `connect: !capturing && !survivors`; `net.rs:64–72` creates offline handles and inserts `NetOffline`; `net/io.rs:227–243` guards connection startup with `if connect` |
| **Boot directly InWorld separately** | `lib.rs:437–441`; `game_plugins.rs:130–142` passes the settings into NetPlugin and CharSelectPlugin |

CaptureMode is not a universal stop switch: focus publishing, embodiment upkeep, scoped FOV, etc. remain registered (`player.rs:221–251,301–315`). Do not assume the engine stops running other unit systems.

### Exhaustive explicit Survivors hooks outside its directory

Repository-wide case-insensitive search, including hidden tracked-style configuration, found these integration sites (all app-source paths below begin `crates/benilla-app/src/`):

- `lib.rs:110`: module declaration; `230–232`: public entry/ACTIVE setter; `305–308`: **only outside call of `survivors::active()`**, forcing start map **1**; `344–348`: title; `435–445`: offline connection, InWorld start and plugin addition.
- `crates/warcraft-survivors/src/main.rs:6`, its `Cargo.toml:1–24` and `build.rs:3–5`: native shim/build stamp; `crates/wenilla/src/lib.rs:30`: wasm shim. `.cargo/config.toml:6–7` and `Makefile:29`: native launch shortcuts. Workspace members already glob `crates/*` (`Cargo.toml:1–3`).
- `shaders.rs:16–17` embeds the mode grade/fill shaders; corresponding WGSL files live in `shaders/survivors_grade.wgsl:1` and `shaders/survivors_fill.wgsl:1`. Optional effects, not required for a basic sibling HUD.
- `glue/backdrop.rs:171–179` has Survivors-aware explanatory text and a **generic** UiScale correction used by its Bevy UI (`screen_scale / ui_scale`). Preserve/reuse it if using glue backdrop builders; no active-mode check.
- `web/index.html:3,67` and `web/boot.js:245`: branding and comment explaining that bridge subscriptions default off. No additional mode dispatch here; dispatch is the Rust wasm entry above.
- False positives, **not mode hooks**: `target/hover.rs:285–287` means objects surviving broad phase; `benilla-world/src/particles.rs:300` means live particles; `benilla-world/src/terrain_stream/collider/web_budget.rs:145` is a cancellation test name.

## 2. Server-less world entry and streaming

`MapDef` contains map id, WoW-space center, leash and embedded creature tables (`crates/benilla-app/src/survivors/data.rs:1388–1407`). Selection distinguishes the pending menu map from the actual `stage` (`survivors/mod.rs:165–193`); `hud::start_run` copies `selection.map` into `selection.stage` (`hud.rs:898–907`). `hero::follow_map` directly mutates `CurrentMap.0` to `stage_def().map` (`hero.rs:83–94`). No server or fabricated login packet is required.

Existing Elwynn entry (`data.rs:1566–1573`):

```rust
MapDef {
    name: "Elwynn Forest",
    zone: "Eastern Kingdoms",
    map: 0,
    center: [-9548.5, 115.0, 60.0],
    leash: 150.0,
    // ...
}
```

**Recommendation for Goldshire:** set `benilla_world::world_map::set_start_map(0)` before plugins/startup and use center `[-9464.0, 62.0, 56.0]` supplied in the task; conversion gives Bevy `(-62, 56, 9464)`. This exact Goldshire location/height is not verified by this source-only research. The API stores an atomic override that takes precedence over native `WOW_MAP`; map catalog and CurrentMap load after `AssetSet::Open` (`crates/benilla-world/src/world_map.rs:14–34,51–59,95–96`). Do not inherit Survivors' hardcoded initial map 1 (`lib.rs:305–308`).

Streaming is driven through **Player resource position**, even though the hero is a local creature. `drive_hero` publishes (`hero.rs:309–314`):

```rust
player.active = true;
player.detached = false;
player.settling = false;
player.world_stale = false;
player.pos = focus;
```

With no hero it uses the arena center and optional ground height (`hero.rs:301–307`); `menu_preview` spawns the local hero at the arena with the same grounding (`hud.rs:856–895`). Stock `publish_view_focus` runs after Input and before Stream (`player.rs:221–225`), converts Player.pos back to WoW coordinates and selects `ViewFocus::body(wow, paced)` (`player/world_focus.rs:79–96`). `stream_terrain` resolves that focus/map, computes a terrain window, and requests/drops tiles (`crates/benilla-world/src/terrain_stream.rs:444–464,492,557–577`). `CurrentMap` changes also produce `MapChange`, clearing map-scoped caches (`world_map.rs:65–79`).

The loading cover has a server-less backstop: InWorld + body focus + nonresident focus raises it (`crates/benilla-app/src/loading_screen.rs:518–527`). Clearing requires `WorldLoadProgress::is_ready()`, `presentable()`, no pending snap/settle/UI load, warmed pipelines, and sustained ready frames (`loading_screen.rs:548–559`). It therefore works without a WorldportMessage. Survivors gates gameplay on `!LoadingScreen::covering()` (`survivors/mod.rs:85–88`), **but drive_hero itself is not gated by the cover** (`mod.rs:60–69`; `hero.rs:210`). Recommendation: explicitly gate Azeroll movement/timer/collection until ready, while still publishing its focus.

## 3. Local units, animation, parenting

`survivors/units.rs:58–113` is the useful spawn adapter:

```rust
fn spawn_unit(commands: &mut Commands, spec: &UnitSpec, pos: Vec3, yaw: f32) -> Entity
// spawned components (excerpt):
Guid(guid),
NetEntity { kind: EntityKind::Unit, display_id: Some(spec.display), scale: spec.scale },
ObjectStore(ObjectFields::from_pairs(&pairs)),
MovementState::default(),
crate::net::ScriptedFacing,
Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
Visibility::default(),
RunEntity,
```

`UnitSpec` supplies unique serial, display, scale, level, faction, packed race/class/sex/power, weapons and sheath (`units.rs:58–69`). Fields include type mask `0x9`, scale bits, health/max health, level/faction and virtual weapons; GUID is `0xF130 << 48 | serial` (`units.rs:29,73–95`). Hero adds Hero and buff Marks (`hero.rs:96–128`); enemies add gameplay Enemy and debuff Marks (`enemies.rs:96–144`). These are **creature display IDs**, not creature entries; no raw model-path argument is accepted by this adapter.

The shared attach query needs NetEntity and no VisualAttached/DespawnFade (`entities/attach/mod.rs:223–245`). Creature displays resolve via `CreatureCatalog::model` and `asset_server.load(m2_url(&m.model_path))`, with skin variations and character-NPC appearance (`entities/display.rs:201–220`). A raw M2 path must go through a lower-level model assembly path, outside this spawn adapter; detailed implementation is left to the model researcher. Generic GameObjects have a parallel helper keyed by GameObjectDisplayInfo, not CreatureDisplayInfo (`survivors/units.rs:115–147`).

Attach writes **root Transform.scale = net.scale**, not `net.scale × DBC scale` (`entities/attach/mod.rs:724–731`):

```rust
t.scale = Vec3::splat(net.scale);
```

Thus local callers must choose the intended final scale. The catalog's DBC scale is available but not automatically multiplied into an in-world unit (see §8). Render parts are attached with `ChildOf(dress.unit)` and local identity transforms (`entities/attach/dress.rs:267–300`).

Animation is data-driven: movement sets `MovementState.speed`/`flags` (`hero.rs:244–259`); forward/strafe bits invoke Walk 4, Run 5 above twice walk speed, Sprint 143 at 11 yd/s, each with fallback IDs (`creature_anim/select.rs:383–393`); Stand is 0, Death is 1 (`select.rs:17–19`). `kill_unit` merges health field 22 = 0 (`survivors/units.rs:149–154`); animation driver reads death from ObjectStore and arms Death (`creature_anim/driver.rs:480–485,570–604`). Local spell visuals use normal messages: `Fx::cast` sends CastEvent and SpellGoTargets; `Fx::impact` sends an Impact CastEvent (`survivors/units.rs:299–343`). Aura Marks merge store slots **and emit FieldChanged** (`units.rs:264–295`), so mutating only a gameplay health/buff resource is insufficient for those visual consumers.

Cleanup: direct `commands.entity(e).despawn()` for reset and preview replacement (`survivors/mod.rs:399–409`; `hud.rs:875–877`); corpse cleanup waits 2.5 seconds then inserts DespawnFade (`enemies.rs:391–409`). GUID serials deliberately survive resets because GUID-keyed caches outlive despawns briefly (`mod.rs:411–415`).

### Can a collected unit be parented to the ball?

**Rendering hierarchy supports it, but the full unit stack is not safe unchanged.** This is a source-based assessment, not a runtime-tested guarantee:

- Mesh children already use ChildOf; attach does not require the unit to be a root (`entities/attach/dress.rs:299–300`; `entities/attach/mod.rs:228–245`). Animation LOD uses the rig's **GlobalTransform** (`creature_anim/lod.rs:46–56`), so its distance/frustum position follows hierarchy. The skin-palette pass also reads joint/root GlobalTransforms after joint updates (`benilla-world/src/rig_palette.rs:616–639`).
- Ground clamp handles every Unit unless another position authority applies; it reads/writes **local Transform as world coordinates**, without a ChildOf exclusion (`net/motion/spline.rs:419–433,475–497,513–545`). Parenting an ordinary Unit therefore risks grounding its local offset against terrain near world origin. ScriptedFacing does not exclude it (the clamp query has no such test).
- Terrain-conforming visuals sample `unit_tf.translation` as world position (`entities/conform.rs:72–77,100–112`). Positioned spell sounds also use local Transform (`sound/spell.rs:20–27,52`); server sound sources do likewise (`sound/zone.rs:1113,1140–1145`). Some spell targeting uses GlobalTransform (`spell/targeting/mod.rs:74,148–153`), so the stack is mixed.

**Recommendation:** keep an unscaled translation root for ball world position and a rotating child for captured visual offsets; gate/remove gameplay movement and ground/conform behavior after collection. A generic marker exclusion in the relevant engine query is cleaner than abusing Embodied or flying/swimming flags. Another option is a visual-only captured copy; whether its rig/material bookkeeping can be cleanly detached needs runtime validation. Do not claim arbitrary NetEntity parenting is already fully supported. Set collected local transforms after any attach that writes scale, and avoid unintentionally scaling all captured offsets when ball size changes (`entities/attach/mod.rs:724–731`).

## 4. Terrain, water and collision

The terrain height API consumes **Bevy space** and returns Bevy Y; unloaded/missing terrain returns None (`crates/benilla-world/src/world_point.rs:167–170`):

```rust
pub fn terrain_height_under(&self, pos: Vec3) -> Option<f32>
```

There is no terrain-normal method in WorldPoint/SurfaceUnderfoot's public point-query surface (`world_point.rs:162–175`; `surface.rs:68–80`). Use a physics down-ray for a normal, or estimate a heightfield gradient (recommendation). The heightfield and physics mesh can disagree (`world_point.rs:1–5`).

Water APIs use **WoW space** and a subject/room claim (`world_point.rs:83–115`):

```rust
pub fn liquid_at(&self, who: Subject, wow: [f32; 3]) -> Option<LiquidHit>
pub fn water_surface_at(&self, who: Subject, wow: [f32; 3]) -> Option<f32>
// Subject::{Player, Eye, Unit(Entity)}: world_point.rs:22–32
```

Physics is reusable by non-player systems: `WorldCollision` is a SystemParam and accepts arbitrary shapes/points, not a Player entity (`crates/benilla-world/src/collision.rs:114–118,160–169,207–230`):

```rust
pub fn cast_body(&self, shape: &Collider, from: Vec3,
                 movement: Vec3, skin_width: f32) -> Option<MoveHitData>
pub fn ray_body(&self, origin: Vec3, dir: Dir3,
                max_distance: f32) -> Option<RayHitData>
pub fn slide_body(&self, shape: &Collider, shape_position: Vec3,
                  velocity: Vec3, delta_time: std::time::Duration,
                  config: &MoveAndSlideConfig,
                  on_hit: impl FnMut(MoveAndSlideHitData) -> MoveAndSlideHitResponse)
                  -> MoveAndSlideOutput
```

Body filters include terrain, doodads/GameObjects and WMO walking colliders, with WMO DETAIL faces excluded (`collision.rs:1–5,20–35,120–127`). `RayHitData` includes world normal and distance (`collision/one_sided.rs:213–238`). `cast_camera(from, movement, liquid)` returns the permitted boom distance (`collision.rs:184–204`). These work against **loaded colliders**, not arbitrary unloaded map geometry.

Survivors hero movement itself adds direction × speed × dt, leashes XZ, samples only terrain, and assigns `pos.y = ground + hm.air` (`hero.rs:244–289`); it has no wall slide or WMO floor query. A separate generic creature clamp runs too (see §3), but this is not a complete collision controller. Azeroll needs its own sphere sweep/slide and floor policy if buildings/props should block it; for a radius R ball, ground height + R is a starting placement policy, not an existing Survivors behavior.

## 5. Camera and coordinates

Conversions are exact (`crates/benilla-app/src/survivors/units.rs:41–47`):

```rust
// WoW (x,y,z) -> Bevy (-y,z,-x); inverse (-bevy.z,-bevy.x,bevy.y)
Vec3::new(-p[1], p[2], -p[0])
```

Bevy Y is up, XZ is ground; yaw uses `(-dir.x).atan2(-dir.z)` (`units.rs:49–55`). Distances/speeds are yards and yards/s, explicitly documented by hero jump/camera constants and world query APIs (`hero.rs:16,27–41`; `world_point.rs:73–78`).

`CamRig` owns distance, menu orbit, yaw, pitch and Fixed/Orbit mode; defaults distance 30 and fixed pitch **61°** (`hero.rs:38,53–73`; an enum comment says 58°, but code wins). `place_camera` uses Time<Real>, mouse wheel and accumulated mouse motion; C toggles modes, wheel multiplies/clamps distance to 1.5–70 yards (`hero.rs:333–368`). The play placement is (`hero.rs:384–396`):

```rust
let look = focus + Vec3::Y * 1.4;
let mut eye = look + Vec3::new(cp * sy, sp, cp * cy) * rig.dist;
// terrain-only eye floor at h + 0.6
let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
```

Recommendation: derive rig distance and look-height from ball radius, retaining a separate world-space center. Current Survivors camera only avoids terrain, not walls; use WorldCollision::cast_camera for that (§4). The stock camera is created with Projection::Perspective and `CAM_FOVY = PI/4` (45° vertical radians), near/far settings (`player/setup.rs:93–107`; `benilla-world/src/view.rs:337–340`). To change FOV, mutate `Projection::Perspective(p).fov` as the stock system does (`player/scoped_view.rs:67–74`). **That stock scoped-view system runs in Input even under CaptureMode and rewrites FOV to the default when no aura applies** (`player.rs:241–251`; `scoped_view.rs:48–74`): order your override after it, expose a small scheduling seam, or keep FOV fixed and grow distance. Camera pose is copied into GlobalTransform early during Update; normal hierarchy propagation is PostUpdate (`benilla-world/src/view.rs:342–350`), so place the camera in Input as Survivors does (`survivors/mod.rs:60–69`).

## 6. Input, including wasm

Use ordinary Bevy resources: `Res<ButtonInput<KeyCode>>`, `Res<ButtonInput<MouseButton>>`, `Res<AccumulatedMouseMotion>`, and `MessageReader<MouseWheel>` (`hero.rs:188–200,334–345`). WASD/arrows become XZ vectors, normalized to avoid diagonal speedup; Orbit rotates movement by camera yaw (`hero.rs:210–227,244–247`). Space jumps (`hero.rs:274–284`). Enter/Escape drive menu/play/pause/results via `NextState<Phase>` (`hud.rs:1050–1079`).

The same Rust input code runs on wasm; tuned winit DefaultPlugins bind `#benilla`, fit it to its parent and prevent default browser event handling (`crates/benilla-world/src/boot.rs:9–21`). Keep the canvas focused: `web/platform.js:63–64` explains that focused buttons eat Space/Enter. Survivors explicitly releases held mouse buttons on WindowFocused(false) or CursorLeft to prevent stuck drag (`hero.rs:317–330`). This is worth mirroring for browser play; there is no mode-specific JS keyboard bridge to replicate in these paths.

## 7. HUD/UI

HUD is **Bevy UI**, not FrameXML: Node, Text/TextFont/TextColor, ImageNode, Button/Interaction and GlobalZIndex (`crates/benilla-app/src/survivors/hud.rs:205–218,246–273,295–308`). Optional custom materials implement cooldown sweeps and gradient fills; their UiMaterialPlugins are registered in `survivors/mod.rs:55–56`. A timer/size/counter HUD needs neither custom shader.

Fonts come from the install via AssetServer (`hud.rs:289–291`; `char_select/mod.rs:852–855`):

```rust
let font = crate::char_select::wow_font(&asset_server); // mpq://Fonts/FRIZQT__.ttf
let number_font: Handle<Font> = asset_server.load("mpq://Fonts/ARIALN.ttf");
```

BLP interface art is loaded through `WorldAssets::sprite_texture` into Assets<Image>, then put into ImageNode, with crop rect and tint (`hud.rs:1466–1485`):

```rust
let Some(h) = assets.sprite_texture(art.path, &mut images) else { /* warn */ continue; };
img.image = h;
```

That system waits for WorldAssets; spell icons similarly resolve the DBC icon path then load a BLP (`hud.rs:1371–1391`). Native-size layout uses `UiScale = window.height() / 768.0` (`hud.rs:1489–1498`), and generic glue backdrops compensate for it (`glue/backdrop.rs:171–179`).

**Recommended minimum:** one startup root Node with Text children tagged Timer/Size/Count; Update changes Text from a mode Run resource, following `refresh_hud` (`hud.rs:1213`) and `refresh_results` (`1647–1678`, timer formats MM:SS). Separate results root + button; use Interaction changes/NextState following `hud::buttons` (`hud.rs:1022–1048`) and keys (§6). Hide panels with **both Visibility and Node.display = Display::None**, otherwise invisible buttons can still take clicks (`hud.rs:2186–2209`). No Lua/addon load is needed with CaptureMode (§1).

## 8. Creature data and real Elwynn population

Survivors' zone tables are **embedded curated constants**, not runtime server spawn data: `MAPS` embeds Foe/name/display/level/speed/scale (`survivors/data.rs:1357–1407,1566–1600`). ROSTERS lazily derive gameplay HP/damage/XP from tuned ladders; display/scale are copied directly (`data.rs:1958–1977,1980–2018`). Scales are chosen for Survivors readability, explicitly targeting particular drawn heights (`data.rs:1364–1366`); they are not a reliable source of canonical creature size.

Available Elwynn examples, directly transcribed (all `crates/benilla-app/src/survivors/data.rs`):

| Creature | Display ID | Mode scale | Line |
|---|---:|---:|---:|
| Young Wolf | 447 | 1.0 | 1575 |
| Timber Wolf | 604 | 1.0 | 1576 |
| Defias Thug | 5035 | 1.15 | 1577 |
| Mangy Wolf | 903 | 1.0 | 1578 |
| Kobold Tunneler | 139 | 1.0 | 1580 |
| Defias Cutpurse | 2361 | 1.15 | 1581 |
| Kobold Miner | 373 | 1.0 | 1584 |
| Gray Forest Wolf | 380 | 1.0 | 1587 |
| Defias Bandit | 2357 | 1.15 | 1588 |
| Defias Rogue Wizard | 2359 | 1.1 | 1590 |
| Hogger | **384** | **1.9** | 1595 |

The task's Hogger creature-entry **448** must not be passed as display ID: source table uses **384**; the table does not contain entry 448 or establish that entry-to-display join. Rabbits/chickens are absent from this Elwynn roster. Known model paths are `Creature\\Rabbit\\Rabbit.m2` and `Creature\\Chicken\\Chicken.m2` (`crates/benilla-formats/tests/m2_idle_anim.rs:8–9`), but these tests do not establish their display IDs or zone membership.

DBC resolution APIs (`crates/benilla-formats/src/creatures.rs:160–182,192–198,232–248`):

```rust
pub fn display_models(&self) -> impl Iterator<Item = (u32, u32)> + '_ // display -> model id
pub fn model(&self, display_id: u32) -> Option<CreatureModel>
pub fn model_scale(&self, display_id: u32) -> Option<f32>
pub fn collision_height(&self, display_id: u32) -> Option<f32>
```

`load_creature_catalog(&mut Chain)` reads CreatureModelData and CreatureDisplayInfo DBCs (`creatures.rs:349–354,380–388`). `CreatureModel` exposes model_path, DBC scale product, texture variants, optional NPC appearance and raw collision_height (`creatures.rs:20–36,240–247`). The existing `entities::Creatures` resource owns a **private catalog**; sibling modules can call its crate-visible collision_height/display_race_sex, but cannot access `catalog.model` directly (`crates/benilla-app/src/entities.rs:232–252`). For an Azeroll catalog, use benilla_formats' public loader over WorldAssets.chain (existing load example: `entities.rs:757–770`), or add a small read-only accessor rather than exposing the entire entities cache.

**Recommended source for real named/placed Elwynn creatures:** obtain entry/name/display/scale/spawn-map/position from the matching server world DB/export, then resolve those displays through the client's DBC catalog. This is a recommendation, not an inspected SQL schema; no such complete Elwynn spawn table was found in this checkout. Client DBC model/display joins do not themselves supply a zone population list (the catalog rows/API above carry visual metadata, while Survivors supplies names and membership manually). For rabbits/chickens, iterate display_models → model and filter model_path, then independently verify names/zone; do not guess IDs. Determine collection size from chosen final scale and suitable model bounds/height, rather than enemy gameplay radius or Survivors' tuned scale; detailed model bounds are delegated to the model researcher. Collision-height is available but is not necessarily the visible full extent (`creatures.rs:192–198`).

## 9. One-shot sound

**Simplest existing crate-visible route by SoundEntries ID:** emit a normal ServerSoundMessage; NetPlugin registers the message even offline (`net.rs:64–72,129`; definitions at `net.rs:2270–2284`):

```rust
sounds.write(crate::net::ServerSoundMessage {
    kind: crate::net::ServerSoundKind::Sound2d,
    sound_id,
    source: None,
});
```

The consumer resolves KitRef::Id and Sfx, with None position for source None (`sound/zone.rs:1103–1124,1140–1171`). It drops requests under the loading cover (`1118–1121`). Pick a nonlooping kit for a one-shot; an arbitrary SoundEntries id is not guaranteed to represent one. For a spatial one-shot, `SpellKitSound::PlayAt { pos, kit_sound }` is already registered and routed to play_kit with Some(pos) (`creature_anim/spell_visual.rs:65–77`; `creature_anim.rs:792`; `sound/spell.rs:91–104`).

For a stock named UI sound, emit `crate::sound::GlueSound("...")`; it is re-exported and routed as 2D SFX (`sound/mod.rs:58`; `sound/glue.rs:28–30,116–137`).

Raw-path core API exists (`sound/kit.rs:729–738`):

```rust
pub(crate) fn play_file(kits: &mut SoundKits, assets: &WorldAssets,
    out: &mut SoundOutput, config: &SoundConfig,
    path: &str, category: SoundCategory) -> Result<()>
```

However `sound::kit` is a **private module**, and SoundKits is re-exported only under cfg(test), so this is **not directly accessible from a sibling mode unchanged** (`sound/mod.rs:23,60–62`). Prefer ID messages; if raw paths are needed, add one small public-to-crate request/re-export seam inside sound instead of instantiating another audio mixer. The implementation handles wasm in-flight file loads/deferred replay (`sound/kit.rs:739–753`). Browser audio contexts need a user gesture; current page resumes them on pointer/key/touch (`web/index.html:10–36`).

## 10. Gotchas and scheduling

- WorldStage is ordered **Net, Input, Stream, Present** in Update (`crates/benilla-world/src/schedule.rs:28–60`). Put ball movement and camera in Input, publish stream focus before Stream, and let Present own the cover. Follow Survivors' chained input systems (`survivors/mod.rs:60–69`). Other chains merely `.after(Input)` need stronger ordering if they must see the final map/focus this frame (`mod.rs:94–117`; `player.rs:221–225`).
- InWorld state drives the engine's WorldLive and InWorldGated (`char_select/mod.rs:43–69`); keep this client state while using a separate mode Phase. The world is not automatically live just because a mode resource exists (`benilla-world/src/schedule.rs:7–17`).
- Initial OnEnter runs **after PostStartup**, an intentional engine ordering change so catalogs/VM/mixer exist (`schedule.rs:65–79`). Don't write NextState in Startup casually; the stated boot-order assumption excludes it (`schedule.rs:68–69`).
- Pause uses `ResMut<Time<Virtual>>::pause/unpause` (`survivors/mod.rs:391–396`); menu/playing/gameover unpause, paused/level-up pause (`mod.rs:134–138`). Hero movement uses generic Time while camera uses Time<Real> (`hero.rs:189,335`). Phase gates are still needed: pausing time gives zero delta, it does not stop input/state systems. GameOver is intentionally unpaused in Survivors; Azeroll should choose its own policy.
- Root Transform assumptions and FOV interference are concrete risks, not solved by CaptureMode (§3/§5). Do not parent WorldCamera to the ball casually: early camera pose publishing explicitly assumes a root camera (`benilla-world/src/view.rs:347–351`).
- Wasm uses page config rather than std process environment (`webenv.rs:1–18,37–44`); engine START_MAP and mode ACTIVE atomics avoid set_var. Follow repository restrictions on wasm threads/std wall clocks (`AGENTS.md:71–75`); use Bevy Time / `bevy::platform::time::Instant`, as existing clamp instrumentation does (`net/motion/spline.rs:501`). No builds were attempted here.
- Native versus wasm launcher return semantics differ: wasm yields the event loop to requestAnimationFrame and discards AppExit (`crates/wenilla/src/lib.rs:24–30`). Keep CPU work incremental; the existing FrameXML browser notes document long main-thread loads (`ui_script/lifecycle.rs:136–141`).
- Keep asset reads through MPQ/WorldAssets. Register the MPQ source before DefaultPlugins and loaders after AssetPlugin (`lib.rs:325–338,449–450`). Repository policy forbids bundling install art (`docs/METHOD.md:40–44`). If adding page-side files, include them in the web-build copy list, and account for both dev and realm pages (`AGENTS.md:65–69`; `scripts/web-build.sh:37–42`).

## Recommended skeleton for an azeroll mode

Recommendations below mirror the cited existing integration, rather than claiming an Azeroll implementation already exists.

1. **New mode-owned files:** `crates/benilla-app/src/azeroll/mod.rs` mirrors Survivors' ACTIVE/active/Plugin, separate Phase, Run resource, RunEntity cleanup and pause/unpause (`survivors/mod.rs:26–69,134–163,391–421`). Keep initialization and world-entry focus independent of gameplay-ready gates.
2. **`azeroll/world.rs` or `data.rs`:** a fixed map 0/Goldshire spawn plus explicit creature/display/size table; mirror `MapDef`, `hero::arena`, `follow_map`, and focus publication (`survivors/data.rs:1389–1405`; `hero.rs:76–94,309–314`). No need to copy the full Survivors map/menu system.
3. **`azeroll/ball.rs` and `camera.rs`:** mirror `hero::drive_hero`, `CamRig`, `drop_stray_buttons`, `place_camera`, replacing hero jump/gait with ball velocity/rotation/growth and WorldCollision queries (§4–6). Keep camera distance tied to radius; fixed FOV minimizes engine changes.
4. **`azeroll/units.rs` / `collect.rs`:** copy the small local UnitSpec/spawn helper and ownership marker rather than depending on private sibling internals (`survivors/units.rs:58–113`). Track collectible radius and captured status separately; stop ground/motion/conform systems from rewriting captured local offsets (§3). Retain unique GUIDs across resets (`survivors/mod.rs:411–415`). Validate a parented animated creature early before implementing mass collection.
5. **`azeroll/hud.rs`:** copy only text/font/root/panel patterns (§7), with timer/diameter/count and results/restart. Skip cards, spells, grade, minimap, sweep/fill materials initially.
6. **New native launcher crate `crates/azeroll/`:** mirror `crates/warcraft-survivors/Cargo.toml:1–24`, `build.rs:3–5`, `src/main.rs:5–16`, replacing the entry call with `run_azeroll`. Workspace glob already includes it (`Cargo.toml:3`).
7. **Required outside-mode Rust hooks, primarily one file:** add `mod azeroll` and `pub fn run_azeroll(BuildId)` in `benilla-app/src/lib.rs`, mirroring lines 110 and 230–233. Resolve one selected mode before constructing GamePlugins; use map 0 instead of line 307's map 1, set an appropriate title (344), disable connection (436), start InWorld (437–441), and install only the selected plugin (443–445). Explicitly resolve conflicting activation flags instead of installing both controllers. Reuse CaptureMode: no additional FrameXML/player/net special cases are inherently needed (§1).
8. **Wasm entry:** change or dispatch `crates/wenilla/src/lib.rs:30` to `run_azeroll` (or add a separate browser launcher if both builds must remain available). Adding only `?azeroll=1` will not undo the current shim's unconditional Survivors ACTIVE setter; selection must be designed at the launcher/lib seam (§1). Keep `#benilla` canvas binding (`benilla-world/src/boot.rs:19`).
9. **Small additional engine seam only if needed:** a generic opt-out for local ground clamping/conforming when entities become captured (§3); raw-file sound request only if ID-based messages do not suffice (§9); FOV ordering seam only if varying FOV (§5). These are identified design needs, not pre-existing APIs. Keep them generic and narrowly scoped; do not add scattered `azeroll::active()` checks.
10. **Optional distribution/config:** launch alias/Makefile and web branding mirror existing sites in §1; new shader embeddings only if custom materials are actually introduced. No mandatory changes found to WorldPlugins, terrain streaming, DBC loading, FrameXML lifecycle, or networking internals for a fixed-map mode using the existing seams (§1–2).
