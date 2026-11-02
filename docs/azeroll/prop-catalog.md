# Azeroll prop catalog

Designer menu for placing themed clusters. Source: `docs/azeroll/data/props.csv` (generated, one row per model in scope). Data is Blizzard's WoW 1.12.1 M2 models, read from the local MPQs; nothing is copied into the repo.

## How to read sizes

- **radius** is the authored vertex bounding sphere (yd, M2 header `0xCC`). This is what the size tiers use.
- **thickness** is the second-largest half-extent of the vertex box. This is the pickup credit the game uses (`ARCHITECTURE.md` D4).
- **name** is the HUD name from the game's table (`names.rs`) where one exists. `name_source` marks the rest as `segmented` (matched against the name word list) or `camel` (split from the file name, not the game's HUD name).
- **theme** is a keyword heuristic on file and folder names. Treat it as a starting point; check the path.

| tier | radius (yd) | rows |
|---|---|---|
| tiny | <0.2 | 36 |
| small | 0.2–0.5 | 167 |
| medium | 0.5–1.2 | 314 |
| large | 1.2–3 | 524 |
| huge | 3–8 | 345 |
| massive | >8 | 164 |

1550 models in scope. Models with collision triangles: 928.

## Themes

### Food and drink

37 models; radius 0.03–2.47 yd, median 0.44 yd.

Most food is small: 21 of 37 are under 0.5 yd and 11 are under 0.2 yd. Only 7 sit in 0.5–1.2 yd, and nothing is above 3 yd. Build clusters from small pieces with a few larger platters or sacks from containers.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Cheese Wheel | 0.05 | tiny | `World\Generic\Human\Passive Doodads\Food\CheeseWheel01.m2` |
| Pie | 0.08 | tiny | `World\Generic\Human\Passive Doodads\Food\Pie01.m2` |
| Orange | 0.25 | small | `World\Generic\PassiveDoodads\Fruits\Fruit_Orange.m2` |
| Orange Slice | 0.27 | small | `World\Generic\PassiveDoodads\Fruits\Fruit_OrangeSlice.m2` |
| Fish Skeleton | 0.30 | small | `World\Generic\PassiveDoodads\Fish\fishskel01.m2` |
| Apple | 0.33 | small | `World\Generic\PassiveDoodads\Fruits\Fruit_Apple.m2` |
| Melon Slice | 0.34 | small | `World\Generic\PassiveDoodads\Fruits\Fruit_MelonSlice01.m2` |
| Pear | 0.44 | small | `World\Generic\PassiveDoodads\Fruits\Fruit_Pear.m2` |
| Banana | 0.48 | small | `World\Generic\PassiveDoodads\Fruits\Fruit_Banana.m2` |
| Banana Bunch | 0.50 | medium | `World\Generic\PassiveDoodads\Fruits\Fruit_BananaBunch.m2` |
| Melon | 0.51 | medium | `World\Generic\PassiveDoodads\Fruits\Fruit_Melon.m2` |
| Bread | 0.73 | medium | `World\Generic\Human\Passive Doodads\Food\bread01.m2` |
| Watermelon | 1.12 | medium | `World\Goober\G_Watermelon.m2` |
| Pumpkin | 1.22 | large | `World\Goober\G_Pumpkin_01.m2` |
| Apple Bobbing Barrel | 1.68 | large | `World\Goober\G_AppleBob_01.m2` |

### Tableware

41 models; radius 0.21–16.36 yd, median 0.54 yd.

Cups, mugs, plates and bottles cluster at 0.2–0.5 yd and 0.5–1.2 yd. Almost nothing is above 3 yd, so use tableware for loose scatter, not for big set pieces.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Stein | 0.21 | small | `World\Azeroth\Westfall\PassiveDoodads\Utensils\Stein.m2` |
| Mug | 0.22 | small | `World\Generic\Human\Passive Doodads\Mugs\Mug01.m2` |
| Frothy Mug | 0.23 | small | `World\Generic\Human\Passive Doodads\Mugs\MugFoam01.m2` |
| Plate | 0.25 | small | `World\Azeroth\Westfall\PassiveDoodads\Utensils\Plate.m2` |
| Bowl | 0.26 | small | `World\Azeroth\Westfall\PassiveDoodads\Utensils\Bowl.m2` |
| Relic | 0.28 | small | `World\Goober\G_RelicNECup.m2` |
| Goblet | 0.33 | small | `World\Generic\Human\Passive Doodads\Goblets\Goblet02.m2` |
| Bottle | 0.38 | small | `World\Generic\Human\Passive Doodads\Bottles\Bottle01.m2` |
| Jug | 0.41 | small | `World\Azeroth\Elwynn\PassiveDoodads\jugs\jug02.m2` |
| Orc Mug | 0.53 | medium | `World\Generic\Orc\Passive Doodads\Mugs\OrcMug01.m2` |
| Golden Goblet | 0.54 | medium | `World\Generic\Human\Passive Doodads\Mugs\Goldgoblet01.m2` |
| Filled Goblet | 0.54 | medium | `World\Generic\Human\Passive Doodads\Mugs\Goldgobletfilled01.m2` |
| Scrying Bowl | 0.54 | medium | `World\Goober\G_ScryingBowl.m2` |
| Orc Bottle | 0.57 | medium | `World\Generic\Orc\Passive Doodads\Bottles\OrcBottle01.m2` |
| Metal Cup | 0.58 | medium | `World\Generic\Human\Passive Doodads\Cups\MetalCup03.m2` |
| Bowl of Apples | 0.81 | medium | `World\Generic\PassiveDoodads\Fruits\FruitBowl_Apples.m2` |
| Fruit Bowl | 0.81 | medium | `World\Generic\PassiveDoodads\Fruits\FruitBowl_Mixed.m2` |
| Empty Fruit Bowl | 0.82 | medium | `World\Generic\PassiveDoodads\Fruits\FruitBowl_Empty.m2` |
| Orc Jug | 0.83 | medium | `World\Generic\Orc\Passive Doodads\Jugs\OrcJug02.m2` |
| Gnome Steel Plate | 1.40 | large | `World\Generic\Gnome\Passive Doodads\Parts\GnomeSteelPlate05.m2` |

