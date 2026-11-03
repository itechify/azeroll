//! Readable names for map doodads, shown on the HUD when the ball rolls one up. Vanilla model
//! stems are run-together words (`armorbracerblue`), so [`display_name`] looks the stem up in a
//! table, and segments unknown stems over a small word list before giving up and capitalising.

/// Stem → display name, sorted by stem for binary search. Keys are lowercase stems with trailing
/// digits and `_`/`-` removed, so `elwynnpine01` is looked up as `elwynnpine`.
static NAMES: &[(&str, &str)] = &[
    ("aeriepeaksbush", "Aerie Bush"),
    ("aeriepeaksrockoutcrop", "Rock Outcrop"),
    ("aeriepeaksstump", "Tree Stump"),
    ("aerietree", "Aerie Tree"),
    ("alteracfruitbucket", "Fruit Bucket"),
    ("alteractree", "Alterac Tree"),
    ("anvil", "Anvil"),
    ("arathirock", "Arathi Rock"),
    ("arathistump", "Arathi Stump"),
    ("arathitree", "Arathi Tree"),
    ("armorbracerblue", "Blue Bracers"),
    ("ashenvalerock", "Ashenvale Rock"),
    ("ashenvaleroots", "Ashenvale Roots"),
    ("ashenvalestump", "Ashenvale Stump"),
    ("ashenvaletree", "Ashenvale Tree"),
    ("ashenvaletreestump", "Ashenvale Stump"),
    ("ashenvalewisps", "Wisps"),
    ("aurablue", "Blue Aura"),
    ("aurablueshort", "Short Blue Aura"),
    ("aurablueverytall", "Tall Blue Aura"),
    ("auragreen", "Green Aura"),
    ("auragreenshort", "Short Green Aura"),
    ("auragreentall", "Tall Green Aura"),
    ("aurapurple", "Purple Aura"),
    ("aurapurpleshort", "Short Purple Aura"),
    ("aurared", "Red Aura"),
    ("aurayellow", "Yellow Aura"),
    ("aurayellowshort", "Short Yellow Aura"),
    ("azr_tree", "Azshara Tree"),
    ("azshararock", "Azshara Rock"),
    ("baby_dw", "Baby Dwarf"),
    ("baby_gn", "Baby Gnome"),
    ("baby_hu", "Baby Human"),
    ("baby_ne", "Baby Night Elf"),
    ("baby_or", "Baby Orc"),
    ("baby_ta", "Baby Tauren"),
    ("baby_tr", "Baby Troll"),
    ("badlandspalmtree", "Palm Tree"),
    ("badlandsrock", "Badlands Rock"),
    ("badlandssunkenwagon", "Sunken Wagon"),
    ("badlandstree", "Badlands Tree"),
    ("ballista", "Ballista"),
    ("ballistaruined", "Ruined Ballista"),
    ("banner", "Banner"),
    ("bannercentaur", "Centaur Banner"),
    ("bannerruined", "Ruined Banner"),
    ("barrel", "Barrel"),
    ("barrellowpoly", "Barrel"),
    ("barrensbush", "Barrens Bush"),
    ("barrensbustedwagon", "Busted Wagon"),
    ("barrenslamppost", "Lamppost"),
    ("barrenstree", "Barrens Tree"),
    ("barrenswagon", "Wagon"),
    ("battlefieldbanneralliance", "Alliance Banner"),
    ("battlefieldbannerhorde", "Horde Banner"),
    ("battlefieldbannerneutral", "Banner"),
    ("battlegladebanner", "Glade Banner"),
    ("battlegladeshield", "Glade Shield"),
    ("battlegladeskullhuman", "Human Skull"),
    ("battlegladeskullorc", "Orc Skull"),
    ("battlegladesword", "Glade Sword"),
    ("battlegladeswordskull", "Skull Sword"),
    ("battlegladewoodspikes", "Wooden Spikes"),
    ("bellow", "Bellows"),
    ("berrybush", "Berry Bush"),
    ("bigcandle", "Big Candle"),
    ("blacksmith", "Blacksmith's Forge"),
    ("blastedlandsrock", "Blasted Rock"),
    ("blastedlandstree", "Blasted Tree"),
    ("bleacherseat1a", "Bleacher Seat"),
    ("bleacherseat1b", "Bleacher Seat"),
    ("bleacherseat1c", "Bleacher Seat"),
    ("bleacherseat1d", "Bleacher Seat"),
    ("bleacherswood", "Wooden Bleachers"),
    ("bone", "Bone"),
    ("bone_bow", "Bone Bow"),
    ("bone_shield", "Bone Shield"),
    ("boneclub", "Bone Club"),
    ("bootybaytree", "Booty Bay Tree"),
    ("bowl", "Bowl"),
    ("bramblestaff", "Bramble Staff"),
    ("brokenbarrel", "Broken Barrel"),
    ("brokencart", "Broken Cart"),
    ("brokenfreezingtrap", "Broken Freezing Trap"),
    ("bubbles", "Bubbles"),
    ("burnedgypsywagon", "Burned Wagon"),
    ("burningash", "Burning Ash"),
    ("burningmidtree", "Burning Tree"),
    ("burningsteppesfence", "Burning Steppes Fence"),
    ("burningsteppesfencepost", "Burning Steppes Fence Post"),
    ("burningsteppestree", "Burning Steppes Tree"),
    ("burningwickerman", "Wicker Man"),
    ("burntstonetree", "Burnt Tree"),
    ("bush_arthastears", "Arthas' Tears"),
    ("bush_blacklotus", "Black Lotus"),
    ("bush_blindweed", "Blindweed"),
    ("bush_bruiseweed", "Bruiseweed"),
    ("bush_dreamfoil", "Dreamfoil"),
    ("bush_firebloom", "Firebloom"),
    ("bush_gravemoss", "Gravemoss"),
    ("bush_gromsblood", "Grom's Blood"),
    ("bush_icecap", "Icecap"),
    ("bush_liferoot", "Liferoot"),
    ("bush_magebloom", "Magebloom"),
    ("bush_mountainsilversage", "Silversage"),
    ("bush_mushroom", "Mushroom"),
    ("bush_peacebloom", "Peacebloom"),
    ("bush_plaguebloom", "Plaguebloom"),
    ("bush_purplelotus", "Purple Lotus"),
    ("bush_sansam", "Sansam"),
    ("bush_silverleaf", "Silverleaf"),
    ("bush_snakeroot", "Snakeroot"),
    ("bush_stardust", "Stardust"),
    ("bush_stranglekelp", "Stranglekelp"),
    ("bush_sungrass", "Sungrass"),
    ("bush_swiftthistle", "Swiftthistle"),
    ("bush_wintersbite", "Wintersbite"),
    ("butterflyorange", "Orange Butterfly"),
    ("butterflypurple", "Purple Butterfly"),
    ("butterflywhite", "White Butterfly"),
    ("cactusapple", "Cactus Apple"),
    ("candelabra", "Candelabra"),
    ("candelabratallwall", "Tall Candelabra"),
    ("candle", "Candle"),
    ("candleblack", "Black Candle"),
    ("candleoff", "Unlit Candle"),
    ("canopylesstree", "Bare Tree"),
    ("carni_wagon", "Carnival Wagon"),
    ("carni_wagon_empty", "Empty Carnival Wagon"),
    ("carnietent_small", "Small Tent"),
    ("carnival_banner", "Carnival Banner"),
    ("catapult", "Catapult"),
    ("cauldronempty", "Empty Cauldron"),
    ("cavekoboldbucket", "Bucket"),
    ("cavekobolddredge", "Dredge"),
    ("cavekoboldminemachine", "Mine Machine"),
    ("cavekoboldropecoil", "Rope Coil"),
    ("caveman_club", "Caveman Club"),
    ("caveminecar", "Mine Cart"),
    ("caveminecarwrecked", "Wrecked Mine Cart"),
    ("caveminepulley", "Mine Pulley"),
    ("caveminewheelbarrow", "Wheelbarrow"),
    ("cavewallbones", "Wall Bones"),
    ("chainedskeleton", "Chained Skeleton"),
    ("chandelier", "Chandelier"),
    ("chest", "Chest"),
    ("chestactive", "Chest"),
    ("cloth_level", "Cloth Bolt"),
    ("coffinlid", "Coffin Lid"),
    ("coketent", "Tent"),
    ("coraltree", "Coral Tree"),
    ("corn", "Corn"),
    ("corncrop", "Corn Crop"),
    ("corncropdead", "Dead Corn"),
    ("corncroprow", "Corn Row"),
    ("corncroprowlowpoly", "Corn Row"),
    ("crate", "Crate"),
    ("cratealliancefirstaid", "Alliance First Aid Crate"),
    ("cratehordefirstaid", "Horde First Aid Crate"),
    ("damwaterfall", "Dam Waterfall"),
    ("darkironkegshotgun", "Keg Shotgun"),
    ("darkportal", "Dark Portal"),
    ("darkshorebush", "Darkshore Bush"),
    ("darkshorelog", "Darkshore Log"),
    ("darkshorerock", "Darkshore Rock"),
    ("darkshoretree", "Darkshore Tree"),
    ("darkstreetlamp", "Street Lamp"),
    ("deadminecargoboxes", "Cargo Boxes"),
    ("deadminecargonetboxes", "Netted Cargo"),
    ("deadminedoor", "Deadmines Door"),
    ("deadminelumberpilelarge", "Large Lumber Pile"),
    ("deadminelumberpilesmall", "Small Lumber Pile"),
    ("deadtreelog", "Dead Log"),
    ("deadwindpassmidtree", "Dead Tree"),
    ("deadwindpassrocktree", "Rock Tree"),
    ("desolacerock", "Desolace Rock"),
    ("diremaul_banner", "Dire Maul Banner"),
    ("dirtmound", "Dirt Mound"),
    ("distillery", "Distillery"),
    ("dragonfootsoldierbones", "Dragonfoot Bones"),
    ("dragonmawbanner", "Dragonmaw Banner"),
    ("dreambellflower", "Dreambell Flower"),
    ("dreamdrippingflower", "Dripping Dreamflower"),
    ("dreamdroopingflower", "Drooping Dreamflower"),
    ("dreamorangeflower", "Orange Dreamflower"),
    ("dreampurpleflower", "Purple Dreamflower"),
    ("dreamspinningflower", "Spinning Dreamflower"),
    ("druidstone", "Druid Stone"),
    ("druidwisp", "Wisp"),
    ("duelingflag", "Dueling Flag"),
    ("durnholdebanner", "Durnholde Banner"),
    ("durotarbush", "Durotar Bush"),
    ("durotarcliffrock", "Durotar Cliff Rock"),
    ("durotarrock", "Durotar Rock"),
    ("durotartree", "Durotar Tree"),
    ("duskwoodbush", "Duskwood Bush"),
    ("duskwooddirectionalsign", "Duskwood Signpost"),
    ("duskwoodfencebottom", "Fence Bottom"),
    ("duskwoodfencepost", "Fence Post"),
    ("duskwoodfencerail", "Fence Rail"),
    ("duskwoodfencesegment", "Fence Segment"),
    ("duskwoodfencetop", "Fence Top"),
    ("duskwoodgraveframe", "Grave Frame"),
    ("duskwoodhaystack", "Haystack"),
    ("duskwoodhaywagon", "Hay Wagon"),
    ("duskwoodhearse", "Hearse"),
    ("duskwoodrockwall", "Rock Wall"),
    ("duskwoodspookybush", "Spooky Bush"),
    ("duskwoodspookytree", "Spooky Tree"),
    ("duskwoodstoneframelong", "Stone Frame"),
    ("duskwoodstoneframeshort", "Stone Frame"),
    ("duskwoodthornbush", "Thornbush"),
    ("duskwoodtree", "Duskwood Tree"),
    ("duskwoodtreestump", "Tree Stump"),
    ("duskwoodwheat", "Wheat"),
    ("dustwallowbush", "Dustwallow Bush"),
    ("dustwallowgroundfog", "Dust Cloud"),
    ("dustwallowtree", "Dustwallow Tree"),
    ("dustwestfall", "Dust"),
    ("dwarfbannerblue", "Blue Banner"),
    ("dwarfbannergold", "Gold Banner"),
    ("dwarfbannerred", "Red Banner"),
    ("dwarfbannersilver", "Silver Banner"),
    ("dwarffemaledeathskeleton", "Dwarf Skeleton"),
    ("dwarfmaledeathskeleton", "Dwarf Skeleton"),
    ("easteregg", "Easter Egg"),
    ("elwynnbush", "Elwynn Bush"),
    ("elwynncampfire", "Campfire"),
    ("elwynncatails", "Cattails"),
    ("elwynncliffrock", "Cliff Rock"),
    ("elwynndetailflowers", "Wildflowers"),
    ("elwynndetailgrass", "Tall Grass"),
    ("elwynndetailrock", "Small Rock"),
    ("elwynnfencesimple", "Fence"),
    ("elwynnfencesimplepost", "Fence Post"),
    ("elwynnfencetop", "Fence Rail"),
    ("elwynnfirtree", "Elwynn Fir Tree"),
    ("elwynnflower", "Elwynn Flower"),
    ("elwynnfootbridge", "Footbridge"),
    ("elwynngrass", "Grass"),
    ("elwynnlillypad", "Lily Pad"),
    ("elwynnlog", "Mossy Log"),
    ("elwynnmediumwaterfall", "Waterfall"),
    ("elwynnmelon", "Melon"),
    ("elwynnminecart", "Mine Cart"),
    ("elwynnmushroom", "Mushroom"),
    ("elwynnpine", "Elwynn Pine"),
    ("elwynnpoppy", "Poppy"),
    ("elwynnreeds", "Reeds"),
    ("elwynnrock", "Elwynn Rock"),
    ("elwynnseaweed", "Seaweed"),
    ("elwynnshrub", "Shrub"),
    ("elwynnstonefence", "Stone Fence"),
    ("elwynnstonefencepost", "Stone Fence Post"),
    ("elwynntallwaterfall", "Tall Waterfall"),
    ("elwynnthornbush", "Thornbush"),
    ("elwynntreecanopy", "Elwynn Canopy Tree"),
    ("elwynntreemid", "Elwynn Tree"),
    ("elwynntreestump", "Tree Stump"),
    ("elwynnvines", "Vines"),
    ("elwynnvineyard", "Vineyard"),
    ("elwynnwheat", "Wheat"),
    ("elwynnwoodfence", "Wooden Fence"),
    ("elwynnwoodpost", "Wooden Post"),
    ("emeralddreamfountaintree", "Dream Tree"),
    ("excavationcrane", "Excavation Crane"),
    ("excavationwaterwagon", "Water Wagon"),
    ("exploding_package", "Explosive Package"),
    ("eyeofazora", "Eye of Azora"),
    ("featherbigblue", "Blue Feather"),
    ("featherbigbrown", "Brown Feather"),
    ("featherbigred", "Red Feather"),
    ("felwoodbush", "Felwood Bush"),
    ("felwoodlamp", "Felwood Lamp"),
    ("felwoodroots", "Felwood Roots"),
    ("felwoodtree", "Felwood Tree"),
    ("felwoodtreestump", "Felwood Stump"),
    ("feralasstone", "Feralas Stone"),
    ("feralastree", "Feralas Tree"),
    ("feralastreestump", "Feralas Stump"),
    ("firecrackerstring_blue", "Blue Firecrackers"),
    ("firecrackerstring_red", "Red Firecrackers"),
    ("fishfountain", "Fish Fountain"),
    ("fishskel", "Fish Skeleton"),
    ("flamecircle", "Flame Circle"),
    ("flatrock", "Flat Rock"),
    ("flint_club", "Flint Club"),
    ("flint_spear", "Flint Spear"),
    ("flintspear", "Flint Spear"),
    ("floatingbarrel", "Floating Barrel"),
    ("floatingboards", "Floating Boards"),
    ("floatingboardsburning", "Burning Boards"),
    ("floatingcoffin", "Floating Coffin"),
    ("floatingcrate", "Floating Crate"),
    ("floatingrocklarge", "Floating Rock"),
    ("floatingrockmedium", "Floating Rock"),
    ("floatingrocksmall", "Floating Rock"),
    ("floatingtree", "Floating Tree"),
    ("flyingsmallinsects", "Insects"),
    ("flyswarms", "Fly Swarm"),
    ("fogbox", "Fog"),
    ("food_level", "Food"),
    ("foodherbs_level", "Herbs"),
    ("foodvendor", "Food Stall"),
    ("footbridge", "Footbridge"),
    ("forsakenbanner", "Forsaken Banner"),
    ("fountainparticles", "Fountain Spray"),
    ("freestandingtorch", "Torch"),
    ("freestandingtorch04_giant", "Giant Torch"),
    ("freestandingtorch04_huge", "Huge Torch"),
    ("freestandingtorch04_hugeblue", "Huge Blue Torch"),
    ("fruit_apple", "Apple"),
    ("fruit_banana", "Banana"),
    ("fruit_bananabunch", "Banana Bunch"),
    ("fruit_melon", "Melon"),
    ("fruit_melonslice", "Melon Slice"),
    ("fruit_orange", "Orange"),
    ("fruit_orangeslice", "Orange Slice"),
    ("fruit_pear", "Pear"),
    ("fruitbowl_apples", "Bowl of Apples"),
    ("fruitbowl_empty", "Empty Fruit Bowl"),
    ("fruitbowl_mixed", "Fruit Bowl"),
    ("furbolgtent", "Furbolg Tent"),
    ("g_alchemyset", "Alchemy Set"),
    ("g_applebob", "Apple Bobbing Barrel"),
    ("g_barrel", "Barrel"),
    ("g_barrelexplode", "Explosive Barrel"),
    ("g_bellship", "Ship Bell"),
    ("g_bombwagon", "Bomb Wagon"),
    ("g_bonfire", "Bonfire"),
    ("g_book", "Book"),
    ("g_bookopenmediumblack", "Black Book"),
    ("g_bookopenmediumblue", "Blue Book"),
    ("g_bookopenmediumbrown", "Brown Book"),
    ("g_bookopenmediumgreen", "Green Book"),
    ("g_bookopenmediumred", "Red Book"),
    ("g_brazier", "Brazier"),
    ("g_brazierorcblue", "Blue Brazier"),
    ("g_brazierorcgreen", "Green Brazier"),
    ("g_brazierorcpurple", "Purple Brazier"),
    ("g_brazierorcred", "Red Brazier"),
    ("g_braziertroll", "Troll Brazier"),
    ("g_buttonbigred", "Button"),
    ("g_cage", "Cage"),
    ("g_cagebase", "Cage"),
    ("g_cagedoorbamboo", "Bamboo Cage Door"),
    ("g_cannon", "Cannon"),
    ("g_chestahnqiraj", "Ahn'Qiraj Chest"),
    ("g_chesttitan", "Titan Chest"),
    ("g_cornucopia", "Cornucopia"),
    ("g_crate", "Crate"),
    ("g_crateanimal", "Animal Crate"),
    ("g_dragoneggblack", "Dragon Egg"),
    ("g_dwarvenmemorial", "Dwarven Memorial"),
    ("g_eggalien", "Alien Egg"),
    ("g_eggspider", "Spider Egg"),
    ("g_epicbrazierblue", "Blue Brazier"),
    ("g_epicbrazieryellow", "Golden Brazier"),
    ("g_essencedistiller", "Essence Distiller"),
    ("g_explodingballista", "Ballista"),
    ("g_firework01blue", "Blue Firework"),
    ("g_fireworklauncher", "Firework Launcher"),
    ("g_fishingbobber", "Fishing Bobber"),
    ("g_flyingmachine", "Flying Machine"),
    ("g_ghost", "Ghost"),
    ("g_gongrfd", "Gong"),
    ("g_gongtroll", "Gong"),
    ("g_holylightwell", "Holy Well"),
    ("g_indiancorn_basket", "Basket of Corn"),
    ("g_jewelblack", "Black Jewel"),
    ("g_jewelblue", "Blue Jewel"),
    ("g_jewelred", "Red Jewel"),
    ("g_keg", "Keg"),
    ("g_kruskspear", "Kruk Spear"),
    ("g_levermetal", "Lever"),
    ("g_minecar", "Mine Cart"),
    ("g_mortar", "Mortar"),
    ("g_netablet", "Stone Tablet"),
    ("g_nutcracker", "Nutcracker"),
    ("g_pilgrimhat", "Pilgrim Hat"),
    ("g_pulsatingplant", "Pulsating Plant"),
    ("g_pumpkin", "Pumpkin"),
    ("g_pumpkinwhat", "Pumpkin"),
    ("g_relicnecat", "Relic"),
    ("g_relicnecup", "Relic"),
    ("g_relicnefigurine", "Relic Figurine"),
    ("g_relicnesphere", "Relic Sphere"),
    ("g_runeblue", "Blue Rune"),
    ("g_scourgerunecirclecrystal", "Rune Crystal"),
    ("g_scroll", "Scroll"),
    ("g_scryingbowl", "Scrying Bowl"),
    ("g_soundportal", "Sound Portal"),
    ("g_sporemushroom", "Spore Mushroom"),
    ("g_squash", "Squash"),
    ("g_squashgreen", "Green Squash"),
    ("g_squashlong", "Long Squash"),
    ("g_stonesofbinding", "Stones of Binding"),
    ("g_tentburninator", "Tent"),
    ("g_torch", "Torch"),
    ("g_torchlever", "Torch Lever"),
    ("g_uldamanmap", "Uldaman Map"),
    ("g_watermelon", "Watermelon"),
    ("g_witchbroom", "Witch Broom"),
    ("g_witchhat", "Witch Hat"),
    ("g_xmastree", "Holiday Tree"),
    ("g_xmaswreath", "Yuletide Wreath"),
    ("gallows", "Gallows"),
    ("gardentreecanopy", "Garden Canopy Tree"),
    ("gardentreehuge", "Huge Garden Tree"),
    ("gemminecar", "Mine Cart"),
    ("gemrock", "Gem Rock"),
    ("generalbook", "Book"),
    ("generalchairloend", "Chair"),
    ("generallantern", "Lantern"),
    ("generaltorch", "Torch"),
    ("generalwoodendummy", "Wooden Dummy"),
    ("genericsmallsignpost", "Signpost"),
    ("genericwroughtgate", "Wrought Gate"),
    ("giantclamactive", "Giant Clam"),
    ("gnolltent", "Gnoll Tent"),
    ("gnomeregandoor", "Gate"),
    ("goblinmeltingpot", "Goblin Melting Pot"),
    ("goblinrocketcart", "Rocket Cart"),
    ("goblintent", "Goblin Tent"),
    ("goblinwagontnt", "TNT Wagon"),
    ("goldgoblet", "Golden Goblet"),
    ("goldgobletfilled", "Filled Goblet"),
    ("goldmine", "Gold Mine"),
    ("goldminetrackcurve", "Curved Mine Tracks"),
    ("goldminetracks", "Mine Tracks"),
    ("goldpilelarge", "Large Gold Pile"),
    ("goldpilemedium", "Gold Pile"),
    ("goldpilesmall", "Small Gold Pile"),
    ("goldshiredirectionalsign", "Goldshire Signpost"),
    ("grapebucket", "Grape Bucket"),
    ("graystone", "Gray Stone"),
    ("greenbottle", "Green Bottle"),
    ("greenbubblingcauldron", "Bubbling Cauldron"),
    ("greengroundfog", "Green Fog"),
    ("greygroundfog", "Gray Fog"),
    ("griffonmemorialtree", "Memorial Tree"),
    ("grindstone", "Grindstone"),
    ("gryphonroost_ruined", "Ruined Gryphon Roost"),
    ("gypsywagon", "Gypsy Wagon"),
    ("hammerquest", "Hammer"),
    ("hangingskulllight", "Hanging Skull Lamp"),
    ("haybail", "Hay Bale"),
    ("haystack", "Haystack"),
    ("herbs_level", "Herbs"),
    ("hidestack", "Hide Stack"),
    ("holyspringwell", "Holy Well"),
    ("horde_cloth", "Cloth Bolt"),
    ("horde_food", "Food"),
    ("horde_furs", "Furs"),
    ("horde_herbs", "Herbs"),
    ("horde_metal", "Metal Ingots"),
    ("hordemaptable", "Map Table"),
    ("housesmoke", "Chimney Smoke"),
    ("humanalchemistsign", "Alchemist Sign"),
    ("humanbaitandtacklesign", "Tackle Sign"),
    ("humanbanksign", "Bank Sign"),
    ("humanblacksmithsign", "Blacksmith Sign"),
    ("humanboathousesign", "Boathouse Sign"),
    ("humanfemaledeathskeleton", "Human Skeleton"),
    ("humanguardtower", "Guard Tower"),
    ("humaninnsign", "Inn Sign"),
    ("humaninnsignpost", "Inn Signpost"),
    ("humanmagicshopsign", "Magic Shop Sign"),
    ("humanmaledeathskeleton", "Human Skeleton"),
    ("humantannersign", "Tanner Sign"),
    ("humantentlarge", "Large Tent"),
    ("humantentmedium", "Tent"),
    ("humanwatchtower", "Watch Tower"),
    ("hyjalrock", "Hyjal Rock"),
    ("hyjaltree", "Hyjal Tree"),
    ("innbarrel", "Inn Barrel"),
    ("innlantern", "Inn Lantern"),
    ("inntabletiny", "Inn Table"),
    ("instanceportal", "Portal"),
    ("ironforgebush", "Ironforge Bush"),
    ("ironforgesignpost", "Ironforge Signpost"),
    ("ironforgesignpostpointer", "Signpost"),
    ("ironforgesnowtree", "Snowy Tree"),
    ("ironforgesnowybush", "Snowy Bush"),
    ("ironforgestatue", "Statue"),
    ("ironforgetree", "Ironforge Tree"),
    ("jar", "Jar"),
    ("jug", "Jug"),
    ("knotted_club", "Knotted Club"),
    ("kodowagon", "Kodo Wagon"),
    ("lamppost", "Lamppost"),
    ("landminealteracvalley", "Landmine"),
    ("leather_level", "Leather"),
    ("leather_shield", "Leather Shield"),
    ("lichcontainer", "Lich Container"),
    ("lightskeletonlaying", "Skeleton"),
    ("lightskeletonsitting", "Seated Skeleton"),
    ("lionstatue", "Lion Statue"),
    ("lochlog", "Loch Modan Log"),
    ("losttreehuts", "Tree Huts"),
    ("luckymoneyenvelope", "Lucky Envelope"),
    ("lunarnewyear_lights", "Lunar Lights"),
    ("lunarnewyear_lightsx", "Lunar Lights"),
    ("lunarnewyearbanner_alliance_hanging", "Alliance Banner"),
    ("lunarnewyearbanner_alliance_standing", "Alliance Banner"),
    ("lunarnewyearbanner_horde_hanging", "Horde Banner"),
    ("lunarnewyearbanner_horde_standing", "Horde Banner"),
    ("lunarnewyearlantern_alliance_hanging", "Lantern"),
    ("lunarnewyearlantern_alliance_standing", "Lantern"),
    ("lunarnewyearlantern_horde_hanging", "Lantern"),
    ("lunarnewyearlantern_horde_standing", "Lantern"),
    ("mageportal_darnassus", "Portal to Darnassus"),
    ("mageportal_ironforge", "Portal to Ironforge"),
    ("mageportal_ogrimmar", "Portal to Orgrimmar"),
    ("mageportal_stormwind", "Portal to Stormwind"),
    ("mageportal_thunderbluff", "Portal to Thunder Bluff"),
    ("mageportal_undercity", "Portal to Undercity"),
    ("meetingstone", "Meeting Stone"),
    ("metalbars01copper", "Copper Bars"),
    ("metalbars01iron", "Iron Bars"),
    ("metalbars01mithril", "Mithril Bars"),
    ("metalbars01truesilver", "Truesilver Bars"),
    ("metalbarstack01copper", "Copper Bar Stack"),
    ("metalbarstack01iron", "Iron Bar Stack"),
    ("metalbarstack01mithril", "Mithril Bar Stack"),
    ("metalbarstack01truesilver", "Truesilver Bar Stack"),
    ("metalbarstack02copper", "Copper Bar Stack"),
    ("metalbarstack02iron", "Iron Bar Stack"),
    ("metalbarstack02mithril", "Mithril Bar Stack"),
    ("metalbarstack02truesilver", "Truesilver Bar Stack"),
    ("metals_level", "Ore"),
    ("mininglamp", "Mining Lamp"),
    ("miningsifter", "Mining Sifter"),
    ("miningtroughlarge", "Large Mining Trough"),
    ("miningtroughsmall", "Small Mining Trough"),
    ("mistletoe", "Mistletoe"),
    ("monestarybossdoor", "Monastery Boss Door"),
    ("monestaryhalldoor", "Monastery Door"),
    ("monestarysecretdoor", "Secret Monastery Door"),
    ("monumentrock", "Monument"),
    ("moongladebush", "Moonglade Bush"),
    ("moongladetree", "Moonglade Tree"),
    ("mug", "Mug"),
    ("mugfoam", "Frothy Mug"),
    ("naga_trident", "Naga Trident"),
    ("newbindstone", "Hearthstone"),
    ("newwaterfall", "Waterfall"),
    ("northshireabbeybust", "Abbey Bust"),
    ("northshiredirectionalsign", "Northshire Signpost"),
    ("nsabbeybell", "Abbey Bell"),
    ("ogremound", "Ogre Mound"),
    ("orangegroundfog", "Orange Fog"),
    ("orcfemaledeathskeleton", "Orc Skeleton"),
    ("orcmaledeathskeleton", "Orc Skeleton"),
    ("orcmug", "Orc Mug"),
    ("orcsignpostpointer", "Orc Signpost"),
    ("outlanddemonglow", "Demon Glow"),
    ("pick", "Pick"),
    ("plainrune", "Rune"),
    ("plaquebronze", "Bronze Plaque"),
    ("plaquesilver", "Silver Plaque"),
    ("plaquestone", "Stone Plaque"),
    ("portcullisactive", "Portcullis"),
    ("postboxdwarf", "Mailbox"),
    ("postboxgnome", "Mailbox"),
    ("postboxhuman", "Mailbox"),
    ("postboxnightelf", "Mailbox"),
    ("postboxorc", "Mailbox"),
    ("postboxtauren", "Mailbox"),
    ("postboxtroll", "Mailbox"),
    ("postboxundead", "Mailbox"),
    ("potbellystove", "Potbelly Stove"),
    ("potbellystovepipe", "Stove Pipe"),
    ("potbellystovewall", "Wall Stove"),
    ("prisoncell", "Prison Cell"),
    ("pulley", "Pulley"),
    ("pumpkinpatch", "Pumpkin Patch"),
    ("rackshieldalliance", "Alliance Shield Rack"),
    ("rackshieldhorde", "Horde Shield Rack"),
    ("redridgebush", "Redridge Bush"),
    ("redridgefallentree", "Fallen Tree"),
    ("redridgerock", "Redridge Rock"),
    ("redridgestump", "Redridge Stump"),
    ("redridgetree", "Redridge Tree"),
    ("replacecrate", "Crate"),
    ("roundtable", "Round Table"),
    ("ruinedcatapult", "Ruined Catapult"),
    ("ruinedfountain", "Ruined Fountain"),
    ("runestone", "Runestone"),
    ("runestonedirectionalsign", "Runestone Signpost"),
    ("sack", "Sack"),
    ("scarletcathedraldoor", "Cathedral Door"),
    ("scourgefemaledeathskeleton", "Scourge Skeleton"),
    ("scourgemaledeathskeleton", "Scourge Skeleton"),
    ("shadowfangdoor", "Shadowfang Door"),
    ("shadowfangfog", "Fog"),
    ("shamanstone", "Shaman Stone"),
    ("shipramp", "Ship Ramp"),
    ("shovel", "Shovel"),
    ("shrineaurablue", "Aura Shrine"),
    ("silverpinebush", "Silverpine Bush"),
    ("silverpinetree", "Silverpine Tree"),
    ("skeletalservant", "Skeletal Servant"),
    ("skullclub", "Skull Club"),
    ("skullspear", "Skull Spear"),
    ("smalldock", "Small Dock"),
    ("snowballmound", "Snowball Pile"),
    ("snowdunmorogh", "Snow Drift"),
    ("spike", "Spike"),
    ("spiked_club", "Spiked Club"),
    ("spring", "Spring"),
    ("steam", "Steam"),
    ("steelspear", "Steel Spear"),
    ("stein", "Stein"),
    ("stone_club", "Stone Club"),
    ("stone_spear", "Stone Spear"),
    ("stonepyre", "Stone Pyre"),
    ("stormwindarcherytarget", "Archery Target"),
    ("stormwinddirectionalsign", "Stormwind Signpost"),
    ("stormwindgate", "Stormwind Gate"),
    ("stormwindgriffonbanner", "Griffon Banner"),
    ("stormwindmagebanner", "Mage Banner"),
    ("stormwindmageportal", "Portal to Stormwind"),
    ("stormwindplanter", "Planter"),
    ("stormwindvendorawning", "Vendor Awning"),
    ("stormwindwindowplantera", "Window Planter"),
    ("stormwindwindowplanterb", "Window Planter"),
    ("stormwindwoodendummy", "Wooden Dummy"),
    ("stranglethorntree", "Stranglethorn Tree"),
    ("summerfest_bonfire_large", "Large Bonfire"),
    ("summerfest_bonfire_small", "Small Bonfire"),
    ("summerfest_brazier", "Brazier"),
    ("summerfest_crate", "Festival Crate"),
    ("summerfest_maypole", "Maypole"),
    ("summerfest_ribbon", "Festival Ribbon"),
    ("summerfest_streamers", "Festival Streamers"),
    ("summerfest_wreath", "Festival Wreath"),
    ("summon_ritual", "Summoning Circle"),
    ("sunkenflame", "Flame"),
    ("swampofsorrowlilypad", "Lily Pad"),
    ("swampplant", "Swamp Plant"),
    ("swampskulls", "Skulls"),
    ("tanarisrock", "Tanaris Rock"),
    ("taurenfemaledeathskeleton", "Tauren Skeleton"),
    ("taurenmaledeathskeleton", "Tauren Skeleton"),
    ("taurensignpostpointer", "Tauren Signpost"),
    ("temporal_displacement", "Time Rift"),
    ("tigerskull", "Tiger Skull"),
    ("titanchest", "Titan Chest"),
    ("titanvase", "Titan Vase"),
    ("tombstonemonument", "Tombstone"),
    ("torch", "Torch"),
    ("torch_out", "Unlit Torch"),
    ("transportship_sails", "Ship Sails"),
    ("trollchest", "Troll Chest"),
    ("trollfemaledeathskeleton", "Troll Skeleton"),
    ("trollmaledeathskeleton", "Troll Skeleton"),
    ("uldamandoor", "Uldaman Door"),
    ("undeadwroughtgate", "Undead Gate"),
    ("valentines_blanket", "Blanket"),
    ("valentinescandle", "Valentine Candle"),
    ("valentineschocolate", "Chocolate"),
    ("valentinescolognebottle", "Cologne Bottle"),
    ("valentinescrate", "Valentine Crate"),
    ("valentinesperfumebottle", "Perfume Bottle"),
    ("valentinesplant", "Valentine Plant"),
    ("valentinesribbon", "Valentine Ribbon"),
    ("valentinesstreamers", "Valentine Streamers"),
    ("valentineswreath", "Valentine Wreath"),
    ("valentineswreathfloating", "Floating Wreath"),
    ("valentineswreathhanging", "Hanging Wreath"),
    ("vineyardcube", "Vineyard Crate"),
    ("vineyardlowpoly", "Vineyard"),
    ("vineyardwired", "Vineyard Trellis"),
    ("waterbasin", "Water Basin"),
    ("waterfall-long", "Long Waterfall"),
    ("weaponcratealliancesword", "Alliance Sword Crate"),
    ("weaponcratehordeaxe", "Horde Axe Crate"),
    ("well", "Well"),
    ("westfallbed", "Westfall Bed"),
    ("westfallbush", "Westfall Bush"),
    ("westfallcabbage", "Cabbage"),
    ("westfalldirectionalsign", "Westfall Signpost"),
    ("westfallfence", "Westfall Fence"),
    ("westfallfencepost", "Westfall Fence Post"),
    ("westfallscarecrow", "Scarecrow"),
    ("westfallshed", "Shed"),
    ("westfalltree", "Westfall Tree"),
    ("westfallwindmill", "Windmill"),
    ("wetlandgrass", "Grass"),
    ("wetlandstree", "Wetlands Tree"),
    ("wicker_bow", "Wicker Bow"),
    ("wicker_shield", "Wicker Shield"),
    ("windowbeam", "Window Beam"),
    ("winterspringbush", "Winterspring Bush"),
    ("winterspringrock", "Winterspring Rock"),
    ("winterspringtree", "Winterspring Tree"),
    ("wizardssphere", "Wizard's Sphere"),
    ("woodclub", "Wooden Club"),
    ("woodcross", "Wooden Cross"),
    ("woodsignpostnice", "Signpost"),
    ("worldtreeportal", "World Tree Portal"),
    ("wreckedrowboat", "Wrecked Rowboat"),
    ("wroughtirondoor", "Wrought Iron Door"),
    ("xmas_lights", "Holiday Lights"),
    ("xmasgift", "Gift"),
    ("xmasropeline", "Rope Line"),
    ("xmasstocking", "Stocking"),
    ("xmastree_largealliance", "Alliance Holiday Tree"),
    ("xmastree_largehorde", "Horde Holiday Tree"),
    ("xmastree_mediumalliance", "Holiday Tree"),
    ("xmastree_mediumhorde", "Holiday Tree"),
];

