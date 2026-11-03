#!/usr/bin/env python3
"""Generates crates/benilla-app/src/azeroll/lions-pride.toml from a room-by-room plan.
Run: python3 docs/azeroll/data/lions-pride-gen.py (from anywhere). Design notes: docs/azeroll/katamari-room-design.md §5.
Positions are WoW (x north, y west, z up). Item offsets are Bevy (east, south) = (-dy, -dx).
Headings: 0 east, pi/2 south, pi west, 3pi/2 north.
Sizing: a prop rolls up once the ball's DIAMETER >= 2.15 x its longest scaled extent
(level.rs: needed radius = extent / COLLECT_RATIO). `at=D` below scales a model so it opens at D."""
import math, os
CAP=os.environ.get('L1_CAPTURE')
SKIP={'cellar':["Crumbs under the captain's table","The empties by the bench","The hearth has gone out","Farley's burst purse under the bar","Farley wants his floor back","The card game that ended badly","The delivery nobody unpacked"],
 'upstairs':["Crumbs under the captain's table","The empties by the bench","The hearth has gone out","Farley's burst purse under the bar","Farley wants his floor back","The card game that ended badly","The delivery nobody unpacked","The cook fled"],
 'hall':["Crumbs under the captain's table","The empties by the bench","Farley wants his floor back"]}