### Books and scrolls

64 models; radius 0.28–57.98 yd, median 0.65 yd.

Good coverage from 0.2 to 3 yd, with no huge items; the one outlier is the Uldaman map at 58 yd. Only 9 models have HUD names, so most entries show the camel-case fallback.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Book Small | 0.28 | small | `World\Generic\Human\Passive Doodads\Books\BookSmall01.m2` |
| Book Medium 07 | 0.38 | small | `World\Generic\Human\Passive Doodads\Books\BookMedium07.m2` |
| Book Small Open 03 | 0.38 | small | `World\Generic\Human\Passive Doodads\Books\BookSmallOpen03.m2` |
| Book Medium 01 | 0.39 | small | `World\Generic\Human\Passive Doodads\Books\BookMedium01.m2` |
| Book Medium 04 | 0.39 | small | `World\Generic\Human\Passive Doodads\Books\BookMedium04.m2` |
| Scroll B 01 | 0.48 | small | `World\Generic\Human\Passive Doodads\Scrolls\ScrollB01.m2` |
| Brown Book | 0.59 | medium | `World\Goober\G_BookOpenMediumBrown.m2` |
| General Book Stack Tall 01 | 0.60 | medium | `World\Generic\Human\Passive Doodads\BookStacks\GeneralBookStackTall01.m2` |
| Scroll | 0.62 | medium | `World\Goober\G_Scroll01.m2` |
| Scroll Map | 0.65 | medium | `World\Generic\Human\Passive Doodads\Scrolls\ScrollMap.m2` |
| Book Large Open 02 | 0.73 | medium | `World\Generic\Human\Passive Doodads\Books\BookLargeOpen02.m2` |
| Book Large 03 | 0.85 | medium | `World\Generic\Human\Passive Doodads\Books\BookLarge03.m2` |
| New Wanted Poster 04 | 0.97 | medium | `World\Generic\Passive Doodads\WantedPosters\NewWantedPoster04.m2` |
| New Wanted Poster 03 | 1.38 | large | `World\Generic\Passive Doodads\WantedPosters\NewWantedPoster03.m2` |
| Painting 05 | 1.82 | large | `World\Generic\Human\Passive Doodads\Artwork\Painting05.m2` |
| Painting 10 | 1.82 | large | `World\Generic\Human\Passive Doodads\Artwork\Painting10.m2` |
| Painting 15 | 2.19 | large | `World\Generic\Human\Passive Doodads\Artwork\Painting15.m2` |
| Painting 12 | 2.40 | large | `World\Generic\Human\Passive Doodads\Artwork\Painting12.m2` |
| Painting 14 | 2.48 | large | `World\Generic\Human\Passive Doodads\Artwork\Painting14.m2` |
| Uldaman Map | 57.98 | massive | `World\Goober\G_UldamanMap.m2` |

### Tools

46 models; radius 0.19–27.60 yd, median 0.85 yd.

Mostly 0.5–1.2 yd. Small hand tools (under 0.5 yd) are sparse, with 7 items under 0.5 yd. Pulleys and machinery reach 12–28 yd.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Carving Chisel 02 | 0.19 | tiny | `World\Generic\Tauren\Passive Doodads\CarvingTools\CarvingChisel02.m2` |
| Mallet 01 | 0.42 | small | `World\Generic\Dwarf\Passive Doodads\Tools\Mallet01.m2` |
| Gnome Tool 03 | 0.45 | small | `World\Generic\Gnome\Passive Doodads\Tools\GnomeTool03.m2` |
| Wrench 01 | 0.50 | medium | `World\Generic\Dwarf\Passive Doodads\Tools\Wrench01.m2` |
| screwdriver 01 | 0.55 | medium | `World\Generic\Dwarf\Passive Doodads\Tools\screwdriver01.m2` |
| Book Medium Open 01 | 0.56 | medium | `World\Generic\Human\Passive Doodads\Books\BookMediumOpen01.m2` |
| Book Medium Open 03 | 0.56 | medium | `World\Generic\Human\Passive Doodads\Books\BookMediumOpen03.m2` |
| Book Medium Open 05 | 0.56 | medium | `World\Generic\Human\Passive Doodads\Books\BookMediumOpen05.m2` |
| Gnome Tool 01 | 0.74 | medium | `World\Generic\Gnome\Passive Doodads\Tools\GnomeTool01.m2` |
| Ball And Chain 01 | 0.76 | medium | `World\Generic\Human\Passive Doodads\BallandChain\BallAndChain01.m2` |
| Carving Chisel 01 | 0.85 | medium | `World\Generic\Tauren\Passive Doodads\CarvingTools\CarvingChisel01.m2` |
| Carving Hatchet 01 | 0.97 | medium | `World\Generic\Tauren\Passive Doodads\CarvingTools\CarvingHatchet01.m2` |
| miningsifter 02 | 1.07 | medium | `World\Generic\Human\Passive Doodads\MiningSifters\miningsifter02.m2` |
| Grindstone | 1.27 | large | `World\Azeroth\Westfall\PassiveDoodads\GrindStone\GrindStone.m2` |
| Anvilmar Lamp Post | 1.95 | large | `World\Generic\Dwarf\Passive Doodads\LampPosts\AnvilmarLampPost.m2` |
| Dark Iron Saw Broken 01 | 2.80 | large | `World\Generic\DarkIronDwarf\Passive Doodads\Tools\DarkIronSawBroken01.m2` |
| Iron Forge Hanging Lantern new | 5.54 | huge | `World\Generic\Dwarf\Passive Doodads\Lanterns\IronForgeHangingLantern_new.m2` |
| Anvilmar Dock 01 | 7.58 | huge | `World\Generic\Dwarf\Passive Doodads\Docks\AnvilmarDock01.m2` |
| Pulley | 12.31 | massive | `World\Generic\PassiveDoodads\Misc\Pulleys\Pulley.m2` |
| Exterior Pulley | 27.60 | massive | `World\Generic\Human\Passive Doodads\Pulleys\ExteriorPulley.m2` |

