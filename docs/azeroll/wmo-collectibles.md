# WMO collectibles: campaign feasibility and decisions

Research snapshot: 2026-11-02. Source and local-data inspection only; no builds,
benchmarks, source edits, or gameplay verification. Designs below are proposals.
This complements [engine-world.md](engine-world.md); current source takes precedence.

## Recommendation

Ship inn furniture through a pre-spawn MODD claim seam. Preserve the inn shell. Keep ordinary buildings engine-owned until collection, then retire a placement
through a generic lifecycle operation and attach a separate visual trophy. Stormwind is one city WMO: the Cathedral requires an authored group/geometry subset,
not a whole-placement pickup. Make that a staged finale with bounded physical scale.
Do not make unrestricted r=100–300 gameplay a prerequisite for shipping the campaign.

## Evidence conventions and data inspection

References are repository-relative `file:line` at this snapshot, not stable anchors. Local data came from `/home/jjd/.t3/projects/azeroll/wow-data/Data`.
Read-only Python MPQ inspection used the existing `/tmp/mpqtool/mpq.py`; no data was
extracted to the repository. Reads checked patch-2, patch, then the relevant base archive; terrain and WMO paths resolved successfully. This helper is a limited
reader, not the runtime parser; confirm manifest values with the runtime before shipping.
Runtime precedence is documented in `crates/benilla-formats/src/chain.rs:79`. Numeric data evidence below names virtual files and record offsets explicitly.
ADT MODF parsing: `crates/benilla-adt/src/lib.rs:326`; coordinate conversion:
`crates/benilla-formats/src/terrain.rs:488`. MODF scale is unused in vanilla.
WMO groups were measured from MOVT vertices, with names from each MOGP's first two
u32 offsets into root MOGN. Do not identify groups by a loose name substring alone.
Group header/subchunk layouts: `crates/benilla-formats/src/models/wmo/group.rs:58`
and `:145`; root MOGI layout: `crates/benilla-formats/src/models/wmo/root.rs:168`.

## 1. Inn furniture: smallest viable ownership transfer

The existing seam is `PlacementClaims.predicate: Fn(&ClaimQuery) -> bool`;
`ClaimQuery` has only MDDF unique_id/model/Bevy-world transform, no map or kind.
Claims latch at registration; changing the predicate does not affect resident rows (`crates/benilla-world/src/terrain_stream/claims.rs:6`, `:13`).
`register_doodad` refcounts shared ADT references, evaluates once, queues
`DoodadClaimed`, and marks the placement already spawned, with no engine entities
(`crates/benilla-world/src/terrain_stream.rs:910`, `:931`, `:953`).
`register_wmo` has no equivalent predicate (`terrain_stream.rs:966`).

MODD props are resolved only after the WMO asset and group forms become ready:
`spawn_loaded_placements` calls `resolve_wmo_doodads` at
`crates/benilla-world/src/terrain_stream/spawn/mod.rs:532`.
The resolver selects MODS set 0 plus the placement's nonzero set and already has the absolute MODD index `di` (`:776`, `:789`). It discards that identity when
constructing `WmoDoodadInst` (`:820`;
`crates/benilla-world/src/terrain_stream/spawn/prop_light.rs:9`).
A prop's final transform is `wmo_world.mul_transform(wmo_doodad_local(...))`;
reuse this exactly, including its quaternion and scale (`spawn/mod.rs:799`, `:822`).

Proposed generic public records, conceptually (names may differ):

```rust
struct WmoPropKey { map: u32, placement: u32, modd: u32 }
struct WmoPropQuery {
    key: WmoPropKey, model: String, transform: Transform,
    // Optional room membership/lighting snapshot, independent of mode rules.
}
// Optional WmoPropClaims predicate; WmoPropClaimed / WmoPropReleased messages.
```

Use a distinct hook initially; sharing transport/types with MDDF claims is fine, but never encode MODD IDs by hashing into today's u32 MDDF namespace.
Expose original indices, not the ordinal in a filtered/selected set; deduplicate
ranges by index defensively. Map ID comes from the streamer's current map context.
A root path/asset fingerprint in campaign manifests detects changed data revisions.

Engine implementation sites:

1. `terrain_stream/claims.rs`: keys, optional predicate, claim/release messages; export/register them alongside existing claims in `terrain_stream.rs:31`.
2. `spawn/prop_light.rs:9`: retain absolute MODD index, path and latched claim state.
3. `spawn/mod.rs::resolve_wmo_doodads`: evaluate before creating any engine prop
   render, probe, effect or collision work; announce once per placement lifetime.
4. `spawn_loaded_placements` must skip claimed props entirely, not merely meshes.
   Account them as settled so `pending_spawns` cannot hang loading (`:539`, `:758`).
5. `terrain_stream.rs::release_placement` (`:1247`) announces releases for claimed MODDs on last placement reference, including a parent released mid-load.
   Whole-world teardown (`:997`) needs equivalent release/reset semantics.

Claim at resolver time without waiting for M2 bounds: an authored allowlist of inn UID and MODD IDs/model families is enough. Assets arrive asynchronously;
Azeroll validates dimensions after load. A bounds-based predicate would require a
second phase and pending-state bookkeeping; it is unnecessary for the first inn. Keep default behavior identical when the hook is absent (D9).

### Collision exclusion is cheap only before welding

WMO walls/floors are two placement colliders: walk and camera, built from WMO group
geometry (`spawn/mod.rs:487`, `:509`). Furniture hulls are separate M2 collision.
They are welded by **MODF placement UID**, not ADT tile and not room:
`spawn/mod.rs:709`, `:729`; `terrain_stream/weld.rs:64`, `:82`.
Several flushed batches can accumulate during loading; each resulting collider
joins the parent `Placement.entities` (`weld.rs:137`).

A claimed MODD must never reach `welds.add_prop`, StaticMerge/StaticGx admission,
probe-slot allocation, particles, ribbons or lights (`spawn/mod.rs:614`, `:652`,
`:660`, `:709`, `:732`). Then its collection needs no weld rebuild.
Removing an already-welded furniture mesh leaves an invisible wall; rebuilding the
whole prop weld is avoidable for L1. WMO structural geometry remains untouched.
Some apparent furniture could be baked into group triangles; audit each selected
object rather than assuming all scenery with a furniture silhouette is MODD.

### Azeroll consumer and indoor collision policy