/// What a word in [`WORDS`] contributes to a segmented name.
#[derive(Clone, Copy)]
enum Role {
    /// Moves in front of the nouns: `armorbracerblue` reads "Blue Bracers".
    Colour,
    /// A WoW zone. Kept only in front of scenery ("Elwynn Tree") or when it is the whole name;
    /// before anything else it is dropped, so `westfallcabbage` reads "Cabbage".
    Zone,
    /// Meaningless as a prefix ("General", "Generic", "Human", "Armor"): dropped when a word
    /// follows.
    Filler,
    /// A plant or rock a zone can flavour, so a zone before it is kept.
    Scenery,
    /// Shown capitalised, as spelled.
    Noun,
    /// Shown as given, for a word whose plural reads better, e.g. "Bracers".
    Plural(&'static str),
}

use Role::{Colour, Filler, Noun, Plural, Scenery, Zone};

/// Words for segmenting a stem the table does not cover, sorted. Stems are lowercase and
/// unseparated, so every word has to be listed in its lowercase spelling.
static WORDS: &[(&str, Role)] = &[
    ("aerie", Zone),
    ("alliance", Noun),
    ("altar", Noun),
    ("alterac", Zone),
    ("amethyst", Noun),
    ("amulet", Noun),
    ("animal", Noun),
    ("anvil", Noun),
    ("apple", Noun),
    ("arathi", Zone),
    ("arch", Noun),
    ("armor", Filler),
    ("ash", Noun),
    ("ashenvale", Zone),
    ("azshara", Zone),
    ("badlands", Zone),
    ("bag", Noun),
    ("banner", Noun),
    ("barrel", Noun),
    ("barrens", Zone),
    ("basin", Noun),
    ("basket", Noun),
    ("bell", Noun),
    ("belt", Noun),
    ("bench", Noun),
    ("big", Noun),
    ("birch", Noun),
    ("black", Colour),
    ("blanket", Noun),
    ("blastedlands", Zone),
    ("blue", Colour),
    ("board", Noun),
    ("boards", Noun),
    ("boat", Noun),
    ("bolt", Noun),
    ("bone", Noun),
    ("book", Noun),
    ("bookshelf", Noun),
    ("boots", Noun),
    ("bottle", Noun),
    ("bow", Noun),
    ("bowl", Noun),
    ("box", Noun),
    ("bracer", Plural("Bracers")),
    ("brass", Noun),
    ("brazier", Noun),
    ("bread", Noun),
    ("bridge", Noun),
    ("broken", Noun),
    ("bronze", Noun),
    ("brown", Colour),
    ("bucket", Noun),
    ("burned", Noun),
    ("burning", Noun),
    ("burningsteppes", Zone),
    ("burnt", Noun),
    ("bush", Scenery),
    ("cabbage", Noun),
    ("cabin", Noun),
    ("cage", Noun),
    ("candle", Noun),
    ("candy", Noun),
    ("cannon", Noun),
    ("canopy", Noun),
    ("cape", Noun),
    ("carrot", Noun),
    ("cart", Noun),
    ("castle", Noun),
    ("cattails", Noun),
    ("cauldron", Noun),
    ("ceramic", Noun),
    ("chain", Noun),
    ("chair", Noun),
    ("cheese", Noun),
    ("chest", Noun),
    ("chimney", Noun),
    ("circle", Noun),
    ("clay", Noun),
    ("cliff", Noun),
    ("cloth", Noun),
    ("club", Noun),
    ("coffin", Noun),
    ("coin", Noun),
    ("copper", Noun),
    ("corn", Noun),
    ("crate", Noun),
    ("crimson", Colour),
    ("crystal", Noun),
    ("cup", Noun),
    ("darkshore", Zone),
    ("darnassus", Zone),
    ("dead", Noun),
    ("deadwind", Zone),
    ("desolace", Zone),
    ("door", Noun),
    ("dragon", Noun),
    ("dream", Noun),
    ("dunmorogh", Zone),
    ("durotar", Zone),
    ("duskwood", Zone),
    ("dust", Noun),
    ("dustwallow", Zone),
    ("dwarf", Noun),
    ("egg", Noun),
    ("elf", Noun),
    ("elwynn", Zone),
    ("eversong", Zone),
    ("fall", Noun),
    ("farm", Noun),
    ("felwood", Zone),
    ("fence", Noun),
    ("feralas", Zone),
    ("fern", Noun),
    ("fire", Noun),
    ("firework", Noun),
    ("fish", Noun),
    ("flag", Noun),
    ("flame", Noun),
    ("flower", Noun),
    ("fog", Noun),
    ("food", Noun),
    ("forest", Noun),
    ("fountain", Noun),
    ("fruit", Noun),
    ("furs", Noun),
    ("gate", Noun),
    ("gem", Noun),
    ("general", Filler),
    ("generic", Filler),
    ("gift", Noun),
    ("glass", Noun),
    ("gloves", Noun),
    ("glow", Noun),
    ("gnome", Noun),
    ("goblet", Noun),
    ("goblin", Noun),
    ("gold", Colour),
    ("golden", Colour),
    ("grass", Noun),
    ("grave", Noun),
    ("gravestone", Noun),
    ("gray", Colour),
    ("green", Colour),
    ("grindstone", Noun),
    ("ground", Noun),
    ("gypsy", Noun),
    ("hammer", Noun),
    ("hay", Noun),
    ("haybail", Noun),
    ("haystack", Noun),
    ("helm", Noun),
    ("helmet", Noun),
    ("herb", Noun),
    ("herbs", Noun),
    ("hide", Noun),
    ("hillsbrad", Zone),
    ("hoe", Noun),
    ("holy", Noun),
    ("hook", Noun),
    ("horde", Noun),
    ("house", Noun),
    ("human", Filler),
    ("hut", Noun),
    ("iron", Noun),
    ("ironforge", Zone),
    ("jar", Noun),
    ("jewel", Noun),
    ("jug", Noun),
    ("keg", Noun),
    ("kettle", Noun),
    ("key", Noun),
    ("knot", Noun),
    ("lamp", Noun),
    ("lamppost", Noun),
    ("lantern", Noun),
    ("leather", Noun),
    ("level", Noun),
    ("light", Noun),
    ("lights", Noun),
    ("lily", Noun),
    ("loch", Zone),
    ("lock", Noun),
    ("log", Noun),
    ("mail", Noun),
    ("map", Noun),
    ("meat", Noun),
    ("melon", Noun),
    ("metal", Noun),
    ("mine", Noun),
    ("mithril", Noun),
    ("moonglade", Zone),
    ("moss", Noun),
    ("mug", Noun),
    ("mulgore", Zone),
    ("mushroom", Noun),
    ("night", Noun),
    ("oak", Noun),
    ("onion", Noun),
    ("orange", Noun),
    ("orc", Noun),
    ("ore", Noun),
    ("orgrimmar", Zone),
    ("owl", Noun),
    ("pad", Noun),
    ("palm", Noun),
    ("pan", Noun),
    ("paper", Noun),
    ("pear", Noun),
    ("pepper", Noun),
    ("pick", Noun),
    ("pie", Noun),
    ("pile", Noun),
    ("pillar", Noun),
    ("pine", Noun),
    ("pink", Colour),
    ("pitcher", Noun),
    ("pitchfork", Noun),
    ("plant", Noun),
    ("plaque", Noun),
    ("plate", Noun),
    ("pole", Noun),
    ("post", Noun),
    ("pot", Noun),
    ("potato", Noun),
    ("pouch", Noun),
    ("pulley", Noun),
    ("pumpkin", Noun),
    ("purple", Colour),
    ("rack", Noun),
    ("rake", Noun),
    ("red", Colour),
    ("redridge", Zone),
    ("reed", Noun),
    ("relic", Noun),
    ("ribbon", Noun),
    ("ring", Noun),
    ("robe", Noun),
    ("rock", Scenery),
    ("roots", Noun),
    ("rope", Noun),
    ("round", Noun),
    ("ruined", Noun),
    ("rune", Noun),
    ("runestone", Noun),
    ("sack", Noun),
    ("sausage", Noun),
    ("scroll", Noun),
    ("shack", Noun),
    ("shadow", Noun),
    ("shelf", Noun),
    ("shield", Noun),
    ("ship", Noun),
    ("shirt", Noun),
    ("shovel", Noun),
    ("shrub", Noun),
    ("sign", Noun),
    ("signpost", Noun),
    ("silithus", Zone),
    ("silver", Colour),
    ("silverpine", Zone),
    ("skeleton", Noun),
    ("skull", Noun),
    ("small", Noun),
    ("snow", Noun),
    ("spear", Noun),
    ("spike", Noun),
    ("spoon", Noun),
    ("spring", Noun),
    ("stables", Noun),
    ("stair", Noun),
    ("statue", Noun),
    ("steam", Noun),
    ("steel", Noun),
    ("stein", Noun),
    ("stone", Noun),
    ("stonetalon", Zone),
    ("stool", Noun),
    ("storm", Noun),
    ("stormwind", Zone),
    ("stranglethorn", Zone),
    ("straw", Noun),
    ("street", Noun),
    ("stump", Scenery),
    ("sword", Noun),
    ("table", Noun),
    ("tablet", Noun),
    ("tall", Noun),
    ("tanaris", Zone),
    ("tauren", Noun),
    ("teldrassil", Zone),
    ("tent", Noun),
    ("thorn", Noun),
    ("thornbush", Noun),
    ("thousandneedles", Zone),
    ("thunderbluff", Zone),
    ("tirisfal", Zone),
    ("tombstone", Noun),
    ("torch", Noun),
    ("totem", Noun),
    ("tower", Noun),
    ("tree", Scenery),
    ("trellis", Noun),
    ("troll", Noun),
    ("trough", Noun),
    ("tunnel", Noun),
    ("turnip", Noun),
    ("undead", Noun),
    ("undercity", Zone),
    ("urn", Noun),
    ("vase", Noun),
    ("vine", Noun),
    ("vines", Noun),
    ("wagon", Noun),
    ("wall", Noun),
    ("watch", Noun),
    ("water", Noun),
    ("waterfall", Noun),
    ("weapon", Noun),
    ("well", Noun),
    ("westfall", Zone),
    ("wetland", Zone),
    ("wetlands", Zone),
    ("wheat", Noun),
    ("wheel", Noun),
    ("white", Colour),
    ("wicker", Noun),
    ("wild", Noun),
    ("window", Noun),
    ("wine", Noun),
    ("winterspring", Zone),
    ("wood", Noun),
    ("wooden", Noun),
    ("wreath", Noun),
    ("yellow", Colour),
];

/// The name shown for a doodad model path, e.g. `World\...\ArmorBracerBlue.m2` reads as
/// "Blue Bracers". Unknown stems are segmented over [`WORDS`], then capitalised as a last resort.
pub(super) fn display_name(model_path: &str) -> String {
    let file = model_path.rsplit(['/', '\\']).next().unwrap_or(model_path);
    let raw = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let stem = raw.to_ascii_lowercase();
    let key = stem.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_' || c == '-');
    if let Ok(index) = NAMES.binary_search_by_key(&key, |&(stem, _)| stem) {
        return NAMES[index].1.to_string();
    }
    let key = if key.is_empty() { stem.as_str() } else { key };
    // Stems the word list cannot spell still carry their file's own capitals
    // (`ReplaceCrate02`), which read far better than one run-together word.
    segment(key).unwrap_or_else(|| camel(&raw[..key.len()]).unwrap_or_else(|| capitalise(key)))
}

/// Splits a CamelCase stem at its humps (`ReplaceCrate` -> "Replace Crate", `BookSmallOpen` ->
/// "Book Small Open"); `None` if it has no humps to split at.
fn camel(stem: &str) -> Option<String> {
    let chars: Vec<char> = stem.chars().collect();
    let mut out = String::new();
    let mut splits = 0;
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1);
        let hump = c.is_ascii_uppercase()
            && prev.is_some_and(|p| {
                p.is_ascii_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()))
            });
        if hump || prev.is_some_and(|p| matches!(p, '_' | '-')) && c != '_' && c != '-' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            splits += 1;
        }
        if c == '_' || c == '-' {
            continue;
        }
        let word_start = i == 0 || prev.is_some_and(|p| matches!(p, '_' | '-'));
        out.push(if word_start {
            c.to_ascii_uppercase()
        } else {
            c
        });
    }
    (splits > 0).then(|| out.trim().to_string())
}

