#!/usr/bin/env python3
"""Generates crates/benilla-app/src/azeroll/stormwind.toml (level id `stormwind`, "Stormwind or Bust!") from
the act plan in docs/azeroll/katamari-room-design.md §7. Run from anywhere; `--ledger` prints the growth
budget per act and the ladder check. Edit this file, not the TOML.

Coordinates: Elwynn content is WoW yards (x north, y west, z up) and is grounded on terrain, so its z only
documents the surveyed ground. City content is authored in the raw Stormwind WMO frame (placement 10047,
transformed by `city()`), is grounded by the ±4 yd WMO floor ray (`indoors = true`), and takes its z from
`stormwind-floors.json`: walkable-floor heights on a 5 yd grid, measured offline from the WMO (the tool
is /home/jjd/az-patches/city/audit.py's reader; regenerate with floors.py from the same directory). A herd
only spawns if its `at` is within 0.7 yd of its floor, and a scene prop only if its floor is within 4 yd of
the anchor, so every city anchor is checked against that grid here, at generation time.

Sizing: a prop rolls up once the ball's DIAMETER >= 2.15 x its longest scaled extent and adds
0.119 x its scaled box volume to r^3 (D4). Boxes come from props.csv; the city pieces' gates and credits
from the D27 runtime part audit. The ladder check asserts that, at every size band's gate, the content
edible below that gate (plus the city's pieces, and half of its own claimed trees) reaches the reserve.

The piece manifest (stormwind-pieces.toml), the claim list and the navigation capsules are the city
agent's and are reproduced here unchanged."""
import csv, json, math, os, sys

PI = math.pi
RATIO = 2.15
FILL = 0.119
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..'))
LEDGER_MODE = '--ledger' in sys.argv

# ---------------------------------------------------------------- the city frame
C_, S_ = math.cos(math.radians(-141.5)), math.sin(math.radians(-141.5))
def city(x, y, z):
    """Raw Stormwind WMO yards -> WoW (placement 10047)."""
    return [round(-8931.634 + C_ * x - S_ * y, 3), round(539.294 + S_ * x + C_ * y, 3), round(101.957 + z, 3)]
def city_heading(dx, dy):
    """`heading` (0 east, pi/2 south, Bevy) of a raw-frame direction."""
    wx, wy = C_ * dx - S_ * dy, S_ * dx + C_ * dy
    return round(math.atan2(-wx, -wy), 4)
H_RAW_X = city_heading(1, 0)     # along raw +X (toward the gate): 0.8993
H_RAW_Y = city_heading(0, 1)     # along raw +Y: -0.6715
def wow_heading(dx, dy): return round(math.atan2(-dx, -dy), 4)

FLOORS = {tuple(map(int, k.split(','))): v for k, v in json.load(open(os.path.join(here, 'stormwind-floors.json'))).items()}
WARN = []
def floor(x, y, expect=None, tol=4.0):
    """The walkable floor nearest (x, y) on the 5 yd grid, nearest to `expect` when several storeys exist."""
    best = None
    for (gx, gy), zs in FLOORS.items():
        d = (gx - x) ** 2 + (gy - y) ** 2
        if d > 7.2 ** 2:
            continue
        for z in zs:
            k = (d, abs(z - expect) if expect is not None else -z)
            if best is None or k < best[0]:
                best = (k, z)
    if best is None:
        sys.exit('no floor near raw (%s, %s)' % (x, y))
    if expect is not None and abs(best[1] - expect) > tol:
        WARN.append('floor at (%s,%s) is %.1f, expected %.1f' % (x, y, best[1], expect))
    return best[1]
def floor_span(x, y, r, z):
    """Warn when any grid floor within r of (x, y) is more than 3.5 yd from z (a prop there would not spawn)."""
    bad = [(gx, gy, gz) for (gx, gy), zs in FLOORS.items() for gz in zs
           if (gx - x) ** 2 + (gy - y) ** 2 <= r * r and abs(gz - z) > 3.5 and -12 < gz < 14]
    if bad:
        WARN.append('scene at (%s,%s) r%s z%.1f: %d grid floors off by >3.5 (e.g. %s)' % (x, y, r, z, len(bad), bad[:2]))

# ---------------------------------------------------------------- the model palette
P = {}
for r in csv.DictReader(open(os.path.join(here, 'props.csv'))):
    P[r['path'].lower()] = (float(r['bbox_x']), float(r['bbox_y']), float(r['bbox_z']))
GH = r'World\Generic\Human\Passive Doodads'; GP = r'World\Generic\PassiveDoodads'; AZ = r'World\Azeroth'
M = {
 # the camp
 'firewood': GH + r'\firewood\FirewoodPile01.m2', 'firewood3': GH + r'\firewood\FirewoodPile03.m2',
 'lumber': GH + r'\PeasantLumber\PeasantLumber01.m2', 'lumberS': GH + r'\LumberPiles\DeadMineLumberPileSmall.m2',
 'lumberL': GH + r'\LumberPiles\DeadMineLumberPileLarge.m2', 'stump': AZ + r'\Elwynn\PassiveDoodads\Trees\Stumps\ElwynnTreeStump01.m2',
 'stump2': AZ + r'\Elwynn\PassiveDoodads\Trees\Stumps\ElwynnTreeStump02.m2', 'axe': GH + r'\Weapons&Armor\HumanAxe01.m2',
 'saw': r'World\Generic\DarkIronDwarf\Passive Doodads\Tools\DarkIronSaw01.m2',
 'barrel': GP + r'\Barrel\Barrel01.m2', 'wfbarrel': AZ + r'\Westfall\PassiveDoodads\Barrel\WestFallBarrel01.m2',
 'crate': GH + r'\Crates\ReplaceCrate01.m2', 'crate2': GH + r'\Crates\ReplaceCrate02.m2', 'swcrate': GH + r'\Crates\StormwindCrate01.m2',
 'graincrate': GH + r'\Crates\GrainCrate01.m2', 'gcrate': r'World\Goober\G_Crate02.m2',
 'sack': GP + r'\Furniture\Containers\Sack02.m2', 'sackflat': GP + r'\Furniture\Containers\Sack01.m2',
 'bucket': GH + r'\Buckets\bucket.m2', 'keg': GH + r'\BeerKegs\BeerKeg01.m2', 'keg2': GH + r'\BeerKegs\BeerKeg02.m2',
 'chest': r'World\Generic\ActiveDoodads\Chest01\Chest01.m2', 'wftable': AZ + r'\Westfall\PassiveDoodads\WestfallTable\WestfallTable.m2',
 'bread': GH + r'\Food\bread01.m2', 'loaf': GH + r'\Food\BreadLoaf01.m2', 'wheel': GH + r'\Food\CheeseWheel01.m2',
 'mug': GH + r'\Mugs\Mug01.m2', 'apple': GP + r'\Fruits\Fruit_Apple.m2', 'jug': AZ + r'\Elwynn\PassiveDoodads\jugs\jug01.m2',
 'wagon': AZ + r'\Westfall\PassiveDoodads\Westfall Wagon\WestfallWagon01.m2',
 # the harvest road
 'haystack': AZ + r'\Elwynn\PassiveDoodads\Haystacks\haystack01.m2', 'haystack2': AZ + r'\Elwynn\PassiveDoodads\Haystacks\haystack02.m2',
 'haypile': GH + r'\HayPiles\ShadowfangHayPile02.m2', 'haypile1': GH + r'\HayPiles\ShadowfangHayPile01.m2',
 'bighay': AZ + r'\Westfall\PassiveDoodads\HayStack\WestFallHayStack01.m2', 'haywagon': AZ + r'\Westfall\PassiveDoodads\HayWagon\WestfallHayWagon.m2',
 'brokencart': AZ + r'\Westfall\PassiveDoodads\BrokenCart\BrokenCart.m2', 'pumpkin': r'World\Goober\G_Pumpkin_02.m2',
 'pumpkin1': r'World\Goober\G_Pumpkin_01.m2', 'watermelon': r'World\Goober\G_Watermelon.m2', 'corn': AZ + r'\Elwynn\PassiveDoodads\CornCrop\corncrop1.m2',
 'scarecrow': AZ + r'\Westfall\PassiveDoodads\Scarecrow\WestFallScarecrow.m2', 'barrow': GP + r'\Misc\WheelBarrow\CaveMineWheelbarrow01.m2',
 # the convoy and the army
 'gypsy': GH + r'\GypsyWagons\StormwindGypsywagon01.m2', 'tree': AZ + r'\Elwynn\PassiveDoodads\Trees\ElwynnTreeCanopy04.m2',
 'cannonballs': GH + r'\CannonBallStack\DeadMineCannonBallStack.m2', 'powder': GH + r'\DeadMinePowderKeg\DeadMinePowderKeg.m2',
 'cannon': GH + r'\Cannon\Cannon01.m2', 'ballista': r'World\Goober\G_ExplodingBallista.m2',
 'trunk': GH + r'\CatapultRuins\CatapultTrunk01.m2', 'catball': GH + r'\CatapultRuins\CatapultBall01.m2',
 'dummy': GH + r'\WoodenDummies\StormWindWoodenDummy01.m2', 'rack': GH + r'\WeaponRacks\GeneralWeaponrack01.m2',
 'shield': GH + r'\Weapons&Armor\WallShield03.m2', 'banner': GH + r'\Banners\StormwindLionBanner.m2',
 'flagpole': GH + r'\FlagPole\Flagpole01.m2',
 # the city
 'tent': GH + r'\VendorTents\StormwindVendorTent01.m2', 'awning': GH + r'\VendorAwnings\StormwindVendorAwning01.m2',
 'planter': GH + r'\PlanterBoxes\StormwindPlanter.m2', 'bench': GH + r'\Benches\StormWindBench01.m2',
 'lamp': GH + r'\Lamps\StormwindStreetlamp01.m2', 'brazier': GH + r'\Braziers\StormwindBrazier01.m2',
 'lion': AZ + r'\Elwynn\PassiveDoodads\Statue\LionStatue.m2', 'fountain': GH + r'\Fountains\StormwindFountain01.m2',
 'goldS': GP + r'\TreasurePiles\GoldPilesmall01.m2', 'goldM': GP + r'\TreasurePiles\GoldPilemedium01.m2', 'goldL': GP + r'\TreasurePiles\GoldPileLarge01.m2',
 'podium': GH + r'\Podiums\DuskwoodPodium01.m2', 'fishrack': AZ + r'\BootyBay\PassiveDoodad\DeadFish\FishRack.m2',
 'holybanner': GH + r'\Banners\HolyBanner01.m2', 'bell': r'World\Goober\G_BellShip.m2',
 'khadgar': GH + r'\Statues\StatueKhadgar.m2', 'alleria': GH + r'\Statues\StatueAlleria.m2', 'turalyon': GH + r'\Statues\StatueTuralyon.m2',
 'kurdran': GH + r'\Statues\StatueKurdran.m2', 'cannonlarge': GH + r'\Cannon\CannonLarge.m2', 'candelabra': GH + r'\Candelabras\CandelabraTall01.m2',
 'signpost': AZ + r'\Elwynn\PassiveDoodads\Signs\Directional\Stormwinddirectionalsign.m2', 'gift': GP + r'\Christmas\XmasGift02.m2',
 'bone': GP + r'\Furniture\Refuse\Bone02.m2', 'skull': AZ + r'\Elwynn\PassiveDoodads\BattleGladeSkullHuman1\BattleGladeSkullHuman1.m2',
 'coins': GP + r'\TreasurePiles\GoldPilesmall01.m2', 'goblet': GH + r'\Goblets\Goblet02.m2', 'bowl': AZ + r'\Westfall\PassiveDoodads\Utensils\Bowl.m2',
 'plate': AZ + r'\Westfall\PassiveDoodads\Utensils\Plate.m2', 'bottle': GH + r'\Bottles\Bottle01.m2', 'jar': AZ + r'\Elwynn\PassiveDoodads\Jars\Jar01.m2',
}
def box(k):
    b = P.get(M[k].lower())
    if b is None:
        sys.exit('no box for ' + k + ' ' + M[k])
    return b