Extend `Props.known`, tombstones, `PropCopy` identity and `Source` to a typed key
such as `MapPropKey::{Adt{map,uid}, Wmo(WmoPropKey)}`.
Current storage is u32-only (`crates/benilla-app/src/azeroll/props.rs:101`, `:118`);
`placements` handles claim/release/collection at `:172`, and `restart` at `:221`.
Share spawning and assembly: frozen ordinary mesh entities, no static diversion,
parented under a mode root (`props.rs:132`, `:271`, `:322`).
Preserve carried roots on release (`:152`); persist collected tombstones until run
reset. Track `resident` separately from `known`: current restart respawns every
known record, which would repopulate distant streamed-out scenery on a long level.
On map/level transition, release live copies and preserve only intended campaign state.

Do not blindly inherit current forest prop collision. `props.rs:339` reduces a
claimed prop to a small solid radius; `:381` caps it at 15% of pickup axis.
Only fences and `Gate` currently use oriented boxes (`collect.rs:334`).
For inn furniture add explicit `SolidShape` policy, at least oriented boxes for
chairs/tables/beds/barrels, sharing movement, recovery and autopilot queries.
A box is conservative for a table's under-space; decide whether tiny balls can go
under it. If required, use a few authored boxes or a removable original M2 hull.
Mode solids currently correct horizontally after movement, not as world triangle
floors (`collect.rs:341`, `:591`; `ball.rs:414` uses WorldCollision down-rays).
Thus collecting a table should not be required to escape from standing on its top.
If furniture must support the ball, register separate removable static hulls and
remove/disable them on **dislodge as well as collection**, before parenting visuals.
Never leave a static physics collider beneath BallSpin.

Lighting fidelity is optional for first delivery: today's frozen copies omit the
MODD interior-light fold. Forward a generic lighting snapshot or reuse interior
classification if the mismatch matters; original probes/groups are available in
`spawn/prop_light.rs:22` and `spawn/mod.rs:801`. Do not freeze a world-room PVS tag
onto an attached trophy: it must remain visible after leaving that room.

### Real inn content and containment

`World\Maps\Azeroth\Azeroth_31_49.adt` contains MODF UID **71414**:
`world\wmo\azeroth\buildings\goldshireinn\goldshireinn.wmo`.
WoW origin **(-9464.247, 24.399, 56.526)**, Euler **(0,-97,0)**,
doodad_set **2**, name_set **2**. Origin is not the safe spawn point.
Root has 12 groups and 874 MODDs; MODS ranges are 0=(0,88), 1=(88,215),
2=(303,338), 3=(641,233). This placement selects **426** MODDs (0 plus 2).
Among those: 18 BARREL01, 16 GENERALCHAIRLOEND01, 3 INNBED, 1 INNBEDCANOPY,
2 INNBENCH, 5 INNTABLE, 3 INNTABLESMALL, 8 INNTABLETINY.
Counts come from MODD 40-byte records and resolved MODN names, not gameplay sizes.
These establish that the requested furniture families really are independently claimable.
MOGP group 5 is commonroom, 3 kitchen, 2 upstairs, 6–9 rooms, 10 cellar;
groups 4/11 are exterior. Verify exact room ownership with MODR, not group names alone.

Portals describe visibility/interior connectivity, not physical doors or a seal.
Instances drive WmoGroupVis and liquid scopes (`spawn/mod.rs:307`, `:418`, `:475`).
Blocking a portal's visibility would hide the outside, not stop the ball.
`design.rs:724` inserts `Gate` on authored `solid=true` props, but these still
become `Collectible` (`props.rs:372`); `solid_push` stops blocking as soon as
contact is Dislodge/RollUp (`collect.rs:348`). This is **not permanent containment**.

Use invisible mode-owned, noncollectible static cuboids on normal body layers,
with authored transforms spanning every exterior exit and overlapping jambs/floor.
They need no render entities and should participate in camera casts where useful.
Keep them tall enough for the largest L1 ball and maximum step; sweep collision
prevents dash tunneling through a thin post-correction box. Do not edit portal data.
Use the same barrier policy in recovery/AI; exclude outside positions from safe-history.
Clear/recreate barriers on level change; validate collider readiness before control.

D10 needs an L1-specific interpretation: intentionally sealed exits eliminate exit
clearance as the win condition, but cannot eliminate internal accessibility.
Preload/resolve the complete 426-record inventory, including NPCs/authored items;
current `design.rs:905` assigns rooms by circle and ±3 yd height, and `:937`
checks additive volume against doorway height. Replace that approximation with
room membership and navigable connections for this multistorey level.
Compute worst-case radius from exact credited box volumes for each room; ensure
inter-room openings and ceiling clearances remain passable at every required phase.
Large beds can require more radius than upstairs passages admit: lower credit,
limit the edible set, or finish downstairs; never rely on Backspace as the solution.
Provide a same-room safe reset/checkpoint and a terminal objective reachable without
exiting. Explicitly test ceiling growth, stairs, cellar, shedding, and barrier corners.

## 2. Whole buildings: retirement plus visual trophies

Recommend **engine placement until pickup**, plus a lightweight mode target record.
Before pickup the original supplies exact walls, floors, portals, lights and liquids;
a large collectible OBB supplies eligibility/contact, not duplicate world collision.
Buildings should not enter the ordinary [1.5,2.15) dislodge/tumble path.
A target stays static until a committed pickup; it may touch at its nearest OBB face,
not only its origin/centre (current generic contact is at `collect.rs:481`).

Add a generic `SuppressPlacement{map,uid,generation}` request and acknowledged
result. Names here specify intent, not an existing API. Persist suppression while
ADT references remain and in a mode-owned tombstone set across re-registration.
A suppressed placement retains refcount/identity but has no active world products;
use an explicit lifecycle enum, not `spawned=true` as a synonym for several states.
`Placements.by_id` is private and shared across tiles (`terrain_stream.rs:128`);
Azeroll should not manipulate its vectors or decrement refs to simulate pickup.

Factor a retirement helper from `release_placement` (`terrain_stream.rs:1247`):

- Cancel unspawned groups/props and subtract pending counts exactly once.
- Drain **all** `p.entities`: WMO meshes, portal instance, liquid surfaces, lights,
  animation/effect owners, prop probes and already-flushed prop hulls.
- Drop queued HullWelds and StaticMerge prop accumulators for this UID, or gate
  their flushes on active lifecycle state. Keeping a suppressed row means their
  current “owner exists” check is insufficient (`weld.rs:137`, `merge.rs:302`).
