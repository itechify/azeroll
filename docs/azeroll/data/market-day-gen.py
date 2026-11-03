#!/usr/bin/env python3
"""Generates crates/benilla-app/src/azeroll/goldshire.toml (level id `market-day`) from a district plan.
Run: python3 docs/azeroll/data/market-day-gen.py   (from anywhere; `--ledger` prints the growth budget).
Design notes: docs/azeroll/katamari-room-design.md §6. The far farms were dropped on 2026-11-03 (sprawl).

Positions are WoW (x north, y west, z up). Item offsets are Bevy (east, south) = (-dy, -dx).
Headings: 0 east, pi/2 south, pi west, 3pi/2 north.
Sizing: a prop rolls up once the ball's DIAMETER >= 2.15 x its longest scaled extent (level.rs:
needed radius = extent / COLLECT_RATIO) and adds 0.119 x its scaled box volume to r^3. `at=D` scales a
model (or the largest of a mix) so it opens at diameter D. Boxes come from props.csv (the M2 headers)."""
import csv, math, os, sys
PI = math.pi; E = 0.0; S = PI / 2; W = PI; N = 3 * PI / 2
RATIO = 2.15; FILL = 0.119
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..'))

# ---------------------------------------------------------------- the model palette
P = {}
for r in csv.DictReader(open(os.path.join(here, 'props.csv'))):
    P[r['path'].lower()] = (float(r['bbox_x']), float(r['bbox_y']), float(r['bbox_z']))
# Every World\Goober\G_Pumpkin_0x model wears the jack-o'-lantern texture (mm_JackOLantern_01: carved face, candle
# glow), so none reads as a harvest pumpkin from the front. The one uncarved pumpkin in the data is Duskwood's
# vine pumpkin; its box is 1.69 x 3.44 x 1.96 because the vine's tendrils are in the header, so at a given
# opening D the gourd itself is about 40 % smaller than the jack was.
PUMPKIN = r'World\Azeroth\Duskwood\PassiveDoodads\Bush\Pumpkin01.m2'
GH = r'World\Generic\Human\Passive Doodads'; GP = r'World\Generic\PassiveDoodads'; AZ = r'World\Azeroth'
M = {
 # food, tiny to small
 'muffin': GH + r'\Food\Muffin01.m2', 'wedge': GH + r'\Food\CheeseWedge01.m2', 'wedgeS': GH + r'\Food\CheeseWedgeSwiss01.m2',
 'wheel': GH + r'\Food\CheeseWheel01.m2', 'wheelS': GH + r'\Food\CheeseWheelSwiss01.m2', 'slice': GH + r'\Food\BreadSlice.m2',
 'half': GH + r'\Food\BreadLoafHalf.m2', 'pie': GH + r'\Food\Pie01.m2', 'loaf': GH + r'\Food\BreadLoaf01.m2',
 'baguette': GH + r'\Food\BreadFrench01.m2', 'bread': GH + r'\Food\bread01.m2',
 'apple': GP + r'\Fruits\Fruit_Apple.m2', 'orange': GP + r'\Fruits\Fruit_Orange.m2', 'pear': GP + r'\Fruits\Fruit_Pear.m2',
 'banana': GP + r'\Fruits\Fruit_Banana.m2', 'bananas': GP + r'\Fruits\Fruit_BananaBunch.m2', 'melon': GP + r'\Fruits\Fruit_Melon.m2',
 'melonslice': GP + r'\Fruits\Fruit_MelonSlice02.m2', 'fruitbowl': GP + r'\Fruits\FruitBowl_Apples.m2', 'fruitmix': GP + r'\Fruits\FruitBowl_Mixed.m2',
 'watermelon': r'World\Goober\G_Watermelon.m2', 'pumpkin': PUMPKIN, 'pumpkin3': PUMPKIN,
 'corn': AZ + r'\Elwynn\PassiveDoodads\CornCrop\corn.m2',
 'turkey': GH + r'\Food&Utensils\TurkeyLeg.m2', 'haunch': GH + r'\Food&Utensils\Haunch.m2', 'boar': GH + r'\Food\RoastBoarPlatter.m2',
 'platter': GH + r'\Food\FishPlatter01.m2', 'fishskel': GP + r'\Fish\fishskel01.m2', 'cleaver': GH + r'\Food&Utensils\Cleaver01.m2',
 'knife': GH + r'\Food&Utensils\KitchenKnife01.m2',
 # drink and tableware
 'mug': GH + r'\Mugs\Mug01.m2', 'foam': GH + r'\Mugs\MugFoam01.m2', 'mug2': GH + r'\Mugs\Mug02.m2', 'stein': AZ + r'\Westfall\PassiveDoodads\Utensils\Stein.m2',
 'stein1': GH + r'\Mugs\Stein01.m2', 'goblet': GH + r'\Goblets\Goblet02.m2', 'bottle': GH + r'\Bottles\Bottle01.m2', 'green': GH + r'\Bottles\GreenBottle01.m2',
 'jug': AZ + r'\Elwynn\PassiveDoodads\jugs\jug01.m2', 'jug2': AZ + r'\Elwynn\PassiveDoodads\jugs\jug02.m2', 'jar': AZ + r'\Elwynn\PassiveDoodads\Jars\Jar01.m2',
 'jar2': AZ + r'\Elwynn\PassiveDoodads\Jars\Jar02.m2', 'orcjug': r'World\Generic\Orc\Passive Doodads\Jugs\OrcJug01.m2', 'orcjug2': r'World\Generic\Orc\Passive Doodads\Jugs\OrcJug02.m2',
 'orcbottle': r'World\Generic\Orc\Passive Doodads\Bottles\OrcBottle01.m2', 'plate': AZ + r'\Westfall\PassiveDoodads\Utensils\Plate.m2',
 'bowl': AZ + r'\Westfall\PassiveDoodads\Utensils\Bowl.m2', 'dirty': GH + r'\Plates\GeneralDirtyPlate01.m2', 'coins': GP + r'\TreasurePiles\GoldPilesmall01.m2',
 'scry': r'World\Goober\G_ScryingBowl.m2', 'skull': AZ + r'\Elwynn\PassiveDoodads\BattleGladeSkullHuman1\BattleGladeSkullHuman1.m2',
 'bone': GP + r'\Furniture\Refuse\Bone02.m2', 'lantern': GH + r'\Lanterns\GeneralLantern01.m2', 'cat': r'World\Goober\G_RelicNECat.m2',
 # cloth
 'yarnR': GH + r'\Clothing\ClothYarnBallRed.m2', 'yarnY': GH + r'\Clothing\ClothYarnBallYellow.m2', 'spoolB': GH + r'\Cloth\ClothSpoolBlue.m2',
 'spoolY': GH + r'\Cloth\ClothSpoolYellow.m2', 'shirt': GH + r'\Clothing\FoldedShirtBrown.m2', 'pants': GH + r'\Clothing\FoldedPantsGrey.m2',
 'cushion': AZ + r'\Westfall\PassiveDoodads\HaremPillow01\HaremPillow01.m2', 'rug': GH + r'\Rugs\StormwindRug02.m2', 'rug1': GH + r'\Rugs\StormwindRug01.m2',
 'rugrack': AZ + r'\Westfall\PassiveDoodads\RugRacks\RugRack02.m2', 'rugrack1': AZ + r'\Westfall\PassiveDoodads\RugRacks\RugRack01.m2',
 # containers
 'bucket': GH + r'\Buckets\bucket.m2', 'pail': GH + r'\Buckets\CaveKoboldBucket.m2', 'gbucket': r'World\Generic\Gnome\Passive Doodads\Buckets\Gnomebucket05.m2',
 'gbucket3': r'World\Generic\Gnome\Passive Doodads\Buckets\Gnomebucket03.m2', 'grapebucket': AZ + r'\Elwynn\PassiveDoodads\GrapeBuckets\GrapeBucket01.m2',
 'sack': GP + r'\Furniture\Containers\Sack02.m2', 'sackflat': GP + r'\Furniture\Containers\Sack01.m2', 'grain': GH + r'\Sacks\PlaguedGrainSack01.m2',
 'grain2': GH + r'\Sacks\PlaguedGrainSack02.m2', 'herbs': GH + r'\Sacks\SackHerbsPeacebloom01.m2', 'kelp': GH + r'\Sacks\SackHerbsStranglekelp01.m2',
 'crate1': GH + r'\Crates\ReplaceCrate01.m2', 'crate2': GH + r'\Crates\ReplaceCrate02.m2', 'crate3': GH + r'\Crates\ReplaceCrate03.m2',
 'swcrate': GH + r'\Crates\StormwindCrate01.m2', 'wfcrate': AZ + r'\Westfall\PassiveDoodads\Crate\WestFallCrate.m2', 'gcrate2': r'World\Goober\G_Crate02.m2',
 'gcrate1': r'World\Goober\G_Crate01.m2', 'orccrate': r'World\Generic\Orc\Passive Doodads\BarrelsandCrates\OrcCrate08.m2',
 'barrel': GP + r'\Barrel\Barrel01.m2', 'wfbarrel': AZ + r'\Westfall\PassiveDoodads\Barrel\WestFallBarrel01.m2', 'barrel2': GP + r'\Furniture\Containers\Barrel02.m2',
 'gbarrel': r'World\Goober\G_Barrel.m2', 'orcbarrel': r'World\Generic\Orc\Passive Doodads\BarrelsandCrates\OrcBarrel03.m2',
 'broken1': GP + r'\Barrel\BrokenBarrel01.m2', 'broken2': GP + r'\Barrel\BrokenBarrel02.m2', 'keg1': GH + r'\BeerKegs\BeerKeg01.m2', 'keg2': GH + r'\BeerKegs\BeerKeg02.m2',
 'gkeg': r'World\Goober\G_Keg.m2', 'applebob': r'World\Goober\G_AppleBob_01.m2', 'chest': r'World\Generic\ActiveDoodads\Chests\ChestActive01.m2',
 'chest2': r'World\Generic\ActiveDoodads\Chest02\Chest02.m2', 'trunk': GH + r'\CatapultRuins\CatapultTrunk02.m2',
 'cage1': r'World\Generic\Tauren\Passive Doodads\AnimalCages\AnimalCage01.m2', 'cage2': r'World\Generic\Tauren\Passive Doodads\AnimalCages\AnimalCage02.m2',
 'cage3': r'World\Generic\Tauren\Passive Doodads\AnimalCages\AnimalCage03.m2', 'gcage2': r'World\Goober\G_Cage02.m2', 'gcage': r'World\Goober\G_Cage.m2',
 # hay, carts, wagons
 'bale1': GH + r'\HayPiles\ShadowfangHayPile01.m2', 'bale2': GH + r'\HayPiles\ShadowfangHayPile02.m2',
 'haystack1': AZ + r'\Elwynn\PassiveDoodads\Haystacks\haystack01.m2', 'haystack2': AZ + r'\Elwynn\PassiveDoodads\Haystacks\haystack02.m2',
 'bighay1': AZ + r'\Westfall\PassiveDoodads\HayStack\WestFallHayStack01.m2', 'bighay2': AZ + r'\Westfall\PassiveDoodads\HayStack\WestFallHayStack02.m2',
 'dwhay': AZ + r'\Duskwood\PassiveDoodads\DuskwoodHaystack\DuskwoodHayStack.m2', 'haywagon': AZ + r'\Westfall\PassiveDoodads\HayWagon\WestfallHayWagon.m2',
 'dwhaywagon': AZ + r'\Duskwood\PassiveDoodads\DuskwoodHaywagon\DuskwoodHayWagon.m2', 'wfwagon': AZ + r'\Westfall\PassiveDoodads\Westfall Wagon\WestfallWagon01.m2',
 'brokencart': AZ + r'\Westfall\PassiveDoodads\BrokenCart\BrokenCart.m2', 'barrow': GP + r'\Misc\WheelBarrow\CaveMineWheelbarrow01.m2',
 'cartwheel': GH + r'\BallistaRuins\BallistaWheel01.m2', 'tent': GH + r'\VendorTents\StormwindVendorTent01.m2', 'awning': GH + r'\VendorAwnings\StormwindVendorAwning01.m2',
 # furniture
 'bench': GH + r'\Benches\InnBench.m2', 'swbench': GH + r'\Benches\StormWindBench01.m2', 'tinytable': GH + r'\Tables\InnTableTiny.m2',
 'smalltable': GH + r'\Tables\InnTableSmall.m2', 'wftable': AZ + r'\Westfall\PassiveDoodads\WestfallTable\WestfallTable.m2', 'sftable': GH + r'\Tables\ShadowfangTable01.m2',
 'wchair': AZ + r'\Westfall\PassiveDoodads\WestfallChair\WestfallChair.m2', 'chair': GH + r'\Chairs\GeneralChairLoEnd01.m2',
 'gchair': r'World\Generic\Gnome\Passive Doodads\Furniture\GnomeChair03.m2', 'podium': GH + r'\Podiums\DuskwoodPodium01.m2',
 'counter': GH + r'\ShopCounter\DuskwoodShopCounter.m2', 'flagpole': GH + r'\FlagPole\Flagpole01.m2',
 'banner': AZ + r'\Elwynn\PassiveDoodads\BattleGladeBanner2\BattleGladeBanner2.m2', 'crimson': GH + r'\Banners\CrimsonBanner01.m2',
 'duelflag': GP + r'\DuelingFlag\DuelingFlag.m2', 'poster': r'World\Generic\Passive Doodads\WantedPosters\NewWantedPoster04.m2',
 'innsign': AZ + r'\Elwynn\PassiveDoodads\Signs\Shop\HumanInnSign.m2', 'fishrack': AZ + r'\BootyBay\PassiveDoodad\DeadFish\FishRack.m2',
 # tools, smithy
 'firewood1': GH + r'\firewood\FirewoodPile01.m2', 'firewood3': GH + r'\firewood\FirewoodPile03.m2', 'lumber': GH + r'\PeasantLumber\PeasantLumber01.m2',
 'plank': AZ + r'\RedRidge\PassiveDoodads\DockPieces\RedRidgeDocksPlank01.m2', 'shovel': AZ + r'\Elwynn\PassiveDoodads\Shovel\Shovel.m2',
 'anvil': AZ + r'\Elwynn\PassiveDoodads\Anvil\Anvil.m2', 'grindstone': AZ + r'\Westfall\PassiveDoodads\GrindStone\GrindStone.m2',
 'hammer': GH + r'\Weapons&Armor\HumanHammer02.m2', 'hammer1': GH + r'\Weapons&Armor\HumanHammer01.m2', 'sword': GH + r'\Weapons&Armor\HumanSword01.m2',
 'sword2': GH + r'\Weapons&Armor\HumanSword02.m2', 'mace': GH + r'\Weapons&Armor\HumanMace01.m2', 'axe': GH + r'\Weapons&Armor\HumanAxe01.m2',
 'spear': GH + r'\Weapons&Armor\HumanSpear01.m2', 'shield': GH + r'\Weapons&Armor\WallShield03.m2', 'lshield': GP + r'\Weapons\leather_shield.m2',
 'breast': GH + r'\Armor\ArmorBreastplateBlue.m2', 'breastG': GH + r'\Armor\ArmorBreastplateGold.m2', 'helm': GH + r'\Armor\ArmorHelmGold.m2',
 'helmG': GH + r'\Armor\ArmorHelmGreen.m2', 'legs': GH + r'\Armor\ArmorLegPlateBlue.m2', 'bracer': GH + r'\Armor\ArmorBracerBlue.m2',
 'shoulder': GH + r'\Armor\ArmorShoulderGold.m2', 'dummy': GH + r'\WoodenDummies\GeneralWoodenDummy02.m2', 'swdummy': GH + r'\WoodenDummies\StormWindWoodenDummy01.m2',
 'boots': GH + r'\Clothing\BootsLeatherBrown01.m2',
 # ground and camp
 'stump1': AZ + r'\Westfall\PassiveDoodads\TreeStumps\WestFallTreeStump01.m2', 'stump2': AZ + r'\Westfall\PassiveDoodads\TreeStumps\WestFallTreeStump02.m2',
 'estump': AZ + r'\Elwynn\PassiveDoodads\Trees\Stumps\ElwynnTreeStump01.m2', 'scarecrow': AZ + r'\Westfall\PassiveDoodads\Scarecrow\WestFallScarecrow.m2', 'dwscarecrow': AZ + r'\Duskwood\PassiveDoodads\DuskwoodScarecrow\DuskScarecrow.m2',
 'churn': AZ + r'\Westfall\PassiveDoodads\ButterChurner\ButterChurner.m2', 'fence': AZ + r'\Elwynn\PassiveDoodads\ElwynnFences\ElwynnWoodFence01.m2',
 'campfire': AZ + r'\Elwynn\PassiveDoodads\Campfire\ElwynnCampfire.m2', 'gnolltent3': AZ + r'\Duskwood\Buildings\GnollTent\GnollTent03.m2',
 'gnolltent2': AZ + r'\Duskwood\Buildings\GnollTent\GnollTent02.m2', 'doghouse': GH + r'\DogHouses\DogHouse.m2', 'orctable': r'World\Generic\Orc\Passive Doodads\Tables\OrcTable01.m2',
 'rope': GH + r'\Ropes\CaveKoboldRopeCoil.m2',
 'gift': GP + r'\Christmas\XmasGift02.m2',   # D33: Fizzwick's Parcel
}
# Boxes not in props.csv (models outside its scope): header extents measured by the first recipes.
FALLBACK = {'skull': (0.28, 0.24, 0.24), 'bone': (0.51, 0.2, 0.2), 'cat': (0.6, 0.35, 0.5), 'plank': (21.64, 1.2, 0.3),
            'duelflag': (1.0, 1.0, 12.0), 'doghouse': (2.2, 2.0, 2.2), 'boots': (0.65, 0.4, 0.5), 'hammer1': (1.2, 0.35, 0.5),
            'sword2': (1.5, 0.1, 0.3), 'mace': (1.1, 0.3, 0.3), 'axe': (1.2, 0.5, 0.2), 'spear': (0.2, 0.2, 2.8), 'lshield': (0.2, 0.9, 1.0),
            'breast': (0.61, 0.5, 0.5), 'breastG': (0.61, 0.5, 0.5), 'helm': (0.54, 0.4, 0.4), 'helmG': (0.54, 0.4, 0.4), 'legs': (0.36, 0.3, 0.3),
            'shoulder': (0.5, 0.4, 0.3), 'orctable': (2.4, 1.6, 0.85), 'stein': (0.20, 0.28, 0.32)}