GOALAREA={'cellar':'Cellar','upstairs':'Upstairs','hall':'North hall'}
PI=math.pi; N=3*PI/2; E=0.0; S=PI/2; W=PI
def off(ax, ay, x, y): return [round(-(y-ay),2), round(-(x-ax),2)]
def head(dx, dy): return round(math.atan2(-dx, -dy), 3)
# model key -> (path, longest header extent in yards at scale 1)
M = {
 'muffin':   (r'World\Generic\Human\Passive Doodads\Food\Muffin01.m2', 0.04),
 'wedge':    (r'World\Generic\Human\Passive Doodads\Food\CheeseWedge01.m2', 0.07),
 'wedgeS':   (r'World\Generic\Human\Passive Doodads\Food\CheeseWedgeSwiss01.m2', 0.07),
 'wheel':    (r'World\Generic\Human\Passive Doodads\Food\CheeseWheel01.m2', 0.09),
 'wheelS':   (r'World\Generic\Human\Passive Doodads\Food\CheeseWheelSwiss01.m2', 0.09),
 'slice':    (r'World\Generic\Human\Passive Doodads\Food\BreadSlice.m2', 0.10),
 'half':     (r'World\Generic\Human\Passive Doodads\Food\BreadLoafHalf.m2', 0.10),
 'pie':      (r'World\Generic\Human\Passive Doodads\Food\Pie01.m2', 0.16),
 'baguette': (r'World\Generic\Human\Passive Doodads\Food\BreadFrench01.m2', 0.37),
 'bread':    (r'World\Generic\Human\Passive Doodads\Food\bread01.m2', 1.40),
 'mug':      (r'World\Generic\Human\Passive Doodads\Mugs\Mug01.m2', 0.33),
 'foam':     (r'World\Generic\Human\Passive Doodads\Mugs\MugFoam01.m2', 0.33),
 'mug2':     (r'World\Generic\Human\Passive Doodads\Mugs\Mug02.m2', 0.53),
 'stein':    (r'World\Azeroth\Westfall\PassiveDoodads\Utensils\Stein.m2', 0.32),
 'stein1':   (r'World\Generic\Human\Passive Doodads\Mugs\Stein01.m2', 0.59),
 'goblet':   (r'World\Generic\Human\Passive Doodads\Goblets\Goblet02.m2', 0.53),
 'bottle':   (r'World\Generic\Human\Passive Doodads\Bottles\Bottle01.m2', 0.71),
 'green':    (r'World\Generic\Human\Passive Doodads\Bottles\GreenBottle01.m2', 0.54),
 'jug':      (r'World\Azeroth\Elwynn\PassiveDoodads\jugs\jug01.m2', 0.69),
 'jug2':     (r'World\Azeroth\Elwynn\PassiveDoodads\jugs\jug02.m2', 0.65),
 'plate':    (r'World\Azeroth\Westfall\PassiveDoodads\Utensils\Plate.m2', 0.50),
 'bowl':     (r'World\Azeroth\Westfall\PassiveDoodads\Utensils\Bowl.m2', 0.50),
 'dirty':    (r'World\Generic\Human\Passive Doodads\Plates\GeneralDirtyPlate01.m2', 0.79),
 'coins':    (r'World\Generic\PassiveDoodads\TreasurePiles\GoldPilesmall01.m2', 1.94),
 'apple':    (r'World\Generic\PassiveDoodads\Fruits\Fruit_Apple.m2', 0.48),
 'orange':   (r'World\Generic\PassiveDoodads\Fruits\Fruit_Orange.m2', 0.48),
 'melon':    (r'World\Generic\PassiveDoodads\Fruits\Fruit_Melon.m2', 1.01),
 'pumpkin':  (r'World\Azeroth\Duskwood\PassiveDoodads\Bush\Pumpkin01.m2', 3.44),   # the vine pumpkin (the only uncarved one; box 1.69 x 3.44 x 1.96 includes its vine)
 'book':     (r'World\Generic\Human\Passive Doodads\Books\BookSmallOpen02.m2', 0.62),
 'bookmed':  (r'World\Generic\Human\Passive Doodads\Books\BookMedium01.m2', 0.60),
 'booklarge':(r'World\Generic\Human\Passive Doodads\Books\BookLarge02.m2', 1.05),
 'scroll':   (r'World\Generic\Human\Passive Doodads\Scrolls\ScrollB01.m2', 0.90),
 'candle':   (r'World\Generic\PassiveDoodads\Lights\CandleOff01.m2', 0.56),
 'fishskel': (r'World\Generic\PassiveDoodads\Fish\fishskel01.m2', 0.57),
 'bone':     (r'World\Generic\PassiveDoodads\Furniture\Refuse\Bone02.m2', 0.51),
 'skull':    (r'World\Azeroth\Elwynn\PassiveDoodads\BattleGladeSkullHuman1\BattleGladeSkullHuman1.m2', 0.28),
 'pail':     (r'World\Generic\Human\Passive Doodads\Buckets\CaveKoboldBucket.m2', 0.74),
 'bucket':   (r'World\Generic\Human\Passive Doodads\Buckets\bucket.m2', 1.07),
 'kettle':   (r'World\Generic\Human\Passive Doodads\Cups\MetalCup03.m2', 1.11),
 'sack':     (r'World\Generic\PassiveDoodads\Furniture\Containers\Sack02.m2', 0.73),
 'sackflat': (r'World\Generic\PassiveDoodads\Furniture\Containers\Sack01.m2', 0.97),
 'crate':    (r'World\Generic\Human\Passive Doodads\Crates\StormwindCrate01.m2', 1.32),
 'chest':    (r'World\Generic\ActiveDoodads\Chest02\Chest02.m2', 0.97),
 'rope':     (r'World\Generic\Human\Passive Doodads\Ropes\CaveKoboldRopeCoil.m2', 1.26),
 'cauldron': (r'World\Generic\Human\Passive Doodads\Cauldrons\Cauldron.m2', 2.02),
 'cleaver':  (r'World\Generic\Human\Passive Doodads\Food&Utensils\Cleaver01.m2', 0.96),
 'knife':    (r'World\Generic\Human\Passive Doodads\Food&Utensils\KitchenKnife01.m2', 1.14),
 'turkey':   (r'World\Generic\Human\Passive Doodads\Food&Utensils\TurkeyLeg.m2', 0.85),
 'haunch':   (r'World\Generic\Human\Passive Doodads\Food&Utensils\Haunch.m2', 1.19),
 'meat':     (r'World\Generic\Human\Passive Doodads\Meat\BloodyMeat02.m2', 1.59),
 'platter':  (r'World\Generic\Human\Passive Doodads\Food\FishPlatter01.m2', 1.95),
 'boots':    (r'World\Generic\Human\Passive Doodads\Clothing\BootsLeatherBrown01.m2', 0.65),
 'bootsdark':(r'World\Generic\Human\Passive Doodads\Clothing\BootsLeatherDark.m2', 0.51),
 'shirt':    (r'World\Generic\Human\Passive Doodads\Clothing\FoldedShirtBrown.m2', 0.52),
 'pants':    (r'World\Generic\Human\Passive Doodads\Clothing\FoldedPantsGrey.m2', 0.51),
 'cushion':  (r'World\Azeroth\Westfall\PassiveDoodads\HaremPillow01\HaremPillow01.m2', 1.10),
 'breast':   (r'World\Generic\Human\Passive Doodads\Armor\ArmorBreastplateBlue.m2', 0.61),
 'legs':     (r'World\Generic\Human\Passive Doodads\Armor\ArmorLegPlateBlue.m2', 0.36),
 'helm':     (r'World\Generic\Human\Passive Doodads\Armor\ArmorHelmGold.m2', 0.54),
 'sword':    (r'World\Generic\Human\Passive Doodads\Weapons&Armor\HumanSword01.m2', 1.50),
 'shield':   (r'World\Generic\Human\Passive Doodads\Weapons&Armor\WallShield03.m2', 1.05),
 'tinytable':(r'World\Generic\Human\Passive Doodads\Tables\InnTableTiny.m2', 1.77),
 'smalltable':(r'World\Generic\Human\Passive Doodads\Tables\InnTableSmall.m2', 2.16),
 'chair':    (r'World\Generic\Human\Passive Doodads\Chairs\GeneralChairLoEnd01.m2', 1.33),
 'wchair':   (r'World\Azeroth\Westfall\PassiveDoodads\WestfallChair\WestfallChair.m2', 0.66),
 'keg':      (r'World\Goober\G_Keg.m2', 2.90),
 'keg2':     (r'World\Generic\Human\Passive Doodads\BeerKegs\BeerKeg02.m2', 1.35),
 'barrel':   (r'World\Generic\PassiveDoodads\Barrel\Barrel01.m2', 0.99),
 'gbarrel':  (r'World\Goober\G_Barrel.m2', 1.33),
 'plank':    (r'World\Azeroth\RedRidge\PassiveDoodads\DockPieces\RedRidgoDocksPlank01.m2'.replace('Ridgo','Ridge'), 21.64),
 'poster':   (r'World\Generic\Passive Doodads\WantedPosters\NewWantedPoster04.m2', 1.45),
 'gift':     (r'World\Generic\PassiveDoodads\Christmas\XmasGift02.m2', 0.88),
}
RATIO=2.15
def sc(models, D):
    """Scale so that the LARGEST of `models` opens at ball diameter D."""
    if isinstance(models,str): models=[models]
    e=max(M[m][1] for m in models); return round(D/(RATIO*e),3)