def ext(k): return max(box(k))
def vol(k, s=1.0):
    b = box(k)
    return FILL * b[0] * b[1] * b[2] * s ** 3
def gate(k, s=1.0): return RATIO * ext(k) * s
def sc(models, D):
    """Scale so the LARGEST of `models` opens at ball diameter D."""
    if isinstance(models, str): models = [models]
    return round(D / (RATIO * max(ext(m) for m in models)), 4)

# The D27 city pieces: (id, name, gate D, r^3 credit), from the runtime part audit (box volume x FILL).
PIECES = [
 (1, 'Trade District townhouse', 86.2, 4213), (2, 'Trade District cobbler', 68.2, 2244), (3, 'Trade District tailor', 70.2, 2457),
 (4, 'Trade District apothecary', 75.7, 3938), (5, 'Trade District general store', 74.5, 2253), (6, 'Flower shop', 76.8, 5118),
 (7, 'Trade District provisions', 94.3, 5687), (8, 'Canal-side townhouse', 78.3, 4486), (9, 'Trade District workshop', 73.4, 4090),
 (10, 'Canal-side shop', 75.7, 3641), (11, 'Wine shop', 84.3, 4596), (12, 'Trade District lodgings', 116.5, 12597),
 (13, 'Stormwind Auction House', 123.8, 14799), (14, 'Stormwind bank', 168.9, 24319),
 (20, 'parapet', 40.6, 65), (21, 'parapet', 47.4, 133), (22, 'parapet', 40.6, 120), (23, 'parapet', 39.7, 280), (24, 'parapet', 42.6, 137),
 (25, 'parapet', 42.7, 138), (26, 'parapet', 40.6, 110), (27, 'parapet', 40.6, 110), (28, 'parapet', 47.4, 133),
 (40, 'Cathedral Square orphanage block', 84.8, 4034), (41, 'Cathedral Square townhouse', 107.4, 5562), (42, 'Cathedral Square shop', 62.3, 1235),
 (43, 'Cathedral Square workshop', 72.7, 4081), (44, 'Cathedral Square lodgings', 96.6, 4237), (45, 'Southern canal shop', 68.1, 3496),
 (46, 'Northern canal shop', 76.9, 3789), (47, 'west bell tower', 52.7, 630), (48, 'south bell tower', 62.6, 790), (49, 'east bell tower', 64.1, 492),
 (50, 'Cathedral Square eastern wall', 138.3, 5190), (51, 'Cathedral Square northern terrace', 60.3, 1289), (52, 'Cathedral Square southern terrace', 133.4, 5154),
]
# The city's own claimed trees (modd-scale-audit): credited at half, since they stand where the ball may not go.
CITY_TREES = [(31.0, 14830 * 0.5), (35.0, 4955 * 0.5), (54.0, 14586 * 0.5)]

# ---------------------------------------------------------------- emitter and ledger
out = []
LEDGER = []   # (gate D, delta r^3, count, scene, model key)
SCENE = ''
ACT = ''
ACT_OF = {}
def L(s=''): out.append(s)
def fmt(v):
    if isinstance(v, bool): return 'true' if v else 'false'
    if isinstance(v, list): return '[' + ', '.join(fmt(x) if isinstance(x, str) else str(x) for x in v) + ']'
    if isinstance(v, str): return '"' + v.replace('\\', '\\\\') + '"'
    return str(v)
def q(p): return "'" + p + "'"
def item(arr, models, count, at=None, **kw):
    """One `[[scenes.items]]`. `at=D` scales the largest model to open at diameter D."""
    L('[[scenes.items]]')
    if isinstance(models, str):
        L('model = %s' % q(M[models]))
    else:
        L('models = [%s]' % ', '.join(q(M[m]) for m in models)); L('pick = "cycle"')
    L('arrangement = "%s"' % arr); L('count = %d' % count)
    if at is not None:
        kw['scale'] = sc(models, at)
    ms = [models] if isinstance(models, str) else models
    s = kw.get('scale', 1.0)
    L('# opens at D %.1f, %.2f r^3 each' % (max(gate(m, s) for m in ms), sum(vol(m, s) for m in ms) / len(ms)))
    for k, v in kw.items():
        if k != 'landmark':
            L('%s = %s' % (k, fmt(v)))
    L()
    if not kw.get('blocker'):
        per = count / len(ms)
        for m in ms:
            LEDGER.append((gate(m, s), vol(m, s) * per, per, SCENE, m, bool(kw.get('landmark'))))
def scene(name, anchor, comment=None, indoors=False, yaw=None):
    global SCENE
    SCENE = name
    ACT_OF[name] = ACT
    L()
    if comment:
        for c in comment.split('\n'): L('# ' + c)
    L('[[scenes]]'); L('name = "%s"' % name); L('anchor = [%s]' % ', '.join(str(a) for a in anchor))
    if yaw is not None: L('yaw = %s' % yaw)
    if indoors: L('indoors = true')
    L()
def cscene(name, x, y, z, span, comment=None):
    """A city scene at raw (x, y): floor from the grid, checked over `span` yards."""
    z = floor(x, y, z)
    floor_span(x, y, span, z)
    scene(name, city(x, y, z), comment, indoors=True)
    return z
def blocker(key, offset, scale=1.0, yaw=None, comment=None):
    """A visible, permanent, inedible obstacle or beacon (D29 `blocker`): never an invisible wall."""
    if comment: L('# ' + comment)
    kw = dict(offset=offset, scale=scale, blocker=True, spacing=0.2)
    if yaw is not None: kw['yaw'] = yaw
    item('line', key, 1, **kw)
def giant(name, x, y, z, m, s, comment=None, yaw=None):
    """One big rooted thing as its own scene, so the floor ray starts at its own floor."""
    cscene(name, x, y, z, 3, comment)
    kw = dict(scale=s, spacing=1.0, rooted=True)
    if yaw is not None: kw['yaw'] = yaw
    item('line', m, 1, **kw)
def act(name):
    global ACT
    ACT = name
    L(); L('# ' + '=' * 96); L('# ' + name); L('# ' + '=' * 96)
def off_wow(ax, ay, x, y): return [round(-(y - ay), 2), round(-(x - ax), 2)]
def off_raw(ax, ay, x, y):
    """Item offset [east, south] for raw (x, y) in a scene anchored at raw (ax, ay)."""
    a = city(ax, ay, 0); p = city(x, y, 0)
    return off_wow(a[0], a[1], p[0], p[1])
def solid(key, offset, scale=1.0, yaw=None, comment=None, **kw):
    """A solid prop: walkable top (D29), a wall until edible. Counted as a landmark (not pre-Square food)."""
    if comment: L('# ' + comment)
    kw2 = dict(offset=offset, scale=scale, solid=True, spacing=0.2, landmark=True)
    if yaw is not None: kw2['yaw'] = yaw
    kw2.update(kw)
    item('line', key, 1, **kw2)
def herd(species, count, at, radius, fit, display=None, model=None, behaviour='skittish'):
    """A crowd. Each animal grounds at its own sample point within `radius`, and only if the floor (or, failing a
    WMO floor, the terrain) is within 0.7 yd of `at`'s z: keep herds on flat ground and `at` at the measured height."""
    L('[[herds]]'); L('species = "%s"' % species)
    if display: L('display = %d' % display)
    if model: L('model = %s' % q(model))
    L('count = %d' % count); L('at = [%s]' % ', '.join(str(a) for a in at)); L('radius = %s' % radius)
    L('fit = %s' % fit); L('behaviour = "%s"' % behaviour); L()
def cherd(species, count, x, y, z, radius, fit, **kw):
    """A city herd at raw (x, y): `at` z must be within 0.7 yd of the floor or it never spawns."""
    herd(species, count, city(x, y, floor(x, y, z, tol=1.5)), radius, fit, **kw)
GUARD = 3258; CITIZEN = 9257; CITIZEN2 = 5038; KID = 338
HORSE = r'Creature\RidingHorse\RidingHorse.m2'; SHEEP = r'Creature\Sheep\Sheep.m2'; CHICKEN = r'Creature\Chicken\Chicken.m2'; RABBIT = r'Creature\Rabbit\Rabbit.m2'