### Weapons and armor

126 models; radius 0.14–10.96 yd, median 0.68 yd.

The widest spread of any theme, from 0.14 to 11 yd. Good for mixed loot and racks. Most entries are armor pieces and weapon racks with camel-case names, so check the path before you place one.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Blue Bracers | 0.14 | tiny | `World\Generic\Human\Passive Doodads\Armor\ArmorBracerBlue.m2` |
| Armor Leather Helm Black | 0.25 | small | `World\Generic\Human\Passive Doodads\Armor\ArmorLeatherHelmBlack.m2` |
| Armor Helm Gold | 0.27 | small | `World\Generic\Human\Passive Doodads\Armor\ArmorHelmGold.m2` |
| Armor Leather Shirt Black | 0.39 | small | `World\Generic\Human\Passive Doodads\Armor\ArmorLeatherShirtBlack.m2` |
| Armor Leather Shirt Brown | 0.39 | small | `World\Generic\Human\Passive Doodads\Armor\ArmorLeatherShirtBrown.m2` |
| Flint Club | 0.43 | small | `World\Generic\PassiveDoodads\Weapons\flint_club01.m2` |
| Boots Leather Black | 0.46 | small | `World\Generic\Human\Passive Doodads\Clothing\BootsLeatherBlack.m2` |
| Skull Club | 0.51 | medium | `World\Generic\PassiveDoodads\Weapons\SkullClub01.m2` |
| Human Bow | 0.65 | medium | `World\Generic\Human\Passive Doodads\Weapons&Armor\HumanBow02.m2` |
| Human Sword | 0.75 | medium | `World\Generic\Human\Passive Doodads\Weapons&Armor\HumanSword01.m2` |
| Skull Sword | 0.81 | medium | `World\Azeroth\Elwynn\PassiveDoodads\BattleGladeSwordSkull\BattleGladeSwordSkull.m2` |
| Spiked Club | 0.95 | medium | `World\Generic\PassiveDoodads\Weapons\spiked_club01.m2` |
| Tauren Weapon Log | 1.01 | medium | `World\Generic\Tauren\Passive Doodads\Weapons\Tauren_WeaponLog01.m2` |
| Wicker Bow | 1.04 | medium | `World\Generic\PassiveDoodads\Weapons\wicker_bow.m2` |
| Flint Spear | 1.37 | large | `World\Generic\PassiveDoodads\Weapons\flint_spear02.m2` |
| Orc Spear | 1.39 | large | `World\Generic\Orc\Passive Doodads\Weapons\OrcSpear03.m2` |
| Naga Trident | 1.46 | large | `World\Generic\PassiveDoodads\Weapons\naga_trident.m2` |
| Skull Spear | 1.53 | large | `World\Generic\PassiveDoodads\Weapons\skullspear01.m2` |
| Tauren Weapon Rack | 1.66 | large | `World\Generic\Tauren\Passive Doodads\Weapons\Tauren_WeaponRack02.m2` |
| Kruk Spear | 1.90 | large | `World\Goober\G_KruskSpear.m2` |

### Furniture

144 models; radius 0.15–12.16 yd, median 1.79 yd.

Heavy in 1.2–3 yd (tables, chairs, beds, shelves). Almost nothing under 0.5 yd, so small furniture is a gap; use props from containers or tableware for tabletop scatter.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Human Skull | 0.15 | tiny | `World\Azeroth\Elwynn\PassiveDoodads\BattleGladeSkullHuman1\BattleGladeSkullHuman1.m2` |
| Orc Skull | 0.19 | tiny | `World\Azeroth\Elwynn\PassiveDoodads\BattleGladeSkullOrc1\BattleGladeSkullOrc1.m2` |
| Bone | 0.26 | small | `World\Generic\PassiveDoodads\Furniture\Refuse\Bone02.m2` |
| Westfall Chair | 0.50 | medium | `World\Azeroth\Westfall\PassiveDoodads\WestfallChair\WestfallChair.m2` |
| Gnome Chair | 0.57 | medium | `World\Generic\Gnome\Passive Doodads\Furniture\GnomeChair03.m2` |
| Round Table | 1.23 | large | `World\Azeroth\Elwynn\PassiveDoodads\Tables\RoundTable\RoundTable.m2` |
| Orc Table | 1.26 | large | `World\Generic\Orc\Passive Doodads\Tables\OrcTable01.m2` |
| Explosive Package | 1.44 | large | `World\Generic\PassiveDoodads\Furniture\Containers\exploding_package.m2` |
| Gnome Table | 1.50 | large | `World\Generic\Gnome\Passive Doodads\Furniture\GnomeTable03.m2` |
| Westfall Table | 1.52 | large | `World\Azeroth\Westfall\PassiveDoodads\WestfallTable\WestfallTable.m2` |
| Stormwind Bench | 1.62 | large | `World\Generic\Human\Passive Doodads\Benches\StormWindBench01.m2` |
| Duskwood Bookshelf | 1.72 | large | `World\Generic\Human\Passive Doodads\BookShelves\DuskwoodBookshelf02.m2` |
| Lich Container | 1.76 | large | `World\Generic\PassiveDoodads\Furniture\Containers\Lichcontainer01.m2` |
| Titan Vase | 1.79 | large | `World\Generic\PassiveDoodads\Furniture\Containers\TitanVase02.m2` |
| Duskwood Bench | 2.19 | large | `World\Generic\Human\Passive Doodads\Benches\DuskwoodBench.m2` |
| Duskwood Table | 2.57 | large | `World\Generic\Human\Passive Doodads\Tables\DuskwoodTable01.m2` |
| Tauren Log Chair | 2.65 | large | `World\Generic\Tauren\Passive Doodads\Chairs\TaurenLogChair02.m2` |
| Alliance Map Table | 2.74 | large | `World\Generic\Human\Passive Doodads\StormWind\AllianceMapTable.m2` |
| Troll Tablet | 2.88 | large | `World\Generic\Troll\Passive Doodads\TrollTablets\TrollTablet.m2` |
| Orc Bench | 4.16 | huge | `World\Generic\ORC\PASSIVE DOODADS\Tables\OrcBench01.m2` |