out=[]
def L(s=''): out.append(s)
def q(p): return "'"+p+"'"
SKIPPING=False
def item(arr, models, count, at=None, **kw):
    if SKIPPING: return
    L('[[scenes.items]]')
    if isinstance(models,str): L('model = %s'%q(M[models][0]))
    else: L('models = [%s]'%', '.join(q(M[m][0]) for m in models)); L('pick = "cycle"')
    L('arrangement = "%s"'%arr); L('count = %d'%count)
    if at is not None:
        kw['scale']=sc(models,at); L('# opens at D %.2f'%at)
    for k,v in kw.items():
        if isinstance(v,bool): L('%s = %s'%(k,'true' if v else 'false'))
        elif isinstance(v,list): L('%s = [%s]'%(k,', '.join(str(x) for x in v)))
        elif isinstance(v,str): L('%s = "%s"'%(k,v))
        else: L('%s = %s'%(k,v))
    L()
def scene(name, anchor, comment=None):
    global SKIPPING
    SKIPPING = bool(CAP) and name in SKIP.get(CAP,[])
    if SKIPPING: return
    L()
    if comment:
        for c in comment.split('\n'): L('# '+c)
    L('[[scenes]]'); L('name = "%s"'%name); L('anchor = [%s]'%', '.join(str(a) for a in anchor)); L('indoors = true'); L()

L('''# Distances are yards, positions are WoW north/west/up. GENERATED by docs/azeroll/data/lions-pride-gen.py
# from the room plan in docs/azeroll/katamari-room-design.md §5; edit the generator, not this file.
# Floors: sunken common-room boards 56.339; the raised rim, bar, north hall and kitchen 56.95;
# the cellar 49.80; upstairs 63.82. The sunken room is a 0.62 yd pit: its three step-rims
# (0.31 risers) and the main staircase are the only ways out for a ball under r 0.9.
# Sizing rule: a prop rolls up once the ball's diameter reaches 2.15 x its longest scaled extent;
# "opens at D" comments give that diameter. Claimed inn chairs (1.33) and kegs (1.35-1.48) therefore
# stay furniture; barrels (0.99), bottles, bowls, steins, bookstacks, sacks and tiny tables are food.
start = [-9467.4, 26.0, 56.339]
start_radius = 0.2
start_indoors = true
start_heading = 0
time_limit = 300
bounds = { centre = [-9465, 20, 56.339], radius = 42 }
claim = [{ centre = [-9465, 25, 57], radius = 70 }]
# Herd actors stand on the inn's real floors (the cellar rats, the hearth cat).
wmo_floors = true

[level]
id = "lions-pride"
title = "Last Call at the Lion's Pride"
subtitle = "The regiment's send-off. Nobody cleaned up. The door is... occupied."
briefing = "Start with the crumbs under the captain's table. Roll up the supper, climb the books onto the tables and the bar, follow the cheese to the cellar rats, and come back for the barrels. The giant keg in the doorway is not going anywhere."
objective = "Grow to 2 yd 14 in across"
music = "goldshire-mambo"
cast = "lions-pride"
par = 130
pace = [0.75, 0.55, 0.4]

[goal]
kind = "size"
diameter = 2.4
area = "Kitchen"
title = "LAST CALL!"
fail = "The lamps went out with the floor still crunchy."

# D33: the level's hidden parcel (scene "The parcel in the fireplace"): the Prince's monocle.
[present]
id = "monocle"
title = "the Monocle"
display = 5377

[[tiers]]
name = "Crumbs under the captain's table"
at = 0
[[tiers]]
name = "Pies, mugs and apples"
at = 0.55
[[tiers]]
name = "Bowls, bottles and the burst purse"
at = 0.8
[[tiers]]
name = "Boots, books and cheese wheels"
at = 1.1
[[tiers]]
name = "Sacks, stacks and the washing up"
at = 1.5
[[tiers]]
name = "Barrels, buckets and crates"
at = 1.9
[[tiers]]
name = "Farley's stools"
at = 2.25

# D33: four Katamari tasks, each decided from the pickup log. `speed` is the comet's job now, and
# `hearth` is gone because the parcel lives in the fireplace.
[[challenges]]
id = "rats"
title = "Pied piper: every cellar rat before the first cheese wheel"
kind = "before"
species = "Cellar Rat"
before_name = "Cheese Wheel"
[[challenges]]
id = "pillows"
title = "Pillow fight: every pillow in the upstairs lounge"
kind = "scene"
scene = "Pillow fight in the lounge"
radius = 6
[[challenges]]
id = "floor"
title = "Farley's floor: every stool and footstool"
kind = "scene"
scene = "Farley wants his floor back"
radius = 9
[[challenges]]
id = "big"
title = "Preposterous: three yards across"
kind = "size"
diameter = 3.0

# No invisible walls inside the inn (playtest 2026-11-03). The one exit is the inner vestibule
# door, and it is blocked by a visible, permanent, inedible giant keg (scene "The door").
# `bounds` above is only the emergency backstop.

[[areas]]
name = "Common room"
centre = [-9468, 24, 56.339]
radius = 11
[[areas]]
name = "Bar"
centre = [-9462, 13.5, 56.95]
radius = 6
[[areas]]
name = "North hall"
centre = [-9457, 32, 56.95]
radius = 6
[[areas]]
name = "Kitchen"
centre = [-9465, -1, 56.95]
radius = 8
enclosed = true
clearance = 2.63
doors = [[-9468.5, 7.76, 56.96]]
[[areas]]
name = "Cellar"
centre = [-9469, -2, 49.8]
radius = 12
enclosed = true
clearance = 3.4
doors = [[-9461.2, 2.6, 56.95]]
[[areas]]
name = "Upstairs"
centre = [-9463, 20, 63.82]
radius = 20
enclosed = true
clearance = 2.67
doors = [[-9472, 20.8, 63.82]]

# Route graph for the autopilot: real walking connections, no props of their own.
[[trails]]
name = "step to the bar"
navigation_only = true
from = [-9468, 24, 56.339]
to = [-9462, 13.5, 56.95]
spacing = 2
[[trails]]
name = "bar to kitchen"
navigation_only = true
from = [-9462, 13.5, 56.95]
to = [-9465, -1, 56.95]
spacing = 2
[[trails]]
name = "cellar stairs"
navigation_only = true
from = [-9465, -1, 56.95]
to = [-9469, -2, 49.8]
spacing = 2
[[trails]]
name = "to the north hall"
navigation_only = true
from = [-9462, 13.5, 56.95]
to = [-9457, 32, 56.95]
spacing = 2
[[trails]]
name = "main staircase"
navigation_only = true
from = [-9468, 24, 56.339]
to = [-9463, 20, 63.82]
spacing = 2

# Moving food: the rats live among the cellar cheese; the cat sleeps in the dead hearth.
[[herds]]
species = "Cellar Rat"
at = [-9467, -6, 49.8]
radius = 4
count = 12
fit = 0.4
behaviour = "skittish"
model = 'Creature\\Rat\\Rat.m2'

[[herds]]
species = "Hearth Cat"
at = [-9457.6, 23.2, 56.97]
radius = 0.4
count = 1
fit = 0.45
behaviour = "passive"
model = 'Creature\\Cat\\Cat.m2'
''')