def box(k):
    b = P.get(M[k].lower())
    if b is None:
        b = FALLBACK.get(k)
    if b is None:
        sys.exit('no box for ' + k)
    return b
def ext(k): return max(box(k))
def sc(models, D):
    """Scale so the LARGEST of `models` opens at ball diameter D."""
    if isinstance(models, str): models = [models]
    # The pumpkin's header box includes its vine, so in a mix it would shrink everything else to 0.68x: a mix
    # scales from its other members, and the (then larger-opening) pumpkin rides along.
    others = [m for m in models if M[m] != PUMPKIN]
    return round(D / (RATIO * max(ext(m) for m in (others or models))), 3)

def off(ax, ay, x, y): return [round(-(y - ay), 2), round(-(x - ax), 2)]
def head(dx, dy): return round(math.atan2(-dx, -dy), 3)
def q(p): return "'" + p + "'"

# ---------------------------------------------------------------- emitter and ledger
out = []
LEDGER = []   # (D_open, delta r^3, count, scene, models)
SCENE = ''
def L(s=''): out.append(s)
def fmt(v):
    if isinstance(v, bool): return 'true' if v else 'false'
    if isinstance(v, list): return '[' + ', '.join(str(x) for x in v) + ']'
    if isinstance(v, str): return '"' + v + '"'
    return str(v)
def item(arr, models, count, at=None, species=None, **kw):
    if (not species and arr in ('line', 'cluster') and count == 1 and at is not None and at >= 6.4 and not kw.get('goal')
            and (models == 'pumpkin' or models == 'pumpkin3' or (not isinstance(models, str) and {M[m] for m in models} == {PUMPKIN}))):
        # A big runner-up. The vine pumpkin's body is a third of the volume its box suggests (see PUMPKIN), so a litter
        # of three stands where the single jack-o'-lantern did and gives about the same growth (0.93 x).
        arr, count = 'cluster', 3
        kw['spacing'] = round(0.3 * at, 2); kw['gap'] = round(0.3 * at, 2)
    L('[[scenes.items]]')
    if not species and not isinstance(models, str):
        if len({M[m] for m in models}) == 1: models = models[0]   # a list of one model (both pumpkin keys) is that model
    if species:
        L('species = %s' % q(species))
        kw.setdefault('spacing', 2.0)
    elif isinstance(models, str):
        L('model = %s' % q(M[models]))
    else:
        L('models = [%s]' % ', '.join(q(M[m]) for m in models)); L('pick = "cycle"')
    L('arrangement = "%s"' % arr); L('count = %d' % count)
    if at is not None:
        kw['scale'] = sc(models, at); L('# opens at D %.2f' % at)
    for k, v in kw.items():
        L('%s = %s' % (k, fmt(v)))
    L()
    if not species and not kw.get('blocker'):
        ms = [models] if isinstance(models, str) else models
        s = kw.get('scale', 1.0)
        per = count / len(ms)
        for m in ms:
            b = box(m)
            LEDGER.append((RATIO * ext(m) * s, FILL * b[0] * b[1] * b[2] * s ** 3 * per, per, SCENE, m))
def scene(name, anchor, comment=None, yaw=None):
    global SCENE
    SCENE = name
    L()
    if comment:
        for c in comment.split('\n'): L('# ' + c)
    L('[[scenes]]'); L('name = "%s"' % name); L('anchor = [%s]' % ', '.join(str(a) for a in anchor))
    if yaw is not None: L('yaw = %s' % yaw)
    L()
def solid(key, x, y, A, yaw=0.0, scale=1.0, comment=None, **kw):
    """One solid prop (a table, a cart): walkable top (D29), blocks until edible."""
    if comment: L('# ' + comment)
    item('line', key, 1, offset=off(A[0], A[1], x, y), yaw=yaw, scale=scale, solid=True, spacing=0.2, **kw)
def blocker(key, x, y, A, yaw=0.0, scale=1.0, comment=None):
    """A visible, permanent, inedible obstacle (D29 `blocker`): the replacement for `[[barriers]]`."""
    if comment: L('# ' + comment)
    item('line', key, 1, offset=off(A[0], A[1], x, y), yaw=yaw, scale=scale, blocker=True, spacing=0.2)