- Despawn PendingCollider owners so late async results cannot recreate bodies
  (`terrain_stream/collider.rs:118`); remove active hulls and invalidate collision
  answers. Existing collider epoch watching is in `collision.rs:82`.
- Release retained WMO **and prop** regions keyed by portal instance. Existing
  cull cleanup observes missing instances (`static_gx/cull.rs:169`); render-side
  buffers follow absent region keys (`static_gx/render.rs:371`). Verify scheduling
  or provide explicit release_instance so a pickup cannot leave a stale-frame city.
- Exclude suppressed rows from `sync_interior_volumes` (`terrain_stream.rs:1146`)
  and invalidate room/fog/liquid/light caches before next movement/render extraction.
  WmoResidency currently keys asset IDs, not placements (`interior.rs:35`): removing
  one of two identical houses may not change its set. Bump a placement-generation
  signal explicitly; the same is essential for partial-city edits.
- Release separately claimed MODDs too; tombstone or retire uncollected children.
  Attached furniture remains attached; never double-credit a building's contents.

Commit after a trophy asset is ready, before the next physics/visibility use, with
acknowledgement before volume/win credit. Apply deferred changes at a defined boundary;
if physics spatial queries synchronize later, freeze motion through that boundary.
If removing a supporting floor, choose a validated terrain/proxy support position;
ADT underneath Stormwind may be holed or unsuitable. Do not drop the ball into it.

A visual trophy uses `Handle<WmoModel>` plus static ModelForms and selected
`submeshes`, filtering `submesh_group` in parallel (`benilla-assets/src/wmo.rs:42`).
Use `spawn_model_entities` with no merge/static_gx diversion, no WmoPortalInstance,
no WmoGroupVis, no physics/liquids/lights, identity local transforms under a trophy root.
Recenter geometry around the chosen model-space bounds centre, preserve world
orientation through inverse BallSpin, then let BallSpin rotate it (D3).
Retain asset/form handles independently of streaming; share materials and mesh handles.
Use frozen exterior/readable lighting; a rolling interior cannot retain world fog/PVS.
Prefer a reduced exterior silhouette over every interior batch and all MODDs.

Claim-before-streaming is simpler only for deliberately authored set pieces with
mode-supplied collision. Otherwise it transfers all pre-pickup floor/portal/liquid
responsibility to Azeroll and duplicates the streamer. Avoid it for the live inn/city.
It is useful for a finale proxy scene or a small house with an approved simple hull.

## 3. Cathedral identity, size, and the extreme-scale problem

Real MODF: map 0, UID **10047**, found in `Azeroth_30_48.adt` (shared references
can occur elsewhere), root `world\wmo\azeroth\buildings\stormwind\stormwind.wmo`.
Origin **(-8931.634,539.294,101.957)**; Euler **(0,38.5,0)**; set/name_set=0.
Root MOHD says **306 groups**, MODD has **6026** records. The measured Cathedral
is embedded in this placement; there is no independent Cathedral MODF for this
geometry. The surveyed area covered tiles x=29..32, y=45..49.

Actual MOGP identities: 109=`cathy_facade01`; 112=`holy01` (Cathedral Square);
95=`cath01`; 134–151 contain the remaining cath rooms; 173–183 the Cathedral
crypt/stairs (description “Cathedral of Light”). Do not use the earlier MOGI-name
listing as an index manifest: actual group-file names establish these indices.
Group 109 MOVT bounds: **(-566.460,-32.441,8.474)..(-439.262,48.931,173.878)**,
axes **127.198 × 81.372 × 165.403 yd**. Façade alone needs D=**355.617 yd**.
Group 112 axes are **195.636 × 184.132 × 131.902 yd**; it includes surrounding
exterior geometry and is not established as a clean cathedral-only mesh.

A conservative candidate assembly is {95,109,112,134..151,173..183}.
Union of its actual MOVT bounds, WMO-local WoW axes:
**(-591.747,-82.807,-29.935)..(-315.143,101.325,173.878)**.
Full axes **276.604 × 184.132 × 203.813 yd**, maximum **276.604 yd**.
At vanilla unit scale D4 requires **D ≥ 594.698 yd**, **r ≥ 297.349 yd**.
Its transformed conservative world AABB is approximately
**(-8736.549,656.177,72.022)..(-8405.452,972.470,275.835)**;
that rotated AABB is for discovery/contact, not the D4 oriented-axis threshold.
This is a measured conservative candidate, **not a verified watertight extraction**.
If authored selection changes, recompute A and credit; façade-only is a different target.
Whole city's actual MOVT union axes are **807.594 × 1059.878 × 348.908 yd**;
collecting the entire city under D4 would require D≈**2278.738 yd**, not 595.
Raw MODF bounds are broader placement/world boxes and must not replace model axes.

Partial-city collection is a second engine feature, not a boolean WMO hide flag.
Retained regions admit groups, but collision aggregates all groups
(`benilla-assets/src/wmo.rs:60`, `:73`), and portals span group connections.
Author a versioned target manifest with selected render geometry/groups, child MODDs,
collision membership, portal boundary policy, safe replacement floor and bounds.
Audit group 112 for other buildings/square geometry; split its submeshes/triangles if
needed or deliberately define the target as “Cathedral precinct”. Keep immutable
asset data shared; make active-group masks and derived collision per placement.
Rebuild/replace walk and camera colliders without selected faces; merely hiding
retained groups leaves walls, floors and interior claims. Disable removed groups in
portal traversal, down-ray classification, fog, probes and liquids as well as drawing.
Props shared by removed/surviving groups need explicit manifest ownership, not
“any removed MODR reference”. Prebuild the variant during a covered stage transition.

### What fails with unchanged large-ball tuning

Derived from D4/D8 and current `ball.rs:136`, `:414`, `:513`:

| r, yd | flat speed, yd/s | base boom, yd | step, yd |
|---:|---:|---:|---:|
| 100 | 147.71 | 702.2 | 45.2 |
| 200 | 209.69 | 1402.2 | 90.2 |
| 300 | 257.14 | 2102.2 | 135.2 |