# ---------------------------------------------------------------- the route
# (name, WoW centre, radius). Elwynn z is the surveyed ground (telemetry tracks); city z is the WMO floor.
AREAS = [
 ('Lumber camp', [-9394, 110, 60], 30), ('Harvest road', [-9322, 258, 67], 18), ('The hill', [-9246, 362, 75.4], 10),
 ('Hill crest', [-9225, 402, 85.7], 8), ('Royal road', [-9168, 436, 93.6], 18), ('Convoy halt', [-9100, 400, 92.8], 20), ('Bridge foot', city(200, 0, -8), 12),
 ('Valley of Heroes', city(145, 0, -8.9), 25), ('Outer gate approach', city(24, 0, -3.0), 4), ('Stormwind arch', city(-25, 0, -8.2), 6),
 ('Stormwind gate', city(-45, 0, -9.3), 12), ('Through the gate', city(-70, 0, -9), 13), ('Trade street', city(-95, 0, -9.9), 16),
 ('Trade District market', city(-130, 0, -8), 24), ('Auction House corner', city(-170, -38, -4.6), 8), ('Canal ramp', city(-193, -30, -0.6), 6),
 ('Trade canal', city(-200, -5, 1.2), 12), ('Trade bridge', city(-232, -5, -1.4), 10),
 ('Canal east bank', city(-250, -10, -3.26), 8), ('Canal crossing', city(-270, -20, -.37), 8), ('Canal west bank', city(-290, -10, -4.36), 8),
 ('Cathedral bridge mouth', city(-288, 8, -4.94), 4), ('Cathedral bridge', city(-310, 8, -4.94), 6),
 # The Square: y 9 is midway between its planter rows (y -2 and 19). Neither the stair nor the apron has a
 # waypoint, and the trails go round the stair's south side and back through the middle: a ball sent up the
 # stair wedges at its foot (runs 7 and 9), a small one on the stair's side (probe 5), and one driven along
 # the Cathedral's facade ends up on its roof (probe 5).
 ('Fair gate', city(-345, 9, -5.3), 8), ('The nursery', city(-390, 9, -4.9), 14), ('Square south', city(-388, -24, -5.0), 8),
 ('South field', city(-414, -42, -5.3), 10), ('South flat', city(-440, -46, -5.2), 14), ('Square middle', city(-396, -28, -5.1), 8),
 ('North field', city(-412, 48, -5.2), 10), ('North flat', city(-436, 60, -5.0), 14),
 ('Cathedral Square', city(-330, -5, -5), 30),
]
NARROW = {'Outer gate approach', 'Stormwind arch', 'Stormwind gate', 'Canal ramp', 'Trade canal', 'Canal east bank', 'Canal crossing', 'Canal west bank',
          'Cathedral bridge mouth', 'Cathedral bridge', 'Fair gate'}

# ================================================================ header
L(r'''# GENERATED by docs/azeroll/data/stormwind-gen.py from the act plan in docs/azeroll/katamari-room-design.md §7;
# edit the generator, not this file. Level 3 of the campaign, "Stormwind or Bust!" (D25-D27, character pass
# 2026-11-03): a Katamari giant-finale stage. Fizzwick's invitation to the Cathedral of Light has to be
# delivered in person, and the kingdom has heard the ball is coming. Seven acts: the lumber camp, the harvest
# road and the Royal Sapling Convoy, the Valley of Heroes and the gate (slip through while you still fit),
# the Trade District on market day, the canals, the Cathedral tree fair in the Square, and the Cathedral.
# Sizing: a prop rolls up once the ball's diameter reaches 2.15 x its longest scaled extent; the "opens at D"
# comments give that diameter. No [[barriers]]: the city's own walls, bridges and gate contain the ball.
start = [-9394, 110, 60]
start_radius = 1.25
# City scenes and crowds ground on WMO floors (D27); outside the walls everything grounds on terrain.
wmo_floors = true
# The city agent's measured navigation capsules: the gate, the streets and the canal bridges (12 yd), ending
# before the Square, which uses ordinary navigation.
collision_navigation = [
 {centre = [-9100.785, 366.412, 96.957], to = [-8896.417, 567.307, 96.957], radius = 35},
 {centre = [-8896.417, 567.307, 96.957], to = [-8778.225, 667.71, 96.957], radius = 35},
 {centre = [-8778.225, 667.71, 96.957], to = [-8742.207, 702.749, 96.957], radius = 12},
 {centre = [-8742.207, 702.749, 96.957], to = [-8732.78, 723.025, 96.957], radius = 12},
 {centre = [-8732.78, 723.025, 96.957], to = [-8710.903, 727.649, 96.957], radius = 12},
 {centre = [-8710.903, 727.649, 96.957], to = [-8701.263, 712.317, 96.957], radius = 12},
 {centre = [-8701.263, 712.317, 96.957], to = [-8668.393, 738.463, 96.957], radius = 12},
]
live_radius_limit = 110
# Facing north-east, up the woodpile toward the road.
start_heading = 0.75
time_limit = 1800
bounds = {centre = [-9000, 470, 90], radius = 680}
# Claims cover the camp field, the Royal road's meadow and the city. The hill road between them claims nothing:
# the two farms beside it ((-9313, 287) and (-9265, 356)) have chairs and barrels that were bait, and wedged a
# street-sized ball against their fences; the road's food there is authored (the convoy's dropped saplings).
claim = [
 {centre = [-9390, 110, 60], to = [-9350, 215, 64], radius = 30},   # stops 67 yd short of the farm's dock at (-9320, 275): its jars were bait (runs 8a, 10a)
 {centre = [-9168, 436, 94], to = [-9070, 440, 94], radius = 40},
 {centre = [-9070, 440, 94], to = [-8870, 600, 95], radius = 60},
 {centre = [-8870, 600, 95], to = [-8600, 800, 106], radius = 130},
]
buildings = []

[level]
id = "stormwind"
title = "Stormwind or Bust!"
subtitle = "The Cathedral has not RSVP'd. The invitation is you."
briefing = "Deliver Fizzwick's invitation to the Cathedral of Light. Start in the lumber camp, eat the harvest road and the Royal Sapling Convoy, and slip through the city gate while you still fit. Market day is on in the Trade District; follow the canal to the tree fair in Cathedral Square, grow up through its saplings, the City Guard and the old trees, then roll up Stormwind district by district and take the Cathedral with you."
objective = "Roll up 24 city structures, reach 128 yd, then take the Cathedral"
unlock_after = "market-day"
cast = "stormwind"
music = "kuru-kuru-azeroth"
# The human clock is generous (30:00); comet par and the speed challenge are separate.
par = 900

[goal]
kind = "finale"
target = "Cathedral of Light"
diameter = 128
city_pieces = 24
area = "Cathedral Square"
title = "Stormwind, rolled up!"

# Optional challenges (D33: four at most, no clock or ram kinds). The ids are what the save file stores.
[[challenges]]
id = "convoy"
title = "Eat every sapling the Royal Convoy dropped"
kind = "scene"
scene = "The Royal Sapling Convoy"
radius = 30

[[challenges]]
id = "watch"
title = "The whole watch: every City Guard in the Valley and the Square"
kind = "collect-all"
species = "Stormwind City Guard"

[[challenges]]
id = "auction"
title = "Going, going, gone: the Auction House"
kind = "collect-all"
name = "Stormwind Auction House"

[[challenges]]
id = "fair"
title = "Festival of the Light: the whole festival ground"
kind = "scene"
scene = "The festival ground"
radius = 30

# D33: the level\'s hidden parcel (scene "Lost in the post", on the southern terrace walk behind the festival giants,
# raw (-455, -78): the canal bed under the Trade bridge is unreachable at D 7): the Thinking Helm.
[present]
id = "thinking-helm"
title = "the Thinking Helm"
display = 4953
''')

# The tiers name the acts' bands; the mood ladder (D18) panics the cast at tier 3 (townsfolk) and loses it at 4+.
# 'Horses and townsfolk' derives from its reps (the adults open at 4.33); the recipe value must agree with it.
TIERS = [('Kindling and crates', 0), ('Haystacks and pumpkins', 3.1), ('Horses and townsfolk', 4.33), ('Saplings', 4.6),
         ('Market stalls', 6.1), ('The fair\'s young trees', 9.0), ('Tents and wagons', 15.0), ('Siege engines', 22.0),
         ('Lions and fountains', 31.0), ('Bell towers', 52.7), ('Whole houses', 68.0), ('The old trees', 89.0),
         ('Whole districts', 110.0), ('The Cathedral', 128)]
for name, at in TIERS:
    L('[[tiers]]'); L('name = "%s"' % name); L('at = %s' % at)
    if name == 'Horses and townsfolk': L('reps = ["adults", "species:Convoy horse"]')
    L()

L('# ' + '-' * 96)
L('# The route: one chain of areas from the woodpile to the Cathedral; the Square is six vignettes in size')
L('# order, and the goal area (the Square\'s east side, where the finale is validated) closes the chain.')
L('# ' + '-' * 96)
for name, c, r in AREAS:
    L('[[areas]]'); L('name = "%s"' % name); L('centre = [%s]' % ', '.join(str(x) for x in c)); L('radius = %s' % r); L()

# ================================================================ ACT I: THE LUMBER CAMP
act('ACT I - The lumber camp (D 2.5 -> 4.4): the woodcutters have gone to lunch')
scene('The woodpile', [-9394, 110, 60],
      'The nursery: the ball starts in the camp\'s kindling, stacked for the city. Kindling first, then the split\n'
      'logs, sacks, jars and bottles round it; barrels and kegs ring it; the lumber spills east toward the sawyers\'\n'
      'table. Dense: every frame should be full of things (the L2 standard).')
item('cluster', ['firewood', 'firewood3'], 20, scale=0.6, spacing=2.8, gap=0.45, jitter=0.08)
item('cluster', ['lumber', 'lumber', 'sack'], 26, scale=1.0, spacing=3.4, gap=0.4, jitter=0.1, offset=[1.0, 1.0])
item('cluster', ['jar', 'jug', 'bottle', 'bucket'], 16, scale=1.0, spacing=3.0, gap=0.35, jitter=0.08, offset=[-2.0, 1.5])
item('grid', ['crate', 'crate2', 'sack'], 15, scale=0.9, spacing=1.3, wobble=0.3, offset=[-4.5, 2.5])
item('ring', ['barrel', 'bucket', 'jar'], 9, scale=1.0, spacing=4.8, wobble=0.6)
item('spill', ['lumber', 'lumber', 'apple'], 16, scale=1.0, spacing=1.0, heading=wow_heading(8, -12), bend=0.06, offset=[1.0, -1.0])
item('grid', 'keg', 4, scale=0.7, spacing=1.5, wobble=0.2, offset=[-6.5, -4.0])
item('cluster', ['firewood', 'firewood3'], 6, scale=0.8, spacing=5.5, gap=0.8, jitter=0.06)