def top(table, models, count, at, lift, x, y, A, gap=0.25, **kw):
    """A group on a table's top: spread uniformly over the top (a `table-set` square inside its edges, not a
    cluster whose stragglers fall off) and `level`, so the spawn samples the ground once at the table's
    spot like the table itself (on a slope, per-prop terrain sank half the meal into the top)."""
    b = box(table)
    sp = round(max(0.15, min(b[0], b[1]) * 0.5 - 0.3), 2)
    item('table-set', models, count, at=at, spacing=sp, gap=gap, lift=lift, level=True, offset=off(A[0], A[1], x, y), **kw)
def stairs(key, x, y, A, heading, count, rise, run, scale, yaw=0.0, comment=None):
    """A D29 `stairs` run: `count` solid steps from (x,y), rising `rise` per step along `heading`."""
    if comment: L('# ' + comment)
    item('stairs', key, count, offset=off(A[0], A[1], x, y), heading=heading, spacing=run, step=rise, scale=scale, yaw=yaw)

# ================================================================ header
L(r'''# Distances are yards, positions are WoW north/west/up. GENERATED by docs/azeroll/data/market-day-gen.py
# from the district plan in docs/azeroll/katamari-room-design.md §6; edit the generator, not this file.
# Level 2 of the campaign, "Market Day Mayhem" (D28, redesigned 2026-11-03): Goldshire's harvest fair as
# a Katamari town stage. The goal is the Prize Pumpkin on the judging stage in the village street; Hogger
# raids the cheese stall early and is an optional bonus at his raiders' camp past the livestock fair.
# Sizing rule: a prop rolls up once the ball's diameter reaches 2.15 x its longest scaled extent; the
# "opens at D" comments give that diameter. There are NO invisible [[barriers]]: every obstacle is a
# visible `blocker` prop (a parked hay wagon, the weaver's tent, two old stumps) or real geometry.
start = [-9446, 57, 56.5]
# Level 1 ends at r 1.25; a crumb under the beer-garden tables starts a little above its first.
start_radius = 0.32
# Facing south: down the village street, where the parade of runners-up leads to the judging stage.
start_heading = 3.14159
time_limit = 660
bounds = { radius = 680 }
claim = [{ centre = [-9450, 60, 57], radius = 650 }]
# Hogger's raiders camp on the forest shelf past the livestock fair; his early pass comes to the cheese stall.
camp = [-9568, 180, 59.5]
patrol = [-9470, 66, 57]
patrol_at = 12
patrol_until = 90

[level]
id = "market-day"
title = "Market Day Mayhem"
subtitle = "Goldshire's harvest fair, and Ma Stonefield's pumpkin is the talk of the street."
briefing = "Market day. Start under the beer-garden tables, follow the runners-up down the street, eat your way round the stalls and the livestock sale, and come back big enough for the Prize Pumpkin on the judging stage. Hogger's gnolls are camped past the fair with everything they stole; roll him up too if you can."
objective = "Roll up the Prize Pumpkin"
unlock_after = "lions-pride"
cast = "goldshire"
par = 280
pace = [0.75, 0.55, 0.4]

[goal]
kind = "collect"
target = "Prize Pumpkin"
# The pumpkin's pickup axis after scale (scene "Judging stage"): the Duskwood pumpkin's longest extent is 3.44 yd, so 5.59 at
# scale 1.625: a ball 12.0 yd across (r 6.0, r^3 216). Hogger's tier (8.6) comes first; the runners-up fill the gap.
axis = 5.59
area = "Judging stage"
title = "PRIZE PUMPKIN, ALL ROLLED UP!"
fail = "The Prize Pumpkin took first prize without you."

# D33: the level's hidden parcel (scene "The coop by the signpost"): Mirela's bandana.
[present]
id = "bandana"
title = "Mirela's Bandana"
display = 4873

# The tiers announce themselves as the ball outgrows things. `at` is the default diameter; with
# `reps` the tier opens when every representative is edible (the cast's adults, a herd's fit).
[[tiers]]
name = "Crumbs and coppers"
at = 0.0

[[tiers]]
name = "Critters"
at = 1.04
reps = ["species:Chicken", "species:Rabbit"]

[[tiers]]
name = "Livestock"
at = 2.58
reps = ["species:Sheep", "species:Cow"]

[[tiers]]
name = "Shoppers and horses"
at = 4.33
reps = ["adults", "species:Farm horse"]

[[tiers]]
name = "Raiders at the gate"
at = 6.02
reps = ["species:Forest Bear", "species:Riverpaw Gnoll"]

[[tiers]]
name = "Hogger"
at = 8.6
reps = ["species:Hogger"]

# At or above 2.15 x the goal axis (12.02), or `contact` calls the pumpkin Dislodge.
[[tiers]]
name = "The Prize Pumpkin"
at = 12.03

# Optional challenges. The ids are what the save file stores, so they never change.
[[challenges]]
id = "hogger"
title = "Roll up Hogger, who raided the cheese stall"
kind = "collect-all"
species = "Hogger"

[[challenges]]
id = "chickens"
title = "Chicken run: every chicken at the fair"
kind = "collect-all"
species = "Chicken"

# D33: `speed` is the comet's job now, `supper` is on every route and `unrammed` nobody planned around.
[[challenges]]
id = "garden"
title = "Last round: the whole beer garden inside sixty seconds"
kind = "within"
scene = "The Lion's Pride beer garden"
radius = 10
secs = 60

[[challenges]]
id = "parade"
title = "Clean sweep: every runner-up on the street"
kind = "scene"
scene = "Parade of the runners-up"
radius = 40

# ---------------------------------------------------------------------------------------
# Districts. The primary route is Beer garden > Market > South junction > South gate > Livestock
# fair > Raiders' camp > Fair gate > Parade home > Judging stage (the stage is a leaf of the trail
# graph, reached only through the camp and the fair's gate, so the autopilot sees the whole town and
# rolls Hogger on the way home). The smithy and the scarecrow field are side branches (tour). The far
# farms of the first version are gone: 400 yd of road to two scenes was sprawl, not a town stage.
# ---------------------------------------------------------------------------------------
[[areas]]
name = "Beer garden"
centre = [-9444, 54, 56.5]
radius = 14

[[areas]]
name = "Market"
centre = [-9478, 74, 56.8]
radius = 18

# The judging stage sits inside the Market's circle; being nearer its own centre wins the point.
[[areas]]
name = "Judging stage"
centre = [-9486, 60, 56.3]
radius = 8

# The south junction: the street's end at the lamppost, where the Westfall road leaves town. The map's
# roadside fence (x -9499, y 63-76) and the gypsy wagon make a 4.5 yd gap north-west of here that a
# ball of r 2+ wedges in, so every trail out of the square goes round the fence's east end, through this.
[[areas]]
name = "South junction"
centre = [-9505, 52, 56.5]
radius = 5

[[areas]]
name = "South gate"
centre = [-9509, 76, 57]
radius = 14

[[areas]]
name = "Smithy yard"
centre = [-9462, 122, 59]
radius = 15

[[areas]]
name = "Livestock fair"
centre = [-9546, 118, 59]
radius = 26

[[areas]]
name = "Raiders' camp"
centre = [-9568, 180, 59.5]
radius = 15

# The fair's gate: Stonefield's giant entries, where the parade home begins (a leaf node between the
# camp and the stage, so the way back from Hogger is the road, not a straight line through the trunks).
[[areas]]
name = "Fair gate"
centre = [-9540, 100, 59]
radius = 7

# The scarecrow contest in the meadow east of the north road: the tour's first branch, Free Roll's
# second size ladder (scarecrows D 5-11). Reached by the lane south of the thicket at (-9420, 55).
[[areas]]
name = "North lane"
centre = [-9420, 46, 57.5]
radius = 5

[[areas]]
name = "Scarecrow field"
centre = [-9393, 60, 60]
radius = 14

# Route graph (navigation only; the lines of goods are authored as scene spills).
# Shoppers' droppings along the street: apples and pears from the greengrocer's bags.
[[trails]]
name = "Down the street"
from = [-9446, 54, 56.5]
to = [-9478, 74, 56.8]
spacing = 3.5
wobble = 1.5
scale = 1.4
models = ['World\Generic\PassiveDoodads\Fruits\Fruit_Apple.m2', 'World\Generic\PassiveDoodads\Fruits\Fruit_Pear.m2', 'World\Generic\PassiveDoodads\Fruits\Fruit_Orange.m2']
# Down the street past the stage to the lamppost at the junction: more of the shoppers' fruit.
[[trails]]
name = "Street to the junction"
from = [-9478, 74, 56.8]
to = [-9505, 52, 56.5]
spacing = 3.5
wobble = 0.5
scale = 1.5
models = ['World\Generic\PassiveDoodads\Fruits\Fruit_Pear.m2', 'World\Generic\PassiveDoodads\Fruits\Fruit_Melon.m2', 'World\Generic\PassiveDoodads\Fruits\Fruit_Apple.m2']
# The potter's seconds, dropped on the Westfall road on the way to his stall.
[[trails]]
name = "Junction to the south gate"
from = [-9505, 52, 56.5]
to = [-9509, 76, 57]
spacing = 3.5
wobble = 0.5
scale = 1.5
models = ['World\Azeroth\Elwynn\PassiveDoodads\jugs\jug01.m2', 'World\Azeroth\Elwynn\PassiveDoodads\Jars\Jar01.m2', 'World\Azeroth\Elwynn\PassiveDoodads\jugs\jug02.m2']
# Hay dropped off the wagons on the way in.
[[trails]]
name = "Westfall road to the fair"
from = [-9509, 76, 57]
to = [-9546, 118, 59]
spacing = 5
wobble = 2.0
scale = 0.9
models = ['World\Generic\Human\Passive Doodads\HayPiles\ShadowfangHayPile02.m2', 'World\Azeroth\Elwynn\PassiveDoodads\Haystacks\haystack01.m2', 'World\Generic\Human\Passive Doodads\Buckets\bucket.m2']
# What the raiders dropped running home.
[[trails]]
name = "Fair to the raiders' camp"
from = [-9546, 118, 59]
to = [-9568, 180, 59.5]
spacing = 5
wobble = 2.0
scale = 1.0
models = ['World\Generic\PassiveDoodads\Barrel\BrokenBarrel02.m2', 'World\Generic\Human\Passive Doodads\Crates\StormwindCrate01.m2', 'World\Generic\PassiveDoodads\Furniture\Refuse\Bone02.m2']
# Back from the camp to the fair's gate with the rest of the loot: kegs and the stolen cheese.
[[trails]]
name = "Raiders back to the fair gate"
from = [-9568, 180, 59.5]
to = [-9540, 100, 59]
spacing = 6
wobble = 2.5
scale = 1.0
models = ['World\Generic\Human\Passive Doodads\BeerKegs\BeerKeg01.m2', 'World\Generic\Human\Passive Doodads\Food\CheeseWheel01.m2', 'World\Generic\Human\Passive Doodads\Sacks\PlaguedGrainSack01.m2']
# The parade home: bales and haystacks fallen off the carts between the fair's gate and the stage.
[[trails]]
name = "Parade home"
from = [-9540, 100, 59]
to = [-9493, 63, 56.3]
spacing = 7
wobble = 3.0
scale = 1.5
models = ['World\Generic\Human\Passive Doodads\HayPiles\ShadowfangHayPile01.m2', 'World\Azeroth\Elwynn\PassiveDoodads\Haystacks\haystack02.m2']
# Horseshoes and nails, more or less: what the smith dropped carrying his wares.
[[trails]]
name = "South gate to the smithy"
from = [-9509, 76, 57]
to = [-9462, 122, 59]
spacing = 6
wobble = 2.0
scale = 1.0
models = ['World\Generic\Human\Passive Doodads\Weapons&Armor\HumanHammer02.m2', 'World\Generic\Human\Passive Doodads\Armor\ArmorHelmGold.m2', 'World\Generic\Human\Passive Doodads\Buckets\bucket.m2']
# The lane out of the garden toward the scarecrow field, south of the thicket: the drovers' crumbs.
[[trails]]
name = "Up the north lane"
from = [-9444, 56, 56.5]
to = [-9420, 46, 57.5]
spacing = 4
wobble = 1.0
scale = 1.3
models = ['World\Generic\PassiveDoodads\Fruits\Fruit_Apple.m2', 'World\Generic\Human\Passive Doodads\Food\bread01.m2', 'World\Generic\Human\Passive Doodads\Mugs\Mug02.m2']
[[trails]]
name = "Lane to the scarecrow field"
from = [-9420, 46, 57.5]
to = [-9393, 60, 60]
spacing = 4
wobble = 1.0
scale = 1.6
models = ['World\Azeroth\Elwynn\PassiveDoodads\CornCrop\corn.m2', 'World\Generic\PassiveDoodads\Fruits\Fruit_Pear.m2', 'World\Azeroth\Elwynn\PassiveDoodads\CornCrop\corn.m2']

# Moving food. One herd per species (scene items with `species` borrow the template).
[[herds]]
species = "Chicken"
at = [-9440, 60, 56.5]
radius = 4
count = 8
fit = 0.35
behaviour = "skittish"
model = 'Creature\Chicken\Chicken.m2'

[[herds]]
species = "Piglet"
at = [-9480, 70, 56.8]
radius = 6
count = 8
fit = 0.62
behaviour = "skittish"
display = 503

[[herds]]
species = "Rabbit"
at = [-9393, 60, 60]
radius = 8
count = 8
fit = 0.48
behaviour = "skittish"
model = 'Creature\Rabbit\Rabbit.m2'

[[herds]]
species = "Sheep"
at = [-9540, 118, 59]
radius = 9
count = 14
fit = 0.8
behaviour = "skittish"
model = 'Creature\Sheep\Sheep.m2'

[[herds]]
species = "Cow"
at = [-9553, 128, 59]
radius = 7
count = 6
fit = 1.2
behaviour = "passive"
model = 'Creature\Cow\Cow.m2'

[[herds]]
species = "Farm horse"
at = [-9556, 108, 59]
radius = 7
count = 5
fit = 1.8
behaviour = "skittish"
model = 'Creature\RidingHorse\RidingHorse.m2'

[[herds]]
species = "Forest Bear"
at = [-9586, 198, 54]
radius = 10
count = 3
fit = 2.3
behaviour = "passive"
display = 1006

[[herds]]
species = "Riverpaw Gnoll"
at = [-9568, 180, 59.5]
radius = 12
count = 8
fit = 2.8
behaviour = "hostile"
display = 175

[[herds]]
species = "Hogger"
at = [-9568, 180, 59.5]
radius = 0
count = 1
fit = 4
behaviour = "hostile"
display = 384
boss = true
''')