/// Segments `stem` over [`WORDS`] and arranges the pieces into a name; `None` if any part of
/// `stem` is not covered by a word.
fn segment(stem: &str) -> Option<String> {
    let mut words = Vec::new();
    for piece in stem.split(['_', '-']).filter(|piece| !piece.is_empty()) {
        words.extend(segment_piece(piece)?);
    }
    arrange(&words)
}

/// Splits `piece` into the fewest [`WORDS`] that spell all of it, so no single word choice
/// can strand the rest. `None` if `piece` cannot be spelled.
fn segment_piece(piece: &str) -> Option<Vec<(&'static str, Role)>> {
    // `best[i]`: the fewest words that spell `piece[i..]`, and the index of the first of them.
    let mut best: Vec<Option<(usize, usize)>> = vec![None; piece.len() + 1];
    best[piece.len()] = Some((0, usize::MAX));
    for start in (0..piece.len()).rev() {
        let mut choice: Option<(usize, usize)> = None;
        for (index, &(word, _)) in WORDS.iter().enumerate() {
            let end = start + word.len();
            if piece.get(start..end) != Some(word) {
                continue;
            }
            let Some((count, _)) = best[end] else {
                continue;
            };
            if choice.is_none_or(|(best_count, _)| count + 1 < best_count) {
                choice = Some((count + 1, index));
            }
        }
        best[start] = choice;
    }
    let mut words = Vec::new();
    let mut at = 0;
    while at < piece.len() {
        let (_, index) = best[at]?;
        let (word, role) = WORDS[index];
        words.push((word, role));
        at += word.len();
    }
    Some(words)
}

