# Azeroll: world geometry and models

Source-only research of this checkout. No Cargo, downloads, runtime measurements, or game-data inspection. References are repository-relative `path:line` anchors; ranges describe the surrounding implementation. Proposed changes are explicitly recommendations, not existing APIs. Orientation: `AGENTS.md:1`, `CLAUDE.md:1`, `docs/METHOD.md:1`, `docs/MAP.md:1`.

## 1. Doodads as entities

**There is a placement record per unique ID, but not necessarily an entity per placement.** `Placements` is a **crate-private** resource keyed by MDDF/MODF `uniqueId`; its private `Placement` stores model handle, world `Transform`, owned entity list, spawned flag, refcount and owner tile. Repeated tile references increment the refcount rather than duplicate the object (`crates/benilla-world/src/terrain_stream.rs:125`, `:148`, `:900`).

Public source records are easier to enumerate than rendered entities:

```rust
// benilla_assets::AdtTile
pub doodads: Vec<Doodad>,
pub wmos: Vec<WmoInstance>,
// benilla_formats::Doodad
pub model: String,
pub position: [f32; 3],
pub rotation: [f32; 3],
pub scale: f32,
pub unique_id: u32,
```

These are raw **WoW world coordinates**, already converted from MDDF's corner-origin disk coordinates; scale has already been divided by 1024. Model identity is a path, not a FileDataID in this pipeline (`crates/benilla-assets/src/adt.rs:27`; `crates/benilla-formats/src/terrain.rs:239`, `:467`). MDDF name IDs resolve through MMID offsets into MMDX strings, not directly through arbitrary string order (`crates/benilla-adt/src/lib.rs:256`, `:267`).

Registration loads `asset_server.load(m2_url(&d.model))` and constructs `Transform { translation: wow_to_bevy(d.position), rotation: placement_rotation(d.rotation), scale: Vec3::splat(d.scale) }` (`crates/benilla-world/src/terrain_stream.rs:911`). WoW→Bevy is `(-y, z, -x)`; one unit remains one yard (`crates/benilla-assets/src/coords.rs:1`, `:10`, `:34`).

The public assembler returns `SpawnedModel { entities, by_batch, host }`. It normally creates one mesh entity per authored batch, plus an optional animation host, cards and separately spawned effects. However eligible batches divert into the default-on retained `static_gx` renderer, or static merge fallback, leaving `by_batch` entries `None` and potentially no mesh entities at all. Diverted geometry has baked placement transforms (`crates/benilla-world/src/terrain_stream/spawn/assemble.rs:44`, `:145`, `:264`, `:361`, `:386`, `:510`; `crates/benilla-world/src/static_gx/mod.rs:1`; `crates/benilla-world/src/terrain_stream/merge.rs:1`).

Ordinary parts have `Mesh3d`, `MeshMaterial3d<WowModelMaterial>`, `Transform`, `ModelPart { kind, blend }`, `PickMesh`, `MeshTag`, and usually a local `Aabb` with `NoAutoAabb`. The assembler uses the static mesh bound unioned with the all-animation bound for animated parts (`crates/benilla-world/src/terrain_stream/spawn/assemble.rs:484`, `:510`, `:526`, `:646`). The inspector's `WorldObject { kind, label, id, detail }` supplies model-path label and placement ID on entities; diverted lanes retain its `Arc`. It is not a complete one-row-per-placement registry (`crates/benilla-world/src/interact/mod.rs:23`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:188`, `:878`, `:885`).

**WMO-internal MODD props:** resolve MODS set 0 plus the MODF-selected set; each MODD M2 loads by path and gets `wmo_world.mul_transform(wmo_doodad_local(...))`. Private `WmoDoodadInst` stores handle, world transform, group memberships, lighting and spawned flag. Props use the same assembler/diversion lanes; their entities join the building's ownership list. Their inspector `id` is the **building's** unique ID, not the MODD index, so `(map, WMO uniqueId, MODD index)` needs a new explicit identity (`crates/benilla-world/src/terrain_stream/spawn/mod.rs:549`, `:614`, `:628`, `:755`, `:769`, `:789`, `:820`; `crates/benilla-world/src/terrain_stream/spawn/prop_light.rs:8`).