### Containers

129 models; radius 0.25–7.06 yd, median 1.13 yd.

Barrels, crates and sacks mostly fall at 0.5–1.2 yd and 1.2–3 yd. This is the densest mid-size band for containers, so it is the easiest place to build a themed cluster.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Jar Orc | 0.25 | small | `World\Generic\Orc\Passive Doodads\Jars\JarOrc03.m2` |
| Sack | 0.46 | small | `World\Generic\PassiveDoodads\Furniture\Containers\Sack02.m2` |
| Westfall Barrel | 0.62 | medium | `World\Azeroth\Westfall\PassiveDoodads\Barrel\WestFallBarrel01.m2` |
| Bucket | 0.69 | medium | `World\Generic\Human\Passive Doodads\Buckets\bucket.m2` |
| Tauren Sack | 0.72 | medium | `World\Generic\Tauren\Passive Doodads\Sacks\TaurenSack02.m2` |
| Chest | 0.78 | medium | `World\Generic\ActiveDoodads\Chest02\Chest02.m2` |
| Grape Bucket | 0.90 | medium | `World\Azeroth\Elwynn\PassiveDoodads\GrapeBuckets\GrapeBucket01.m2` |
| Orc Crate | 0.91 | medium | `World\Generic\Orc\Passive Doodads\BarrelsandCrates\OrcCrate01.m2` |
| Stone Tablet | 0.99 | medium | `World\Goober\G_NETablet.m2` |
| Stormwind Crate | 1.10 | medium | `World\Generic\Human\Passive Doodads\Crates\StormwindCrate01.m2` |
| Cauldron | 1.25 | large | `World\Generic\Human\Passive Doodads\Cauldrons\Cauldron.m2` |
| Horde Axe Crate | 1.34 | large | `World\Generic\PassiveDoodads\WeaponCrates\WeaponCrateHordeAxe.m2` |
| Animal Cage | 1.44 | large | `World\Generic\Tauren\Passive Doodads\AnimalCages\AnimalCage01.m2` |
| Horde First Aid Crate | 1.60 | large | `World\Generic\PassiveDoodads\CratesFirstAid\CrateHordeFirstAid01.m2` |
| Animal Crate | 2.11 | large | `World\Goober\G_CrateAnimal.m2` |
| Cage | 2.16 | large | `World\Goober\G_CageBase.m2` |
| Bamboo Cage Door | 2.56 | large | `World\Goober\G_CageDoorBamboo.m2` |
| Curved Mine Tracks | 3.76 | huge | `World\Azeroth\Elwynn\Buildings\GoldMine\GoldmineTrackCurve.m2` |
| Keg Shotgun | 4.26 | huge | `World\Goober\DarkIronKegShotgun.m2` |
| Tent | 6.30 | huge | `World\Goober\G_TentBurninator.m2` |

### Farm

49 models; radius 0.19–34.19 yd, median 2.23 yd.

Only 50 rows. Most (35) are 1.2 yd and up. Corn and wheat fields reach 26–34 yd, so they are landmarks. Only 3 items sit under 0.5 yd, so small farm clusters need containers or food as fillers.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Corn | 0.19 | tiny | `World\Azeroth\Elwynn\PassiveDoodads\CornCrop\corn.m2` |
| Plagued Grain Sack 02 | 0.46 | small | `World\Generic\Human\Passive Doodads\Sacks\PlaguedGrainSack02.m2` |
| Kitchen Knife | 0.58 | medium | `World\Generic\Human\Passive Doodads\Food&Utensils\KitchenKnife.m2` |
| Haystack | 0.87 | medium | `World\Azeroth\Elwynn\PassiveDoodads\Haystacks\haystack01.m2` |
| Grain Sacs 02 | 0.98 | medium | `World\Generic\Dwarf\Passive Doodads\TavernStuff\GrainSacs02.m2` |
| Crate Grain 01 | 0.99 | medium | `World\Generic\Human\Passive Doodads\Crates\CrateGrain01.m2` |
| corncropdead | 1.07 | medium | `World\Azeroth\Elwynn\PassiveDoodads\CornCrop\corncropdead.m2` |
| Butter Churner | 1.26 | large | `World\Azeroth\Westfall\PassiveDoodads\ButterChurner\ButterChurner.m2` |
| Shadowfang Hay Pile 02 | 1.42 | large | `World\Generic\Human\Passive Doodads\HayPiles\ShadowfangHayPile02.m2` |
| Dog House | 1.85 | large | `World\Generic\Human\Passive Doodads\DogHouses\DogHouse.m2` |
| Duskwood Straw | 2.02 | large | `World\Azeroth\Duskwood\PassiveDoodads\Straw\DuskwoodStraw02.m2` |
| West Fall Scarecrow | 2.23 | large | `World\Azeroth\Westfall\PassiveDoodads\Scarecrow\WestFallScarecrow.m2` |
| Plow | 2.82 | large | `World\Azeroth\Westfall\PassiveDoodads\Plow\Plow.m2` |
| Corn Row | 3.78 | huge | `World\Azeroth\Elwynn\PassiveDoodads\CornCrop\CornCropRow.m2` |
| Corn Crop | 4.22 | huge | `World\Azeroth\Elwynn\PassiveDoodads\CornCrop\corncrop1.m2` |
| aquaduct corner 2 | 4.46 | huge | `World\Azeroth\Westfall\PassiveDoodads\Aquaduct\aquaduct_corner2.m2` |
| miningtroughlarge | 7.36 | huge | `World\Generic\Human\Passive Doodads\MiningTroughs\miningtroughlarge.m2` |
| Dark Hay Pile Large 01 | 11.50 | massive | `World\Generic\DarkIronDwarf\Passive Doodads\HayPiles\DarkHayPileLarge01.m2` |
| West Fall Grain Silo 01 | 17.32 | massive | `World\Azeroth\Westfall\Buildings\GrainSilo\WestFallGrainSilo01.m2` |
| Corn Crop Low Poly | 34.19 | massive | `World\Azeroth\Elwynn\PassiveDoodads\CornCrop\CornCropLowPoly.m2` |