Dash multiplies speed by 1.5; beats/large-scale framing can lengthen the boom further.
At the candidate threshold base boom is **2083.642 yd**, far beyond detailed farclip
777. Even r=100 has little room for scenery behind the ball; at r=200 the ball's
near surface is itself about 1200 yd from the eye. CPU/shader far walls still apply
although projection far is 3000 (`benilla-world/src/view.rs:242`, `:272`).
Fog ends at min(zone fog, farclip) (`lighting/resolve.rs:17`). Increasing projection
alone changes neither fog nor residency. Window at 777 is ±26 chunks (~867 yd)
about the **ball**, not a 2100-yard-away camera (`terrain_stream/window.rs:85`).
WDL horizon is no substitute for nearby detailed geometry seen from such a camera.
There is no demonstrated automatic simplified moving WMO LOD; using full city batches
as trophies creates a transform/draw/memory problem, even if they fit the view.
73 support rays stay constant in count but become sparse across a 600-yard footprint;
ray length is 4+2r, and swept-sphere broadphase overlaps many city triangles.
Step height now reaches roof scale and can snap onto unintended floors. Sparse samples
can miss small supports or choose disconnected roofs. None of these costs are measured.
At ~9000-yard coordinates f32 spacing is roughly 0.001 yd: precision is secondary to
camera/collision/residency here. Depth range and tiny detail still need care; do not
start with an origin-rebase rewrite as the proposed fix.

### Least-risk finale

Use a Katamari-inspired change of playable detail tier: retire/bury tiny trophies,
retain recognizable silhouettes, and change the stage representation at a deliberate
beat. This is a design analogy, not a claim about that game's internal implementation.
Cap ordinary live traversal at a playtested radius (initial candidate 10–20 yd,
not a performance guarantee). Enter a covered/preloaded Cathedral finale scene,
with bounded camera boom and a reduced exterior Cathedral assembly in local space.
Use a scripted eligibility moment and explicit finale objective instead of pretending
the unchanged physical D4 gate was met. Show the Cathedral lifting and becoming a
recognizable rotating trophy; end the stage before free traversal exposes missing city floors.
The final tableau can use the same map 0 data with a mode camera/proxy layer.

If strict D4 is nonnegotiable, uniformly compress **all** relevant finale geometry,
positions, colliders, ball radius and distances in a local scene, and track displayed
campaign size separately. Scaling just the visible ball does not fix support, camera
or city collision. At r=297, factor ~0.05 yields physical r≈14.9; this is an authored
alternate representation, not a new global WoW scale. It costs more than a scripted
pickup and needs explicit conversion of velocity, gravity/step policy and growth credit.

## 4. Elwynn-to-Stormwind residency and claims

Goldshire (-9450,60) to the proposed gate (-9000,500) is ~629 yd straight;
the candidate Cathedral bounds centre (~-8571,814) is ~1158 yd from Goldshire.
These are planar distances, not validated traversable road lengths or gate coordinates.
The current 680-yard leash physically prevents the latter (`level.rs:35`, `ball.rs:822`),
and the 650-yard claim circle leaves city doodads engine-owned (`props.rs:124`).
Use per-level immutable claim domains: L1 inn UID; L2 current circle; L3 a union of
corridor capsules/area polygons plus explicitly selected WMO targets. Install the whole
L3 domain before streaming, not a moving claim circle: decisions latch at registration.
For same-map stage changes either reload affected placements under cover or offer an
explicit reclassification transaction; replacing the predicate alone is insufficient.

Streaming already follows the ball through ViewFocus; it requests nearest tiles,
one per paced frame, with ~4 ms tile/placement budgets and a 150-placement spawn cap
(`terrain_stream.rs:63`, `:669`, `:708`; `spawn/mod.rs:46`).
At farclip 777 the outer-minus-inner margin is only ~67 yd: at 257 yd/s that is
~0.26 s, less with dash. Whole-tile requests can add lead, but cold web downloads,
full Stormwind decoding and collider creation can exceed it. No throughput guarantee.
Prewarm city root, group forms and collision variants before the approach; decouple
look-ahead residency from draw farclip through a small generic prefetch corridor/window.
Gate speed/control on collision-ready corridor coverage, not just terrain under the
centre (`ball.rs:519`). Budget for swept radius + next motion + stopping distance.
Use loading cover/checkpoints at chapter boundaries if browser readiness falls behind.
Spawn only resident claimed copies; spatially index loose targets and prune inactive
registry payloads, retaining compact tombstones. Do not eagerly spawn a 1500-yard world.

## 5. Trophy and wasm budget

The 500 cap counts roots, not mesh batches (`collect.rs:29`, `:644`). Each copy
creates batch entities and moving transforms; generic pickup can compare each loose
object against every trophy (`collect.rs:490`) and seat against prior trophies (`:138`).
A single detailed WMO can dominate cost despite counting as one trophy.
Use budgets for visible batches/vertices, animated parts, retained asset memory and
maximum trophy roots. Freeze effects/rigs; use a handful of exterior-only WMO trophies.
Bury small items earlier at scale transitions; bake older rigid opaque/cutout trophies
into ball-local material groups, not world-static regions. Avoid one expensive rebuild
per pickup: batch during transitions or stagger work. Transparent parts need a separate
budget/order policy. Preserve exact D4 credit independently from visual simplification.
No full-detail city trophy. Stock WmoModel loads group files together
(`benilla-assets/src/wmo.rs:1`); a group-selection trophy alone does not save load memory.
For wasm prioritize prebuilt reduced geometry, bounded allocation peaks, and readiness
without threads/std time. Benchmark cold/warm local and hosted data on actual WebGPU;
report CPU frame time, draw/vertex counts, WASM/GPU memory and transition peak latency.

## Recommended architecture decisions to make now so the finale isn't painted into a corner

- Typed map/placement/subobject keys from day one; tombstones outlive stream residency.
- Separate collectible target/credit, physical solid, visual trophy and source lifecycle.
- Keep structural WMOs engine-owned until retirement; MODD claims exclude every lane.
- Keep per-placement masks/lifecycle separate from immutable shared WmoModel assets.
- Permanent containment uses noncollectible solids; furniture collision is explicit policy.
- Chapter configs own start, claim domain, bounds, scale policy and reset behavior.
- Allow an explicit scripted finale policy; do not promise unrestricted giant-ball physics.
- Author a versioned Cathedral assembly and reduced trophy; final bounds remain auditable.

## Phased implementation and acceptance gates

1. **L1 seam:** typed keys/messages, resolver claim, release/reset accounting, furniture
   copies. Verify selected set indices, no duplicate render/hull after stream-out/in,
   carried-root survival, failed loads, and default engine behavior with no consumer.