# ================================================================ 1. the beer garden (nursery)
A = (-9444, 55, 56.5)
scene("The supper tray", A,
 "START. The Lion's Pride has rolled its tables out for market day and Bessie dropped the supper\n"
 "tray. Everything within 3 yd opens between D 0.55 and 0.9; the mug ring is the 'soon'; two spills\n"
 "leave the garden: the runners-up down the street, and the crumbs west to the cheese stall.")
item('cluster', 'muffin', 14, at=0.55, spacing=1.1, gap=0.16, jitter=0.06, offset=off(*A[:2], -9445.5, 56.5))
item('cluster', ['wedge', 'wedgeS', 'slice'], 14, at=0.6, spacing=1.3, gap=0.18, jitter=0.06, offset=off(*A[:2], -9444.5, 55.5))
item('cluster', ['half', 'wheel', 'half', 'loaf'], 18, at=0.68, spacing=1.6, gap=0.2, jitter=0.06, offset=off(*A[:2], -9443.5, 53.5))
item('cluster', ['pie', 'pie', 'half'], 12, at=0.78, spacing=1.7, gap=0.25, jitter=0.06, offset=off(*A[:2], -9446, 54))
item('ring', ['mug', 'foam', 'mug', 'stein'], 14, at=0.9, spacing=2.6, wobble=0.25, jitter=0.05, offset=off(*A[:2], -9444.5, 55))
item('ring', ['apple', 'orange', 'pear'], 10, at=1.0, spacing=3.3, wobble=0.3, jitter=0.05, offset=off(*A[:2], -9444, 55))
item('cluster', ['stein1', 'goblet', 'bowl', 'plate', 'mug2'], 12, at=1.15, spacing=1.5, gap=0.3, jitter=0.05, topple=0.3, topple_lift=0.1, offset=off(*A[:2], -9446.5, 51.5))
item('cluster', ['turkey', 'haunch', 'turkey'], 6, at=1.3, spacing=1.6, gap=0.35, jitter=0.05, offset=off(*A[:2], -9443, 59))
item('cluster', 'coins', 8, at=0.75, spacing=0.9, gap=0.25, offset=off(*A[:2], -9442, 57.5))
item('cluster', 'cleaver', 1, at=1.4, spacing=0.2, tilt=0.5, offset=off(*A[:2], -9442.5, 54.5))

A = (-9441, 51, 56.5)
scene("The Lion's Pride beer garden", A,
 "Three trestle tables along the inn's north-west corner (solid: walkable tops, D29), the last round\n"
 "on top, bale steps up to the middle one, the kegs rolled out of the cellar, two benches. Farley is\n"
 "polishing, Hopkins is barking, Remy is asleep under the end table.")
for i, (x, y) in enumerate([(-9436, 49.5), (-9441, 49.5)]):
    solid('smalltable', x, y, A, yaw=0.0, comment='trestle table %d (top 1.13), in a row along the inn side' % (i + 1))
    top('smalltable', ['foam', 'mug2', 'foam', 'jug2', 'stein1', 'pie'], 7, 1.1, 1.13, x, y, A, gap=0.3, jitter=0.05)
stairs('bale1', -9441, 53.3, A, heading=E, count=6, rise=0.19, run=0.5, scale=0.45, yaw=0.0,
       comment="six hay bales up to the middle table from the street side (0.19 a step); 3 yd clear of the other tables")
item('line', ['jar', 'jug', 'jar2', 'jug2'], 8, at=1.3, spacing=0.7, heading=S, wobble=0.15, jitter=0.05, offset=off(*A[:2], -9445, 52.5))
item('cluster', ['bottle', 'green', 'jug', 'bottle'], 10, at=1.5, spacing=1.8, gap=0.35, topple=0.4, topple_lift=0.15, jitter=0.05, offset=off(*A[:2], -9437, 53))
item('spill', ['orange', 'apple', 'orange', 'pear'], 12, at=1.05, spacing=0.6, heading=head(-8, 2), bend=0.06, jitter=0.08, offset=off(*A[:2], -9436, 54.6))
item('heap', ['keg1', 'keg2', 'keg1'], 6, at=3.0, spacing=1.8, step=1.0, gap=1.1, tilt=0.06, jitter=0.05, offset=off(*A[:2], -9432, 51))
item('line', 'gkeg', 1, at=6.0, spacing=0.2, yaw=0.3, offset=off(*A[:2], -9432, 56), tilt=0.0)
L("# The quality-control team's empties, a line of them along the bench.")
item('line', ['mug', 'foam', 'stein', 'mug'], 9, at=0.95, spacing=0.5, heading=S, wobble=0.1, topple=0.3, topple_lift=0.1, offset=off(*A[:2], -9438, 58.5))
item('line', 'bench', 2, at=4.5, spacing=5.0, heading=S, yaw=0.0, offset=off(*A[:2], -9441, 59.5))
item('cluster', ['crate2', 'crate3', 'sack'], 6, at=2.3, spacing=1.6, gap=0.6, jitter=0.05, offset=off(*A[:2], -9431, 46))

A = (-9441, 61, 56.5)
scene("The coop by the signpost", A,
 "The feed sack burst by the inn's hen coop: grain and bread on the ground, the hens on it, the\n"
 "feed bucket kicked over, the crates they climbed. Pip and Poppy are chasing them.")
item('cluster', ['corn', 'corn', 'half', 'slice', 'muffin'], 20, at=0.65, spacing=1.8, gap=0.2, jitter=0.1, tilt=0.15, offset=off(*A[:2], -9441, 61))
item('cluster', ['grain', 'grain2', 'sackflat'], 5, at=1.9, spacing=1.0, gap=0.7, topple=0.4, topple_lift=0.15, jitter=0.05, offset=off(*A[:2], -9437.5, 62.0))
item('cluster', 'bucket', 1, at=2.2, spacing=0.2, topple=1.0, topple_lift=0.4, offset=off(*A[:2], -9442.5, 63))
L("# D33 parcel pocket (replay-design.md 3.2): eight crates in a U on the start's flat. Inner width 1.6 yd, open to the beer")
L("# garden (lower y) through a 0.95 yd gap between the two jambs: a ball under D 1.05 slips in, from D 1.9 the crates are")
L("# food (solid, not blockers). The parcel lies at the back, the hens go in and out. Never a bot target (replay.rs).")
for x, y in [(-9440.25, 62.1), (-9440.25, 63.0), (-9442.75, 62.1), (-9442.75, 63.0),    # the two side walls
             (-9440.75, 61.2), (-9442.25, 61.2),                                        # the jambs, 1.5 apart (a capture: D 1.24 passed at 1.84; the crate is narrower than its box)
             (-9441.05, 63.95), (-9441.95, 63.95)]:                                      # the back wall (a capture showed the pocket open at the back)
    item('line', 'crate1', 1, at=1.9, spacing=0.2, solid=True, yaw=0.0, offset=off(*A[:2], x, y))
item('line', 'gift', 1, at=0.8, spacing=1.0, present=True, name="Fizzwick's Parcel", yaw=0.0, offset=off(*A[:2], -9441.5, 63.3))
item('cluster', ['apple', 'pear', 'apple', 'orange'], 10, at=0.95, spacing=2.0, gap=0.3, jitter=0.1, offset=off(*A[:2], -9443, 59))
item('cluster', None, 4, species='Chicken', spacing=2.2, offset=off(*A[:2], -9440, 60))

# ================================================================ 2. the two lines out of the garden
A = (-9452, 56, 56.5)
scene("Parade of the runners-up", A,
 "Every pumpkin that did not win, lined up down the east side of the street in order of size, each\n"
 "one bigger than the last, ending at the judging stage. The line is the level's spine: a small ball\n"
 "follows the pears, a big one comes back for the giants. Ribbons on the big ones.")