**Enumeration conclusion:** iterate loaded `Assets<AdtTile>` and deduplicate `Doodad.unique_id`, filter by world distance, then resolve `Assets<M2Model>` bounds. A Bevy query over `WorldObject` misses diverted objects and repeats multi-batch objects. Exact streamer tile membership and placement ownership need an accessor: `TerrainStreamer.tiles`, `TileState` and `Placements.by_id` are private; the existing public residency method exposes only counts (`crates/benilla-world/src/terrain_stream.rs:64`, `:93`, `:102`, `:127`; `crates/benilla-assets/src/adt.rs:27`; `crates/benilla-assets/src/m2.rs:31`).

## 2. Removing and attaching doodads

**Despawn is not permanent collection.** Ordinary spawn is guarded by `!p.spawned`, so deleting a mesh alone does not immediately trigger ordinary placement respawn. At last tile-ref release the placement is removed; stream-in registers it anew. Owner-tile handoff can explicitly despawn remaining entities and set `spawned = false`. A collected-ID tombstone must therefore survive stream-out/in and be consulted at registration/spawn/handoff (`crates/benilla-world/src/terrain_stream/spawn/mod.rs:152`, `:536`; `crates/benilla-world/src/terrain_stream.rs:900`, `:1193`, `:1214`).

**Hiding a mesh is not enough.** `apply_model_visibility` is the model-part visibility authority and writes `Visibility::Inherited`/`Hidden` based on its own verdict, so a one-off write can be overwritten. A hidden dedicated parent is appropriate for fresh entity-based copies, but the streamed placement has no universal root. Retained draws have no part entity to hide, and merged meshes contain multiple placements (`crates/benilla-world/src/model_render/visibility.rs:1`, `:48`, `:210`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:227`; `crates/benilla-world/src/terrain_stream/merge.rs:5`).

**Do not reparent existing streamed geometry as the general solution.** Retained items store their own `Transform`, merged vertices bake world placement, billboard cards can have baked pivots, and collision hulls bake world coordinates. Default hull welding combines ADT objects by tile and WMO props by building. Despawning/reparenting ordinary parts neither removes retained/merged visuals nor removes welded collision (`crates/benilla-world/src/static_gx/mod.rs:131`; `crates/benilla-world/src/terrain_stream/merge.rs:5`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:474`; `crates/benilla-world/src/terrain_stream/collider.rs:167`; `crates/benilla-world/src/terrain_stream/weld.rs:1`). Retained fading can also exile geometry back to ordinary entities; it is not a permanent entity identity (`crates/benilla-world/src/static_gx/mod.rs:1`).

**Fresh moving copies are supported.** The public `benilla_world::terrain_stream::spawn_model_entities(...)` explicitly accepts local transforms for moving parents. The production precedent is WMO GameObject cargo:

```rust
// Important arguments, omitting the material/animation resources:
spawn_model_entities(
    /* ... */, forms.slices(&prop.handle), prop.local, /* ... */,
    None, // no fixed-world EmitterFade
    radius, center, anim_bound, Some((m, now)), /* registries ... */,
    true, card_owner,
    None, None, // no static merge; no retained-world diversion
);
commands.entity(entity).add_children(&ents);
```

This is an excerpt with omissions, not a compilable replacement signature (`crates/benilla-world/src/terrain_stream/spawn/assemble.rs:55`; `crates/benilla-app/src/entities/wmo_props.rs:273`). Use a collectible-local root under the ball; parent all returned entities under it with a local attachment transform. Cards require a real following owner: they remain world roots and are excluded from the returned list when `card_owner` is supplied. Animation host and parts must share the moving ancestry (`crates/benilla-world/src/terrain_stream/spawn/assemble.rs:89`, `:474`, `:592`, `:635`). A mode-owned wrapper is still needed to coordinate loading, handles, materials, cards, optional effects and original suppression; no single `collect_doodad(uid)` API was found in the placement/spawn code above.