scene("The sawyers' lunch", [-9386, 98, 59.2],
      'Three sawyers and a foreman, lunch knocked off the trestle (solid: a walkable top, food at D 6 for anyone\n'
      'who comes back) onto the ground, stumps for seats, the saw left in a log, bones from the dog. The foreman\n'
      'is asleep on the hay and will not wake.')
solid('wftable', [0, 0], comment='the trestle')
item('pile', ['bread', 'loaf', 'wheel', 'mug', 'mug', 'apple', 'jug', 'bowl', 'plate'], 18, spacing=0.55, offset=[2.0, 1.0], scale=1.0)
item('ring', ['stump', 'stump2'], 4, scale=0.3, spacing=4.2, wobble=0.5)
item('cluster', ['haypile', 'haypile1'], 4, scale=0.6, spacing=2.4, gap=0.5, offset=[-5.5, 3.0])
item('cluster', ['bone', 'bone', 'skull'], 7, scale=1.0, spacing=1.6, gap=0.3, offset=[-3.0, 1.0])
item('line', 'axe', 3, scale=1.0, spacing=0.8, offset=[2.0, 3.5], heading=0.3)
item('line', 'saw', 1, scale=0.5, spacing=1.0, offset=[-3.0, -4.0])
item('cluster', 'chest', 1, scale=0.85, spacing=1.6, gap=0.6, offset=[5.0, 2.0])

scene('The lumber wagon', [-9380, 136, 62.3],
      'The camp\'s wagon (solid, D 20: the thing that is too big) with its load spilled off the back in a thick\n'
      'line toward the road north-east: the line that pulls the ball out of the camp.')
solid('wagon', off_wow(-9380, 136, -9375, 133), yaw=0.9, comment='the wagon, parked 6 yd off the road line (the trail ran through it)')
item('spill', ['lumber', 'lumber', 'lumberS', 'sack'], 28, scale=0.6, spacing=1.2, heading=wow_heading(32, 64), bend=0.04, offset=[-4.0, -3.0])
item('cluster', 'lumberL', 1, scale=0.6, spacing=2.2, gap=0.8, offset=[-6.0, 3.0])
item('cluster', ['wfbarrel', 'bucket'], 6, scale=1.0, spacing=2.6, gap=0.5, offset=[6.0, -3.5])
blocker('signpost', [-2.0, -9.0], scale=2.0, comment='the Stormwind signpost at the camp gate: the road north')

scene('Stacked for winter', [-9400, 122, 60.0],
      'The season\'s cords in a grid (the camp\'s own band, D 2.6), two old stumps the saw left (D 3.8), and the\n'
      'sacks of charcoal: the camp\'s last food before the road.')
item('grid', ['firewood', 'firewood3', 'sack'], 18, scale=0.62, spacing=1.6, wobble=0.25)
item('ring', ['stump', 'stump2'], 2, scale=0.3, spacing=8.0, wobble=1.0)
item('cluster', ['sack', 'sackflat', 'bucket'], 12, scale=1.0, spacing=3.0, gap=0.45, offset=[6.0, 4.0])
herd('Camp rabbit', 4, [-9384, 116, 61.0], 5, 0.48, model=RABBIT)
herd('Camp hen', 4, [-9388, 96, 59.2], 5, 0.35, model=CHICKEN)

# ================================================================ ACT II: THE HARVEST ROAD
act('ACT II - The harvest road (D 4.4 -> 5.3): the farms have stacked their hay by the road for the city')
scene('Hay for the city', [-9322, 258, 67],
      'A double line along the road edge, pointing north: small stacks (D 3.1) and corn stooks, the big stack\n'
      'beside it (D 4.5), the hay piles the carters dropped (D 4.8), and the apples that rolled off the cart.\n'
      'The sheep are loose. A signpost at the line\'s end points on to the hill.')
item('line', ['haystack', 'haystack', 'sack'], 15, scale=1.0, spacing=1.8, wobble=0.7, heading=wow_heading(37, 4), offset=[0.0, 4.0])
item('line', ['corn', 'corn', 'sack'], 14, scale=0.5, spacing=1.9, wobble=0.6, heading=wow_heading(37, 4), offset=[3.0, 4.0])
item('cluster', 'haystack2', 1, scale=1.0, spacing=4.0, gap=0.9, offset=[-6.0, 4.0])
item('cluster', ['haypile', 'haypile1'], 2, scale=0.8, spacing=4.5, gap=0.8, offset=[5.0, -4.0])
item('spill', ['apple', 'apple', 'jug'], 16, scale=1.0, spacing=0.9, heading=wow_heading(30, 6), bend=0.08, offset=[-2.0, 0.0])
blocker('signpost', [-4.0, -16.0], scale=2.0, comment='signpost: the hill road')
herd('Farm sheep', 10, [-9316, 252, 67.1], 4, 0.8, model=SHEEP)   # kept off the farm's dock at (-9335, 271): run 8a chased one into it

scene('The pumpkin cart', [-9335, 246, 65.7],
      'Hobbs\'s cart lost a wheel on the hill (solid, D 11: a landmark, not food) and the pumpkins rolled toward\n'
      'the road: small ones first, the prize one at the cart; a watermelon, and the Hobbses\' lunch.')
solid('brokencart', off_wow(-9335, 246, -9343, 246), yaw=2.2, comment='the cart, on its side, 8 yd west of the road (run 8a: a ball pinned on it at the trail end; the validator: the hay scene\'s sacks were inside it when a literal offset put it north instead)')
item('spill', ['pumpkin', 'apple', 'pumpkin'], 9, scale=0.45, spacing=1.2, heading=wow_heading(10, 8), bend=0.08, offset=off_wow(-9335, 246, -9340, 248))
item('cluster', 'watermelon', 1, scale=1.0, spacing=2.4, gap=0.6, offset=[2.5, -2.5])
item('cluster', 'pumpkin1', 1, scale=0.8, spacing=1.8, gap=0.8, offset=[-2.5, -3.0])
item('cluster', 'barrow', 1, scale=0.8, spacing=2.0, gap=1.0, offset=[4.0, 3.0])
item('cluster', ['bread', 'apple', 'mug', 'jug', 'bowl'], 12, scale=1.0, spacing=2.2, gap=0.3, offset=[3.0, 0.0])

scene('The scarecrow', [-9345, 232, 65.3],
      'A scarecrow in the corn (D 4.6) on the west side of the road, by the tipped cart; the farm across the road\n'
      '(the map\'s, with its dock and wagon) is the landmark, and stays out of the claims.')
item('line', 'scarecrow', 1, scale=0.55, spacing=1.0)
item('grid', 'corn', 12, scale=0.45, spacing=1.6, wobble=0.3, offset=[-3.0, -3.0])
item('cluster', ['haypile', 'haypile1'], 2, scale=0.9, spacing=2.4, gap=0.7, offset=[5.0, -1.0])
item('cluster', ['bucket', 'sack', 'jug'], 8, scale=1.0, spacing=2.4, gap=0.4, offset=[-4.0, 3.0])
herd('Farm hen', 5, [-9342, 238, 65.5], 5, 0.35, model=CHICKEN)

# ================================================================ ACT II-b: THE ROYAL CONVOY
act('ACT II-b - The Royal Sapling Convoy (D 5.3 -> 6.3): the King\'s trees for the Cathedral fair, halted before the Valley')
ROAD = wow_heading(48, 108)   # the road climbs north-north-west from the harvest road to the Royal road
UX, UY = 48 / math.hypot(48, 108), 108 / math.hypot(48, 108)
scene('The hill crest', [-9225, 402, 85.7],
      'Where the hill road levels: a signpost to Stormwind and the first dropped sapling pots.')
blocker('signpost', [0.0, 0.0], scale=2.0, comment='signpost: Stormwind')
item('cluster', ['sack', 'jug', 'apple'], 8, scale=1.0, spacing=2.4, gap=0.4, offset=[2.0, 2.0])

scene("The quartermaster's dump", [-9168, 436, 93.6],
      'The convoy\'s supplies, unloaded at the top of the hill where the road levels: fodder (D 6), grain, a stack\n'
      'of cannon balls for the gate (D 4.2), the quartermaster\'s crates, bottles, jars and buckets, and a signpost.')
item('cluster', ['haypile', 'haypile1'], 2, scale=0.9, spacing=4.5, gap=0.8, offset=[-5.0, 3.0])
item('heap', ['sack', 'sackflat'], 14, scale=1.0, spacing=2.0, step=0.35, offset=[4.0, 2.0])
item('grid', 'powder', 6, scale=1.2, spacing=0.9, wobble=0.1, offset=[-1.0, -4.5])
item('cluster', ['barrel', 'crate', 'bottle', 'jar', 'bucket'], 18, scale=1.0, spacing=3.6, gap=0.4, offset=[5.0, -3.0])
blocker('signpost', [8.0, 6.0], scale=2.0, comment='signpost: the Valley of Heroes')

# The convoy halted on the open meadow where the road meets the Valley of Heroes (x -9108..-9072,
# y 360..426 is flat, roamed ground at 92.3-93.5): a column of four wagons up its east side, 15 yd apart.
COL = wow_heading(0.42, 0.91)      # the column's direction (65 deg): the one line on the meadow 7 yd clear of its fence and trees
WAGONS = [(-9101, 379), (-9095, 393), (-9088, 406), (-9082, 420)]   # 15 yd pitch, from the validator's grid of the meadow
# where each wagon's spill and provisions lie: the side of the column the map shows clear
SPILLS = [((-9106, 383), (-9107, 378)), ((-9087, 389), (-9086, 394)), ((-9092, 412), (-9094, 408)), ((-9089, 423), (-9091, 419))]
scene('The Royal Sapling Convoy', [-9092, 392, 92.8],
      'Four wagons halted in a column on the meadow before the Valley (solid, D 25: edible only in the city sweep,\n'
      'so they stay as the landmark) with the saplings they were carrying spilled off the road side of each one,\n'
      'in size order, and their provisions underfoot. The escort has formed two lines; the horses are loose. The\n'
      'saplings are the ball\'s first trees: they will grow up round.')
for i, (px, py) in enumerate(WAGONS):
    solid('gypsy', off_wow(-9092, 392, px, py), yaw=0.5, comment='convoy wagon %d' % (i + 1))
    s_ = [0.018, 0.021, 0.024, 0.027][i]
    (sx, sy), (qx, qy) = SPILLS[i]
    item('spill', 'tree', 3, scale=s_, spacing=1.8, heading=wow_heading(-6, 2), bend=0.1, rooted=True,
         offset=off_wow(-9092, 392, sx, sy))
    item('cluster', ['sack', 'jug', 'apple', 'bottle', 'crate2'], 7, scale=1.0, spacing=2.4, gap=0.35, offset=off_wow(-9092, 392, qx, qy))