item('spill', ['pear', 'apple', 'orange', 'pear'], 10, at=0.85, spacing=0.7, heading=S, bend=0.04, jitter=0.1, tilt=0.15, offset=off(*A[:2], -9452, 56))
item('spill', ['melon', 'bananas', 'melon', 'fruitbowl'], 7, at=1.4, spacing=1.2, heading=S, bend=0.04, jitter=0.1, offset=off(*A[:2], -9459, 56))
item('spill', ['watermelon', 'melon', 'watermelon'], 5, at=2.0, spacing=1.6, heading=S, bend=0.04, jitter=0.1, offset=off(*A[:2], -9465, 56.5))
L("# From here the runners-up are pumpkins, two to a size, with a ribbon on the stake beside the row.")
for x, D, n in [(-9467.5, 2.8, 2), (-9471, 3.8, 2), (-9474.5, 5.0, 2), (-9478, 6.4, 1)]:
    item('line', ['pumpkin', 'pumpkin3'], n, at=D, spacing=1.0 + D * 0.5, heading=S, jitter=0.05, offset=off(*A[:2], x, 56.5))

A = (-9453, 60, 56.3)
scene("Crumbs to the cheese stall", A,
 "Hodge walked from the cheese stall to the beer garden eating a loaf; the trail runs back the way he came.")
item('spill', ['half', 'wedge', 'wheel', 'slice', 'half'], 16, at=0.7, spacing=0.65, heading=head(-15, 10), bend=0.05, jitter=0.08, offset=off(*A[:2], -9453, 60))
item('spill', ['wheel', 'wedgeS', 'wheelS', 'pie'], 10, at=1.0, spacing=0.9, heading=head(-15, 10), bend=0.05, jitter=0.08, offset=off(*A[:2], -9461, 65.5))
item('scatter', ['muffin', 'coins', 'slice', 'wedge'], 24, at=0.6, spacing=2.2, jitter=0.1, tilt=0.2, offset=off(*A[:2], -9456, 62))

# ================================================================ 3. the market: stalls round the street
A = (-9468, 70, 56.6)
scene("Cheese and bread stall", A, yaw=0.0, comment=
 "Gouda's stall on the west side of the street. Two trestle tables of bread and cheese (solid tops,\n"
 "sack steps up to the first), the Grand Wheel and its tower on the ground, Hogger's last raid (the\n"
 "third table on its side, wedges everywhere), the stall cat, and the hens that got there first.")
for i, (x, y) in enumerate([(-9466.5, 72.5), (-9470, 72.5)]):
    solid('smalltable', x, y, A, yaw=1.5708, comment='trestle %d (top 1.13)' % (i + 1))
    top('smalltable', ['loaf', 'half', 'muffin', 'pie', 'baguette', 'wedge'], 9, 0.95, 1.13, x, y, A, jitter=0.05)
stairs('sackflat', -9466.6, 69.4, A, heading=W, count=6, rise=0.205, run=0.5, scale=0.7, yaw=0.0,
       comment="flour sacks stacked up to the first table from the street side")
item('cluster', ['wedge', 'wedgeS', 'wheel', 'slice'], 16, at=0.7, spacing=1.8, gap=0.2, jitter=0.08, tilt=0.2, offset=off(*A[:2], -9467, 68))
L("# Gouda's cheese tower: wheels at market sizes, and the Grand Wheel (a cheese 1 yd across).")
item('heap', ['wheel', 'wheelS', 'wheel'], 12, at=1.6, spacing=1.1, step=0.11, gap=0.35, jitter=0.06, offset=off(*A[:2], -9466, 67))
item('cluster', 'wheel', 1, at=2.4, spacing=0.2, yaw=0.7, offset=off(*A[:2], -9463.5, 69))
item('line', 'smalltable', 1, scale=1.0, spacing=0.2, topple=1.0, topple_lift=0.55, offset=off(*A[:2], -9464, 76.5), yaw=0.4)
item('spill', ['wedge', 'wheel', 'half', 'wedgeS'], 12, at=0.8, spacing=0.6, heading=head(-3, -8), bend=0.3, jitter=0.08, offset=off(*A[:2], -9465, 76))
item('cluster', ['bread', 'bread', 'baguette'], 5, at=1.6, spacing=1.4, gap=0.5, jitter=0.05, tilt=0.2, offset=off(*A[:2], -9464.5, 74))
item('cluster', 'cat', 1, at=1.5, spacing=0.2, lift=1.13, level=True, yaw=2.5, offset=off(*A[:2], -9470, 72.5))
L("# The knife-grinder's awning, pegged under the map's tree clump in the middle of the square (two trunks")
L("# 2.9 yd apart and three bushes at (-9473,76)): a blocker, so the clump is one obstacle the planner routes")
L("# round instead of a gap a ball of r 2-3 wedges in (runs A3, B2).")
blocker('awning', -9474, 77.8, A, yaw=0.3, scale=1.6, comment="the awning under the trees, sealed against the fence rail at (-9475,80) and the second trestle")
item('cluster', None, 3, species='Chicken', spacing=2.0, offset=off(*A[:2], -9470, 76))

A = (-9479, 79, 56.7)
scene("Greengrocer's produce stand", A,
 "Fenwick's two long tables (solid), a plank up the near one, the apple pyramid, the melon heap and\n"
 "the small pumpkins on the ground, corn in its husks, feed sacks, and the apple-bobbing barrel with\n"
 "a queue of children. Dame Perrin and Goodwife Ashby are gossiping across the apples.")
L("# The tables stand 2.5 yd south-west of the first version's: the knife-grinder's blocker awning (6.4 x 5.4 yd at")
L("# scale 1.6, centred at (-9474, 77.8)) covered the first table and the whole plank (placement survey 2026-11-03).")
for i, (x, y) in enumerate([(-9480.0, 81.5), (-9484.0, 81.5)]):
    solid('wftable', x, y, A, yaw=1.5708, comment='long table %d (top 1.07)' % (i + 1))
    top('wftable', ['apple', 'pear', 'orange', 'bananas', 'apple', 'melon'], 10, 1.0, 1.07, x, y, A, gap=0.3, jitter=0.06)
L("# A dock plank leans up the east side of the first table (4.3 yd long: never edible before the stage). Its box")
L("# centre sits 0.8 yd east of its origin; the high end overlaps the table's east edge (y 80.15) by 0.1.")
item('line', 'plank', 1, scale=0.2, pitch=0.25, yaw=1.5708, lift=0.55, solid=True, spacing=0.2, offset=off(*A[:2], -9480.0, 78.95))
item('heap', ['apple', 'apple', 'orange', 'pear'], 16, at=0.85, spacing=0.8, step=0.1, gap=0.22, jitter=0.06, offset=off(*A[:2], -9478.5, 84.5))
item('heap', ['melon', 'melonslice', 'melon'], 9, at=1.35, spacing=0.9, step=0.2, gap=0.35, jitter=0.06, offset=off(*A[:2], -9487.5, 83.0))
item('heap', ['pumpkin', 'pumpkin3'], 6, at=2.1, spacing=1.2, step=0.35, gap=0.6, jitter=0.08, offset=off(*A[:2], -9484.5, 78))
item('scatter', 'corn', 14, at=0.75, spacing=0.7, jitter=0.15, tilt=0.15, offset=off(*A[:2], -9481.5, 85.3))
item('line', ['grain', 'grain2', 'herbs', 'grapebucket'], 5, at=2.6, spacing=1.3, heading=E, wobble=0.3, topple=0.2, topple_lift=0.2, jitter=0.05, offset=off(*A[:2], -9486, 80))
item('line', 'applebob', 1, at=5.8, spacing=0.2, yaw=0.4, offset=off(*A[:2], -9473, 84.5))
item('cluster', None, 3, species='Chicken', spacing=2.0, offset=off(*A[:2], -9483, 84))

A = (-9479, 88, 56.8)
scene("Fishmonger at the horse trough", A,
 "Marlene keeps her catch cold in the village water basin (a real one, at the fence post). A small\n"
 "table of fish platters, the fish rack, buckets, bones underfoot, and Old Pete fishing in the trough.")
solid('tinytable', -9480.5, 88.5, A, yaw=0.3, comment='the fish table (top 1.12)')
top('tinytable', ['platter', 'platter', 'bowl'], 4, 1.3, 1.12, -9480.5, 88.5, A, gap=0.4, jitter=0.05)
item('cluster', ['fishskel', 'fishskel', 'bone'], 9, at=0.8, spacing=1.4, gap=0.3, jitter=0.1, tilt=0.3, offset=off(*A[:2], -9479, 90.5))
item('cluster', ['bucket', 'pail', 'gbucket'], 5, at=2.2, spacing=1.3, gap=0.6, jitter=0.05, offset=off(*A[:2], -9482.5, 91))
item('line', 'fishrack', 1, at=5.0, spacing=0.2, yaw=1.2, offset=off(*A[:2], -9484, 89))
item('cluster', ['platter', 'knife'], 3, at=1.5, spacing=0.9, gap=0.5, tilt=0.3, offset=off(*A[:2], -9477.5, 90))

A = (-9459, 73, 56.6)
scene("Guards' picnic", A,
 "The guard post, with the watch on its lunch break: a round of pies on an upturned crate, the\n"
 "helmets and shields set down to eat, a spear leaning on the signpost. Dughan is in charge.")
solid('wftable', -9459, 72, A, yaw=0.0, comment='the watch table (top 1.07)')
top('wftable', ['pie', 'stein1', 'bread', 'turkey', 'plate', 'goblet'], 9, 1.05, 1.07, -9459, 72, A, gap=0.3, jitter=0.05)
# (Flour sacks, not the first version's 0.4-scale crates: those are Dislodge-state at the start radius 0.32, so the
# ball knocked the stair apart instead of climbing it; sacks at 0.7 stay Wall below r 0.51. Placement pass D35.)
stairs('sackflat', -9459.2, 69.1, A, heading=W, count=5, rise=0.225, run=0.5, scale=0.7, yaw=0.0,
       comment="four small crates stacked up to the table from the street side")
item('cluster', ['helm', 'helmG', 'shield', 'lshield', 'bracer', 'legs'], 10, at=1.5, spacing=2.0, gap=0.35, jitter=0.1, tilt=0.3, offset=off(*A[:2], -9456.5, 75))
item('cluster', ['stein1', 'mug2', 'bottle', 'haunch'], 8, at=1.2, spacing=1.6, gap=0.3, topple=0.3, topple_lift=0.1, jitter=0.05, offset=off(*A[:2], -9461.5, 75.5))
item('cluster', ['spear', 'sword'], 3, at=3.2, spacing=0.8, gap=0.4, topple=0.6, topple_lift=0.15, offset=off(*A[:2], -9456, 71))
item('cluster', ['breast', 'breastG'], 2, at=1.6, spacing=0.8, gap=0.5, offset=off(*A[:2], -9462, 71))

A = (-9514, 77, 57.8)
scene("Potter's stall", A,
 "Clayton's pots on two small tables south-west of the wagon (5 yd clear of it and 9 yd clear of the\n"
 "map's fence by the signpost, where the first version wedged), a crate stair up the first, the seconds\n"
 "heaped on the ground, and the ones that did not survive the cart ride. Lady Vance is browsing.")
for i, (x, y) in enumerate([(-9512.5, 78), (-9515.5, 78)]):
    solid('tinytable', x, y, A, yaw=1.5708, comment='pot table %d (top 1.12)' % (i + 1))
    top('tinytable', ['jug', 'jar', 'jug2', 'jar2', 'bowl'], 6, 1.3, 1.12, x, y, A, gap=0.3, jitter=0.05)