/// Orders segmented words for reading: colours first, then the rest in stem order. Fillers are
/// dropped, and a zone is dropped unless it is the whole name or stands before scenery.
fn arrange(words: &[(&str, Role)]) -> Option<String> {
    let mut colours = Vec::new();
    let mut nouns = Vec::new();
    for (index, &(word, role)) in words.iter().enumerate() {
        match role {
            Colour => colours.push(show(word, role)),
            Filler => {}
            Zone => {
                let next = words[index + 1..]
                    .iter()
                    .find(|(_, role)| !matches!(role, Colour | Filler));
                if next.is_none_or(|(_, role)| matches!(role, Scenery)) {
                    nouns.push(show(word, role));
                }
            }
            Scenery | Noun | Plural(_) => nouns.push(show(word, role)),
        }
    }
    let name: Vec<String> = colours.into_iter().chain(nouns).collect();
    (!name.is_empty()).then(|| name.join(" "))
}

/// A segmented word as it reads in a name.
fn show(word: &str, role: Role) -> String {
    match role {
        Plural(text) => text.to_string(),
        _ => capitalise(word),
    }
}

/// Title-cases each `_`/`-` separated piece of `stem`.
fn capitalise(stem: &str) -> String {
    stem.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars
                .next()
                .map(|c| c.to_ascii_uppercase())
                .unwrap_or_default();
            format!("{first}{}", chars.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        assert!(NAMES.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn looks_up_stems_ignoring_case_and_trailing_digits() {
        assert_eq!(
            display_name(r"World\Azeroth\Elwynn\ElwynnPine01.m2"),
            "Elwynn Pine"
        );
        assert_eq!(
            display_name(r"World\Generic\PassiveDoodads\Barrel02.mdx"),
            "Barrel"
        );
        assert_eq!(
            display_name("World/Generic/PassiveDoodads/Crate03.m2"),
            "Crate"
        );
    }

    #[test]
    fn segments_unknown_stems_over_words() {
        assert_eq!(
            display_name(r"World\Test\ironspikebarrel.m2"),
            "Iron Spike Barrel"
        );
        assert_eq!(
            display_name(r"World\Test\ironspike_barrel02.m2"),
            "Iron Spike Barrel"
        );
    }

    #[test]
    fn map_stems_that_read_run_together_have_names() {
        assert_eq!(display_name(r"world\x\replacecrate02.mdx"), "Crate");
        assert_eq!(
            display_name(r"world\x\stormwindwoodendummy01.mdx"),
            "Wooden Dummy"
        );
    }

    #[test]
    fn splits_camel_case_stems_the_word_list_misses() {
        assert_eq!(
            display_name(r"World\Generic\Crates\ZzqxKiwiPear02.m2"),
            "Zzqx Kiwi Pear"
        );
        assert_eq!(
            display_name(r"World\Generic\Food\CheeseWedgeSwiss01.m2"),
            "Cheese Wedge Swiss"
        );
    }

    #[test]
    fn capitalises_stems_it_cannot_segment() {
        assert_eq!(display_name(r"World\Test\zzqxkiwi.m2"), "Zzqxkiwi");
        assert_eq!(display_name(r"World\Test\zzqx_kiwi01.m2"), "Zzqx Kiwi");
    }

    #[test]
    fn tmp_print_sample() {
        for s in [
            "trolldungeonsacks",
            "blastedlandsbonepile",
            "bush_arthastears",
            "gunshopfireworksbarrel",
            "bankportraitplate",
            "tauren_weaponaxe",
            "aurapurpleshort",
            "baby_ne",
            "spiderpod",
            "duskwoodbellows",
            "energytrailhourglass",
            "g_eggalien",
            "cage",
            "centaurhornmouthpiece",
            "kodoelevatormachine",
            "orgrimmarfloatingembers",
            "moongladebush",
            "cavekoboldspidereggssack",
            "shadowfanggate",
            "50x",
            "gallows",
            "armorhelmgreen",
            "mulgra",
            "griffonmemorialtree",
            "bowl",
            "naxxramas_frostwyrm_slimefall",
            "ahnqirajdoor",
            "deadtreelog",
            "summerfest_ribbon",
            "wailingdruidbed",
            "brokengatewall",
            "silithidtankhuskhead",
            "g_epicbrazierblue",
            "dwarvebone",
            "deadminepowderkeg",
            "flowerswreath",
            "g_cagedoorbamboo",
            "undercitysignpostpointer",
            "armorleatherhelmbrownflat",
            "cot_portalparticles",
            "undeadsign_tavern",
            "dwarfsign_bakery",
            "humanbraziermagic",
            "fishfrenzygreen",
            "g_immolationtrap",
            "dwarvenbed",
            "lochmodanshurb",
            "hordectfflag",
            "nightelfsignpost",
            "azsharabubble",
            "generallantern",
            "greenbottle",
            "armorbracerblue",
            "generalbook",
            "westfallcabbage",
            "wetlandgrass",
            "stein",
        ] {
            println!(
                "SAMPLE {s} => {}",
                display_name(&format!("World\\X\\{s}.m2"))
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The Catalogue's categories (D31)
// ---------------------------------------------------------------------------------------------

/// What kind of thing a Catalogue entry is, for grouping and for Fizzwick's line about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Category {
    Food,
    Furniture,
    Goods,
    Critters,
    Folk,
    Greenery,
    Buildings,
    Odds,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Food,
        Category::Furniture,
        Category::Goods,
        Category::Critters,
        Category::Folk,
        Category::Greenery,
        Category::Buildings,
        Category::Odds,
    ];

    pub fn from_index(i: u8) -> Self {
        Self::ALL.get(i as usize).copied().unwrap_or(Category::Odds)
    }

    pub fn title(self) -> &'static str {
        match self {
            Category::Food => "Food & drink",
            Category::Furniture => "Furniture & household",
            Category::Goods => "Tools & goods",
            Category::Critters => "Critters & beasts",
            Category::Folk => "Folk",
            Category::Greenery => "Trees & greenery",
            Category::Buildings => "Buildings & landmarks",
            Category::Odds => "Odds and ends",
        }
    }
}

/// Keyword rules, checked in order: the first group with a word in the name wins. Barrels and
/// crates are goods, not drink; the furniture words come before the food ones so a "Pie Table" is
/// a table.
const CATEGORY_WORDS: &[(Category, &[&str])] = &[
    (
        Category::Buildings,
        &[
            "house",
            "cathedral",
            "tower",
            "gate",
            "bridge",
            "fence",
            "wall",
            "keep",
            "barracks",
            "building",
            "stormwind",
            "windmill",
            "church",
            "inn ",
            "shed",
            "stable",
        ],
    ),
    (
        Category::Goods,
        &[
            "barrel",
            "crate",
            "sack",
            "bucket",
            "anvil",
            "wagon",
            "cart",
            "plank",
            "rope",
            "wood pile",
            "firewood",
            "fire wood",
            "hay",
            "keg",
            "tool",
            "hammer",
            "plow",
            "chest",
        ],
    ),
    (
        Category::Furniture,
        &[
            "chair",
            "table",
            "stool",
            "bed",
            "pillow",
            "bench",
            "bookshelf",
            "shelf",
            "candle",
            "lamp",
            "lantern",
            "rug",
            "hearth",
            "fireplace",
            "book",
            "boots",
            "cabinet",
            "cauldron",
            "chandelier",
        ],
    ),
    (
        Category::Food,
        &[
            "bread", "loaf", "cheese", "pie", "muffin", "apple", "pumpkin", "mug", "bottle",
            "wine", "ale", "meat", "ham", "fish", "stew", "cake", "cup", "turnip", "carrot",
            "corn",
        ],
    ),
    (
        Category::Greenery,
        &[
            "tree", "pine", "bush", "stump", "sapling", "flower", "grass", "fern", "log", "oak",
            "bramble", "shrub", "mushroom",
        ],
    ),
];

/// The category of a rolled-up thing. `folk` marks an authored cast member (a creature unit from a
/// cast file), `kind` a creature or the boss; a keyword table decides the props.
pub(super) fn category(name: &str, kind: super::Kind, folk: bool) -> Category {
    match kind {
        super::Kind::Boss => return Category::Folk,
        super::Kind::Creature => {
            return if folk {
                Category::Folk
            } else {
                Category::Critters
            }
        }
        super::Kind::Prop => {}
    }
    let lower = name.to_ascii_lowercase();
    CATEGORY_WORDS
        .iter()
        .find(|(_, words)| words.iter().any(|w| lower.contains(w)))
        .map_or(Category::Odds, |(c, _)| *c)
}

#[cfg(test)]
mod category_tests {
    use super::*;
    use crate::azeroll::Kind;

    #[test]
    fn signature_names_land_in_their_categories() {
        let p = |n| category(n, Kind::Prop, false);
        assert_eq!(p("Muffin"), Category::Food);
        assert_eq!(p("Cheese Wheel"), Category::Food);
        assert_eq!(p("Mug"), Category::Food);
        assert_eq!(p("Chair"), Category::Furniture);
        assert_eq!(p("Captain's Table"), Category::Furniture);
        assert_eq!(p("Barrel"), Category::Goods);
        assert_eq!(p("Fire Wood Pile"), Category::Goods);
        assert_eq!(p("Hay Pile"), Category::Goods);
        assert_eq!(p("Elwynn Pine"), Category::Greenery);
        assert_eq!(p("Cathedral of Light"), Category::Buildings);
        assert_eq!(p("Stormwind Guard Tower"), Category::Buildings);
        assert_eq!(p("Mysterious Doodad"), Category::Odds);
        assert_eq!(category("Rat", Kind::Creature, false), Category::Critters);
        assert_eq!(
            category("Innkeeper Farley", Kind::Creature, true),
            Category::Folk
        );
        assert_eq!(category("Hogger", Kind::Boss, false), Category::Folk);
    }

    #[test]
    fn categories_index_round_trip_and_have_titles() {
        for (i, c) in Category::ALL.iter().enumerate() {
            assert_eq!(Category::from_index(i as u8), *c);
            assert!(!c.title().is_empty());
        }
        assert_eq!(Category::from_index(200), Category::Odds);
    }
}