# ---------------------------------------------------------------- S1 nursery (start D 0.40)
A=(-9466.8, 25.6, 56.339)
scene("Crumbs under the captain's table", A,
 "START, 2 yd south-west of the captain's double table (the ball must not start on the book stairs).\n"
 "Everything within 3 yd opens between D 0.40 and 0.75; the two supper spills point at the west and\n"
 "east step-rims; the mug ring is the 'soon'; a book staircase climbs onto the table where the meal sits.")
item('cluster','muffin',14,at=0.38,spacing=1.2,gap=0.16,jitter=0.06)
item('cluster',['wedge','wedgeS'],10,at=0.42,spacing=1.4,gap=0.18,jitter=0.06,offset=[0.4,0.5])
item('cluster',['half','wheel','half','slice'],18,at=0.46,spacing=1.7,gap=0.2,jitter=0.06,offset=[-0.3,-0.4])
item('cluster','muffin',8,at=0.52,spacing=1.6,gap=0.25,jitter=0.06,offset=[0.9,-0.6])
item('spill',['half','wheel','half'],14,at=0.52,spacing=0.42,heading=head(-5,+3),bend=0.05,offset=off(A[0],A[1],-9465.6,25.2),jitter=0.06)
item('spill',['half','wheel','half'],8,at=0.52,spacing=0.42,heading=head(-4,-5),bend=0.05,offset=off(A[0],A[1],-9465.6,23.8),jitter=0.06)
item('spill','pie',8,at=0.58,spacing=0.55,heading=head(-5,+3),bend=0.05,offset=off(A[0],A[1],-9466.4,25.8),jitter=0.06)
L("# The pit is y 19.5..28.5 here (its step-rims): the rings stay inside it.")
item('ring',['mug','foam','mug'],14,at=0.62,spacing=1.8,wobble=0.2,jitter=0.05)
item('ring',['apple','orange'],10,at=0.66,spacing=2.2,wobble=0.2,jitter=0.05)
item('ring',['stein','goblet','bowl','plate'],12,at=0.80,spacing=2.6,wobble=0.2,jitter=0.05)
item('cluster','coins',10,at=0.48,spacing=1.0,gap=0.25,offset=off(A[0],A[1],-9462.6,20.9))
item('cluster','goblet',4,at=0.72,spacing=0.8,gap=0.3,topple=1.0,topple_lift=0.1,offset=off(A[0],A[1],-9464.6,20.8))
item('cluster','wheel',8,at=0.9,spacing=1.6,gap=0.4,offset=off(A[0],A[1],-9468.5,21.8))
L("# Books up onto the captain's table (top 57.09, its south edge at x -9463.4 on the y 24.4 line between the")
L("# chairs): six open books, 0.12 yd a step, the last one's far edge 0.1 yd over the table (placement survey).")
L("# 0.13 a step so the last top (57.12) stands over the table's 57.09: a last step even 0.02 under its surface")
L("# stalls the live ball at the table's bevelled edge (scripted climb 2026-11-03).")
item('stairs','book',6,spacing=0.3,step=0.13,heading=N,yaw=0.0,offset=off(A[0],A[1],-9465.05,24.4))
L("# The meal on the table (a claimed D22 table, its slab top 57.09: lift 0.76 puts it on the slab), in two rows")
L("# down the strip of top no chair back reaches (x -9462.2..-9462.6, y 23.8..25.2; a lamp stands at y 25.5).")
item('line',['bowl','plate','stein','goblet'],7,at=0.95,spacing=0.22,heading=W,wobble=0.03,lift=0.76,offset=off(A[0],A[1],-9462.25,24.5),jitter=0.05)
item('line',['half','wheel','pie','muffin'],7,at=0.6,spacing=0.22,heading=W,wobble=0.03,lift=0.76,offset=off(A[0],A[1],-9462.55,24.5),jitter=0.05)
item('line','candle',2,at=1.0,spacing=1.2,heading=W,lift=0.76,offset=off(A[0],A[1],-9462.4,24.5))