### Market and stall goods

59 models; radius 0.12–5.47 yd, median 0.67 yd.

Spread from 0.2 to 3 yd, with a second group in 3–8 yd (stalls, awnings, crates of goods). Good for stall clusters. Nothing is above 5.5 yd.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Cloth Yarn Ball Red | 0.12 | tiny | `World\Generic\Human\Passive Doodads\Clothing\ClothYarnBallRed.m2` |
| Cloth Spool Blue Low | 0.16 | tiny | `World\Generic\Human\Passive Doodads\Cloth\ClothSpoolBlueLow.m2` |
| Gun Shop Ammo Box Blue | 0.24 | small | `World\Generic\Human\Passive Doodads\GunShop\GunShopAmmoBoxBlue.m2` |
| Gun Shop Bomb | 0.27 | small | `World\Generic\Human\Passive Doodads\GunShop\GunShopBomb.m2` |
| Cloth Yarn Red Low | 0.29 | small | `World\Generic\Human\Passive Doodads\Cloth\ClothYarnRedLow.m2` |
| Gun Shop Ammo Box Blue Block | 0.35 | small | `World\Generic\Human\Passive Doodads\GunShop\GunShopAmmoBoxBlueBlock.m2` |
| Hanging Shirt Clothesline Brown | 0.49 | small | `World\Generic\Human\Passive Doodads\Clothing\HangingShirtClotheslineBrown.m2` |
| Gun Shop Fireworks 02 | 0.49 | small | `World\Generic\Human\Passive Doodads\GunShop\GunShopFireworks02.m2` |
| Gun Shop Fireworks 04 | 0.50 | small | `World\Generic\Human\Passive Doodads\GunShop\GunShopFireworks04.m2` |
| Hanging Pants Clothesline Brown | 0.60 | medium | `World\Generic\Human\Passive Doodads\Clothing\HangingPantsClotheslineBrown.m2` |
| Gun Shop Gun Stock | 0.66 | medium | `World\Generic\Human\Passive Doodads\GunShop\GunShopGunStock.m2` |
| Cloth Bolt Red | 0.67 | medium | `World\Generic\Human\Passive Doodads\Cloth\ClothBoltRed.m2` |
| Wicker Shield | 0.69 | medium | `World\Generic\PassiveDoodads\Weapons\wicker_shield.m2` |
| Leather Shield | 0.88 | medium | `World\Generic\PassiveDoodads\Weapons\leather_shield.m2` |
| Gun Shop Target 03 | 0.91 | medium | `World\Generic\Human\Passive Doodads\GunShop\GunShopTarget03.m2` |
| Alliance Shield Rack | 0.97 | medium | `World\Generic\PassiveDoodads\ShieldRacks\RackShieldAlliance01.m2` |
| Duskwood Shop Counter 02 | 3.17 | huge | `World\Generic\Human\Passive Doodads\ShopCounter\DuskwoodShopCounter02.m2` |
| Stormwind Vendor Tent 01 | 5.20 | huge | `World\Generic\Human\Passive Doodads\VendorTents\StormwindVendorTent01.m2` |
| Baker Shop 01 | 5.47 | huge | `World\Generic\Human\Passive Doodads\Signs\BakerShop01.m2` |
| Weaponsmith Shop 01 | 5.47 | huge | `World\Generic\Human\Passive Doodads\Signs\WeaponsmithShop01.m2` |

### Signs and lamps

350 models; radius 0.41–43.35 yd, median 2.52 yd.