stairs('bale1', -9512.6, 74.2, A, heading=W, count=6, rise=0.225, run=0.5, scale=0.45, yaw=0.0, comment="hay bales up to the first pot table from the street side (the first version's 0.4-scale crates are Dislodge-state at the start radius; bales are 1.5 yd wide, so a ball on the slope stays on them)")
item('heap', ['jug', 'jug2', 'jar', 'orcjug', 'jar2', 'orcbottle'], 14, at=1.7, spacing=1.3, step=0.3, gap=0.4, jitter=0.08, offset=off(*A[:2], -9517, 74))
item('cluster', ['plate', 'bowl', 'dirty', 'plate'], 10, at=0.9, spacing=1.3, gap=0.25, jitter=0.1, tilt=0.3, offset=off(*A[:2], -9510.0, 72.3))
item('cluster', ['orcjug', 'orcjug2'], 3, at=3.3, spacing=1.0, gap=0.8, jitter=0.05, offset=off(*A[:2], -9518, 79))
item('cluster', ['grapebucket', 'gbucket3'], 3, at=2.8, spacing=1.0, gap=0.7, offset=off(*A[:2], -9509, 74))

A = (-9492, 78, 56.6)
scene("Mirela's wagon, unloading", A,
 "The gypsy wagon on the square is the map's own; its goods come off on the street side: a crate\n"
 "stack, barrels, the merchant's lunch, a suspiciously large cheese, and a helm left on the barrels.\n"
 "The wagon's far side is a pocket only a small ball fits into (the old stumps stand at both ends).")
item('heap', ['crate1', 'crate2', 'crate3', 'swcrate'], 14, at=2.9, spacing=1.8, step=0.55, gap=0.8, tilt=0.08, topple=0.1, topple_lift=0.3, jitter=0.06, offset=off(*A[:2], -9490, 76))
item('heap', ['wfbarrel', 'barrel', 'barrel2'], 8, at=2.4, spacing=1.5, step=0.9, gap=0.9, tilt=0.06, topple=0.25, topple_lift=0.45, jitter=0.05, offset=off(*A[:2], -9487.5, 79.5))
item('line', ['grain', 'herbs', 'grain2', 'kelp'], 6, at=2.2, spacing=1.2, heading=S, wobble=0.3, topple=0.2, topple_lift=0.2, jitter=0.05, offset=off(*A[:2], -9491.5, 75))
item('spill', ['bread', 'melon', 'apple', 'bottle', 'green', 'pear', 'plate'], 14, at=1.2, spacing=0.8, heading=head(5, -6), bend=0.3, jitter=0.1, tilt=0.2, offset=off(*A[:2], -9489, 80))
item('cluster', 'wheel', 1, scale=12, spacing=0.2, yaw=0.7, offset=off(*A[:2], -9483.5, 78.0))
item('cluster', 'boar', 1, at=4.0, spacing=0.2, yaw=2.1, offset=off(*A[:2], -9488, 77))
item('cluster', 'helm', 1, at=1.3, spacing=0.15, tilt=0.3, offset=off(*A[:2], -9488.8, 78.0))
L("# Two old stumps the wagon is parked between, at the ends of its far-side lane: permanent, inedible")
L("# blockers (D29). The lane between them is a small-ball pocket (the map's post, harness, rope, bucket).")
blocker('stump1', -9490.5, 89, A, yaw=0.4, scale=1.5, comment='north stump')
blocker('stump1', -9503.5, 89, A, yaw=2.3, scale=1.9, comment='south stump, wide enough to meet the wagon')
L("# A third stump closes the corner between the map's roadside fence (x -9499, y 63-76) and the wagon,")
L("# and sits over the map's own crate and three barrels at the wagon's south-west corner, which lured")
L("# balls of r 1.7-3 into a 4.5 yd dead end (runs 5-7): inside a blocker they are hidden from the planner.")
blocker('stump1', -9499.5, 77.5, A, yaw=1.1, scale=1.6, comment='the fence-corner stump')
L("# The merchant's horses' hay fills the lane itself: the map's harness, rope, bucket and post sit inside")
L("# this blocker, so no ball of any size plans a path to them (runs 5-6 still reached for them).")
blocker('bighay1', -9497, 88.5, A, yaw=0.2, scale=1.15, comment="the haystack in the wagon's lee")

# ================================================================ 4. the south gate: the weaver, the big goods, the pumpkin wagon

A = (-9507, 99, 58)
scene("Weaver's tent", A,
 "Flaxen's tent is pitched in the gap between the bare trees and the big Elwynn trunk (a permanent\n"
 "blocker: tent pegs). Her goods spread toward the square: yarn, spools, folded cloth, the rugs and\n"
 "the rug racks. Mirela tells fortunes at the tent's corner.")
blocker('tent', -9507, 99, A, yaw=-0.49, scale=1.25, comment="the tent, long axis along the gap (bare tree to trunk)")
L("# Her goods lie on the open road side of the tent, never in the pocket between the tent and the stump.")
item('cluster', ['yarnR', 'yarnY', 'spoolB', 'spoolY'], 16, at=0.8, spacing=1.8, gap=0.22, jitter=0.08, offset=off(*A[:2], -9513, 94))
item('cluster', ['shirt', 'pants', 'cushion', 'shirt'], 10, at=1.4, spacing=1.8, gap=0.4, jitter=0.06, tilt=0.1, offset=off(*A[:2], -9515, 90))
item('cluster', ['rug', 'rug1'], 3, at=4.2, spacing=2.0, gap=1.5, jitter=0.05, offset=off(*A[:2], -9517, 95))
item('line', ['rugrack', 'rugrack1'], 2, at=5.5, spacing=3.0, heading=S, yaw=0.0, offset=off(*A[:2], -9512, 98))
item('cluster', ['scry', 'skull', 'orcbottle', 'lantern'], 6, at=1.7, spacing=1.2, gap=0.4, jitter=0.05, offset=off(*A[:2], -9511, 92))
item('cluster', ['chest2', 'crate3'], 3, at=2.4, spacing=1.2, gap=0.7, offset=off(*A[:2], -9518, 92))
L("# A loaded hay wagon parked in the gap between the two Elwynn trunks at (-9514,103) and (-9518,114),")
L("# the pocket a ball of r 4-5 used to wedge in. Permanent blocker; nothing edible is placed beside it.")
blocker('haywagon', -9516.5, 108.5, A, yaw=1.22, scale=1.1, comment='the wagon, long axis along the gap')

A = (-9528, 92, 58.8)
scene("Stallholders' big goods", A,
 "What the stallholders brought and could not unpack, left on the Westfall road out of town: awnings,\n"
 "cages, a strongbox, a catapult trunk somebody is selling as a bench, the inn's shop counter. The ball\n"
 "of r 2.5-4 eats here on the way to the fair.")
item('ring', ['awning', 'gcage2', 'chest', 'trunk', 'awning', 'gcage'], 6, at=7.0, spacing=7.0, jitter=0.04, offset=off(*A[:2], -9528, 92))
item('cluster', ['cage1', 'cage2', 'cage3', 'chest2'], 6, at=4.6, spacing=4.5, gap=2.4, jitter=0.05, offset=off(*A[:2], -9527, 90))
item('cluster', ['counter', 'bench'], 2, at=5.5, spacing=5.0, gap=4.0, offset=off(*A[:2], -9533, 96))
item('cluster', ['bucket', 'sack', 'barrel', 'gbarrel', 'orcbarrel'], 10, at=2.9, spacing=5.0, gap=1.0, jitter=0.08, tilt=0.1, offset=off(*A[:2], -9529, 94))
item('cluster', None, 3, species='Piglet', spacing=3.0, offset=off(*A[:2], -9523, 90))

A = (-9528, 80, 58.8)
scene("The pumpkin wagon", A,
 "The wagon that brought the big entries, tipped on the verge of the Westfall road, its load rolled off\n"
 "in order of size (D 7 to 9.5): the second ladder of giants, between the fair and the parade home.")
item('line', 'haywagon', 1, at=10.8, spacing=0.2, yaw=2.2, offset=off(*A[:2], -9532, 77))
for x, y, D in [(-9521, 70, 7.0), (-9523, 76, 7.6), (-9525, 83, 8.3), (-9529, 87, 9.0), (-9534, 83, 9.5)]:
    item('line', ['pumpkin', 'pumpkin3'], 1, at=D, spacing=0.2, offset=off(*A[:2], x, y))
item('cluster', ['bale2', 'bale1', 'haystack1'], 5, at=4.2, spacing=3.0, gap=1.4, jitter=0.06, offset=off(*A[:2], -9531, 71))

# ================================================================ 5. the judging stage
A = (-9486, 57.5, 56.3)
scene("Judging stage", A,
 "Ma Stonefield's Prize Pumpkin, grown since spring, on a two-tier bale plinth in the middle of the\n"
 "street (both tiers solid: walkable, never edible here at 17+ yd). the Duskwood pumpkin at 1.625 is 5.59 yd:\n"
 "the [goal] axis, a ball 12 yd across. Eight bale steps climb the north face to the trophies on the\n"
 "ledge; banners at the corners; the contest flag over it all. The runners-up end at the foot.")
item('line', 'bale1', 1, scale=2.4, spacing=0.2, yaw=0.0, solid=True, offset=off(*A[:2], -9486, 57.5))
item('line', 'bale2', 1, scale=2.0, spacing=0.2, yaw=0.0, solid=True, lift=1.0, offset=off(*A[:2], -9486, 57.5))
item('cluster', 'pumpkin', 1, scale=1.625, spacing=0.2, yaw=0.4, lift=2.08, goal=True, offset=off(*A[:2], -9486, 57.5))
stairs('bale1', -9478.6, 57.5, A, heading=S, count=5, rise=0.25, run=0.55, scale=0.6, yaw=0.0,
       comment="five bale steps from the north (0.25 a step, 2.8 yd run) to the plinth's ledge at 1.0; food from D 4.3, so a mid-size ball never gets stuck climbing them")
L("# The trophies on the north ledge (lift 1.0): cups, ribbons (helms), the pie that bribed the judges.")
L("# (The ledges are 0.65 yd strips: bale1 spans x -9482.5..-9490.25 and the upper tier -9483.15..-9489.65; surveyed.)")
item('line', ['goblet', 'helm', 'pie', 'goblet', 'helmG'], 7, at=1.2, spacing=0.45, heading=E, wobble=0.05, lift=1.0, level=True, jitter=0.05, offset=off(*A[:2], -9482.8, 57.4))
item('line', ['goblet', 'stein1', 'bottle'], 4, at=1.3, spacing=0.6, heading=E, wobble=0.05, lift=1.0, level=True, jitter=0.05, offset=off(*A[:2], -9489.95, 57.4))
item('ring', 'crimson', 4, at=9.9, spacing=7.0, yaw=0.0, wobble=0.0, offset=off(*A[:2], -9486, 57.5))
item('ring', 'flagpole', 2, scale=1.0, spacing=6.5, yaw=0.0, offset=off(*A[:2], -9486, 57.5))
item('cluster', 'duelflag', 1, scale=1.0, spacing=0.2, yaw=0.0, offset=off(*A[:2], -9491, 64))
L("# The last of the runners-up, at the south foot of the stage: the ones that lost by a hair. (Nothing is")
L("# authored within 8 yd of the tree clump east of the stage at (-9483,47), the first version's wedge.)")
item('line', ['pumpkin', 'pumpkin3'], 2, at=8.2, spacing=5.0, heading=W, jitter=0.03, offset=off(*A[:2], -9495, 62))
item('line', 'pumpkin', 1, at=10.0, spacing=0.2, yaw=0.9, offset=off(*A[:2], -9496, 47))
item('line', 'pumpkin3', 1, at=11.0, spacing=0.2, yaw=2.0, offset=off(*A[:2], -9497, 60))
item('cluster', ['pumpkin', 'pumpkin3', 'watermelon'], 5, at=2.2, spacing=3.0, gap=0.8, jitter=0.15, offset=off(*A[:2], -9479, 64))