# ---------------------------------------------------------------- empties (pit south end)
A=(-9476.5, 25.0, 56.339)
scene("The empties by the bench", A,
 "South end of the pit, under the stairs' first flight: the crates and barrels of empties the regiment\n"
 "worked through, jugs and bottles underfoot, split sacks. The pit's own 1.0 to 2.1 yd band.")
L("# The pit's south end is x -9469..-9478, y 22.5..30; the stair's lower flight starts at y 22 and the window")
L("# seats at x -9478.3, so the groups stay inside that box (placement survey 2026-11-03).")
item('cluster',['bottle','jug','bottle','jug2'],16,at=1.25,spacing=1.6,gap=0.4,topple=0.4,topple_lift=0.25,jitter=0.05,offset=off(A[0],A[1],-9475.5,25.5))
item('cluster',['green','goblet','mug2'],12,at=1.0,spacing=1.5,gap=0.35,topple=0.3,topple_lift=0.25,jitter=0.05,offset=off(A[0],A[1],-9474.0,24.3))
item('cluster',['sack','sack','pail'],8,at=1.55,spacing=1.5,gap=0.6,topple=0.3,topple_lift=0.2,offset=off(A[0],A[1],-9475.0,28.0))
item('cluster',['crate','barrel','crate','bucket'],8,at=2.1,spacing=1.5,gap=0.9,offset=off(A[0],A[1],-9476.3,26.4),jitter=0.04)
item('cluster',['boots','bootsdark','shirt'],5,at=1.3,spacing=1.0,gap=0.5,offset=off(A[0],A[1],-9473.5,23.4))

# ---------------------------------------------------------------- S2 hearth
A=(-9459.6, 22.3, 56.339)
scene("The hearth has gone out", A,
 "The fireplace in the north wall. Its firebox floor is 0.66 yd above the boards: a book staircase\n"
 "leads up to the cold ashes, the cat, its fish bones, the kettle and the ash pail.")
L("# The firebox floor (56.96) starts at x -9458.9: six books, the last one's top 57.07 at its lip (a seventh")
L("# would stand on the firebox floor itself, 0.74 above the sixth).")
L("# On the y 23.5 line: at y 23.1 a claimed chair (y <= 23.0) pushed the ball off the first book (scripted climb).")
item('stairs','book',6,spacing=0.32,step=0.12,heading=N,yaw=0.0,offset=off(A[0],A[1],-9460.9,23.5))
item('cluster',['fishskel','bone','bone','fishskel'],8,at=0.9,spacing=0.6,gap=0.3,on=56.96,offset=off(A[0],A[1],-9457.2,23.2),jitter=0.1)
item('cluster','muffin',8,at=0.55,spacing=0.6,gap=0.2,on=56.96,offset=off(A[0],A[1],-9457.4,22.4),jitter=0.06)
item('cluster','pail',1,at=1.3,spacing=0.3,on=56.96,offset=off(A[0],A[1],-9457.3,21.4))
item('cluster','kettle',1,at=1.6,spacing=0.3,on=56.96,offset=off(A[0],A[1],-9457.3,23.8))
L("# The regimental trophies on the hearthstone (top 57.5, x -9458.25..-9459.25, y 18.5..20.75), up four books from")
L("# the firebox floor. (The first version aimed nine books at the 0.75 yd rail along y 18 and the panel-fronted")
L("# counter east of the chimney; a ball can reach neither top: placement survey 2026-11-03.)")
item('stairs','book',4,spacing=0.3,step=0.15,heading=E,yaw=0.0,climb=0.2,on=56.96,offset=off(A[0],A[1],-9458.7,22.0))
item('line',['goblet','green','goblet','skull','goblet'],5,at=1.1,spacing=0.4,heading=E,wobble=0.05,on=57.5,offset=off(A[0],A[1],-9458.45,19.6),jitter=0.05)
item('line',['green','goblet','skull','goblet','green'],5,at=1.1,spacing=0.4,heading=E,wobble=0.05,on=57.5,offset=off(A[0],A[1],-9459.0,19.6),jitter=0.05)

# ---------------------------------------------------------------- the parcel (D33)
A=(-9457.5, 23.1, 57.0)
scene("The parcel in the fireplace", A,
 "Fizzwick's lost post, 1 yd into the cold firebox on the book stair's axis, y 23.1 (floor 57.0, 0.66 above the boards). Small: up the\n"
 "book stair of 'The hearth has gone out' while the books are steps. Big: from D 2.0 the ball steps\n"
 "the lip. Between D 0.67 and 2.0 it is out of reach on purpose (come back for it). Opens at D 0.45.\n"
 "Never a bot target, never solid, never the goal. replay-design.md 3.1.")
item('line','gift',1,at=0.45,spacing=1.0,present=True,name="Fizzwick's Parcel",yaw=0.0)

# ---------------------------------------------------------------- S3 bar
A=(-9462.5, 15.5, 56.95)
scene("Farley's burst purse under the bar", A,
 "The raised rim east of the pit. Coins run from the east step to the bar foot, the purse lies\n"
 "split, steins line the counter, and nine books climb to the last round on the bar top (1.15 yd).")