item('cluster', ['sack', 'crate', 'barrel'], 8, scale=1.0, spacing=3.0, gap=0.5, offset=off_wow(-9092, 392, -9094, 400))
herd('Convoy horse', 6, [-9098, 384, 92.9], 8, 1.8, model=HORSE)

scene('The toll gate', [-9070, 430, 93.1],
      'Where the meadow meets the Valley of Heroes: the toll-keeper\'s chest, the guards\' own hay, and the\n'
      'signpost pointing at the gate. The Valley beyond has the map\'s hay piles in a line to the gate.')
item('cluster', ['haypile', 'haypile1'], 1, scale=1.0, spacing=2.5, gap=0.8)
item('cluster', 'chest', 1, scale=1.0, spacing=1.6, gap=0.8, offset=[2.0, 1.5])
item('cluster', ['barrel', 'crate2', 'sack', 'jug'], 10, scale=1.0, spacing=2.6, gap=0.4, offset=[-2.0, -1.5])
blocker('signpost', off_wow(-9070, 430, -9076, 431), scale=2.0, comment='signpost: Stormwind, through the gate (beside the road, 6 yd from the toll chest)')
herd('Toll guard', 6, [-9072, 428, 93.1], 4, 1.8, display=GUARD)   # terrain here (the Valley's deck ends at raw x 195)

# ================================================================ ACT III: THE VALLEY OF HEROES AND THE GATE
act('ACT III - The Valley of Heroes and the gate (D 6.3 -> 6.8): slip through while you still fit')
cscene('The Valley watch', 100, 0, -5.5, 14,
       'The statues of the heroes are far too big (and will stay so); the City Guard has turned out in the Valley\n'
       'with the hay the Valley is littered with. A line of hay, barrels and crates leads to the gate.')
item('line', ['haypile', 'barrel', 'crate', 'haypile1', 'sack'], 15, scale=0.72, spacing=3.0, wobble=1.2, heading=H_RAW_X + PI, offset=off_raw(100, 0, 75, 0))
item('cluster', ['barrel', 'crate', 'jug'], 8, scale=1.0, spacing=2.4, gap=0.4, offset=off_raw(100, 0, 112, -4))
cherd('Stormwind City Guard', 10, 100, 0, -5.5, 12, 1.8, display=GUARD)
cscene('Under the portcullis', -10, 0, -8.0, 14,
       'The hay the watch dropped in the gate passage itself: a line on the axis from the sill (raw x 20) to the\n'
       'inner arch, so the passage is never empty (runs 8a and 10a wandered the flanking bays for 30-76 s).')
item('line', ['haypile', 'haypile1', 'sack'], 5, scale=0.5, spacing=4.5, wobble=0.6, heading=H_RAW_X, offset=off_raw(-10, 0, -22, 0))
L('# The welcome committee: six guards in a line across the gate and their captain are cast NPCs (stormwind-npcs.toml);')
L('# they brace in a line, one of them charges, and all of them are 1.8 yd tall.')

cscene('The barricade that is not', -85, 0, -9.5, 14,
       'The merchants rolled their barrels across the inner street to stop the ball, twice. Barrels are food,\n'
       'and the second line points down the street to the plaza.')
item('line', ['barrel', 'crate', 'barrel', 'crate2'], 7, scale=1.0, spacing=1.2, wobble=0.3, heading=H_RAW_Y)
item('line', ['sack', 'barrel', 'jug', 'crate2'], 6, scale=1.0, spacing=1.4, wobble=0.3, heading=H_RAW_Y, offset=off_raw(-85, 0, -93, 0))
item('spill', ['apple', 'bread', 'jug', 'sack'], 14, scale=1.0, spacing=1.0, heading=H_RAW_X + PI, bend=0.06, offset=off_raw(-85, 0, -96, 0))
cherd('Street citizen', 8, -85, 0, -9.5, 10, 1.8, display=CITIZEN2)

# ================================================================ ACT IV: THE TRADE DISTRICT
act('ACT IV - The Trade District on market day (D 6.8 -> 7.2): stalls, lots and the bank\'s withdrawal')
cscene('Market day', -130, 0, -8.0, 22,
       'The market round the plaza: three stalls and two awnings (solid; D 8.5-15, so they are the promise and\n'
       'the sweep collects them), barrows and goods underfoot everywhere, the map\'s planters (D 11.4) and street\n'
       'trees (D 31) for later. Shoppers everywhere. Grand-Uncle Thrum is haggling in the middle of it.')
# Every solid on the plaza is at least 14 yd from any other and 8 yd from the path, so no gap is narrower
# than the ball (run 3 wedged between two stalls and an awning on the plaza's north side). The plaza is a
# terraced bowl (-5.3 on its rim, -8.2 in the middle, 0.3 yd kerbs between): each solid and each heap of goods
# is its own scene on a flat disc of the validator's grid, at that disc's floor.
item('cluster', 'barrow', 1, scale=0.8, spacing=6.0, gap=1.0, offset=off_raw(-130, 0, -133, -3))
item('cluster', ['barrel', 'crate', 'sack', 'crate2', 'jug'], 20, scale=1.0, spacing=7.0, gap=0.6, jitter=0.08)
for k, (x, y, z) in enumerate([(-154, 5, -7.7), (-121, 20, -5.4), (-146, 28, -5.3)]):
    cscene('Market day stall %d' % (k + 1), x, y, z, 4)
    solid('tent', [0.0, 0.0], comment='stall')
for k, (x, y, z) in enumerate([(-131, -10, -7.9), (-100, -2, -9.9)]):
    cscene('Market day awning %d' % (k + 1), x, y, z, 3)
    solid('awning', [0.0, 0.0], comment='awning')
cscene('Market day goods 1', -139, -12, -7.6, 4, None)
item('cluster', ['bucket', 'jug', 'apple', 'bread', 'bottle', 'jar'], 24, scale=1.0, spacing=3.0, gap=0.35)
cscene('Market day goods 2', -146, 4, -7.8, 4, None)
item('cluster', ['apple', 'bread', 'loaf', 'wheel', 'bowl'], 16, scale=1.0, spacing=2.5, gap=0.3)
cscene('Market day goods 3', -133, -3, -8.0, 4, None)
item('cluster', ['sack', 'jug', 'bottle', 'crate2'], 12, scale=1.0, spacing=2.8, gap=0.35)
cherd('Market shopper', 12, -130, 0, -8.0, 10, 1.8, display=CITIZEN)
cherd('Market shopper', 6, -118, 14, -7.0, 10, 1.8, display=CITIZEN2)
cherd('Market guard', 6, -150, -16, -7.2, 10, 1.8, display=GUARD)

# The street north of the Auction House is 5 yd wide between its corner buttress and the city's own planter
# at (-179, 4): a ball over r 3.2 cannot pass it, so the route to the canal leaves the plaza by its south-west
# ramp (20 yd wide, rising to the canal walk at (-200, -30)). A parked wagon fills the dead-end street so it
# reads closed, and the lots stand along the Auction House's east face.
cscene('Road closed', -180, 0, -5.5, 4,
       'A brewer\'s wagon parked across the street north of the Auction House (a blocker: visible, solid, never\n'
       'edible). The way to the canal is the ramp at the plaza\'s south-west corner.')
blocker('gypsy', [0.0, 0.0], scale=1.0, yaw=H_RAW_Y, comment='the parked wagon')
L('# The Auction House\'s lots, lined up along its east face in size order: a barrel, a crate, a keg, a hay pile, a')
L('# barrow, then a cart and a wagon (solid, on the plaza\'s open north-east, 14 yd clear of anything), and the')
L('# Auction House itself (D 124). Only the first five are edible now; the rest are the promise, and the sweep')
L('# collects them. Each lot is its own scene on its own flat cell of the plaza (the validator\'s grid).')
for i, (m, s_, x, y, z) in enumerate(zip(['barrel', 'swcrate', 'keg', 'haypile', 'barrow'], [1.0, 1.2, 1.0, 0.9, 0.8], [-133, -133, -134, -133, -133], [-8, -14, -20, -32, -38], [-7.9, -7.6, -6.9, -5.4, -5.4])):
    cscene('Lot %d' % (i + 1), x, y, z, 2, None)   # 12 yd off the face: a ball at the lot must not touch the wall
    item('line', m, 1, scale=s_, spacing=1.0)
for m, s_, x, y, z in [('brokencart', 0.8, -125, 38, -5.3), ('gypsy', 1.0, -108, 40, -7.6)]:
    cscene('Lot ' + m, x, y, z, 4, None)
    solid(m, [0.0, 0.0], scale=s_, comment='lot ' + m)
cscene('The lots\' change', -131, -24, -6.6, 3, None)
item('cluster', ['coins', 'coins', 'goblet', 'bottle'], 10, scale=1.0, spacing=2.0, gap=0.3)
cherd('Bidder', 8, -150, -20, -7.4, 5, 1.8, display=CITIZEN)

cscene("The bank's withdrawal", -104, -36, -6.0, 6,
       'The bank is moving its gold out in a hurry, down its steps into the street: small piles (D 4.2) spilled\n'
       'toward the plaza, a medium one (D 6.9), and a large one (D 15) the guards are standing on. The bank\n'
       '(D 169) is always too big. (The street has a pit at raw (-105, -30) and the plaza a monument at\n'
       '(-118, -24): everything keeps south of both.)')
item('spill', ['goldS', 'coins', 'jug', 'goblet'], 12, scale=1.0, spacing=1.0, heading=city_heading(-8, -4), bend=0.04, offset=off_raw(-104, -36, -104, -36))
item('pile', 'goldM', 1, scale=1.0, spacing=1.0, offset=off_raw(-104, -36, -102, -36))
item('pile', 'goldL', 1, scale=1.0, spacing=1.0, offset=off_raw(-104, -36, -113, -41))
item('pile', ['chest', 'coins', 'goblet'], 6, scale=1.0, spacing=1.4, offset=off_raw(-104, -36, -102, -39))
cherd('Bank guard', 5, -104, -36, -6.0, 4, 1.8, display=GUARD)