A = (-9486, 72.5, 56.6)
scene("The judges' table", A,
 "Appleton and Cobblepot's table north-west of the stage (solid, top 1.68), north of the street line\n"
 "the crowd walks: score scrolls, cups, the pies that came with the entries. Their chairs behind it,\n"
 "the auctioneer's podium, the bard and the dancers' ring of mugs on the grass toward the stairs.")
solid('sftable', -9486, 72.5, A, yaw=0.0, comment='the judges table (top 1.68)')
top('sftable', ['goblet', 'pie', 'plate', 'goblet', 'bottle', 'stein1'], 9, 1.25, 1.68, -9486, 72.5, A, gap=0.3, jitter=0.05)
stairs('bale1', -9486.2, 77.2, A, heading=E, count=8, rise=0.205, run=0.5, scale=0.45, yaw=0.0, comment="hay bales up to the judges' table from the west (Wall-state at the start radius, unlike the first version's crates, and wide enough to stay on)")
item('line', 'wchair', 2, at=1.6, spacing=4.0, heading=S, yaw=0.0, topple=0.0, offset=off(*A[:2], -9486, 75.3))
item('cluster', ['mug', 'foam', 'stein', 'goblet'], 10, at=1.0, spacing=2.4, gap=0.4, topple=0.3, topple_lift=0.1, jitter=0.05, offset=off(*A[:2], -9478, 66))
item('cluster', 'podium', 1, at=3.2, spacing=0.2, yaw=1.0, offset=off(*A[:2], -9483, 70.5))

A = (-9501, 49, 56.3)
scene("Last year's champion", A,
 "Last year's winner, sulking beside the cart it arrived on (the cart lost a wheel on the hill), on the\n"
 "street south of the junction, well clear of the tree clump east of the stage. A 3.5 yd pumpkin:\n"
 "edible at D 7.7, a landmark from the street long before that.")
item('cluster', 'pumpkin3', 1, at=7.7, spacing=0.2, yaw=1.1, offset=off(*A[:2], -9501, 49))
item('line', 'brokencart', 1, at=10.5, spacing=0.2, yaw=2.2, offset=off(*A[:2], -9505, 46))
item('cluster', 'cartwheel', 1, at=5.8, spacing=0.2, topple=1.0, topple_lift=0.3, offset=off(*A[:2], -9500, 45))
item('cluster', ['bale2', 'haystack1', 'bale1'], 4, at=3.8, spacing=2.4, gap=1.2, jitter=0.05, offset=off(*A[:2], -9502, 52.5))

# ================================================================ 6. the livestock fair (the flat past the south gate)
A = (-9546, 118, 59)
scene("Livestock fair: the sale ring", A,
 "The flat beyond the south gate, where the farms bring their stock to sell: open fence rows for\n"
 "pens (solid, short, never a ring), the auctioneer's podium, feed, buckets and troughs, hay bales for\n"
 "the stock to stand on. Sheep, cows, horses and piglets are the herds; drovers and farmers the crowd.")
for (x, y, yaw) in [(-9536, 112, 0.0), (-9536, 124, 0.0), (-9556, 136, 1.5708), (-9560, 110, 1.5708)]:
    item('line', 'fence', 3, at=6.5, spacing=3.1, heading=(S if yaw == 0.0 else E), yaw=yaw, solid=True, offset=off(*A[:2], x, y))
item('cluster', 'podium', 1, at=3.2, spacing=0.2, yaw=0.8, offset=off(*A[:2], -9548, 106))
item('cluster', ['bale1', 'bale2', 'bale1', 'haystack1'], 10, at=3.6, spacing=5.0, gap=1.3, jitter=0.08, offset=off(*A[:2], -9544, 120))
item('cluster', ['bucket', 'pail', 'gbucket', 'grain', 'sack', 'shovel'], 14, at=2.4, spacing=7.0, gap=0.8, jitter=0.1, tilt=0.15, offset=off(*A[:2], -9546, 118))
item('cluster', ['churn', 'cage3', 'barrow'], 4, at=4.9, spacing=5.0, gap=2.0, jitter=0.05, offset=off(*A[:2], -9538, 104))
item('cluster', ['apple', 'corn', 'melon', 'corn'], 14, at=1.2, spacing=4.0, gap=0.3, jitter=0.1, offset=off(*A[:2], -9541, 114))
item('cluster', None, 6, species='Piglet', spacing=4.0, offset=off(*A[:2], -9536, 106))
item('cluster', None, 4, species='Chicken', spacing=2.5, offset=off(*A[:2], -9530, 118))
item('cluster', 'scarecrow', 1, at=8.3, spacing=0.2, yaw=0.5, offset=off(*A[:2], -9540, 130))

A = (-9556, 100, 59)
scene("Hay for the fair", A,
 "The hay the stock will eat, stacked at the fair's south edge: haystacks (D 8.5-10) and two loaded\n"
 "wagons (D 9.5-11), the giant end of the ladder before the runners-up.")
item('ring', ['bighay1', 'bighay2', 'dwhay'], 3, at=9.0, spacing=7.5, jitter=0.04, offset=off(*A[:2], -9558, 100))
item('cluster', ['bighay2', 'bighay1'], 2, at=10.2, spacing=4.0, gap=7.0, jitter=0.03, offset=off(*A[:2], -9565, 120))
item('line', ['haywagon', 'dwhaywagon'], 2, at=10.8, spacing=10.0, heading=S, yaw=0.3, offset=off(*A[:2], -9556, 89))
item('cluster', ['haystack2', 'haystack2', 'bale2'], 6, at=5.0, spacing=4.5, gap=1.6, jitter=0.08, offset=off(*A[:2], -9548, 94))

A = (-9530, 134, 59)
scene("The auctioneer's crates", A,
 "The packing crates and cages the auctioneer's goods came in, stacked at the fair's north corner by\n"
 "the hay cart that lost its wheel: the chunkiest food of the level (D 9-11.5).")
item('cluster', ['gcrate1', 'gcrate2', 'gcrate1'], 4, at=11.3, spacing=5.0, gap=5.5, jitter=0.03, offset=off(*A[:2], -9528, 134))
item('cluster', ['gcage2', 'chest', 'gcage'], 3, at=10.0, spacing=5.0, gap=5.0, jitter=0.03, offset=off(*A[:2], -9535, 142))
item('cluster', ['swcrate', 'wfcrate', 'orccrate', 'gcrate2'], 8, at=5.2, spacing=4.0, gap=1.6, jitter=0.06, offset=off(*A[:2], -9524, 128))
item('cluster', ['firewood1', 'firewood3', 'lumber'], 8, at=3.6, spacing=3.5, gap=1.2, jitter=0.06, offset=off(*A[:2], -9540, 140))
item('line', 'brokencart', 1, at=9.3, spacing=0.2, yaw=0.8, offset=off(*A[:2], -9533, 126))
item('cluster', ['gcrate1', 'gcrate2', 'gcage2'], 4, at=8.0, spacing=4.0, gap=4.0, jitter=0.04, offset=off(*A[:2], -9522, 138))

A = (-9538, 96, 59)
scene("Stonefield's entries", A,
 "Ma Stonefield's other pumpkins, the ones too big for the street, unloaded at the fair's gate in order of\n"
 "size (D 6.5 to 8.5) with three haystacks (D 8-8.8) behind them: the band that carries the ball from the\n"
 "livestock to the giants.")
for x, y, D in [(-9533, 92, 6.5), (-9536, 96, 7.0), (-9540, 100, 7.5), (-9544, 96, 8.0), (-9541, 90, 8.5), (-9536, 88, 7.8)]:
    item('line', ['pumpkin', 'pumpkin3'], 1, at=D, spacing=0.2, offset=off(*A[:2], x, y))
item('ring', ['bighay1', 'bighay2', 'dwhay'], 3, at=8.6, spacing=6.5, jitter=0.03, offset=off(*A[:2], -9548, 88))
item('cluster', ['watermelon', 'melon', 'fruitbowl', 'bananas'], 8, at=2.6, spacing=3.0, gap=0.6, jitter=0.08, offset=off(*A[:2], -9537, 94))

# ================================================================ 7. the smithy yard (tour branch)
A = (-9462, 122, 59)
scene("Smithy's market stall", A,
 "Argus has set the forge's wares out in the yard: the weapon heap by the grindstone, armour on the\n"
 "ground, the anvil with a hammer on it, quench buckets, firewood, the training dummies (two are the\n"
 "map's own). Bo runs his loop through it; the rabbits live in the grass behind.")
item('heap', ['sword', 'sword2', 'mace', 'axe', 'hammer', 'hammer1', 'spear'], 16, at=3.3, spacing=1.5, step=0.14, tilt=0.3, jitter=0.1, offset=off(*A[:2], -9458, 125))
item('cluster', ['breast', 'breastG', 'helm', 'helmG', 'bracer', 'shoulder', 'legs', 'shield'], 14, at=1.5, spacing=1.6, gap=0.35, jitter=0.1, tilt=0.25, offset=off(*A[:2], -9465, 123.5))
item('cluster', 'grindstone', 1, at=3.9, spacing=0.2, yaw=0.3, offset=off(*A[:2], -9455, 128))
item('cluster', 'anvil', 1, at=3.2, spacing=0.2, yaw=1.0, offset=off(*A[:2], -9467, 118))
item('cluster', 'hammer', 1, at=3.0, spacing=0.2, lift=0.29, level=True, yaw=0.2, offset=off(*A[:2], -9467, 118))
item('cluster', ['bucket', 'gbucket', 'pail'], 4, at=2.3, spacing=1.4, gap=0.8, topple=0.3, topple_lift=0.4, offset=off(*A[:2], -9469, 121))
item('heap', ['firewood3', 'firewood1'], 8, at=3.6, spacing=2.2, step=0.6, gap=1.2, tilt=0.08, jitter=0.06, offset=off(*A[:2], -9460, 131))
item('cluster', ['dummy', 'swdummy'], 2, at=6.0, spacing=3.0, gap=2.5, topple=0.0, offset=off(*A[:2], -9466, 128))
item('cluster', ['mug', 'stein1', 'loaf', 'turkey', 'apple'], 10, at=1.0, spacing=2.0, gap=0.3, jitter=0.06, offset=off(*A[:2], -9462, 119))
L("# The barrels in a row along the smithy's west wall, outside it (the wall is at y 119.5; the first version's")
L("# cluster stood half inside the building).")
item('line', ['barrel', 'keg2', 'swcrate'], 5, at=3.0, spacing=1.4, heading=S, wobble=0.15, jitter=0.05, offset=off(*A[:2], -9452.5, 122.3))
item('cluster', None, 6, species='Rabbit', spacing=4.0, offset=off(*A[:2], -9470, 128))