## 3. WMOs (buildings)

MODF registration deduplicates by unique ID and uses scale `Vec3::ONE` (1.12 placement scale is unused). Geometry is flattened into submeshes with `submesh_group` mapping; ordinary entities are per batch, not necessarily one entity per group. `WmoGroupVis { instance, groups }` ties parts to portal visibility. Eligible geometry instead diverts into retained regions (`crates/benilla-world/src/terrain_stream.rs:934`; `crates/benilla-assets/src/wmo.rs:42`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:345`, `:381`, `:418`).

The `WmoPortalInstance` is a logical entity with a **copied** `world_from_local: p.transform.compute_affine()`, handle, name set and per-group visibility/fog/liquid state. It is not a transform parent of all building geometry. Changing its `Transform`, hiding it, or scaling it alone does not move/hide the building (`crates/benilla-world/src/terrain_stream/spawn/mod.rs:316`, `:439`). The placement also independently spawns world-space walking/camera hulls, group liquids, WMO lights and MODD props (`crates/benilla-world/src/terrain_stream/spawn/mod.rs:442`, `:480`, `:509`, `:522`, `:532`).

Bounds are public on `WmoModel`: MOGI `group_bounds`, `group_nav` boxes, per-group collision bounds and a union `collision_bounds`. MOGI boxes parse in `parse_wmo_group_infos`; MODF's on-disk six-float placement box is **currently ignored**. For visual size, union group boxes or all render vertices rather than relying solely on collision bounds, which exclude non-walking geometry (`crates/benilla-assets/src/wmo.rs:26`, `:58`, `:71`, `:83`; `crates/benilla-formats/src/models/wmo/root.rs:168`; `crates/benilla-adt/src/lib.rs:342`).

**Moving a full streamed WMO is not cheap/safe via a root edit.** The portal graph, room/PVS tags, retained regions, lighting probes and baked colliders all need coherent handling; `WmoResidency` is separately synchronized from registered placements. A visual-only building trophy is simpler: assemble its group submeshes under a ball child with no static diversion, no `WmoGroupVis`/portal instance, no world collider or interior residency, and intentionally choose which props to copy. This is a proposed representation, not the current streamed-WMO behavior (`crates/benilla-assets/src/wmo.rs:48`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:318`, `:418`, `:590`; `crates/benilla-world/src/terrain_stream.rs:1112`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:55`). Runtime cost of a whole Goldshire inn copy is unmeasured.

## 4. Bounds and collectability size

M2 header parsing reads render AABB and bounding sphere, then the separate collision box/sphere; collision triangles and vertices are also read (`crates/benilla-m2/src/lib.rs:121`, `:135`, `:423`). `M2Bounds` exposes render `sphere_radius`, `bbox_min/max`, and `vert_max_from_origin` in **model-local yards before scale**; `M2Model.bounds: Option<M2Bounds>` preserves it (`crates/benilla-formats/src/models/bounds.rs:14`, `:158`; `crates/benilla-assets/src/m2.rs:31`).

Existing helpers:

```rust
pub fn m2_fade(bounds: &Option<M2Bounds>, scale: f32) -> (f32, Vec3)
pub fn m2_anim_bound(bounds: &Option<M2Bounds>) -> Option<Aabb>
```

`m2_fade` returns `(sphere_radius * scale, Bevy-space local box center)`; missing bounds return **infinity**, unsuitable as an automatic gameplay size. The helper's center still needs `placement.transform_point(center)`. `m2_anim_bound` converts min/max correctly across negated/swapped axes (`crates/benilla-world/src/terrain_stream/spawn/mod.rs:899`, `:908`). Recommend authored radius × absolute uniform scale, with an explicit finite/positive check and geometry-derived fallback. For nonuniform scale use the maximum absolute axis conservatively; that extension is a gameplay recommendation.

Entity `Aabb`s are generally **batch-local**, not a placement's whole-model bound; `DoodadFade.radius/local_center` exist but are crate-private. Store the model bound and scaled radius on the mode's collectible record rather than reconstructing them from one arbitrary mesh entity (`crates/benilla-world/src/terrain_stream/spawn/assemble.rs:526`; `crates/benilla-world/src/model_fade.rs:15`). WMO visual radius can be half the diagonal of the union of group AABBs, transformed/scaled by the mode; existing `collision_bounds` is a whole-building collision broad phase, not an authored render sphere (`crates/benilla-assets/src/wmo.rs:66`, `:83`).

## 5. Spawning arbitrary M2s and display IDs

**Public path route:** `let h: Handle<M2Model> = asset_server.load(benilla_assets::m2_url(path));`, wait for `Assets<M2Model>::get(&h)`, request render forms, then assemble. `m2_url` normalizes case, backslashes and `.mdx`/`.mdl` to one `mpq://...m2` asset key (`crates/benilla-assets/src/lib.rs:217`; `crates/benilla-world/src/terrain_stream.rs:911`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:156`). Keep the handle alive in a mode-owned model cache, as the streamed `Placement` does (`crates/benilla-world/src/terrain_stream.rs:148`).

```rust
// Public ModelForms methods; poll require_* until true.
pub fn require_static(&mut self, model: impl Into<ModelKey>, priority: i32) -> bool
pub fn require_rigged(&mut self, model: impl Into<ModelKey>, priority: i32) -> bool
pub fn slices(&self, model: impl Into<ModelKey>) -> FormSlices<'_>
```

Forms share mesh handles and per-batch bounds per model. The assembler signature and all supporting resources are at `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:57`; exports at `crates/benilla-world/src/terrain_stream.rs:50`; form APIs at `crates/benilla-world/src/model_forms.rs:65`, `:100`. This is a low-level assembly API, not a scene asset you can instantiate in one call.

**Animation:** passing `Some((m, now))` permits an animation host. Classification distinguishes static, global-sequence-only and loader-idle sequence animation; it can also arm sound events without moving bones. Lazy skinned twins allocate at wake. For deliberately frozen copies, request static forms, pass `m2: None`, and use the static slices; omit emitter/ribbon/light spawning. Material UV/RGB/alpha loops can still register from submeshes, so freezing *all* effects requires an explicit policy beyond disabling bones (`crates/benilla-world/src/doodad_anim/mod.rs:30`, `:66`, `:133`, `:149`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:114`, `:412`, `:439`, `:505`, `:574`, `:630`). Parenting support and billboard caveats are in §2.

**GameObjectDisplayInfo route:** app-private `new_gameobject_display(catalog, display_id, asset_server)` asks `GameObjectCatalog::model_path(id)` and chooses WMO versus M2 by extension. Creature `new_creature_display(...)` asks `CreatureCatalog::model(id)` and retains model path, skin variations and NPC appearance, rejecting missing/zero-scale rows (`crates/benilla-app/src/entities/display.rs:201`, `:223`). These catalog adapters live in `benilla-formats`, above raw DBC parsing; use this resolution rather than guessing a display ID is a file ID.

App visuals are driven by `NetEntity { kind, display_id, scale }`, cached display construction and `attach_entity_visuals` querying entities without `VisualAttached`. This is not a public engine-level `spawn_gameobject(id, transform)` function; reuse within `benilla-app` would require the app's initialized catalog/entity pipeline and choosing the relevant kind/scale. M2-by-path is the smaller dependency for scenery copies (`crates/benilla-app/src/net.rs:275`; `crates/benilla-app/src/entities.rs:925`, `:958`, `:1000`; `crates/benilla-app/src/entities/attach/mod.rs:226`). Creature rigging, skin selection and animations are a richer pipeline than rigid scenery; `DisplayModel` stores skeleton, inverse bindposes and animations (`crates/benilla-app/src/entities/display.rs:112`, `:135`).

GameObject skeletal animation is state-driven (`GAMEOBJECT_STATE`, plus one-shot motions), rather than merely treating every object as a looping doodad. A mode using only display ID/path does not automatically supply the state/type inputs; choose a frozen model or explicitly initialize that machinery (`crates/benilla-app/src/go_anim.rs:1`, `:39`, `:92`, `:117`). Catalog loaders read GameObjectDisplayInfo and the creature DBC chain from the local patch chain (`crates/benilla-formats/src/gameobjects.rs:48`; `crates/benilla-formats/src/creatures.rs:348`).

## 6. Collision

Yes: M2 collision hulls are separate coarse triangle lists (`bounding_vertices`/`bounding_triangles`), optional on `M2Model`. A trunk can collide while its canopy does not; hull-less models do not collide (`crates/benilla-formats/src/models/collision.rs:14`, `:31`, `:43`; `crates/benilla-assets/src/m2.rs:35`). Terrain is also a hole-masked triangle collider (`crates/benilla-world/src/terrain_stream/collider.rs:192`).

WMO collision is derived from group vertices/indices and MOPY flags, with separate walking and camera/LOS sets; asset-level group triangle lists, AABBs and column grids support interior queries. The low-level WMO group representation/parser does not retain MOBN/MOBR BSP nodes: unhandled chunks are skipped; runtime physics rebuilds its own trimeshes (`crates/benilla-formats/src/models/collision.rs:154`; `crates/benilla-wmo/src/lib.rs:126`, `:258`, `:313`, `:322`; `crates/benilla-assets/src/wmo.rs:58`; `crates/benilla-world/src/terrain_stream/collider.rs:271`).

Public `WorldCollision` is a Bevy `SystemParam`, not restricted to player entities:

```rust
pub fn cast_body(&self, shape: &Collider, from: Vec3,
                 movement: Vec3, skin_width: f32) -> Option<MoveHitData>
pub fn ray_body(&self, origin: Vec3, dir: Dir3,
                max_distance: f32) -> Option<RayHitData>
pub fn slide_body(&self, shape: &Collider, shape_position: Vec3,
                  velocity: Vec3, delta_time: Duration,
                  config: &MoveAndSlideConfig,
                  on_hit: impl FnMut(MoveAndSlideHitData) -> MoveAndSlideHitResponse)
                  -> MoveAndSlideOutput
```

Pass `Collider::sphere(ball_radius)` for a non-player ball sweep/slide; body casts are one-sided and use Default|Walk layers. `cast_camera` already demonstrates a sphere cast, but uses different camera collision rules. Player movement uses `cast_mover`/`slide_mover`, variants applying mover exclusions (`crates/benilla-world/src/collision.rs:112`, `:124`, `:160`, `:188`, `:207`, `:212`, `:261`; `crates/benilla-app/src/player/mover.rs:80`, `:482`). No ball-specific overlap/collection helper was found here; use a mode spatial index for pickup sphere tests and world physics for solid movement.

**Critical removal consequence:** default hulls weld many placements together. Visual collection alone leaves invisible obstacles. Options are mode-specific separate collectible colliders, regenerating the affected weld, or excluding collectible hulls from welding at initial spawn. Debug toggles `WOW_NO_HULL_WELD` and `WOW_NO_DOODAD_BODIES` demonstrate existing alternate paths, but are process-level levers, not a per-object gameplay solution (`crates/benilla-world/src/terrain_stream/weld.rs:1`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:238`; `crates/benilla-world/src/terrain_stream/collider.rs:153`).

## 7. Terrain queries and streaming

```rust
pub fn terrain_height_under(streamer: &TerrainStreamer,
    adt_tiles: &Assets<AdtTile>, bevy_pos: Vec3) -> Option<f32>
pub fn terrain_height_under_cached<'a>(streamer: &TerrainStreamer,
    adt_tiles: &'a Assets<AdtTile>, bevy_pos: Vec3,
    cache: &mut Option<((i32, i32), &'a AdtTile)>) -> Option<f32>
```

These return terrain height as WoW Z (also Bevy Y), `None` for unavailable tile/hole/off-ring; they do not return an inn floor. They resolve through streamer's private tile handles and then `benilla_formats::terrain_height_at`. For ground normals, `ChunkMesh.normals` holds MCNR vertex normals but this height API has no normal return; derive the triangle normal/interpolate these normals, or use a collision ray hit for the actual surface (`crates/benilla-world/src/terrain_stream/queries.rs:76`; `crates/benilla-formats/src/terrain.rs:36`, `:132`, `:205`).

Streaming takes `ViewFocus` plus `WorldCamera` and live `ViewDistance`. Use `ViewFocus::body([wow_x,wow_y,wow_z], paced)` to follow the ball; camera-only default follows the camera, and detached mode follows the free eye. Streamer computes `StreamWindow::at(view.farclip, center[0], center[1])`, gates wanted tiles through WDT, and requests `mpq://World/Maps/{dir}/{dir}_{tx}_{ty}.adt` (`crates/benilla-world/src/terrain_stream.rs:229`, `:242`, `:257`, `:556`, `:589`, `:680`).

Window: inner half-width `1 + trunc(farclip/33.3333)` chunks; outer `max(inner+2,8)`; whole tiles intersecting outer window load, with one extra keep-band chunk for eviction. At default farclip 350: inner 11, outer 13 (~433 yards per axis), typically 2×2–3×3 tiles. At 777: inner 24, outer 26 (~867 yards), 4×4–5×5 tiles depending on focus within tile (`crates/benilla-world/src/terrain_stream/window.rs:1`, `:35`, `:55`, `:85`, `:130`, `:150`). Public `TerrainStreamer::residency()` reports counts only, not coordinates (`crates/benilla-world/src/terrain_stream.rs:93`).

Growing the ball can grow `ViewDistance.farclip`, but normal settings clamp 177..777 (default 350). The camera projection far plane is separately 3000; increasing it alone does not extend detailed terrain, which has CPU/shader farclip walls. Fog end is `min(zone fog end, farclip)`, so increasing farclip may still leave zone fog limiting sight. Beyond 777 requires deliberate mode-specific range/CVar/fog policy and checking the projection/horizon/render budgets, not just a camera setting. Tiny doodads also have independent size-based fades (40–50, 100–125, 150–200-yard bands); attached trophies likely need a mode policy to avoid fading away when the large-ball camera pulls back (`crates/benilla-world/src/view.rs:64`, `:76`, `:242`, `:255`, `:272`; `crates/benilla-world/src/lighting/resolve.rs:17`; `crates/benilla-world/src/model_fade.rs:29`).

## 8. Performance

Default static scenery is neither uniformly per-placement draws nor ordinary GPU instancing: the retained renderer bakes batches into cell/region buffers and issues indexed draws over admitted material/group runs (`draw_indexed(..., 0..1)`). Fallback static merge combines geometry per tile/cell/material or WMO placement/room-set/material. Animated, billboard, material-loop and transparent/order-dependent families are excluded from static consolidation (`crates/benilla-world/src/static_gx/mod.rs:1`; `crates/benilla-world/src/static_gx/render.rs:731`, `:749`; `crates/benilla-world/src/terrain_stream/merge.rs:1`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:232`).

Moving copies without diversion create roughly `copy count × authored batch count` ordinary mesh entities, plus optional host/joints/cards/effects. Meshes and deduplicated materials can be shared; exact Bevy draw-call batching is not established by this source-only research and must not be assumed one draw per model or one draw per copy (`crates/benilla-world/src/model_forms.rs:65`; `crates/benilla-world/src/terrain_stream.rs:130`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:145`, `:168`, `:510`).

500–2000 rigid copies therefore mean hundreds to thousands of changing global transforms/bounds and per-part visibility/fade work each ball rotation. Visibility's unchanged-row optimization reads `GlobalTransform`; rotating ancestry defeats that stillness. Animated copies add per-instance pose/palette allocation, lazy rigs and material sampling; copying torch lights/particles multiplies those systems too. These are scaling risks, not measured frame-time estimates (`crates/benilla-world/src/model_render/visibility.rs:48`, `:80`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:505`, `:612`, `:630`; `crates/benilla-world/src/doodad_anim/mod.rs:133`; `crates/benilla-world/src/rig_palette.rs:190`, `:298`; `crates/benilla-assets/src/m2.rs:37`).

Cheapest proposed progression: shared static forms, no rigs/effects/colliders on trophies; keep new/large trophies as separate objects, then periodically bake older small opaque/alpha-test trophies into **ball-local**, material-grouped meshes so one parent rotation moves the aggregate. Existing static-merge machinery demonstrates combining `(RenderSubmesh, Transform)` inputs, but its world-cell ownership/fade anchors cannot be used unchanged for a moving ball. Bound the number of visible tiny trophies; later simplify geometry or use explicit impostors. Existing billboard cards are authored camera-facing model parts, not an automatic general-purpose model LOD generator (`crates/benilla-world/src/terrain_stream/merge.rs:5`, `:52`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:474`).

## 9. Content discovery and Goldshire tiles

Runtime inventory can read `Assets<AdtTile>.doodads`: retain `(map ID, uniqueId, model path, placement transform)`, deduplicate across tiles, then load/cache each distinct M2's bounds for size classification. The format parser already resolves MMDX/MMID/MDDF, converts coordinates and scales. Include WMO MODD sets separately using `(map ID, WMO uniqueId, original MODD index)`; preserve that index when selecting set ranges. The current resolver enumerates the original index but does not store it in `WmoDoodadInst` (`crates/benilla-assets/src/adt.rs:27`; `crates/benilla-formats/src/terrain.rs:467`; `crates/benilla-adt/src/lib.rs:267`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:769`, `:789`, `:820`). No actual Elwynn asset list or measured model sizes was obtained without reading game data.

Engine math is `T=533.3333`, `offset=32*T`, `tile_x=floor((offset-wow_y)/T)`, `tile_y=floor((offset-wow_x)/T)` for this positive grid range; filenames keep this swapped ordering. Thus **WoW (-9464, 62) → tile (31,49), `World/Maps/Azeroth/Azeroth_31_49.adt`**, approximately chunk (510,795), local chunk (14,11). These are arithmetic results from `crates/benilla-wdt/src/lib.rs:23`, `:27`, `:41`, `:50`, and filename construction at `crates/benilla-world/src/terrain_stream.rs:680`, not confirmation that a particular inn mesh lies entirely in that tile.

Adjacent bounds, approximate yards, lower edge exclusive / upper edge inclusive under the engine's grid convention:

| Tile | WoW X range (north/south) | WoW Y range (west/east) |
|---|---|---|
| `Azeroth_31_49` | -9600 .. -9066.67 | 0 .. 533.33 |
| `Azeroth_32_49` | -9600 .. -9066.67 | -533.33 .. 0 |
| `Azeroth_31_48` | -9066.67 .. -8533.33 | 0 .. 533.33 |
| `Azeroth_32_48` | -9066.67 .. -8533.33 | -533.33 .. 0 |
| `Azeroth_31_50` | -10133.33 .. -9600 | 0 .. 533.33 |

Table derived from `tile_to_world` (`crates/benilla-wdt/src/lib.rs:50`). At the supplied point, default 350-yard window predicts wanted tile x=31..32, y=48..50 (six tiles); at 777 it predicts x=30..33, y=48..51 (sixteen), before WDT filtering. Computation: chunk (510,795) ±13 or ±26, integer-divide by 16 (`crates/benilla-world/src/terrain_stream/window.rs:35`, `:55`, `:85`). A wider Elwynn candidate scan should use tile math over the desired world-space region and the WDT's presence flags, not hand-picked filenames (`crates/benilla-world/src/terrain_stream.rs:589`).

## Recommended approach for Azeroll

1. **Represent map collectibles independently of render entities.** Add a mode registry/spatial index keyed by `(map, MDDF uniqueId)` with path/handle, original transform, center/radius and collected tombstone. For WMO furniture use `(map, MODF uniqueId, MODD index)`. Expose a small world placement adapter that selects collectible placements before retained rendering, merging and hull welding, keeping noncollectible scenery optimized. This addresses the private registry, multi-batch entities and missing MODD identity described in §1 (`crates/benilla-world/src/terrain_stream.rs:125`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:188`, `:614`, `:820`).

2. **Suppress originals across all lanes and stream cycles.** Initial prototype: keep selected collectible models on an ordinary entity lane with dedicated ownership roots and separately removable hulls; on pickup suppress their entire source record and consult tombstones on respawn/handoff. A retained-world production solution needs per-placement suppression/rebaking in retained, merge and welded-collision lanes; current coarse owner release is not an object-level removal API (`crates/benilla-world/src/static_gx/mod.rs:565`; `crates/benilla-world/src/terrain_stream/merge.rs:5`; `crates/benilla-world/src/terrain_stream/weld.rs:1`; `crates/benilla-world/src/terrain_stream.rs:1193`).

3. **Attach fresh copies to the ball.** Use normalized M2 handle + shared static forms + `spawn_model_entities` with local transform, no static diversion, and no fixed-world fade. Parent all returned parts under a mode-owned attachment root; handle following cards explicitly. Start with rigid non-billboard props and no trophy collision, emitters or lights; expand to animations intentionally (`crates/benilla-assets/src/lib.rs:219`; `crates/benilla-world/src/model_forms.rs:105`; `crates/benilla-app/src/entities/wmo_props.rs:273`; `crates/benilla-world/src/terrain_stream/spawn/assemble.rs:592`).

4. **Treat WMOs as a later tier.** Keep full world WMO collision/portal behavior until collected; removal must retire the complete placement and its retained region, hulls, props, lights/liquids and interior participation. Attach a simplified visual group aggregate as the trophy, rather than trying to rotate a live streamed interior (`crates/benilla-world/src/terrain_stream/spawn/mod.rs:316`, `:442`, `:480`, `:522`, `:532`; `crates/benilla-world/src/terrain_stream.rs:1112`, `:1214`).

5. **Move one sphere, collect against a separate size index.** Reuse `WorldCollision` sphere sweep/slide for terrain and remaining solid objects; use scaled authored spheres/AABBs for pickup eligibility and contact. Remove collectible hulls when consumed. Use terrain height for outdoor grounding, collision down-rays for WMO floors, and a deliberate normal/radius-growth policy (`crates/benilla-world/src/collision.rs:160`, `:207`, `:212`; `crates/benilla-world/src/terrain_stream/queries.rs:76`; `crates/benilla-world/src/terrain_stream/spawn/mod.rs:908`).

6. **Budget trophy detail and streaming separately.** Stream around the ball using `ViewFocus::body`; grow farclip within the supported range first, accounting for fog and small-object fade. Share mesh/material assets, freeze trophy rigs/effects, then consolidate old attachments in ball-local material groups and cap tiny-object detail. Benchmark 500/2000-copy cases on native and wasm before promising a draw/frame budget; this research establishes the mechanisms, not measured capacity (`crates/benilla-world/src/terrain_stream.rs:248`; `crates/benilla-world/src/view.rs:64`; `crates/benilla-world/src/lighting/resolve.rs:17`; `crates/benilla-world/src/model_forms.rs:65`; `crates/benilla-world/src/terrain_stream/merge.rs:5`).