# The breadcrumb to the canal: a continuous line of goods from the plaza's centre to the Auction House's
# south-east corner, round it and up the ramp to the canal walk, so the way out is never in doubt.
L('# The way to the canal: sacks and jugs dropped by the porters from the plaza\'s south-west corner, round the')
L('# Auction House and up the ramp to the towpath (each drop on its own measured floor: the ramp climbs 5 yd).')
PATH_OUT = [(-138, -30, -5.5), (-148, -44, -4.0), (-158, -46, -3.8), (-170, -44, -4.6), (-181, -38, -3.1), (-187, -33, -1.9), (-193, -31, -0.5), (-199, -24, 0.0), (-201, -16, 0.6)]   # the ramp's middle, 4 yd off the planter block at (-190, -22)
for i, (x, y, z) in enumerate(PATH_OUT):
    cscene('The way to the canal %d' % (i + 1), x, y, z, 2)
    item('cluster', ['sack', 'jug', 'crate2', 'apple', 'bottle'], 4, scale=1.0, spacing=1.6, gap=0.4)

L('# The masons\' yard: the Cathedral\'s statuary waiting on the plaza for the festival, oversize (D 115-123), walls')
L('# now; the Trade District\'s share of the last doubling, and the reason the sweep comes back through the gate street.')
for i, (x, y, m, s_) in enumerate([(-115, 36, 'khadgar', 1.9), (-148, 30, 'lion', 4.2), (-110, -28, 'khadgar', 1.9), (-170, 19, 'alleria', 2.7), (-92, 14, 'lion', 4.2),
                                     (-118, -42, 'khadgar', 1.9), (-75, -12, 'alleria', 2.7), (-160, -46, 'lion', 4.2), (-138, -46, 'khadgar', 1.9), (-130, 46, 'fountain', 5.3)]):
    giant('Trade District statue %d' % (i + 1), x, y, -7, m, s_)

# ================================================================ ACT V: THE CANALS
act('ACT V - The canals (D 7.2): the towpath, the lions, and the old trees on the banks')
cscene('The towpath', -200, -10, 1.1, 3,
       'The porters\' barrels and sacks along the canal walk from the ramp\'s top to the Trade bridge: the line\n'
       'that carries the ball along the towpath and over the bridge.')
for (x, y) in [(-200, -24), (-200, -18), (-201, -12), (-201, -6), (-205, -5), (-211, -5), (-217, -5), (-223, -5)]:
    item('cluster', ['sack', 'jug', 'apple'], 3, scale=1.0, spacing=1.2, gap=0.4, offset=off_raw(-200, -10, x, y))
cscene('The fishmongers', -249, -22, -3.6, 8,
       'Barrels of the morning\'s catch along the east towpath, racks, a barrow; fishermen who will not look up.\n'
       'The two canal lions (the map\'s, D 11.6) guard the bridge landing: too big now.')
item('line', ['barrel', 'bucket', 'jug'], 9, scale=1.0, spacing=1.4, wobble=0.3, heading=H_RAW_Y)
item('line', 'fishrack', 1, scale=0.7, spacing=3.0, offset=off_raw(-249, -22, -251, -26))
item('cluster', ['bucket', 'crate2', 'sack', 'bone'], 10, scale=1.0, spacing=2.2, gap=0.4, offset=off_raw(-249, -22, -249, -19))
cherd('Fishmonger', 3, -249, -26, -3.7, 4, 1.8, display=CITIZEN2)
L('# Across the canal: dropped goods across the crossing and up to the Cathedral bridge, each drop on its own floor.')
for i, (x, y, z) in enumerate([(-262, -18, -3.0), (-267, -18, -1.0), (-276, -19, -0.9), (-279, -18, -2.5), (-289, -8, -4.4), (-289, 2, -4.9), (-294, 8, -4.9), (-302, 8, -4.9), (-316, 8, -4.9), (-326, 8, -4.9)]):
    cscene('Across the canal %d' % (i + 1), x, y, z, 2)
    item('cluster', ['sack', 'jug', 'apple', 'bottle'], 3, scale=1.0, spacing=1.2, gap=0.4)
L('# The canal plazas: more of the statuary, waiting by the water for the festival (D 108-114), seen from the bridges.')
for i, (x, y, m, s_) in enumerate([(-247, 28, 'khadgar', 1.75), (-248, -44, 'lion', 3.9), (-294, 30, 'khadgar', 1.75), (-294, -36, 'lion', 3.9), (-247, 12, 'alleria', 2.35), (-294, -14, 'khadgar', 1.75), (-294, 16, 'lion', 3.9)]):
    giant('Canal statue %d' % (i + 1), x, y, -4, m, s_)
cherd('Bridge guard', 4, -300, 8, -4.9, 4, 1.8, display=GUARD)
# D33: Fizzwick's third parcel. The spec's spot (the canal bed under the Trade bridge) is a floor the ray finds,
# but a ball that arrives at 7 yd no longer fits under the 4 yd deck and the bed is a 7 yd drop off the towpath
# with no way back up, so the fallback applies: the southern terrace walk west of the south flat, up the ramp
# at raw (-450, -60 .. -70) that climbs 5.5 yd in 10: a ramp reward for a human who goes looking.
cscene('Lost in the post', -455, -78, 1.2, 3,
       'Fizzwick\'s third parcel, left on the southern terrace walk behind the festival\'s giants, up the ramp at\n'
       'the south flat\'s west end: a reward for a human who goes looking (the autopilot never does).')
item('line', 'gift', 1, at=4.0, spacing=1.0, present=True, name="Fizzwick's Parcel", yaw=0.0)
item('cluster', ['candelabra', 'sack', 'jug'], 6, scale=1.0, spacing=2.0, gap=0.4, offset=[2.0, 1.0])


# ================================================================ ACT VI: CATHEDRAL SQUARE, THE FESTIVAL OF THE LIGHT
act('ACT VI - Cathedral Square, the Festival of the Light (D 7 -> 112): grow up through the festival')
# The Square is dressed for the Festival: offering crates, pumpkins and kegs, the pilgrims' carts and wagons,
# the festival tents, the Cathedral's bells brought down, the tithe, the City Guard's salute battery, the
# Square's fountain and lions, and the Cathedral's new statuary of the heroes of the Second War, which the
# masons' yard delivered in every size it had. No trees except the city's own planters (the user's note).
# Geometry rule (probes, 2026-11-03): each band is a DENSE FRONT the ball eats from its small end, rows
# across the direction of travel, each row sized so that it opens the next (1.08 radius reserve), and the
# bands follow the ball's path west along the Square's floor to the stair foot, then south and north round
# the flats; nothing stands on the stair, which wedges a r 15-33 ball (probes 3 and 6, runs 7 and 9). Everything on the
# ladder is `rooted` (D29 Planted): a wall until edible, never flung (probe 5: unrooted pumpkins got knocked
# away and the first heap row never opened).
BALL = [3.5 ** 3]   # the ball's r^3 on arrival in the Square (the pre-Square budget), advanced by each row
ZONES = [   # (name, axis along travel, start, end, fixed coord, expected z, row width, min row gap)
 # From the validator's collision grid of the Square: the east strip's centre lane is a raised planter (x -365..-377,
 # y 2..14, 1.5 yd high) with planter rows at y -5..1 and 16..21; the stair's first riser is x -400 (y -4..20) and its
 # balustrades reach y 24..39 (x -404..-420); kerbs cross the flats at y -46 and -30 (south) and 46 (north); the wings'
 # walls are at y -64 and 68, the west wall at x -450.
 ('east lane', 'x', -366, -377, -8, -4.9, 14, 2.2),    # y -15..-1, south of the big planter
 ('planter gap', 'x', -378, -384, -8.5, -4.9, 5, 2.2), # between the small planters (y -11..-6)
 ('nursery', 'x', -386, -395, 5, -4.8, 24, 2.2),       # y -7..17; the posts at x -396 end it
 ('south nursery', 'x', -386, -399, -26, -4.9, 32, 2.2), # the open middle south of the nursery, y -42..-10 (the tree at (-389, -24) is a prop)
 ('south field', 'x', -402, -424, -44, -5.3, 24, 2.5), # y -56..-32 (the wing's wall is at y -58 west of x -410)
 ('south flat', 'y', -32, -45, -438, -5.2, 20, 3.0),   # x -448..-428, between the kerbs at y -30 and -46
 ('south flat 2', 'y', -52, -62, -438, -5.0, 20, 3.0), # south of the kerb at y -46 (a big heap's box reaches 4 yd), to the wall at y -64
 ('north flat', 'y', 48, 64, -437, -5.0, 14, 3.0),     # x -444..-430 (posts at x -446..-448), to the wall at y 68
 ('north field', 'x', -424, -404, 56, -5.2, 18, 2.5),  # y 47..65, north of the stair's balustrade and the posts at y 42-45
 ('east field', 'x', -399, -386, 30, -4.9, 16, 2.5),   # y 22..38, the wall at y 40
 ('north lane', 'x', -384, -366, 26, -4.9, 8, 2.5),    # y 22..30, the planters' north side, on the way home
]
class Cursor:
    def __init__(self): self.zi = 0; self.pos = None
    def next_row(self, depth):
        while self.zi < len(ZONES):
            name, ax, start, end, fixed, z, width, gap = ZONES[self.zi]
            if self.pos is None: self.pos = start
            sign = 1 if end > start else -1
            centre = self.pos + sign * depth / 2
            if abs(centre + sign * depth / 2 - start) <= abs(end - start) + 0.01:
                self.pos = centre + sign * (depth / 2 + gap)
                x, y = (centre, fixed) if ax == 'x' else (fixed, centre)
                return (x, y, floor(x, y, z), width, H_RAW_Y if ax == 'x' else H_RAW_X)
            self.zi += 1; self.pos = None
        sys.exit('out of Square zones')