# ================================================================ 8. the raiders' camp (bonus: Hogger)
A = (-9568, 180, 59.5)
scene("Riverpaw raiders' camp", A,
 "Hogger's gnolls on the forest shelf 60 yd past the fair, with everything they stole from the market:\n"
 "the cooking fire ringed with the feast, Stormwind crates and kegs, the loot wagon, Hogger's table laid\n"
 "for one, the cage of hens they could not be bothered to pluck. Three tents; the dog's house. On the\n"
 "primary route (fair > camp > fair gate > stage), so the ball arrives at D 9-11 and Hogger is food.")
item('cluster', 'campfire', 1, scale=1.0, spacing=0.2, offset=off(*A[:2], -9568, 180))
item('cluster', ['haunch', 'turkey', 'boar', 'bone', 'bone', 'skull', 'stein1', 'mug2'], 16, at=2.0, spacing=4.0, gap=0.5, jitter=0.1, tilt=0.2, topple=0.2, topple_lift=0.1, offset=off(*A[:2], -9568, 180))
item('ring', ['gnolltent3', 'gnolltent2', 'gnolltent3'], 3, at=8.5, spacing=9.0, wobble=1.0, jitter=0.04, offset=off(*A[:2], -9570, 182))
item('heap', ['swcrate', 'keg1', 'gbarrel', 'swcrate', 'chest2'], 12, at=5.0, spacing=3.5, step=1.0, gap=1.6, tilt=0.08, topple=0.15, topple_lift=0.6, jitter=0.06, offset=off(*A[:2], -9561, 186))
item('cluster', ['wfwagon'], 1, at=10.5, spacing=0.2, yaw=0.9, offset=off(*A[:2], -9577, 172))
item('cluster', ['cage1', 'doghouse'], 2, at=4.8, spacing=2.5, gap=2.5, offset=off(*A[:2], -9566, 169))
item('cluster', None, 3, species='Chicken', spacing=1.5, offset=off(*A[:2], -9566, 169))
solid('orctable', -9573, 187, A, yaw=0.6, comment="Hogger's table (top 0.85)")
item('cluster', ['boar', 'goblet', 'pie'], 3, at=2.4, spacing=0.6, gap=0.4, lift=0.85, offset=off(*A[:2], -9573, 187))
item('cluster', ['innsign'], 1, at=4.8, spacing=0.2, yaw=0.2, topple=1.0, topple_lift=0.4, offset=off(*A[:2], -9564, 190))
item('cluster', ['chair', 'wchair', 'cushion', 'rug', 'poster'], 8, at=3.5, spacing=4.0, gap=1.2, jitter=0.06, tilt=0.1, topple=0.2, topple_lift=0.3, offset=off(*A[:2], -9575, 189))

A = (-9558, 150, 59.5)
scene("Gnoll outskirts", A,
 "The raiders' gate at the fair's south-west corner, in sight of the sale ring: skulls on stakes, two\n"
 "gnoll banners, the barrels they emptied on the way home, and the loot trail running on to the camp.")
item('cluster', ['skull', 'bone', 'bone', 'skull'], 8, at=1.6, spacing=2.4, gap=0.5, jitter=0.1, tilt=0.3, offset=off(*A[:2], -9558, 150))
item('cluster', ['broken1', 'broken2', 'barrel'], 6, at=5.0, spacing=4.0, gap=1.5, jitter=0.08, topple=0.4, topple_lift=0.3, offset=off(*A[:2], -9555, 146))
item('line', 'banner', 2, at=4.6, spacing=5.0, heading=W, yaw=0.0, offset=off(*A[:2], -9561, 154))
item('cluster', ['swcrate', 'keg2', 'wheel'], 5, at=3.4, spacing=2.5, gap=1.0, jitter=0.06, offset=off(*A[:2], -9560, 160))

# ================================================================ 9. the scarecrow field (tour branch, Free Roll)
A = (-9393, 60, 60)
scene("The scarecrow contest", A,
 "The fair's other contest, in the meadow east of the north road: five scarecrows in a ring, each on\n"
 "its own straw, in order of size (D 5 to 10), the champion scarecrow on a bale in the middle (D 11), the\n"
 "corn stooks they guard, the judges' lunch, the map's own barrels and crate, and the rabbits that were\n"
 "not scared. A second size ladder for Free Roll, 8 yd clear of the two big trunks.")
for (x, y, D) in [(-9387, 54, 5.0), (-9386, 66, 6.0), (-9397, 69, 7.2), (-9401, 58, 8.6), (-9395, 51, 10.0)]:
    item('line', 'scarecrow', 1, at=D, spacing=0.2, offset=off(*A[:2], x, y))
    item('cluster', ['bale2', 'haystack1'], 2, at=3.6, spacing=1.2, gap=1.0, jitter=0.05, offset=off(*A[:2], x, y))
item('line', 'bale1', 1, scale=1.6, spacing=0.2, yaw=0.4, solid=True, offset=off(*A[:2], -9393, 60))
item('line', 'dwscarecrow', 1, at=11.0, spacing=0.2, lift=0.67, yaw=1.0, offset=off(*A[:2], -9393, 60))
item('scatter', 'corn', 22, at=0.75, spacing=0.9, jitter=0.15, tilt=0.15, offset=off(*A[:2], -9393, 66.0))
item('cluster', 'corn', 12, at=1.3, spacing=2.0, gap=0.3, jitter=0.1, tilt=0.2, offset=off(*A[:2], -9387.5, 64.5))
item('cluster', ['loaf', 'wedge', 'half', 'mug', 'foam', 'pie', 'apple'], 16, at=0.75, spacing=1.8, gap=0.2, jitter=0.08, offset=off(*A[:2], -9398, 65))
item('cluster', ['stein1', 'bottle', 'jug', 'bowl'], 8, at=1.2, spacing=1.6, gap=0.35, topple=0.3, topple_lift=0.1, jitter=0.05, offset=off(*A[:2], -9400, 63))
item('cluster', ['pumpkin', 'pumpkin3', 'watermelon'], 5, at=3.4, spacing=3.0, gap=1.0, jitter=0.1, offset=off(*A[:2], -9388, 60))
item('cluster', ['bucket', 'sackflat', 'barrow', 'grain'], 6, at=2.6, spacing=3.0, gap=0.9, jitter=0.08, tilt=0.1, offset=off(*A[:2], -9398, 55))
item('cluster', ['gcrate2', 'chest'], 2, at=7.5, spacing=3.0, gap=3.0, offset=off(*A[:2], -9402, 66))


text = '\n'.join(out) + '\n'
dst = os.path.join(root, 'crates', 'benilla-app', 'src', 'azeroll', 'goldshire.toml')
# MD_CAPTURE=camp|fair|stage|heap|guardpost writes a tooling variant to /tmp/az-l2r/capture-<name>.toml that starts a
# mid-size ball beside that vignette, for screenshots of places the primary autopilot route never shows.
CAP = os.environ.get('MD_CAPTURE')
if CAP:
    starts = {'camp': ('[-9552, 164, 59.5]', 2.4, 3.9), 'hogger': ('[-9552, 164, 59.5]', 4.4, 3.9), 'hogger2': ('[-9556, 150, 59.5]', 4.4, 3.9), 'field': ('[-9412, 50, 58]', 1.2, 0.3), 'fair': ('[-9530, 112, 59]', 2.0, 3.6), 'stage': ('[-9470, 60, 56.5]', 1.0, 3.14159),
              'heap': ('[-9479, 73, 56.5]', 1.0, 2.55),
              # r 1.05 at the crossroads guard post, 0.7 yd from the Stormwind Guard (the spot the unstuck pass found clear at
              # that radius): the ram-fairness fixture.
              'guardpost': ('[-9456.5, 75.6, 58.0]', 1.05, 3.14159),
              # D33 replay fixtures (MD_R sets the radius): the coop's gap from the beer garden, the apple-bob queue.
              'coop': ('[-9441.5, 57.0, 56.5]', float(os.environ.get('MD_R', '0.45')), 1.5708),
              'bob': ('[-9467.5, 79.0, 56.7]', float(os.environ.get('MD_R', '0.8')), 1.5708)}
    if CAP == 'pumpkins':
        # Audit scene: candidate pumpkin models, each at four yaws, scale 1, in rows just south of the start.
        text += '\n[[scenes]]\nname = "Pumpkin audit"\nanchor = [-9446, 57, 56.5]\n\n'
        audit = os.environ.get('MD_AUDIT', r'World\Goober\G_Pumpkin_01.m2,World\Goober\G_Pumpkin_02.m2,World\Goober\G_Pumpkin_03.m2').split(',')
        for row, path in enumerate(audit):
            for col in range(4):
                text += ("[[scenes.items]]\nmodel = '%s'\narrangement = \"line\"\ncount = 1\nspacing = 0.2\n"
                         "yaw = %s\noffset = [%s, %s]\nscale = %s\n\n" % (path, round(col * PI / 2, 3), -7 + 4.6 * col, -8 - 5.0 * row, (0.9, 1.2, 1.6, 2.4)[col]))
        text = text.replace('start_radius = 0.32', 'start_radius = 1.0', 1).replace('start_heading = 3.14159', 'start_heading = 0.0', 1)
    elif CAP == 'campvisit':
        # The normal start; the autopilot's route ends at the camp instead of the stage, so a full run walks
        # garden > market > gate > fair > camp (and rolls Hogger there) before the pumpkin calls it home.
        text = text.replace('area = "Judging stage"', 'area = "Raiders\' camp"', 1)
    else:
        at, r, h = starts[CAP]
        text = text.replace('start = [-9446, 57, 56.5]', 'start = ' + at, 1).replace('start_radius = 0.32', 'start_radius = %s' % r, 1)
        text = text.replace('start_heading = 3.14159', 'start_heading = %s' % h, 1)
    dst = '/tmp/az-l2r/capture-%s.toml' % CAP
    if os.environ.get('MD_R'): dst = '/tmp/az-l2r/capture-%s-r%s.toml' % (CAP, os.environ['MD_R'])
import tomllib
try:
    parsed = tomllib.loads(text)
    for sc_ in parsed['scenes']:
        assert sc_.get('name') and sc_.get('anchor') and sc_.get('items'), 'incomplete scene %r' % sc_
        for it in sc_['items']:
            assert ('model' in it) ^ ('models' in it) ^ ('species' in it), 'item needs one of model/models/species: %r' % it
            assert 'count' in it and 'arrangement' in it, it
except Exception as e:
    sys.exit('generated TOML is not a valid recipe: %s' % e)
open(dst, 'w').write(text)
print('wrote', dst, 'lines', len(out), 'items in ledger', len(LEDGER))

if '--ledger' in sys.argv:
    # Cumulative authored growth (r^3) against the ball volume at each diameter. Claimed map doodads
    # (bushes, fences, lamps) and the herds are extra on top of this.
    LEDGER.sort()
    print('%6s %9s %9s %6s' % ('D', 'cum r3', 'need r3', 'ratio'))
    for D in [0.7, 0.8, 1.0, 1.3, 1.6, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 7.0, 8.6, 10.0, 11.0, 12.03]:
        cum = sum(v for d, v, *_ in LEDGER if d <= D)
        need = (D / 2) ** 3
        print('%6.2f %9.3f %9.3f %6.2f' % (D, cum, need, cum / need))
    if '--scenes' in sys.argv:
        from collections import defaultdict
        by = defaultdict(lambda: [0.0, 0])
        for d, v, n, s, m in LEDGER:
            by[s][0] += v; by[s][1] += n
        for s, (v, n) in by.items():
            print('%-36s %8.2f r3 %5d props' % (s, v, n))