L("# Coins from the east step toward the bar foot, starting below the 0.75 yd rail along y 18..18.75.")
item('spill','coins',14,at=0.55,spacing=0.38,heading=head(+4.5,-5),bend=0.05,offset=off(A[0],A[1],-9466.0,17.4))
item('cluster','sack',1,at=1.4,spacing=0.3,topple=1.0,topple_lift=0.15,offset=off(A[0],A[1],-9464.0,15.4))
item('cluster','coins',8,at=0.7,spacing=0.7,gap=0.3,offset=off(A[0],A[1],-9464.0,15.4))
item('cluster',['wedge','half','wheel','muffin'],14,at=0.55,spacing=1.2,gap=0.22,offset=off(A[0],A[1],-9464.6,14.4),jitter=0.06)
L("# Steins: a row along the bar top's north end (clear of the claimed keg at x -9457.3..-9458.5) and one at the")
L("# foot of the counter south of the book stair.")
item('line','stein',5,at=0.85,spacing=0.5,heading=S,wobble=0.05,on=58.13,offset=off(A[0],A[1],-9461.0,10.9))
item('line','stein',5,at=0.85,spacing=0.55,heading=S,wobble=0.06,offset=off(A[0],A[1],-9464.4,13.15))
L("# Bowls in a row along y 16.9: north of the rug and stools (x -9458.75..-9461.5, y 13.25..16.5), south of the rail.")
item('line',['bowl','plate','bowl'],14,at=1.0,spacing=0.38,heading=S,wobble=0.08,offset=off(A[0],A[1],-9463.7,16.9),jitter=0.05)
item('cluster',['bottle','jug','bottle','green'],14,at=1.35,spacing=0.75,gap=0.4,topple=0.4,topple_lift=0.15,offset=off(A[0],A[1],-9465.2,14.3),jitter=0.05)
L("# Nine books up to the bar top (58.13; its front edge is y 12.4 on this line, surveyed): the last one's")
L("# origin stays over the floor (props snap to the highest surface under them) and its edge 0.15 over the top.")
item('stairs','book',9,spacing=0.3,step=0.135,heading=E,yaw=0.0,offset=off(A[0],A[1],-9462.6,15.0))
L("# On the bar top (y 10.6..12.2, x -9458.8..-9465.7): a row of the last round down its length.")
item('line',['foam','mug2','foam','jug2','foam','mug2'],12,at=1.1,spacing=0.5,heading=S,wobble=0.15,on=58.13,offset=off(A[0],A[1],-9462.6,11.65),jitter=0.05)
item('line','wheel',2,at=1.4,spacing=0.8,heading=S,on=58.13,offset=off(A[0],A[1],-9464.9,11.2))

# ---------------------------------------------------------------- north hall
A=(-9456.0, 32.0, 56.95)
scene("The delivery nobody unpacked", A,
 "The entrance hall. Crates and sacks beside the keg pyramid, apples rolling toward the hearth counter.")
item('cluster',['crate','sack','sack','chest','sack'],9,at=2.0,spacing=1.0,gap=0.6,offset=off(A[0],A[1],-9457.0,32.2),jitter=0.04)
item('spill',['apple','apple','orange'],12,at=1.0,spacing=0.35,heading=head(-4,-3.5),bend=0.05,offset=off(A[0],A[1],-9455.6,34.2))
item('cluster',['jug','bottle','mug2'],8,at=1.4,spacing=0.9,gap=0.4,topple=0.3,topple_lift=0.15,offset=off(A[0],A[1],-9459.0,31.0))
item('cluster',['wedge','wheel','half','slice'],12,at=0.55,spacing=0.9,gap=0.22,offset=off(A[0],A[1],-9457.0,30.8),jitter=0.06)

# ---------------------------------------------------------------- west nook
A=(-9473.5, 35.5, 56.95)
scene("The card game that ended badly", A,
 "The west nook off the rim: two claimed tables and five chairs. Winnings on the floor, a skull as the stake.")
item('cluster','coins',10,at=0.6,spacing=1.4,gap=0.3,offset=off(A[0],A[1],-9473.2,35.5))
item('cluster',['goblet','mug2','green','stein1'],14,at=1.1,spacing=2.0,gap=0.36,jitter=0.05)
item('cluster','skull',1,at=0.9,spacing=0.3,offset=off(A[0],A[1],-9472.0,33.5))
item('cluster',['wedge','muffin','half'],10,at=0.55,spacing=1.4,gap=0.22,offset=off(A[0],A[1],-9471.5,36.5),jitter=0.06)
item('cluster',['boots','shirt','pants'],6,at=1.4,spacing=1.6,gap=0.5,offset=off(A[0],A[1],-9475.5,38.5))

# ---------------------------------------------------------------- S4 kitchen
A=(-9465.0, 0.0, 56.95)
scene("The cook fled", A,
 "Through the 2.6 yd door. The cheese trail runs from the door past the prep counter and down the cellar\n"
 "stairs; the washing-up is heaped, the larder crate tipped, a plank leans up the counter, the cauldron\n"
 "sits pulled off the stove, and the cook's prize pumpkin waits in the corner for a 2.3 yd ball.")