2. **L1 playability:** permanent exit solids, exact-room inventory, furniture solids and
   safe progression. Audit all exterior exits and every required passage at worst-case
   radius; play from 0.25 through beds without recovery dependency. Test dash/shedding.
3. **L3 foundations:** level claim corridor, resident copy lifecycle, prefetch/readiness,
   spatial query/trophy budgets. Travel Goldshire→city with cold browser data before
   adding building pickup. Measure initial city load and memory peaks.
4. **Small whole-WMO pilot:** keep an isolated house live, prepare trophy, request
   suppression, acknowledge, attach. Verify retained on/off and merge/weld variants,
   pending collider cancellation, two identical houses, reset and stream reload.
5. **Finale:** visually audit 109/112 and Cathedral rooms/crypt, define support and
   subset mask, prebuild city variant, stage scripted pickup/reduced trophy under cover.
   If exact clean extraction is costly, end in a proxy tableau; do not retire the city.

Highest-risk unknowns: clean Cathedral exterior ownership within group 112; safe
post-removal support over city ADT; indoor furniture accessibility at its required
pickup sizes; and real browser city-load/large-trophy performance. None was runtime-tested.


## D22 implementation: L1 MODD seam and furniture solids

Implemented `WmoPropKey { map, placement, modd }`, immutable predicate snapshots at
MODF registration, and claim/release messages. The resolver classifies selected
absolute MODD indices before requesting M2s or creating any prop products. Claimed
records never enter engine render/probe/effect/light/collision/weld/merge lanes and
never add pending work. Last-reference release and world teardown announce each key.
With a predicate, overlapping selected ranges are deduplicated. Default-off preserves
the original selection and spawning path, including its range ordering.

`furniture.rs` owns residency and run tombstones and uses `props.rs` frozen-copy
assembly (exact WMO × local transform, M2 box sizing, typed `Source::WmoProp`).
Release preserves attached roots and ball ancestry; reset clears tombstones and
recreates only resident furniture. A streamed-out remembered prop is not respawned.

Furniture collision uses explicit `SolidShape::{Box, Legs}` policy, independent of
pickup size/volume. Tables, chairs and benches have four corner legs in the lower
half and an upper-half slab; this deliberately conservative chair-back approximation
lets a sufficiently tiny ball pass between legs. Other items use a 95% oriented box.
Movement and Unstuck share `solid_push`; autopilot uses the same generated boxes and
vertical overlap. Solids apply only in D6 Wall state; Dislodge and RollUp remove the
wall. **Mode solids supply no floor support**: balls cannot stand on furniture yet.
Copies retain the existing frozen matte treatment, without MODD interior-light folds.
Engine loading treats claims as settled; L1 control readiness must also wait for the
mode copies to finish loading/assembly (not yet included in the engine loading bar).

The static embedded `wmo-claims.toml` allowlist targets map 0 / MODF UID 71414.
`root` documents the expected asset; the generic query contains model path and key,
so it does not validate a root fingerprint. Light sources, smoke, structural/decorative
fixtures, shelves and wardrobes remain engine-owned. These fixtures may constrain
navigation and still require a room/clearance audit before L1 can ship.

Read-only MPQ enumeration of the inn selected set (426 records; 0 plus 2), grouped
by exact MODN path, follows. `inn_inventory_from_install` also resolves the runtime
parser's selected list when WOW_DATA is present.

Claimed records: **353**.

| Count | Model | Owner |
|---:|---|---|
| 2 | `world\azeroth\duskwood\passivedoodads\webs\webdangle02.mdx` | engine |
| 3 | `world\azeroth\duskwood\passivedoodads\webs\webstretch01.mdx` | engine |
| 9 | `world\azeroth\elwynn\passivedoodads\jars\jar01.mdx` | claim |
| 11 | `world\azeroth\elwynn\passivedoodads\jars\jar02.mdl` | claim |
| 16 | `world\azeroth\elwynn\passivedoodads\jars\jar02.mdx` | claim |
| 10 | `world\azeroth\elwynn\passivedoodads\jars\jar03.mdl` | claim |
| 3 | `world\azeroth\westfall\passivedoodads\rugracks\rugrack01.mdx` | engine |
| 31 | `world\azeroth\westfall\passivedoodads\utensils\bowl.mdx` | claim |
| 23 | `world\azeroth\westfall\passivedoodads\utensils\stein.mdx` | claim |
| 3 | `world\dungeon\goldshireinn\innbed\innbed.mdx` | claim |
| 1 | `world\dungeon\goldshireinn\innbedcanopy\innbedcanopy.mdx` | claim |
| 2 | `world\dungeon\goldshireinn\innchandelier\innchandelier.mdl` | engine |
| 6 | `world\dungeon\goldshireinn\innpillow\innpillow.mdx` | claim |
| 2 | `world\generic\human\passive doodads\animalheads\duskwoodboarhead01.mdx` | engine |
| 1 | `world\generic\human\passive doodads\animalheads\duskwooddeerhead01.mdx` | engine |
| 1 | `world\generic\human\passive doodads\artwork\stormwindpainting01.mdl` | engine |
| 2 | `world\generic\human\passive doodads\artwork\stormwindpainting01.mdx` | engine |
| 1 | `world\generic\human\passive doodads\artwork\stormwindpainting02.mdx` | engine |
| 1 | `world\generic\human\passive doodads\artwork\stormwindpainting03.mdx` | engine |
| 1 | `world\generic\human\passive doodads\artwork\stormwindpainting05.mdx` | engine |
| 25 | `world\generic\human\passive doodads\beerkegs\beerkeg01.mdx` | claim |
| 19 | `world\generic\human\passive doodads\beerkegs\beerkeg02.mdx` | claim |
| 2 | `world\generic\human\passive doodads\benches\innbench.mdx` | claim |
| 3 | `world\generic\human\passive doodads\bookshelves\abbeyshelf01.mdx` | engine |
| 3 | `world\generic\human\passive doodads\bookshelves\duskwoodbookshelf01.mdx` | engine |
| 8 | `world\generic\human\passive doodads\bookshelves\duskwoodbookshelf02.mdx` | engine |
| 29 | `world\generic\human\passive doodads\bookstacks\generalbookstackshort01.mdx` | claim |
| 16 | `world\generic\human\passive doodads\bookstacks\generalbookstacktall01.mdx` | claim |
| 19 | `world\generic\human\passive doodads\bottles\greenbottle02.mdl` | claim |
| 18 | `world\generic\human\passive doodads\bottles\greenbottle02.mdx` | claim |
| 3 | `world\generic\human\passive doodads\candelabras\generalcandelabra01.mdl` | engine |
| 11 | `world\generic\human\passive doodads\candelabras\generalcandelabra01.mdx` | engine |
| 2 | `world\generic\human\passive doodads\cargonets\deadminecargonethangshort.mdx` | engine |
| 16 | `world\generic\human\passive doodads\chairs\generalchairloend01.mdx` | claim |
| 5 | `world\generic\human\passive doodads\clothing\bootsleatherdark.mdx` | claim |
| 5 | `world\generic\human\passive doodads\clothing\foldedpantsbrown.mdx` | claim |
| 5 | `world\generic\human\passive doodads\clothing\foldedpantsgrey.mdx` | claim |
| 6 | `world\generic\human\passive doodads\clothing\foldershirtgreen.mdx` | claim |
| 4 | `world\generic\human\passive doodads\clothing\hangingshirtbrown.mdx` | claim |
| 7 | `world\generic\human\passive doodads\crates\stormwindcrate01.mdx` | claim |
| 1 | `world\generic\human\passive doodads\flowers\flowerswreath02.mdx` | engine |
| 1 | `world\generic\human\passive doodads\food&utensils\cleaver01.mdx` | claim |
| 2 | `world\generic\human\passive doodads\food&utensils\haunch.mdx` | claim |
| 1 | `world\generic\human\passive doodads\food&utensils\turkeyleg.mdx` | claim |
| 5 | `world\generic\human\passive doodads\hanginglanterns\generalhanginglantern01.mdl` | engine |
| 1 | `world\generic\human\passive doodads\meat\bloodymeat01.mdx` | claim |
| 1 | `world\generic\human\passive doodads\meat\bloodymeat02.mdx` | claim |
| 7 | `world\generic\human\passive doodads\mugs\mug02.mdl` | claim |
| 3 | `world\generic\human\passive doodads\plates\generaldirtyplate01.mdl` | claim |
| 1 | `world\generic\human\passive doodads\rugs\generalbearskinrug01.mdx` | engine |
| 4 | `world\generic\human\passive doodads\rugs\stormwindrug01.mdx` | engine |
| 6 | `world\generic\human\passive doodads\rugs\stormwindrug02.mdx` | engine |
| 5 | `world\generic\human\passive doodads\tables\inntable.mdx` | claim |
| 3 | `world\generic\human\passive doodads\tables\inntablesmall.mdx` | claim |
| 8 | `world\generic\human\passive doodads\tables\inntabletiny.mdx` | claim |
| 4 | `world\generic\human\passive doodads\wardrobe\duskwoodwardrobe01.mdx` | engine |
| 2 | `world\generic\human\passive doodads\wardrobe\duskwoodwardrobe02.mdx` | engine |
| 18 | `world\generic\passivedoodads\barrel\barrel01.mdx` | claim |
| 16 | `world\generic\passivedoodads\furniture\containers\sack01.mdx` | claim |
| 1 | `world\generic\passivedoodads\furniture\containers\sack02.mdx` | claim |
| 1 | `world\generic\passivedoodads\particleemitters\housesmoke.mdx` | engine |