CUR = Cursor()
def front(name, steps, comment=None, reserve=1.08, next_gate=None, rooted_from=0.0):
    """A dense front: for each (model, scale) in gate order, as many as open the next step (or `next_gate`)."""
    scene_z = None; k = 0
    for i, (m, s_) in enumerate(steps):
        g = gate(m, s_)
        assert 2 * BALL[0] ** (1 / 3) >= g * reserve, (name, m, s_, 2 * BALL[0] ** (1 / 3), g)
        nxt = gate(*steps[i + 1]) if i + 1 < len(steps) else next_gate
        need = ((reserve * nxt) / 2) ** 3 if nxt else BALL[0]   # no next gate: one of the last thing, as the landmark
        count = max(1, math.ceil((need - BALL[0]) / vol(m, s_)))
        # festival things may overlap by half along a row and stand close in depth: a crowd, not a grid
        # Things that pile (the tithe, the offerings) pack like the old tree rows did, into one heap; the
        # statuary, lions and fountains stand apart as each band's landmarks.
        heap = m in ('goldL', 'goldM', 'pumpkin', 'haypile')
        b = box(m); along = max(b[0], b[1]) * s_; depth = max(2.0, min(b[0], b[1]) * s_ * (0.1 if heap else 0.35))
        while count > 0:
            while True:   # a slot narrower than the piece's shrunk box (the validator's test) is skipped
                x, y, z, width, heading = CUR.next_row(depth)
                if width >= 0.5 * along + 1.0: break
            width = width - 0.25 * along - 0.5   # inset each row end by the piece's quarter-box plus half a yard
            per_row = max(1, int(width / max(along * (0.15 if heap else 0.45), 1.0)) + 1)
            n = min(count, per_row)
            if scene_z is None or abs(z - scene_z) > 3.0:
                scene_z = z
                cscene(name if k == 0 else name + ' (row %d)' % (k + 1), x, y, z, 3, comment if k == 0 else None)
            item('line', m, n, scale=s_, spacing=round(width / max(n - 1, 1), 3) if n > 1 else 1.0, wobble=0.2,
                 heading=heading, rooted=(g >= rooted_from), offset=off_raw(cscene_anchor[0], cscene_anchor[1], x, y))
            BALL[0] += n * vol(m, s_)
            if LEDGER_MODE: print('   row %-30s k%-2d %-10s s%.2f n%-3d at (%.1f,%.1f,%.1f) -> r^3 %.0f D %.1f' % (name[:30], k, m, s_, n, x, y, z, BALL[0], 2 * BALL[0] ** (1 / 3)))
            count -= n; k += 1
    return k
# cscene() records its anchor for off_raw
_cscene = cscene
def cscene(name, x, y, z, span, comment=None):
    global cscene_anchor
    cscene_anchor = (x, y)
    return _cscene(name, x, y, z, span, comment)

# The way in: the ticket queue and a line of offerings along the east strip, between the Square's planter
# rows (y -2 and 19), from the bridge to the festival ground.
cscene('The fair gate', -352, 9, -5.2, 4,
       'Offering crates, candles and pumpkins dropped along the east strip, between the Square\'s planter rows,\n'
       'from the bridge to the festival ground: the pilgrims\' queue and the line in.')
item('line', ['swcrate', 'pumpkin', 'candelabra', 'keg', 'sack'], 5, scale=1.0, spacing=2.0, wobble=0.5, heading=H_RAW_X + PI, offset=off_raw(-352, 9, -356, 8))
item('line', ['keg', 'sack', 'swcrate', 'pumpkin', 'candelabra'], 5, scale=1.0, spacing=2.2, wobble=0.5, heading=H_RAW_X + PI, offset=off_raw(-352, 9, -367, -8))   # round the big planter, into the south lane
item('cluster', ['barrel', 'crate', 'swcrate', 'sack', 'jug'], 12, scale=1.0, spacing=3.0, gap=0.45, offset=off_raw(-352, 9, -349, 20))
cherd('Pilgrim', 10, -349, 16, -5.2, 8, 1.8, display=CITIZEN)

front('The festival ground',
      [('pumpkin', 1.2), ('goldL', 0.6), ('goldL', 1.0), ('goldL', 1.6)], next_gate=gate('lion', 1.2),
      comment='The Festival of the Light on the Square: the harvest offerings (pumpkins by the hundred) and then the\n'
              'tithe, heaped on the stair foot in ever bigger piles (D 6 -> 25). The ball arrives at about 7 yd and\n'
              'eats west, a row at a time; each row opens the next. The growers watch from the stair.')
cherd('Grower', 8, -384, -14, -4.9, 6, 1.8, display=CITIZEN2)
front("The Guard's cordon on the steps",
      [('lion', 1.2), ('goldL', 2.2), ('goldL', 2.9)], next_gate=gate('lion', 1.8),
      comment='The City Guard holds the grand stair: a brace of the Square\'s lions (D 34), the tithe heaped between them\n'
              '(D 35-46), thirty guards in ranks, and the salute battery on the apron\'s lip.')
cherd('Stormwind City Guard', 24, -416, 5, 0.7, 7, 1.8, display=GUARD)
# No blocker at the stair foot: a line of the Guard's big guns across it (run 12a) reached into the nursery rows,
# pinned a D 24 ball between the guns and the rooted lions, and the game's Unstuck threw it 464 yd back to the
# Valley. The stair is kept off the route (no waypoint on it) and the Cathedral is taken whole from the Square.
cscene('The salute battery', -432, 7, 4.6, 2, 'Five cannon on the apron\'s lip (D 22) and their shot, the City Guard\'s salute for the festival.')
item('line', 'cannon', 5, scale=1.0, spacing=3.2, wobble=0.2, heading=H_RAW_Y, yaw=0.0, offset=off_raw(-432, 7, -428, 7.5))
item('line', ['cannonballs', 'catball'], 4, scale=1.5, spacing=3.5, heading=H_RAW_Y, offset=off_raw(-432, 7, -430, 7))
front("The Cathedral's new statues",
      [('lion', 1.8), ('goldL', 3.3)], next_gate=gate('khadgar', 1.0),
      comment='On the Cathedral apron, flanking its door: the statuary the masons\' yard delivered for the festival\n'
              '(the lions D 51, Khadgar D 62), holy banners between them, the bishop\'s procession under them.')
# (No banners on the apron's south wing: its 6.0 floor is x -446..-440 on one grid and absent on the other; three props.)
cherd("Bishop's procession", 10, -441, 8, 4.6, 6, 1.8, display=CITIZEN)
front('The tithe',
      [('khadgar', 1.0), ('goldL', 4.0)], next_gate=None,   # the Square's ground ends here; the corner giants carry on
      comment='South of the stair, on the flat: the tithe the whole kingdom sent, in heaps the size of houses (D 57-71),\n'
              'the festival fountain (D 65) and the lions the yard sent oversize (D 68). The Square\'s bell towers and\n'
              'houses open in this band.')
giant('The festival fountain', -392, 30, -4.9, 'fountain', 3.0, comment='The festival fountain (D 65), set up in the east field by the orphanage: the tithe\'s last piece.')
# From D 87 the city is the food (G3): the Square's houses, the corner giants (D 89 -> 121), the canals' shops and
# the statuary on the canal plazas and in the Trade District (D 115-123), and the Cathedral.
for pid, pname, g, c in PIECES:
    if 40 <= pid <= 52 and g <= 100:
        BALL[0] += c * 0.5
cscene("The orphans' plot", -349, 56, -5.3, 5,
       'The orphanage\'s corner of the festival, in the 10 yd strip between the orphanage and the Square\'s east\n'
       'wall: the children\'s lantern procession laid out in a ring, the harvest entry (a pumpkin the size of a\n'
       'cart, D 15), and the orphans, who cheer.')
item('ring', ['candelabra', 'brazier', 'candelabra'], 8, scale=1.5, spacing=2.6, wobble=0.2, offset=off_raw(-349, 56, -352.5, 58))
item('line', 'pumpkin', 1, scale=3.0, spacing=1.0, offset=off_raw(-349, 56, -352, 66))
item('cluster', ['pumpkin', 'pumpkin1'], 4, scale=1.8, spacing=2.5, gap=1.2, offset=off_raw(-349, 56, -352, 49))
cherd('Orphan', 6, -351, 64, -5.3, 3, 1.1, display=KID)

L('# The corner giants: the band between the tithe and the city (D 71 -> 121), at the corners of the flats and')
L('# along the east strip the ball came in by (15 yd apart, so a 7 yd ball passes): the heroes, in bronze, oversize.')
for i, (x, y, m, s_, yaw) in enumerate([(-444, -38, 'khadgar', 1.15, None), (-438, -55, 'lion', 2.7, H_RAW_X), (-416, -50, 'alleria', 1.8, None), (-406, -52, 'khadgar', 1.3, None), (-414, -40, 'lion', 3.0, H_RAW_X),
                                          (-437, 57, 'alleria', 2.1, None), (-412, 56, 'khadgar', 1.5, None), (-424, 56, 'lion', 3.4, H_RAW_X),
                                          (-348, -12, 'khadgar', 1.7, None), (-346, 6, 'lion', 3.8, H_RAW_Y), (-348, 24, 'khadgar', 1.9, None), (-346, 28, 'lion', 4.2, None)]):
    giant('Square giant %d' % (i + 1), x, y, -5.0, m, s_, yaw=yaw)
print('Square ladder: %d rows; r^3 after the flats %.0f (D %.0f)' % (0, BALL[0], 2 * BALL[0] ** (1 / 3)))

# ================================================================ trails
L(); L('# ' + '-' * 96); L('# Trails: the Elwynn road carries the convoy\'s drops; the city trails are navigation only.'); L('# ' + '-' * 96)
ROAD_DROPS = {
 ('Lumber camp', 'Harvest road'): ('lumber', 1.0, 9),
 ('Harvest road', 'The hill'): ('haystack', 0.9, 11),
 ('The hill', 'Hill crest'): ('tree', 0.02, 14),
 ('Hill crest', 'Royal road'): ('tree', 0.02, 14),
 ('Royal road', 'Convoy halt'): ('sack', 1.0, 6),
}
for (a, pa, _), (b, pb, _) in zip(AREAS, AREAS[1:]):
    L('[[trails]]'); L('name = "%s"' % (a + ' to ' + b)); L('from = [%s]' % ', '.join(str(x) for x in pa)); L('to = [%s]' % ', '.join(str(x) for x in pb))
    drop = ROAD_DROPS.get((a, b))
    if drop:
        m, s, sp = drop
        L('spacing = %s' % sp); L('model = %s' % q(M[m])); L('scale = %s' % s); L('wobble = 1.5')
        if m == 'tree': L('tilt = 0.0')
        n = int(math.dist(pa[:2], pb[:2]) / sp)
        LEDGER.append((gate(m, s), vol(m, s) * n, n, 'trail ' + a + ' to ' + b, m, False))
        ACT_OF['trail ' + a + ' to ' + b] = 'ACT I' if a == 'Lumber camp' else ('ACT II' if a == 'Harvest road' else 'ACT II-b')
    else:
        L('spacing = 8'); L('navigation_only = true')
        if b in NARROW: L('arrival_radius = 3')
    L()