item('spill',['wheel','wedge','wheel'],18,at=0.6,spacing=0.42,heading=head(+6,-4),bend=0.04,offset=off(A[0],A[1],-9467.6,6.6))
L("# The trail ends in a pile at the head of the cellar stair (props on its treads cut into the risers, and the")
L("# spawn ray cannot reach the cellar); the cellar scene picks the cheese up again below.")
item('cluster',['wheel','wedge','wheelS'],9,at=0.8,spacing=0.5,gap=0.25,offset=off(A[0],A[1],-9464.5,3.6))
item('cluster',['wedge','muffin','slice','wheel'],14,at=0.5,spacing=1.6,gap=0.22,offset=off(A[0],A[1],-9466.5,0.5),jitter=0.06)
item('heap','dirty',10,at=1.5,spacing=1.3,step=0.06,gap=0.3,offset=off(A[0],A[1],-9466.8,-3.6))
item('cluster',['bowl','plate','bowl'],12,at=1.0,spacing=1.6,gap=0.32,offset=off(A[0],A[1],-9467.5,2.5),jitter=0.05)
item('spill',['apple','orange','apple'],12,at=1.0,spacing=0.5,heading=S,bend=0.05,offset=off(A[0],A[1],-9460.5,5.5),jitter=0.05)
item('spill',['melon','bread'],6,at=1.8,spacing=0.9,heading=S,bend=0.05,offset=off(A[0],A[1],-9466.5,5.8),jitter=0.05)
item('cluster',['bucket','pail','bucket'],6,at=2.0,spacing=0.8,gap=0.5,topple=0.3,topple_lift=0.3,offset=off(A[0],A[1],-9468.8,3.6))
L("# A dock plank (inedible in this level) leans from the floor up the SOUTH side of the prep counter (top 58.3,")
L("# 1.35 yd; its south edge x -9465.3 for y -0.5..-2.75). The first version leaned it east over a second counter")
L("# with its foot over the hole to the cellar. The plank's box centre sits 0.88 yd south of its origin at this scale.")
item('line','plank',1,scale=0.22,pitch=0.29,yaw=0.0,lift=0.59,solid=True,climb=0.2,offset=off(A[0],A[1],-9467.8,-1.6))
item('cluster','sackflat',6,at=1.7,spacing=0.9,gap=0.5,tilt=0.2,offset=off(A[0],A[1],-9468.0,-2.8))
L("# On the counter's free end (y -1.75..-2.75; the claimed meat and haunch hold the rest).")
L("# The counter's free strip is only y -1.75..-2.25 (the claimed meat, haunch and bowls hold the rest).")
item('heap',['turkey','haunch','bread','cleaver','knife','bread'],6,at=1.8,spacing=0.2,step=0.05,gap=0.15,on=58.3,offset=off(A[0],A[1],-9463.4,-1.95))
item('line',['bowl','baguette','plate','baguette'],4,at=0.9,spacing=0.25,heading=S,wobble=0.03,on=58.3,offset=off(A[0],A[1],-9464.6,-1.95))
item('cluster','platter',1,at=2.2,spacing=0.3,on=58.3,offset=off(A[0],A[1],-9464.9,-1.0))
item('cluster','cauldron',1,at=2.3,spacing=0.3,offset=off(A[0],A[1],-9467.4,-0.6))
item('cluster',['bone','meat','bone','bone'],7,at=1.6,spacing=0.9,gap=0.4,offset=off(A[0],A[1],-9466.6,-4.0))
item('cluster','pumpkin',1,at=2.3,spacing=0.3,offset=off(A[0],A[1],-9470.2,5.6))

# ---------------------------------------------------------------- S5 cellar
A=(-9468.0, -2.0, 49.8)
scene("The rats' cheese hoard", A,
 "Down the stairs: the cheese the rats have been dragging off for years, the giant wheels, the Colossal\n"
 "Swiss, the claimed barrels, and what became of the last patron who came looking.")
item('cluster',['wedge','wheel','wedgeS'],12,at=0.6,spacing=1.0,gap=0.22,offset=off(A[0],A[1],-9466.3,-8.0),jitter=0.06)
item('spill',['wedge','wheel'],12,at=0.9,spacing=0.5,heading=head(-7,+3),bend=0.05,offset=off(A[0],A[1],-9464.2,-8.6))
item('heap',['wheel','wedge','wheelS','wheel','wedgeS'],30,at=1.2,spacing=2.4,step=0.07,gap=0.26,offset=off(A[0],A[1],-9468.0,-6.0),jitter=0.06)
item('cluster',['wheel','wheelS'],8,at=1.6,spacing=2.6,gap=0.5,offset=off(A[0],A[1],-9468.0,-6.0))
item('ring','wheel',3,at=2.0,spacing=2.6,wobble=0.2,offset=off(A[0],A[1],-9468.0,-6.0))
item('cluster','wheelS',1,at=2.3,spacing=0.5,offset=off(A[0],A[1],-9470.5,3.0))
item('cluster',['rope','bone','fishskel','bone','skull','rope'],10,at=1.5,spacing=1.8,gap=0.4,offset=off(A[0],A[1],-9475.5,-8.0),jitter=0.05)
item('cluster',['bottle','green','jug'],8,at=1.3,spacing=1.1,gap=0.4,topple=0.6,topple_lift=0.15,offset=off(A[0],A[1],-9471.5,4.3))
item('cluster',['gbarrel','bucket','sackflat'],6,at=2.2,spacing=1.0,gap=0.8,offset=off(A[0],A[1],-9477.0,-4.0))

# ---------------------------------------------------------------- S6 upstairs
A=(-9470.0, 24.0, 63.82)
scene("Boots in the hall", A, "Top of the stairs: the guests who slept through it left their boots out for polishing.")
item('line',['boots','bootsdark'],6,at=1.3,spacing=0.75,heading=W,wobble=0.1,offset=off(A[0],A[1],-9471.0,24.0))
item('spill',['half','wedge','muffin','wheel'],12,at=0.55,spacing=0.5,heading=head(+3,+5),bend=0.05,offset=off(A[0],A[1],-9471.5,22.0),jitter=0.06)
item('cluster',['candle','pail'],3,at=1.4,spacing=1.0,gap=0.4,offset=off(A[0],A[1],-9474.0,24.0))

A=(-9460.0, 30.0, 63.82)
scene("Pillow fight in the lounge", A, "The upstairs lounge: cushions everywhere, William Pestle asleep through all of it.")
item('scatter','cushion',10,at=1.7,spacing=1.0,offset=off(A[0],A[1],-9459.5,30.0),tilt=0.3)
item('cluster',['goblet','green','mug2'],8,at=1.1,spacing=1.6,gap=0.4,topple=0.3,topple_lift=0.15,offset=off(A[0],A[1],-9458.0,27.0))