### Verification of the implementation

- `cargo test --profile play -p benilla-app --lib azeroll`, with WOW_DATA: **105 passed**.
  The final test binaries were built with both app/world packages selected to reuse
  the app's dependency features; the app suite was run again after the engine build.
  The install-gated inventory test asserts 426 selected / 353 claimed records.
- Full `benilla-world` lib suite (play, no install): **565 passed, 2 failed, 6 ignored**.
  All four added seam tests pass. The unchanged failures are
  `lighting::global_light::tests::the_torch_commits_the_raw_authored_product`
  (fixture omits the existing `FoliageCutout` resource) and
  `particles::buffer::tests::write_order::without_the_edge_the_tripwire_names_it`
  (expects a debug assertion to panic in the optimized profile).
- Native `cargo build --profile play -p azeroll` passed with app-only
  `--config profile.play.package.benilla-app.codegen-units=255` to shorten codegen.
- Native floor fixture (`AUTO=60`, `FLOOR_TEST=1`, radii 0.25/0.5/1.2,
  `DEBUG_RADIUS=1.2`) announced exactly **353** furniture claims. At r=0.5 it
  collected Jar MODD **548** and Stein **606**. Floor telemetry matched bottom/support.
- Targeted common-room path from WoW (-9465.2,25.6,56.4) toward
  (-9462.34,25.58,56.4): at **r=0.15**, reached within **0.13 yd** of the table
  centre, longest measured stall **0.11 s**. At **r=1.2**, correctly dislodged a
  chair (axis 1.33) and remained blocked by the full-sized table (axis 2.73).
  A follow-up at **r=1.5** collected Chair MODD **466**, verifying D4's threshold.
- Common-room and pickup captures were viewed: furniture appeared once, lights and
  fireplace remained, and the carried chair was visible. Small-ball camera occlusion
  by a nearby chair remains a presentation limitation. Live stream-out/in was not
  exercised; release/reset/tombstone behavior is covered by the lifecycle tests.
- The existing D10 audit now reports `Inn ground floor upper=2.535853 exit=2.0432432`.
  This is concrete evidence that L1 still needs authored progression/clearance work;
  it is not safe to call the inn level complete from the seam/solids alone.
  Run logs: `/tmp/az-furn/{run,targeted,pickup}.log`; two evidence PNGs are saved
  outside git in `/home/jjd/az-patches/`.
- Final WebGPU wasm check passed using the requested `wenilla --no-default-features
  --features webgpu` command and shared `target-wasm` cache, with WASI_SDK pointing
  at the existing project SDK. Four warnings remain in unchanged code.

## D25 implementation pilot (L3)

The generic engine seam now lives in `terrain_stream/retirement.rs`: opt-in snapshots,
map/UID requests and acknowledgements, persistent suppression tombstones, and Ready
restoration. The Stream transaction drains placement-owned products and separately
cancels queued WMO prop welds/merges; interior classification is invalidated per
placement, not only by asset-set changes. Registering a still-suppressed UID settles
without creating products or adding pending work. The retained renderer reaps regions
when their portal instance disappears. Engine defaults publish no snapshots.

Azeroll's `buildings.rs` prepares exterior-group render-only trophies before requesting
retirement. It gates nearest-face OBB contact by D4 using the complete render vertex
box (cached per asset), checks terrain support, and credits only on acknowledgement.
Its recipe allowlist contains only surveyed farmhouses 54190 and 125746. Retained
forms/materials supply the reduced 28% trophy; no moving world collider is created.
Building trophy roots deliberately avoid ordinary dislodgement/shedding.