# ================================================================ the cast (stormwind-npcs.toml)
# Named witnesses, each placed in the vignette they belong to (P8). Guards brace in a line, then run; a
# `brave` one charges first. City NPCs ground on the WMO floor by a 0.7 yd ray, so their z is the grid's.
NPCS = []
def npc(name, scene, role, activity, display, at, yaw=0.0, indoors=False, **kw):
    rows = ['[[npc]]', 'name = "%s"' % name, 'scene = "%s"' % scene, 'role = "%s"' % role, 'activity = "%s"' % activity,
            'display = %d' % display, 'at = [%s]' % ', '.join(str(round(a, 2)) for a in at), 'yaw = %s' % yaw, 'indoors = %s' % fmt(indoors)]
    for k, v in kw.items():
        if isinstance(v, list) and v and isinstance(v[0], (list, tuple)):
            rows.append('%s = [%s]' % (k, ', '.join('[%s]' % ', '.join(str(x) for x in p) for p in v)))
        else:
            rows.append('%s = %s' % (k, fmt(v)))
    NPCS.append('\n'.join(rows) + '\n')
def cnpc(name, scene, role, activity, display, x, y, z, yaw=0.0, **kw):
    npc(name, scene, role, activity, display, city(x, y, floor(x, y, z, tol=1.5)), yaw, indoors=True, **kw)

# Act I
npc('Foreman Dunnock', 'lumber camp', 'villager', 'sleep', 1291, [-9389, 103.5, 59.5], 1.2, quirk='sleeper')
npc('Sawyer Hob', 'lumber camp', 'villager', 'eat', 3266, [-9384.5, 97, 59.2], 2.4, carry='bread')
npc('Sawyer Tam', 'lumber camp', 'villager', 'eat', 5038, [-9387.5, 99.5, 59.2], 5.6, carry='mug', partner='Sawyer Hob')
npc('Sawyer Welf', 'lumber camp', 'villager', 'work', 1298, [-9400, 125, 60], 0.9, carry='hammer')
# Act II
npc('Carter Hobbs', 'harvest road', 'villager', 'work', 1299, [-9308, 252, 68.6], 2.2, carry='pumpkin', partner='Goody Hobbs')
npc('Goody Hobbs', 'harvest road', 'villager', 'chat', 9257, [-9312, 249, 68.6], 5.3, carry='apple', partner='Carter Hobbs')
npc('Farmer Stonefield', 'harvest road', 'villager', 'work', 1292, [-9330, 268, 67.4], 3.9)
# Act II-b: the escort in two lines flanking the road, the sergeant ahead of them
npc('Sergeant Yorus', 'convoy', 'guard', 'work', 3258, [-9100, 376, 92.8], COL + PI, quirk='brave')
for i, (px, py) in enumerate([(-9104, 388), (-9098, 400), (-9092, 412)]):
    for side in (-1, 1):
        npc('Convoy guard %s' % 'ABCDEF'[i * 2 + (side > 0)], 'convoy', 'guard', 'work', [3258, 3257][(i + side) % 2], [px + side * 2, py - side * 3, 92.8], COL + PI)
npc('Quartermaster Fenwick', 'convoy', 'villager', 'work', 1300, [-9166, 438, 93.6], 1.0, carry='bottle')
npc('Toll-keeper Pell', 'toll gate', 'villager', 'bark', 1289, [-9068, 428, 93.1], 4.0)
# Act III: the welcome committee across the gate
cnpc('Captain Dunmore', 'gate', 'guard', 'work', 3258, -28, 0, -9.3, H_RAW_X, quirk='brave')
for i, y in enumerate([-5, -3, -1, 1, 3, 5]):
    cnpc('Gate guard %s' % ['Alder', 'Brom', 'Cole', 'Dace', 'Eddard', 'Finn'][i], 'gate', 'guard', 'work', [3257, 3258][i % 2], -34, y, -9.3, H_RAW_X)
# Act IV
cnpc('Auctioneer Jaxon', 'auction house', 'villager', 'bark', 1287, -148, -14, -7.6, 2.0)
# D33: Grand-Uncle Thrum Fizzwick, haggling in the Trade market (replay-design.md 4.3). A dwarf, by marriage;
# display 1626 at scale 2.0 stands about 3.2 yd, so his window opens at D 6.9, just under the ball's size here.
cnpc('Grand-Uncle Thrum', 'market', 'cousin', 'bark', 1626, -127, -8, -8.0, 1.0, cousin='thrum', scale=2.0, catch=[6.9, 10.4],
     carry='mug', caption='Three silver? THREE? For a HAT? I will roll you up myself, laddie.')
cnpc('Banker Vosk', 'bank', 'villager', 'work', 3340, -100, -36, -5.9, 1.0, quirk='polish')
cnpc('Cobbler Pratt', 'market', 'villager', 'bark', 3254, -126, 14, -7.0, 3.0, carry='bread')
# Act V
cnpc('Wet Pete', 'canal', 'villager', 'fish', 3271, -249, -27, -3.7, H_RAW_X, carry='fish')
# Act VI
cnpc('Ticket-seller Marla', 'fair gate', 'villager', 'bark', 9257, -349, 25, -5.2, 3.5)
cnpc('Grower Hollis', 'nursery', 'villager', 'work', 5527, -372, -14, -4.7, 0.5, carry='apple')
cnpc('Marshal Haggard', 'muster', 'guard', 'work', 3258, -401, -36, -5.3, H_RAW_X, quirk='brave')
cnpc('Matron Nightingale', 'orphanage', 'villager', 'chat', 5038, -406, 40, -5.1, 2.0)
for i, (x, y) in enumerate([(-404, 38), (-408, 37), (-410, 42), (-404, 45), (-407, 47), (-411, 39)]):
    cnpc('Orphan %s' % ['Pip', 'Sal', 'Tom', 'Nell', 'Bo', 'Wren'][i], 'orphanage', 'kid', 'chat', [338, 262][i % 2], x, y, -5.1, i * 1.0)
cnpc('Bishop Farthing', 'avenue', 'villager', 'chat', 1291, -432, 12, 4.6, H_RAW_X)
cast_text = ('# GENERATED by docs/azeroll/data/stormwind-gen.py: the cast of "Stormwind or Bust!", each witness in the\n'
             '# vignette they belong to. Fields as in goldshire-npcs.toml (npcs.rs): role guard/villager/kid, activity,\n'
             '# quirk (sleeper / polish / brave), carry, partner. City NPCs are indoors = true (WMO floor).\n\n' + '\n'.join(NPCS))
if not LEDGER_MODE:
    cdest = os.path.join(root, 'crates/benilla-app/src/azeroll/stormwind-npcs.toml')
    open(cdest, 'w').write(cast_text)
    print('wrote', cdest, len(NPCS), 'npcs')

# ================================================================ write
text = '\n'.join(out) + '\n'
dest = os.path.join(root, 'crates/benilla-app/src/azeroll/stormwind.toml')
if not LEDGER_MODE:
    open(dest, 'w').write(text)
    print('wrote', dest, len(text), 'bytes')
for w in WARN:
    print('WARNING:', w)

# ---------------------------------------------------------------- the ledger and the ladder check
def r3(D): return (D / 2) ** 3
GOAL_D = 128.0
start = 1.25 ** 3
gates = sorted(set([d for _, d in TIERS[1:]] + [7.0, 9.0, 12.0, 17.0, 25.0, 29.0, 36.0, 43.0, 46.0, 62.3, 80.0, 92.0, 100.0, 108.0, 116.0, 122.0, 128.0]))
def cum(D, pieces=True, city_trees=0.5):
    v = start + sum(dv for g, dv, *_ in LEDGER if g <= D)
    if pieces: v += sum(c for _, _, g, c in PIECES if g <= D)
    v += sum(c * (city_trees / 0.5) for g, c in CITY_TREES if g <= D)
    return v
print('\nLADDER (authored + pieces + half the city\'s own trees): at each gate, r^3 available / r^3 needed')
ok = True
for D in gates:
    have = cum(D); need = r3(D)
    flag = '' if have >= 1.15 * need else '   <-- SHORT'
    if D <= 7.0: flag = ''   # the pre-Square band is capped, not reserved (see below)
    ok &= not flag
    print('  D %6.1f  r %5.1f  have %9.0f  need %9.0f  x%.2f%s' % (D, D / 2, have, need, have / need, flag))
def act_of(scene_name): return ACT_OF.get(scene_name, '?').split(' - ')[0]
PRE_ACTS = ('ACT I', 'ACT II', 'ACT II-b', 'ACT III', 'ACT IV', 'ACT V')
pre_items = [e for e in LEDGER if e[0] <= 7.2 and act_of(e[3]) in PRE_ACTS and not e[5]]
pre = start + sum(e[1] for e in pre_items)
print('\nPRE-SQUARE BUDGET: authored food at D <= 7.2 before the Square (landmarks excluded) r^3 %.1f -> r %.2f alone;'
      ' with ~15 of the city\'s own barrels and hay (the autopilot) r %.2f; with all 45 (a thorough human) r %.2f.'
      ' The canal bridges admit r 4.0 (probe), not 4.5.' % (pre - start, pre ** (1 / 3), (pre + 15) ** (1 / 3), (pre + 45) ** (1 / 3)))
for a in PRE_ACTS:
    print('   %-9s %6.1f' % (a, sum(e[1] for e in pre_items if act_of(e[3]) == a)))
if pre - start > 46:
    print('PRE-SQUARE BUDGET EXCEEDED (limit 46 r^3)'); ok = False
if LEDGER_MODE:
    print('\nPER ACT / SCENE (gate D, r^3, count)')
    cur = None
    for g, dv, n, s, m, lm in sorted(LEDGER, key=lambda e: (ACT_OF.get(e[3], 'z'), e[3], e[0])):
        a = ACT_OF.get(s, 'trails')
        if a != cur:
            cur = a; tot = sum(x[1] for x in LEDGER if ACT_OF.get(x[3], 'trails') == a)
            print('--', a, ' total r^3 %.1f' % tot)
        print('   %-34s %-12s D %6.1f  %9.1f  x%d%s' % (s[:34], m, g, dv, n, '  (landmark)' if lm else ''))
if not ok:
    print('\nLADDER SHORT: a size band has less than 1.15x reserve; fix the content before shipping.')
    sys.exit(1)