Biggest theme, but most of it is large or huge (signs on posts, lamp posts, banners). Only one item sits under 0.5 yd, and only 44 sit in 0.5–1.2 yd. Small lamps and candles are a clear gap. Text signs (bread, armory) now go here, not to food.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Duskwood Signpost | 0.59 | medium | `World\Azeroth\Elwynn\PassiveDoodads\Signs\Directional\Duskwooddirectionalsign.m2` |
| Westfall Signpost | 0.59 | medium | `World\Azeroth\Elwynn\PassiveDoodads\Signs\Directional\Westfalldirectionalsign.m2` |
| Lantern | 0.77 | medium | `World\Generic\Human\Passive Doodads\Lanterns\GeneralLantern01.m2` |
| Tackle Sign | 1.08 | medium | `World\Azeroth\Elwynn\PassiveDoodads\Signs\Shop\HumanBaitandTackleSign.m2` |
| Tanner Sign | 1.08 | medium | `World\Azeroth\Elwynn\PassiveDoodads\Signs\Shop\HumanTannerSign.m2` |
| Mailbox | 1.19 | medium | `World\Generic\PassiveDoodads\PostBoxes\PostBoxGnome.m2` |
| Golden Brazier | 1.34 | large | `World\Goober\G_EpicBrazierYellow.m2` |
| Tauren Lamppost | 1.95 | large | `World\Generic\Tauren\Passive Doodads\LampPosts\TaurenLampPost02.m2` |
| Orc Brazier Street Lamp | 2.14 | large | `World\Generic\Orc\Passive Doodads\LampPosts\OrcBrazierStreetLamp.m2` |
| Orc Sign Bread | 2.23 | large | `World\Generic\Orc\Passive Doodads\Signs\OrcSign_Bread.m2` |
| Orc Sign General | 2.23 | large | `World\Generic\Orc\Passive Doodads\Signs\OrcSign_General.m2` |
| Duskwood Lamppost | 2.25 | large | `World\Generic\Human\Passive Doodads\LampPosts\DuskwoodLamppost.m2` |
| Tauren Sign Wine | 2.50 | large | `World\Generic\Tauren\Passive Doodads\Signs\TaurenSign_Wine.m2` |
| Tauren Sign Leather Armor | 2.53 | large | `World\Generic\Tauren\Passive Doodads\Signs\TaurenSign_LeatherArmor.m2` |
| Crimson Banner | 2.72 | large | `World\Generic\Human\Passive Doodads\Banners\CrimsonBanner01.m2` |
| Dwarf Sign Cheese | 4.13 | huge | `World\Generic\Dwarf\Passive Doodads\Signs\DwarfSign_Cheese.m2` |
| Undead Sign Bread | 4.40 | huge | `World\Generic\Undead\Passive Doodads\Signs\UndeadSign_Bread.m2` |
| Undead Sign Leather Armor | 4.40 | huge | `World\Generic\Undead\Passive Doodads\Signs\UndeadSign_LeatherArmor.m2` |
| Bonfire | 4.88 | huge | `World\Goober\G_Bonfire.m2` |
| Westfall Light House | 19.80 | massive | `World\Azeroth\Westfall\Buildings\Lighthouse\WestfallLighthouse.m2` |

### Fences and walls

54 models; radius 0.61–83.18 yd, median 2.34 yd.

Fence sections are 0.6 yd and up, most at 1.2–8 yd. No small wall pieces under 0.6 yd. Use them as long-run segments, not as single pickups.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Crimson Wall Shield | 0.61 | medium | `World\Generic\Human\Passive Doodads\Weapons&Armor\CrimsonWallShield01.m2` |
| Wall Shield | 0.61 | medium | `World\Generic\Human\Passive Doodads\Weapons&Armor\WallShield03.m2` |
| Wall Sword | 0.75 | medium | `World\Generic\Human\Passive Doodads\Weapons&Armor\WallSword01.m2` |
| Stone Fence Post | 0.77 | medium | `World\Azeroth\Elwynn\PassiveDoodads\ElwynnFences\ElwynnStoneFencePost.m2` |
| Burningsteppes Fence Chain | 0.80 | medium | `World\Azeroth\BurningSteppes\PassiveDoodads\Fences\BurningSteppesFenceChain.m2` |
| Tauren Wall Scroll | 0.83 | medium | `World\Generic\Tauren\Passive Doodads\TaurenWallScrolls\TaurenWallScroll03.m2` |
| Gate Post | 1.15 | medium | `World\Azeroth\Duskwood\PassiveDoodads\IronGate\GatePost.m2` |
| Duskwood Gate | 1.21 | large | `World\Azeroth\Duskwood\PassiveDoodads\Gates\DuskWoodGate01.m2` |
| Burning Steppes Fence | 1.72 | large | `World\Azeroth\BurningSteppes\PassiveDoodads\Fences\BurningSteppesFence.m2` |
| Fence | 1.76 | large | `World\Azeroth\Elwynn\PassiveDoodads\ElwynnFences\ElwynnFenceSimple.m2` |
| Fence Rail | 2.08 | large | `World\Azeroth\Duskwood\PassiveDoodads\Fence\DuskwoodFenceRail.m2` |
| Generic Fence Post | 2.08 | large | `World\Generic\Human\Passive Doodads\StoneFence\GenericFencePost.m2` |
| Fence Bottom | 2.48 | large | `World\Azeroth\Duskwood\PassiveDoodads\Fence\DuskwoodFenceBottom.m2` |
| Fence Top | 2.48 | large | `World\Azeroth\Duskwood\PassiveDoodads\Fence\DuskwoodFenceTop.m2` |
| Wooden Fence | 2.48 | large | `World\Azeroth\Elwynn\PassiveDoodads\ElwynnFences\ElwynnWoodFence01.m2` |
| Orc Fence Post | 2.77 | large | `World\Generic\Orc\Passive Doodads\OrcFence\OrcFencePost.m2` |
| Stone Fence | 3.02 | huge | `World\Azeroth\Elwynn\PassiveDoodads\ElwynnFences\ElwynnStoneFence.m2` |
| Fence Segment | 3.21 | huge | `World\Azeroth\Duskwood\PassiveDoodads\Fence\DuskWoodFenceSegment.m2` |
| Orc Fence | 4.20 | huge | `World\Generic\Orc\Passive Doodads\OrcFence\OrcFence.m2` |
| Stormwind Gate | 83.18 | massive | `World\Azeroth\Elwynn\Buildings\StormwindEntrance\StormwindGate.m2` |