This implements the pilot mechanism, not phase 4 acceptance. Required live matrix:
retained renderer on/off × merge/weld on/off; pickup during prop/hull loading; repeated
identical houses; last-reference release and re-entry; Ready restore; Free Roll; native
and cold browser route. Verify no stale region/collider for even a single physics tick.
Phase 5 remains unimplemented: no Cathedral group mask or claimed Cathedral win exists.
Group 112 must be audited/excluded before the staged finale, as described in D25.

Claim-union check against `survey/placements.csv` (4,333 MDDF rows): current L1/L2
union claims 4,333; adding L3 still claims 4,333 (zero incremental surveyed copies).
This is inventory evidence, not a frame-time benchmark; unsurveyed tile populations,
claim-copy streaming lifecycle, and the city load still require live measurement.
The real farmhouse root has two groups: 0 flags `0x809` (exterior), 1 `0x2805`
(interior), verified read-only from the installed WMO group files. The pilot trophy
therefore selects group 0 and omits group 1; no guessed Cathedral subset is enabled.

A read-only M2-header inventory calculation (`benilla-m2` bounds at header 0xb4)
checked the authored cargo gates/box-volume ladder. Initial 0.8× barrels/crates open
at D=1.71/2.05, firewood at 3.49; the opening arena supplies enough volume to open
its hay at 5.96 without leaving. Full-size wagons open at 24.70, midsize trees at
26.30, and the final 1.6× wagons at 39.51. Authored arena capacity alone exceeds
r=32 before including trail/map props or buildings. This only proves theoretical
food capacity/gates; collision, spacing, travel order, and timed pacing need gameplay.

Validation of the pilot code: the play-profile Azeroll suite passes 140 tests; all
53 terrain-stream tests pass (including the two new retirement tests); native
`cargo build --profile play -p azeroll` and the requested WebGPU wasm check pass.
The initial r=30 fixture spawned overlapping a house's support and correctly awarded
no building credit. Its support guard was then generalized to retain a validated
terrain floor (`2 + 0.5r` maximum support difference) and settle onto it at commit.
A straight-line r=30 approach also jammed against surrounding scenery; this is
positive evidence that large-radius route clearance still needs playtesting, not a
reason to increase the cap. The final support-settlement pilot is tracked separately.

### Remaining Cathedral increment: implementation handoff

1. Add `GoalKind::Finale` additively. Reuse `goal.diameter` for the physical minimum
   and `goal.area` for the trigger. Require both, plus a ready assembly, before
   `Run.won`/`Flow.finale` are set; an unknown/unready target must not silently win.
   Use an authored safe approach trigger, not giant-ball traversal through the gates.
2. Version the UID 10047 manifest with an asset fingerprint. Start with groups
   95,109,134..151,173..183. Audit 112 visually/triangle-wise; exclude it until its
   neighboring square/building geometry is separated. Compute selected MOVT bounds
   and the assembly pivot from the actual selected batches, not the whole-city box.
3. Prepare exterior render forms/materials ahead of the trigger, with the same
   entity-hosted, no-collider trophy lane as the house pilot. Keep handles alive
   independently of streaming. Missing files/forms keep the goal pending with a
   recoverable failure path rather than removing any city geometry.
4. Add a default-empty per-instance render-group mask to the WMO/retained renderer
   publication boundary. Apply it after portal visibility computation, including
   entity-lane WmoGroupVis batches; PVS must not overwrite it next frame. Do not mutate
   shared WmoModel assets, retire the whole city, or mask group 112 opportunistically.
   The terminal sequence retains original city collision/portals and forbids motion.
5. Drive the lift/arc/reduced attachment from a small `finale.rs` state machine using
   real time. Reuse progress.rs's existing input freeze, slow motion, fanfare/music
   duck and results transition, coordinating its duration so the flight completes.
   Frame the Cathedral with a camera override that restores on every exit/skip.
6. Keep the absurd displayed diameter in presentation state; never set the physical
   BallState radius to the real Cathedral's ~300 yd radius. Author a dramatic pullback
   inside the measured far-clip envelope; keep streaming focus/support at the safe
   trigger during the shot.
7. Clear the render mask/override on Results, Ready, Select, interruption and before
   Results→Free Roll. The ordinary L3 Free Roll radius limit remains 40 yd. A skipped
   or restarted finale must not leave missing city groups or a slowed clock.
8. Acceptance: actual masked city/visible flying assembly in native and WebGPU;
   skip/restart/Free Roll cleanup; missing-asset fallback; no physical body added to
   BallSpin; no change in L1/L2 draw/collision behavior with an empty render mask.

Final live-pilot evidence: UID 54190 prepared at D=60.03 with measured render-box
axis **18.87 yd**, then retired with **685.07** normalized volume credit. The ball
settled to terrain, continued moving/collecting, and ran at roughly **57–61 FPS**.
The exterior trophy seat was subsequently moved outward so its silhouette is not
lost in the clump. UID 125746, renderer-off/merge-off/weld-off combinations, stream
reload/reset and cold-browser runs remain acceptance work.

The first complete normal-size L3 probe timed out at **600 s**, **r=21.79**,
**894 pickups**. It advanced through the lumber/farm arenas but repeatedly stalled
on the city approach and hunted in the hills. At the end, instrumented ball movement
was ~0.41 ms mean / 0.75 ms p95 and collection ~0.20 / 0.25 ms, with ~60 FPS; the
failure was route/content clearance, not measured frame time. The final cargo was
moved from (-9050,460) to the already-traversed clearing (-9140,400), with 120 royal
wagons, and the goal area moved to (-9110,410). This correction still needs a complete
normal-size run; the original failed run is not a pass for the revised recipe.

The corrected final-band fixture passes: starting at r=19.5 in the clearing, it
reached r≈32.2 / D≈64.4 in **2.6 s**, triggered the ordinary win/results flow and
held ~60 FPS. The capture frames the Stormwind gate behind the clump. This proves
reachable final cargo and a working terminal size goal, **not** an 8–10 minute pass
from the ordinary start. The burst is intentionally dense and still needs pacing
review as part of a complete revised-recipe run. Evidence is preserved outside git
under `/home/jjd/az-patches/l3/`, including both fixture recipes and logs.


