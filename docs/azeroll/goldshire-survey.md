# Goldshire level-design survey

This survey describes client-map scenery around `START=(-9450,60,57)` in WoW world yards:
**+X is north, +Y is west, +Z is up**. Distances below are horizontal. The 600-yard inventory
circle sits inside the 1200×1200-yard terrain/screenshot square. The current playable leash
is only 520 yd and collectible claim radius only 450 yd; extending the designed level to the
farms requires revisiting both. [Mode constants](../../crates/benilla-app/src/azeroll/level.rs).

## Evidence and reproduction

The primary evidence is this checkout's own installed 1.12.1 ADT/M2/WMO data, decoded by the
engine and collected by [survey.rs](../../crates/benilla-app/src/azeroll/survey.rs).
[placements.json](survey/placements.json), [placements.csv](survey/placements.csv), and
[terrain.csv](survey/terrain.csv) contain the measurements; [data conventions](survey/README.md)
explain sizes, missing values and coverage. Original map placements are distinguished from the
mode's random scatter and local creatures. Those synthetic entities may appear in images but
are not evidence of original WoW object clusters. In particular, chickens/merchants/NPCs are
server spawns, not something an ADT inventory can establish.

A separate [NPC-anchor CSV](survey/reference-npc-anchors.csv) uses the author-maintained
[ClassicDB base SQL](https://github.com/classicdb/database/blob/5df7f7c28debfc8b5660e796505ef36c16b4a0bc/Full_DB/ClassicDB_1_7_z2684.sql).
That is an emulator database snapshot, not Blizzard's server data or an assertion that those
NPCs exist in this offline mode. It supplies named anchors where client model paths are generic.

```bash
PATH=$HOME/.cargo/bin:$PATH cargo build --profile play -p azeroll
WOW_NOVSYNC=1 WOW_BG=1 WOW_DATA=$PWD/wow-data/Data \
  WOW_AZEROLL_SURVEY=$PWD/docs/azeroll/survey \
  timeout 900 target/play/azeroll > /tmp/az-survey.log 2>&1
```

The module registers no resources or systems without `WOW_AZEROLL_SURVEY`. It keeps the mode in
Ready, moves the ball/player streaming focus, uses 550-yd farclip, and waits for presentable
streaming at each stop. Nine square orthographic captures cover the bounding square, followed by
six oblique views. Terrain is sampled on a 10-yd lattice; one-yard collision probes around the
inn separately investigate floors. Only this module and minimal plugin registration change code.
Art references in `survey/img/` are gitignored and must not be committed.

## Capture results and local images

The one requested run completed its survey at 04:02:18 UTC on 2026-11-02 (local evening
2026-11-01): **4,333 doodads + 22 WMOs = 4,355 placements**, all with resolved bounds;
**14,384 terrain columns**, **1,417 reporting water**. The full square has 14,641 possible
lattice columns, so **257 are unreported** (holes/missing ground; no heights were invented).
An [independent 12-ADT inventory check](survey/inventory-validation.json) found exactly the
same placement IDs, with zero missing or extra records. Build passed with the shared play target.
The capture finished before the command's timeout terminated the process (exit 143); clean
application shutdown was not observed. [Run summary](survey/run-summary.json).

The [stitched overview](survey/img/overview-grid.png) is **1212×1296**, with north up, west left,
and coordinate bounds printed on every cell. The nine individual overheads are
`survey/img/overhead-<row>-<column>.png` (rows north→south, columns west→east), each 400×428
including its label; [nominal bounds](survey/shots.json) are also machine-readable. Grid labels
bound the view, not each visible roof or the inventory circle. Tree canopies obscure ground
corridors even from above.

| Oblique reference | What the capture supports |
|---|---|
| [Blacksmith](survey/img/blacksmith.png) | Readable smithy, junction and part of inn. |
| [Square](survey/img/square.png) | Readable junction and inn exterior/roof. |
| [Inn](survey/img/inn.png) | Heavily canopy-obstructed; use square/blacksmith views plus portal/floor data. |
| [North road](survey/img/north-road.png) | Partial road visible through foliage. |
| [Farms](survey/img/farms.png) | Canopy-obstructed shot centred on the Fargodeep approach, **not a useful farm-building elevation**; farm positions come from the southern overheads and placement records. |
| [Forest edge](survey/img/forest-edge.png) | Dense canopy, confirming the camera obstruction problem; not a clear camp-layout view. |

All individual images are ≤1280 pixels wide. The single-run constraint was preserved: obscured
views were not used to infer unobserved ground or followed by another client run. A follow-up
photographic pass should use cleared viewpoints or a survey-only foliage hide, especially for
inn/farm elevations. These are local design references and are excluded from git.

## Landmarks and routes

Building rows give **MODF placement origins**, not front doors. They are exact to the displayed
precision; road/shore/cluster anchors are rounded designer waypoints. Survey z values should be
re-sampled at each final spawn. The overheads show a wooded road junction, rather than a large
empty paved town square.

| Landmark | WoW (x, y, z), yd | What is actually there |
|---|---|---|
| Lion's Pride Inn | **(-9464.25, 24.40, 56.53)** | `goldshireinn.wmo`, uid 71414; conservative group-box footprint **39.00×61.18 yd**, radius 37.10. |
| Inn main outside doorway | **(-9459.16, 44.37, 56.95)** at threshold | Portal 0: **4.42 yd wide ×3.78 yd high**. The west-facing approach is from the junction, not from the building origin. |
| Inner vestibule doorway | **(-9459.93, 38.11, 56.95)** at threshold | Portal 1, same width/height; two aligned restrictions must be cleared. |
| Inn ground floor | **(-9463,16,56.96)** and **(-9460,32,56.97)** | Downward collision rays starting at z=62 hit the interior floor here. Rays from z=67 instead hit the upper floor at **63.82**. Do not place food on the underlying ADT terrain or use the WMO origin's z. |
| Inn narrow interior opening | **(-9468.54,7.76,56.96)** at base | Portal 9, **2.63 yd horizontal opening**. Treat the kitchen/back-room route as a small-ball branch, not a later-stage exit. |
| Blacksmith / forge building | **(-9460.03,94.20,56.53)** | `goldshireblacksmith.wmo`, uid 96048; conservative footprint **64.45×45.45 yd**, radius 41.30. Smith Argus's separate database anchor is (-9456.65,87.86,58.43). Forge/anvil content can be embedded WMO furniture, so “no ADT forge” does not mean an empty smithy. |
| Town junction / START | **(-9450,60,55.96)** ground | A road fork between inn and smithy; useful orientation hub. Signpost anchors at **(-9430.83,63.52,56.62)** and **(-9500.34,67.05,56.32)**. |
| Two-storey house northeast | **(-9387.92,-68.18,64.94)** | `humantwostory.wmo`, uid 54705; roof/box footprint 46.99×48.69 yd. Lumber pile near (-9387,-81,63.73). |
| Other nearby houses | **(-9173.68,2.04,76.28)**; **(-9312.81,287.12,66.43)**; **(-9265.06,355.54,72.58)** | Generic farm-house models; the northwest house has a real nearby cabbage plot. Do not infer named NPC ownership from a repeated model. |
| Western lakeside house | **(-9439.89,467.93,49.17)** | Farm-house WMO above Mirror Lake; wheelbarrow near (-9446,472,52.12). |
| Northshire road | Junction → approximately **(-9300,-35,71.17)** → **(-9118.93,-178.93,69.32)** gate origin | Climbs northeast into the abbey approach. Another gate WMO at (-9056.64,-43.94,87.93); do not confuse the elevated gate geometry with flat road ground. |
| Stormwind road | Junction → northwest through **(-9382,113,~60)** toward **(-9050,460,60.66)** | The northwest overview visibly reaches the Stormwind entrance bridge/causeway. City geometry can intersect the survey even when its MODF origin is outside the inventory circle. |
| Westfall road | Southern junction signpost **(-9500,67,56.32)** → southwest corridor → bridge **(-9741.52,539.38,33.61)** | Existing bridge WMO is near the survey's southwest edge, **561 yd** from START. Road destination continues beyond the survey; the bridge origin is not its walkable deck height. |
| Redridge road | From the southern junction east/southeast, south of Crystal Lake | Use the visible road strip in overhead-1-1/1-2, e.g. **(-9580,-80)** as a route-finding anchor; this is not the distant Redridge boundary. Do not route breadcrumbs straight across the lake. |
| Crystal Lake | Representative water column **(-9450,-340)**; surface **57.63** | Large east-side lake. One connected sampled lobe spans X[-9530,-9400], Y[-470,-230]; additional water/stream cells continue north. The centre terrain is about 55.75, under water. |
| Mirror Lake | Approximately **(-9400,460)**; surface **48.64** | A distinct western lake: major sampled water patch X[-9440,-9350], Y[410,540]. Do not confuse it with Crystal Lake east of town. |
| Small pond west of town | X **[-9460,-9410]**, Y **[150,170]**, surface **55.91** | A small real water obstacle behind/near the smithy approach; 14 wet lattice columns in this patch. |
| Stonefield Farm house | **(-9882.72,321.13,33.63)** | Farm WMO uid 18580; barn **(-9847.44,348.48,35.13)**, wagon **(-9863.60,363.04,36.13)**. House is **505 yd** from START: within survey/leash, outside current claim radius. Ma Stonefield's database anchor (-9895,332.42,36.55) supports the identification. |
| Maclure Vineyards | Barn **(-9910.23,21.57,30.12)**; farmhouse **(-10015.28,39.16,31.06)** | Real vineyard doodads near (-9920,92), (-9956,111), (-9963,61/89); silo **(-9975.33,11.04,30.93)** and hay wagon **(-9905.01,37.88,31.41)**. Barn is 462 yd away; house 566 yd, beyond the current leash. |
| Fargodeep Mine | **(-9833.62,193.52,29.09)** model origin | `md_goldmine_varianta.wmo`, uid 210351, **406 yd** from START. Underground group-box union reaches z≈-0.30; the model origin is not an entrance or surface floor. An exterior-linked portal near **(-9841.82,184.89)** spans z=22.51–30.63. |
| Jasperlode Mine | **(-9154.65,-594.39,62.90)** model origin | Adjacent-ADT check identifies `md_spidermine.wmo`, uid 71446, **718 yd** away, outside survey circle/leash. Exterior-linked opening centre **(-9180.62,-604.48,67.44)**, base z=63.38. No terrain-grid or screenshot coverage claim is made for that opening. |
| Immediate forest edge / current Hogger | **(-9600,260,48.15)** ground | Southwest wooded slopes, not an authored gnoll camp established by this inventory. The forest also begins almost immediately behind the village buildings: there is no clean concentric “forest tier” boundary. |

Door values come from [inn portal vertices](survey/inn-portals.json) and
[collision floor probes](survey/inn-floor-probes.csv). Extra mine coordinates come from
[adjacent ADTs](survey/nearby-mine-anchors.json) and [exterior-linked portals](survey/mine-exterior-portals.json).

## Existing clusters and usable exterior pockets

| Cluster / candidate | Anchor | Design use and limits |
|---|---|---|
| Merchant wagon | **(-9497.19,83.45,56.59)** | Actual gypsy wagon, rope coil/bucket near (-9499,88/89), and **three barrels + a crate around (-9501,79)**. Excellent merchant-unloading story without inventing a new landmark. |
| Inn porch goods | **(-9473,41.5,58.04)** | Three real jars; water basin **(-9471.06,44.35,56.46)** and fence rails at the approach. Extend with a food spill, keeping the exit lane clear. |
| Smithy training corner | **(-9462,115–127,58–59)** | Two wooden dummies and archery target. Useful medium-object cluster; don't put the tiny-food opening here. |
| Northern village goods | **(-9385,60,60)** | Two barrels, crate, bucket, jar. A ready-made second merchant/household stop about 65 yd north of START. |
| Northwest kitchen garden | **(-9282,343,75.53)** props | Existing orderly cabbage rows, approximately 0.28–0.42 visual bounding radius, beside the farm-house WMO. Raised/sloped ground; not a perfectly flat pad. |
| Stonefield harvest | **(-9864,363,36.13)** | Wagon and barn complement the house and nearby field. Strong farm scene, but most objects here are not currently claimed. |
| Maclure harvest | **(-9920,80,~32.5)** | Vineyard rows and hay wagon provide a much flatter exterior sequence than the forest. Grapevines have very large visual spheres (~20.54 yd); their actual pickup thickness needs a gameplay check. |

These are **terrain-flatness candidates, not collision-cleared spawn pads**. Each range below
uses the 3×3 lattice columns in a 20×20-yard square. Trees, fences, roof projections and
claimed props must still be checked. None of these nine-column patches reported water.

| Candidate centre (x,y) | Terrain z range | Suitability |
|---|---:|---|
| (-9450,60), central junction | 55.89–56.94 | Best reliable opening exterior, 1.05 yd relief; keep the road and inn mouth readable. |
| (-9490,60), south junction | 55.99–57.22 | Merchant breadcrumbs/medium goods, 1.23 yd relief. |
| (-9380,60), northern goods | 59.83–61.49 | Small household cluster, 1.66 yd relief. |
| (-9550,140), southwest approach | 58.88–60.43 | Transition pocket; nearby trees require a clearance sweep. |
| (-9840,350), Stonefield barn side | 36.18–38.33 | Farm cluster, 2.15 yd relief; maintain a wide exterior bypass. |
| (-9920,80), Maclure rows | 32.14–33.06 | Flattest listed farm patch, 0.92 yd relief; existing vines subdivide it. |
| (-9600,260), current Hogger | 47.29–49.00 | Local 20-yd patch is modestly sloped; the surrounding 40-yd square rises **46.94–53.48**. Requires a deliberately cleared arena/bypass, not just more props. |

## Navigation hazards

- **Inn aperture:** its main 4.42×3.78-yard portal limits a bare sphere by **height first**;
  theoretical radius ≤1.89 yd, before attached trophies and the vestibule's second doorway.
  Exit the inn well before the 7-yd-diameter tier. The 2.63-yd interior opening is stricter.
  The earlier reported wedge at roughly 5 yd diameter is entirely plausible geometry, not a reason
  to make that route carry larger balls. Portal bounds do not prove collision-clear aperture.
- **Vertical ambiguity:** inn floors near 56.96 and 63.82 differ by almost 7 yd. One ADT table prop
  is at **(-9475.12,39.36,71.11)**. A proximity-only object selector can target something upstairs,
  while a terrain-grounded spawn can land under the building.
- **Water:** Crystal Lake, Mirror Lake, a small village pond, and southwest river columns all occur
  in the sample square. A 10-yd grid cannot resolve the exact bank; keep breadcrumb lanes several
  yards back and do local collision/water queries for final placements.
- **Slope transitions:** the compact Hogger area's surrounding 40-yd square has 6.54 yd relief;
  the northeast road climbs from village ground around 56 to ~71 at (-9300,-35), and the northern
  gate structures reach substantially higher ground. Do not treat distance rings as equal effort.
- **Fences and foliage:** 606 wooden-fence placements plus 173 posts divide the surveyed scene.
  The canopy frequently hides paths in overhead images and chase views. Breadcrumbs need a
  traversable ground corridor, not just line-of-sight between cluster centres.
- **Mines:** Fargodeep has underground branches and surface holes; Jasperlode is beyond the current
  level. Neither is an appropriate large-ball shortcut. A WMO origin or union box is not a door.
- **Synthetic collision differs:** claimed props are mode-owned copies, while buildings retain
  engine collision. Validate routes at the intended ball size with trophies; these measurements
  identify hazards but are not a traversability playtest.

## Hogger: the finale is currently relocated

`HOGGER_AT=(-9600,260,60)` is **250 yd** from START. The run grounds him to approximately
**z=48.15**, so the constant's z=60 is a hint, not the camp floor. This is a compact southwest
forest encounter, not his original Riverpaw spawn area.

The cited ClassicDB snapshot places entry 448 at **(-9946,604.266,38.2862)** with movement type 2
(a waypoint-moving spawn), **736 yd** from START. Therefore even that northern spawn anchor is
outside the 600-yard survey, 520-yard leash and 450-yard claim radius. A precise original patrol
cannot be reconstructed from client placement data; do not turn an external approximate camp
coordinate into a surveyed floor measurement. A faithful Riverpaw-camp finale needs a separate
southwest extension and new streaming/claim/leash design. Keeping the current point is a sensible
compact-level choice if it is explicitly a relocated Hogger encounter, dressed with new gnoll
camp props rather than described as the real camp.

## Size progression: what the map does not supply

Thresholds use **pickup thickness**, not visual bounding-sphere radius: a prop is edible at
`pickup <= 0.65 * ball_radius`. For a diameter D, maximum edible thickness is `0.325 D`.
The placement inventory's radius is useful for visual planning, not a replacement for the
second-largest M2 half-extent used by gameplay. [Growth and collection](ARCHITECTURE.md).

| Ball diameter | Maximum edible thickness at tier entry | Missing level-design content |
|---|---:|---|
| Start 0.5 yd | 0.163 yd | A guaranteed dense opening of crumbs, coins, small utensils; original ADT scenery is not a tabletop layout. Supply a deliberate inn spill instead of random outdoor scatter. |
| Critters 1.2 yd | 0.39 yd | A legible trail of food/bottles into a chicken yard; the actual chickens must be placed by the mode. |
| Creatures 3 yd | 0.975 yd | Cohesive farm stock and merchant parcels; avoid random livestock rings in the square. |
| NPCs/wagons 7 yd | 2.275 yd | People, carts and commodity clusters tied to open exterior pockets; ADTs contain no ordinary server NPC population. |
| Trees/structures 13 yd | 4.225 yd | A continuous transition between medium props and trees. Buildings remain engine-owned WMOs, so this label currently promises collection the mode cannot provide. |
| Hogger 20 yd | 6.5 yd | A staged, readable finale and escape room around the camp. Current Hogger size=4 is already edible at **diameter 12.31 yd**, before the 13-yd tier. |

Use distinct cluster stories: inn food spill → merchant goods/forge scrap → stock and harvest
→ woodcutting/forest edge → a relocated Riverpaw camp. Link them with sparse, consistently edible
breadcrumbs rather than filling all forest ground with random props. Keep growing balls outside
buildings once their diameter approaches passage width.