### Vehicles

69 models; radius 1.24–24.91 yd, median 4.71 yd.

Most (46 of 69) are 3–8 yd, 19 are 1.2–3 yd, and none is under 1.2 yd. There is no small cart for early levels.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Wrecked Mine Cart | 1.41 | large | `World\Generic\PassiveDoodads\Misc\MineCars\CaveMineCarWrecked02.m2` |
| Mine Cart | 1.43 | large | `World\Goober\G_MineCar.m2` |
| Meat Wagon Wheel | 1.50 | large | `World\Generic\Undead\Passive Doodads\MeatWagonPieces\MeatWagonWheel.m2` |
| Wheelbarrow | 1.65 | large | `World\Generic\PassiveDoodads\Misc\WheelBarrow\CaveMineWheelbarrow01.m2` |
| Rocket Cart | 2.93 | large | `World\Generic\Goblin\PassiveDoodads\GoblinRocketCarts\GoblinRocketCart02.m2` |
| Broken Cart | 3.28 | huge | `World\Azeroth\Westfall\PassiveDoodads\BrokenCart\BrokenCart.m2` |
| Bomb Wagon | 3.87 | huge | `World\Goober\G_bombwagon.m2` |
| Hay Wagon | 4.17 | huge | `World\Azeroth\Duskwood\PassiveDoodads\DuskwoodHaywagon\DuskwoodHayWagon.m2` |
| Westfall Hay Wagon | 4.66 | huge | `World\Azeroth\Westfall\PassiveDoodads\HayWagon\WestfallHayWagon.m2` |
| Westfall Wagon | 4.97 | huge | `World\Azeroth\Westfall\PassiveDoodads\Westfall Wagon\WestfallWagon01.m2` |
| Ballista | 5.33 | huge | `World\Goober\G_ExplodingBallista.m2` |
| Ruined Catapult | 5.39 | huge | `World\Azeroth\Elwynn\PassiveDoodads\RuinedCatapult\RuinedCatapult.m2` |
| Catapult | 5.47 | huge | `World\Azeroth\Elwynn\PassiveDoodads\Catapult\Catapult.m2` |
| Cannon | 5.63 | huge | `World\Generic\Human\Passive Doodads\Cannon\Cannon01.m2` |
| Rake Cart | 6.22 | huge | `World\Azeroth\Westfall\PassiveDoodads\RakeCart\RakeCart.m2` |
| TNT Wagon | 6.64 | huge | `World\Generic\Goblin\PassiveDoodads\GoblinTNTWagon\GoblinWagonTNT_01.m2` |
| Meat Wagon | 6.69 | huge | `World\Generic\Undead\Passive Doodads\MeatWagon\MeatWagon.m2` |
| Gypsy Wagon | 7.09 | huge | `World\Azeroth\Duskwood\Buildings\GypsyWagon\GypsyWagon.m2` |
| Stormwind Gypsy Wagon | 7.13 | huge | `World\Generic\Human\Passive Doodads\GypsyWagons\StormwindGypsywagon01.m2` |
| Ruined Ballista | 7.44 | huge | `World\Azeroth\Elwynn\PassiveDoodads\BallistaRuined\BallistaRuined.m2` |

### Plants and trees

105 models; radius 0.44–99.73 yd, median 6.74 yd.

Trees and stumps dominate at 3 yd and up (79 of 105). Only 11 plants sit under 1.2 yd, because the ground-cover and flower folders are excluded from this catalog.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Vineyard Trellis | 1.37 | large | `World\Azeroth\Elwynn\PassiveDoodads\Vineyard\VineyardWired.m2` |
| Westfall Tree Stump | 1.49 | large | `World\Azeroth\Westfall\PassiveDoodads\TreeStumps\WestFallTreeStump02.m2` |
| Elwynn Bush | 1.66 | large | `World\Azeroth\Elwynn\PassiveDoodads\Bush\ElwynnBush08.m2` |
| Berry Bush | 2.26 | large | `World\Generic\ActiveDoodads\Bushes\BerryBush01.m2` |
| Duskwood Bush | 2.74 | large | `World\Azeroth\Duskwood\PassiveDoodads\Bush\DuskwoodBush06.m2` |
| Elwynn Pine | 2.99 | large | `World\Azeroth\Elwynn\PassiveDoodads\Trees\ElwynnTree01\ElwynnPine02.m2` |
| Tree Stump | 3.02 | huge | `World\Azeroth\Elwynn\PassiveDoodads\Trees\Stumps\ElwynnTreeStump02.m2` |
| Pulsating Plant | 3.18 | huge | `World\Goober\G_PulsatingPlant.m2` |
| Elwynn Tree | 6.31 | huge | `World\Azeroth\Elwynn\PassiveDoodads\Trees\ElwynnTreeMid01.m2` |
| Duskwood Brown Tree | 12.29 | massive | `World\Azeroth\Duskwood\PassiveDoodads\Trees\DuskwoodBrownTree.m2` |
| Duskwood White Tree | 12.30 | massive | `World\Azeroth\Duskwood\PassiveDoodads\Trees\DuskwoodWhiteTree.m2` |
| Westfall Tree | 12.30 | massive | `World\Azeroth\Westfall\PassiveDoodads\Trees\WestFallTree01.m2` |
| Spore Mushroom | 13.59 | massive | `World\Goober\G_SporeMushroom.m2` |
| Bare Tree | 13.90 | massive | `World\Azeroth\Elwynn\PassiveDoodads\Trees\CanopylessTree02.m2` |
| Vineyard | 15.55 | massive | `World\Azeroth\Elwynn\PassiveDoodads\Vineyard\VineyardLowpoly.m2` |
| Elwynn Fir Tree | 21.74 | massive | `World\Azeroth\Elwynn\PassiveDoodads\Trees\ElwynnFirTree01.m2` |
| Duskwood Tree | 32.69 | massive | `World\Azeroth\Duskwood\PassiveDoodads\Trees\DuskwoodTree06.m2` |
| Elwynn Canopy Tree | 32.72 | massive | `World\Azeroth\Elwynn\PassiveDoodads\Trees\ElwynnTreeCanopy02.m2` |
| Duskwood Tree Canopy | 40.40 | massive | `World\Azeroth\Duskwood\PassiveDoodads\Trees\DuskwoodTreeCanopy03.m2` |
| Spooky Tree | 43.23 | massive | `World\Azeroth\Duskwood\PassiveDoodads\Trees\DuskwoodSpookyTree01.m2` |