### D27: Stormwind city decomposition

The city uses the existing placement streamer with a private derived WMO, rather than a
second mode-owned streamer. UID10047 keeps its tile references and immutable source asset.
A versioned embedded manifest guards306 groups with the source root fingerprint, separates
36 ordinary structures plus the Cathedral, and appends synthetic group IDs after the
permanent source groups. Mixed groups285 and112 use raw-XYZ triangle-centroid selectors;
whole groups cover standalone buildings/rooms. Streets, canal beds, bridges' supporting
decks, low floors and foundation covers stay permanent. The outdoor representation omits
closed interior render batches while retaining their walls/floors until pickup. No retired
room can keep claiming a portal or interior-light subject.

Per-piece render products, static_gx regions, walking/camera colliders and owned MODDs retire
in one Stream transaction. Pending MODDs are cancelled; claimed loose copies are released,
while carried copies survive. Support is checked over the ball's footprint with retiring
colliders excluded. D4 max-axis eligibility, contact and full OBB-volume credit use the
original piece. Credit and a45% exterior trophy appear only after retirement acknowledgement.
Ready cancels outstanding merge/weld products and rebuilds the changed city once. Tombstones
survive streaming. Ungated world/map teardown releases snapshots and derived strong handles,
keeping authored plans/tombstones. Whole suppression rejects part requests, and combined
whole/part restoration queues one rebuild. Part snapshots publish on change after a single
collider-product index pass. Lifecycle/conservation tests cover these seams.

`WOW_WMO_PART_AUDIT=/absolute/audit.toml` produces a versioned metadata report: source
fingerprint, bounds, max axis, box volume, selected render/walking triangles and MODDs per
stable piece ID. Source totals are306 groups,837,867 MOVT vertices,909,573 MOVI triangles and
6,026 MODDs. The separate UID10047 entry in `wmo-claims.toml` lists522 reachable loose props,
including69 trees.91 shallow planters remain scenery. TreeFacades293–304 are distant,
trunkless canopy cards, omitted in this outdoor representation; real MODD trees remain.
No global claim of every city MODD occurs.

Increment1 validated a40.08yd-axis townhouse: D80 correctly refused it, D96 retired it with
4310.91 normalized credit, and Ready rebuilt it for another pickup with fresh colliders.
The current street-sized route enters near r3 and crosses the measured canal bridge before
reaching the Cathedral tree fair. Its reproducible generator is `data/stormwind-gen.py`.
The tree ladder uses audited Canopy04 bounds (axis82.90032yd, normalized D4 volume26565.5116),
with an8% radius reserve before every rung;723 rooted authored trees supplement real city trees,
market cargo and104 guards/citizens. `rooted = true` prevents ballistic dislodging before
normal pickup size, keeping the basal trunk solid; other scene items default to false. A live-mover gate regression covers three segments at
r2.3/r2.96. Navigation ends at the open Square, and late autopilot food preference stays in
the terminal district. These are planner choices; player collision and pickup rules remain
unchanged.

A generic optional support skin renders retained upward collision-only faces0.035yd below
the unchanged collider. Source upward floor batches supply material and affine XY-to-UV
mapping, keeping original street texel density/alignment. Triangles are clipped to authored
city bounds. The Cathedral keeps a bounded floor foundation after lifting. A separately
capped64-instance building skin uses exterior meshes of structures actually collected,
with roofs outward and slots distributed over both hemispheres. It adds no credit/collision.

The orange plane initially seen behind the Cathedral is detailed ADT terrain, not added
support geometry: it survives NO_SKIN, NO_LIQUID, NO_WDL, NO_FOG, NO_SKY, NO_M2 and NO_WMO
comparisons, and disappears under NO_TERRAIN. From the exact same lens, the untouched stock
WMO shows the same plane. Entity-lane census at that pose found stock306 resident groups,
23 drawn/209 batches; derived138 resident groups,25 drawn/203 batches. Stock-only backdrop
cards295/304 do not cover it. The extraordinary aerial lens was looking beyond the city's
rear boundary. The new establishing lens looks inward from the Cathedral's rear quarter,
with Stormwind as backdrop; terrain is unchanged. Comparison captures and draw records
live in `/home/jjd/az-patches/city/visual-source-{stock,derived}.png` and the corresponding
`source-*-draw.txt` files. Set STATIC_GX/MERGE=0 when using WOW_WMO_DRAW_AUDIT for this census.

The finale requires24 structures and physicalD160 at Cathedral Square. It hides only the
Cathedral synthetic group, leaving112 and permanent decks live, and attaches a reduced
exterior trophy with a595yd displayed final result. The physical radius remains unchanged.
The source mask stays held while Results displays its trophy; Ready, Select and Free Roll
restore it. The clock is30min, par15min, speed challenge11min and Free Roll capr110.

Final-route native evidence: run14 won in351.7s (5:51.7), with zero game Unstuck and five
planner escapes. It triggered the Cathedral at physicalD210.84 and recorded595yd. Frame
p50/p95/p99 were16.7/17.8/19.8ms, and Free Roll cleared the display/mask. The second normal
run15 won in337.5s (5:37.5), zero game Unstuck and six planner escapes, physicalD209.26 at
the trigger. It used the embedded registry with no file override; both recipes have FNV
e9297de5287c97e5. Run15 frame p50/p95/p99 were16.7/18.6/33.3ms. An actual Ready-after-three
probe restored/recollected parts43/49/40 for identical normalized credits4093.30/493.79/
4046.47 and fresh colliders; the sole telemetry relocation was the intentional Ready reset.

The final terrace audit found that the large tree rows had originally been authored below
the rising floor, so their placement rays missed and most future fuel never spawned. All
rows now use measured group112 floor heights. The installed regression checks every actual
`plan_item` position against the same ray contract. A separate installed gate-guard test
spans five radii and five lane offsets; recovery rays remain below the entrance overhang
through rawX=-90. Canal navigation capsules extend12yd around the measured bridge rather
than6yd, covering the detour that previously dropped the planner beneath its rail.

Current checks pass: native play build;4472 full app tests without data (10 ignored);246
installed-data Azeroll tests;59 terrain-stream tests; WebGPU wasm check. The three known
full-app installed-data failures are legacy enGB-only localization assumptions against enUS
client data. Browser runtime performance remains unmeasured. Pacing is currently about6min
automated rather than the requested10–12min; the next character pass should separate the
fair into deliberate district vignettes, preserving the floor/volume/support guarantees.