A=(-9463.0, 15.0, 63.82)
scene("The scribe's avalanche", A, "The bookroom: the shelves gave up. Books, scrolls and candle stubs across the floor.")
item('cluster',['bookmed','scroll','bookmed','booklarge','bookmed'],18,at=1.4,spacing=2.4,gap=0.35,topple=0.5,topple_lift=0.15,jitter=0.05)
item('cluster','candle',4,at=1.0,spacing=1.2,gap=0.4,offset=off(A[0],A[1],-9461.0,12.0))

A=(-9463.0, -2.0, 63.82)
scene("The guard who slept through it", A, "Bedroom one: a Stormwind guard's kit, dropped where he fell into bed.")
item('cluster',['breast','legs','helm','legs','boots'],6,at=1.3,spacing=1.8,gap=0.4,tilt=0.2,lift=0.12)
item('cluster',['sword','shield'],2,at=2.1,spacing=1.0,gap=0.5,topple=1.0,topple_lift=0.1,offset=off(A[0],A[1],-9464.5,-1.0))

A=(-9473.0, 34.0, 63.82)
scene("The honeymoon suite", A, "The west bedroom: two goblets, one bottle, every cushion on the floor.")
item('cluster',['goblet','green','goblet'],4,at=1.1,spacing=0.9,gap=0.35,offset=off(A[0],A[1],-9474.5,32.0))
item('scatter','cushion',5,at=1.8,spacing=0.8,offset=off(A[0],A[1],-9473.5,32.6),tilt=0.3)

# ---------------------------------------------------------------- S7 door
A=(-9459.93, 38.11, 56.95)
scene("The door", A,
 "The inner vestibule door, 4.4 yd wide and 3.9 high, with a 4.6 yd keg wedged in it. It never becomes\n"
 "edible (blocker): this is the level's only containment, and the player can see why.")
item('line','keg',1,scale=1.6,yaw=0.0,blocker=True)
item('cluster','poster',1,at=2.3,spacing=0.3,offset=off(A[0],A[1],-9461.5,36.0))

# ---------------------------------------------------------------- finish: the common room furniture
A=(-9472.0, 25.0, 56.339)
scene("Farley wants his floor back", A,
 "The end game in the pit: stools and footstools pulled out for the toast, pony kegs, a crate of empties.\n"
 "Everything here opens between 1.9 and 2.4 yd; the claimed tables, bench and chairs stay as furniture.")
item('cluster',['wchair','chair','wchair'],8,at=2.25,spacing=4.0,gap=1.3,offset=off(A[0],A[1],-9473.0,25.0),jitter=0.04)
item('ring','tinytable',3,at=2.35,spacing=3.6,offset=off(A[0],A[1],-9474.0,24.0))
item('cluster',['crate','barrel','gbarrel','crate'],6,at=2.1,spacing=1.0,gap=1.0,offset=off(A[0],A[1],-9470.8,25.5))
item('line','keg2',3,at=2.3,spacing=1.6,heading=S,offset=off(A[0],A[1],-9472.6,29.4))
item('cluster',['mug','foam','stein','mug'],16,at=0.75,spacing=3.5,gap=0.6,topple=0.5,topple_lift=0.1,offset=off(A[0],A[1],-9473.0,25.0),jitter=0.05)
item('cluster',['bucket','pail','sack'],6,at=1.9,spacing=3.2,gap=0.8,offset=off(A[0],A[1],-9470.5,26.5),jitter=0.04)

here=os.path.dirname(os.path.abspath(__file__))
root=os.path.abspath(os.path.join(here,'..','..','..')) if here.endswith(os.path.join('docs','azeroll','data')) else '/home/jjd/.t3/worktrees/azeroll/t3-cb88fe91'
dst=os.path.join(root,'crates','benilla-app','src','azeroll','lions-pride.toml')
text='\n'.join(out)+'\n'
if CAP:
    dst='/tmp/az-l1r/tour-%s.toml'%CAP
    text=text.replace('area = "Kitchen"\ntitle','area = "%s"\ntitle'%GOALAREA[CAP])
FIX=os.environ.get('LP_FIX')
if FIX:
    # Capture fixtures (D33): the same level with the ball started somewhere else, at LP_R.
    spots={'parcel':[-9462.0,23.1,56.339],      # the foot of the hearth's book stair
           'lip':[-9459.6,21.4,56.339],         # on the boards at the firebox lip
           'cellarfar':[-9466.5,5.5,49.8],
           'cellar':[-9466.5,-1.5,49.8]}        # the cellar, beside the cheese hoard
    r=os.environ.get('LP_R','0.25')
    x,y,z=spots[FIX]
    import re
    text=re.sub(r'^start = \[.*\]$','start = [%s, %s, %s]'%(x,y,z),text,count=1,flags=re.M)
    text=re.sub(r'^start_radius = .*$','start_radius = %s'%r,text,count=1,flags=re.M)
    if os.environ.get('LP_LIMIT'):
        text=re.sub(r'^time_limit = .*$','time_limit = %s'%os.environ['LP_LIMIT'],text,count=1,flags=re.M)
    os.makedirs('/tmp/az-replay',exist_ok=True)
    dst='/tmp/az-replay/lp-%s-r%s.toml'%(FIX,r)
open(dst,'w').write(text)
print('wrote',dst,'lines',len(out))