### Rocks

43 models; radius 0.40–29.16 yd, median 5.42 yd.

Rock outcrops run from 0.4 to 29 yd, with most at 3 yd and up (28 of 43). Only 10 are under 1.2 yd.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Stone Plaque | 0.40 | small | `World\Generic\PassiveDoodads\Plaque\PlaqueStone01.m2` |
| Stone Club | 0.43 | small | `World\Generic\PassiveDoodads\Weapons\stone_club01.m2` |
| Tombstone | 0.75 | medium | `World\Azeroth\Duskwood\PassiveDoodads\Tombs\TombStone08.m2` |
| Stone Frame | 0.85 | medium | `World\Azeroth\Duskwood\PassiveDoodads\StoneFrames\DuskWoodStoneFrameShort.m2` |
| Stones of Binding | 1.14 | medium | `World\Goober\G_StonesOfBinding.m2` |
| Stone Pyre | 2.66 | large | `World\Azeroth\Elwynn\PassiveDoodads\StonePyre\StonePyre.m2` |
| Gem Rock | 3.07 | huge | `World\Generic\ActiveDoodads\Rocks\GemRock01.m2` |
| Hearthstone | 5.63 | huge | `World\Generic\PassiveDoodads\BindStone\NewBindStone.m2` |
| Meeting Stone | 7.67 | huge | `World\Generic\ActiveDoodads\MeetingStones\Meetingstone04.m2` |
| Gray Stone | 8.20 | massive | `World\Generic\PassiveDoodads\GrayStone\GrayStone02.m2` |
| Monument | 9.19 | massive | `World\Azeroth\Elwynn\PassiveDoodads\MonumentRock\MonumentRock.m2` |
| Shaman Stone | 10.54 | massive | `World\Generic\PassiveDoodads\ShamanStone\ShamanStone01.m2` |
| Runestone | 10.86 | massive | `World\Generic\PassiveDoodads\PlainRune\RuneStone.m2` |
| Cliff Rock | 10.95 | massive | `World\Azeroth\Elwynn\PassiveDoodads\CliffRocks\ElwynnCliffRock02.m2` |

### Structures

80 models; radius 0.64–36.32 yd, median 5.47 yd.

Almost all 3–8 yd and up (ruins, bridges, gates, statues). Nothing under 0.6 yd. Use structures as landmarks, not as roll-up material.

| name | radius (yd) | tier | path |
|---|---|---|---|
| Well | 2.08 | large | `World\Generic\PassiveDoodads\Well\Well.m2` |
| Altar | 2.39 | large | `World\Generic\Human\Passive Doodads\Altars\Altar01.m2` |
| General Altar | 2.40 | large | `World\Generic\Human\Passive Doodads\Altars\GeneralAltar01.m2` |
| Distillery | 3.04 | huge | `World\Azeroth\Elwynn\Buildings\Distillery\Distillery.m2` |
| Trellis | 4.14 | huge | `World\Generic\Human\Passive Doodads\Arches\trellis02.m2` |
| Shack | 5.05 | huge | `World\Generic\Human\Passive Doodads\Shack\Shack.m2` |
| Gold Mine | 5.24 | huge | `World\Azeroth\Elwynn\Buildings\GoldMine\goldmine.m2` |
| General Chimney | 5.79 | huge | `World\Generic\Human\Passive Doodads\Chimneys\GeneralChimney01.m2` |
| Snow Fountain | 6.01 | huge | `World\Generic\Human\Passive Doodads\Fountains\SnowFountain.m2` |
| Duskwood Chimney | 6.07 | huge | `World\Azeroth\Duskwood\PassiveDoodads\Ruins\DuskwoodChimney01.m2` |
| Ruined Fountain | 7.12 | huge | `World\Azeroth\Elwynn\PassiveDoodads\RuinedFountain\RuinedFountain.m2` |
| Westfall Fountain | 7.12 | huge | `World\Azeroth\Westfall\PassiveDoodads\WestfallFountain\WestfallFountain.m2` |
| Stormwind Fountain | 7.12 | huge | `World\Generic\Human\Passive Doodads\Fountains\StormwindFountain01.m2` |
| Lion Statue | 9.34 | massive | `World\Azeroth\Elwynn\PassiveDoodads\Statue\LionStatue.m2` |
| Blacksmith's Forge | 13.77 | massive | `World\Azeroth\Elwynn\Buildings\BlackSmith\BlackSmith.m2` |
| Footbridge | 13.77 | massive | `World\Azeroth\Elwynn\Buildings\Bridges\ElwynnFootbridge.m2` |
| Small Dock | 16.36 | massive | `World\Azeroth\Elwynn\PassiveDoodads\SmallDock\SmallDock.m2` |
| Guard Tower | 20.42 | massive | `World\Azeroth\Elwynn\Buildings\HumanGuardTower\HumanGuardTower.m2` |
| Watch Tower | 24.99 | massive | `World\Azeroth\Elwynn\Buildings\HumanWatchTower\HumanWatchTower.m2` |
