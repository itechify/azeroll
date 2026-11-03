//! Everything Fizzwick says that is not the hub's briefing: the goal and sixty-seconds pop-ins, the
//! results remarks, the try-again lines for a run that missed, and the Catalogue's quips. Words
//! only, so a writer can edit them without reading logic (`docs/azeroll/ritual-design.md` §1.6,
//! §3.5 and §4.4). Tables are keyed by level index (the order of `campaign::LEVELS`) with a generic
//! pool behind each; [`pick`] never repeats a line within a session until a pool runs dry.

/// A line table: the lines of each level, and the lines any level may use.
pub(super) struct Pool {
    pub per_level: [&'static [&'static str]; 3],
    pub generic: &'static [&'static str],
}

/// The lines already said this session, so none is heard twice until its pool is spent.
#[derive(bevy::prelude::Resource, Default, Debug)]
pub(super) struct Said(Vec<&'static str>);

/// One line from `pool` for `level`, not said before in this session. `roll` is any random number.
/// When every candidate has been said, those are forgotten and the pool starts over. An empty pool
/// (or an unknown level with an empty generic pool) says nothing.
pub(super) fn pick(pool: &Pool, level: usize, said: &mut Said, roll: u64) -> &'static str {
    let own = pool.per_level.get(level).copied().unwrap_or(&[]);
    let all: Vec<&'static str> = own.iter().chain(pool.generic).copied().collect();
    if all.is_empty() {
        return "";
    }
    let mut fresh: Vec<&'static str> = all
        .iter()
        .copied()
        .filter(|l| !said.0.contains(l))
        .collect();
    if fresh.is_empty() {
        said.0.retain(|l| !all.contains(l));
        fresh = all;
    }
    let line = fresh[(roll % fresh.len() as u64) as usize];
    said.0.push(line);
    line
}

/// One line from a flat list, with the same no-repeat rule.
pub(super) fn pick_flat(
    lines: &'static [&'static str],
    said: &mut Said,
    roll: u64,
) -> &'static str {
    pick(
        &Pool {
            per_level: [&[], &[], &[]],
            generic: lines,
        },
        0,
        said,
        roll,
    )
}

/// The goal met (size) or the target rolled up.
pub(super) const GOAL: Pool = Pool {
    per_level: [
        &[
            "Big enough! Now, Farley would like his FLOOR back. All of it.",
            "That is the size. This is not the end. Last call means LAST call.",
            "Goal! Keep rolling. The regiment left more than crumbs.",
            "You have reached the size. Do not stop. Nobody stops at the size.",
        ],
        &[
            "The Prize Pumpkin! First prize! The judges are reconsidering everything.",
            "Pumpkin acquired. Ma Stonefield's face is a picture. I have the picture.",
            "That is the biggest vegetable I have ever filed. And I file vegetables.",
        ],
        &[
            "The Cathedral has accepted the invitation. Involuntarily. It counts.",
            "A whole city! Unscheduled! The committee is going to need a bigger table.",
            "Stormwind, rolled up. I shall tell the King. Eventually. By letter.",
        ],
    ],
    generic: &["Goal! Splendid! Unscheduled, but splendid!"],
};

/// One minute left, the goal not met.
pub(super) const SIXTY_BEFORE: Pool = Pool {
    per_level: [
        &[
            "One minute! The candle is a stub! Eat the NEAREST thing!",
            "Sixty seconds! Books, boots, barrels: I am not fussy and neither are you.",
        ],
        &[
            "One minute to judging! The pumpkin is smug. Make it less smug.",
            "Sixty seconds! Roll up something with legs, it goes faster.",
        ],
        &[
            "One minute! The Cathedral is RIGHT THERE. It has not moved. Cities don't.",
            "Sixty seconds. A house. Any house. Houses are very filling.",
        ],
    ],
    generic: &[
        "One minute left! Panic is permitted. Stopping is not.",
        "Sixty seconds! I believe in you. I have also prepared a form if not.",
    ],
};

/// One minute left, the goal already met.
pub(super) const SIXTY_AFTER: Pool = Pool {
    per_level: [&[], &[], &[]],
    generic: &[
        "One minute! Everything you eat now is PURE PROFIT.",
        "Sixty seconds of pure, unregulated rolling. Go.",
        "One minute left and the room is yours. Be greedy. Be a gnome about it.",
        "Sixty seconds! Make it preposterous or make it home. Preposterous, please.",
    ],
};

/// Twice the request.
pub(super) const PREPOSTEROUS: Pool = Pool {
    per_level: [
        &[
            "Preposterous! That was a supper and now it is a WEATHER SYSTEM.",
            "Twice the size! Farley has gone to lie down. On nothing. You took the beds.",
        ],
        &[
            "Preposterous! You have rolled up the fair AND the people judging it.",
            "That is not a katamari, that is a SECOND GOLDSHIRE. Carry on.",
        ],
        &[
            "Preposterous! The map is going to need redrawing. I have crayons.",
            "Twice the request! Stormwind will be listed as 'formerly'.",
        ],
    ],
    generic: &[
        "Preposterous! Unscheduled! I am writing a new category!",
        "PREPOSTEROUS! My monocle has fallen off. I do not own a monocle.",
        "That is beyond the chart. I have drawn more chart. It is beyond that too.",
    ],
};

/// The level's big named thing rolled up.
pub(super) const LANDMARK: Pool = Pool {
    per_level: [
        &[
            "The captain's table! The captain is under it. The captain is next.",
            "A whole table! With the supper still on it. Efficient.",
        ],
        &[
            "HOGGER! Finally! The cheese stall sends its regards.",
            "You rolled up Hogger. Marshal Dughan will be furious. He wanted the credit.",
        ],
        &[
            "A BUILDING. You have rolled up a building. With people's things in it.",
            "A house! Was anyone home? Do not answer that. Roll up the next one.",
        ],
    ],
    generic: &[],
};

/// Filler, for a tier when nothing else has used a slot.
pub(super) const TIER: &[&str] = &[
    "Bigger! Excellent. Keep it up, I am taking notes.",
    "That's a new tier of thing. There are more tiers. Tiers all the way up.",
    "Growing nicely. Like a gnome. Which is to say: eventually.",
    "Oho! That used to be a wall. Now it is a snack.",
];

/// A run that ended under the goal.
pub(super) const TRY_AGAIN: Pool = Pool {
    per_level: [
        &[
            "The candle went out. The floor is still crunchy. Farley is weeping. Again?",
            "You were THIS close. I am holding my fingers apart. You cannot see them. Again!",
            "Last call was called. We shall simply call it again. Louder.",
            "The regiment left the crumbs. The crumbs are still there. This is a two-crumb problem.",
        ],
        &[
            "The pumpkin took first prize without you. It gloated. Pumpkins can gloat. Again!",
            "Market's shut. The stalls are counting their stock and most of it is still there.",
            "The judges have gone home. The judges will come back. The judges have no choice.",
        ],
        &[
            "The Cathedral did not RSVP. Neither did you, apparently. We go again.",
            "Stormwind is still standing. This is not the report I wanted to file.",
            "A city is a big ask. I did say it was rather large. I am saying it again.",
        ],
    ],
    generic: &[
        "The candle has opinions about that. Shall we try again, with feeling?",
        "That was a practice run. I have decided this retroactively. Again!",
        "Nobody saw that. I saw that. Nobody else saw that. Again!",
        "The fuses survived. The schedule did not. Once more!",
        "Close! By gnomish standards. Which are small. Again!",
    ],
};

/// A size level, past the goal, with nothing left the clump can take: he points at the Finish
/// button under the clock. The words name the button ("Call it!"), not a key, because the prompt
/// beside it follows the player's device.
pub(super) const NUDGE: Pool = Pool {
    per_level: [
        &[
            "Farley's floor is spotless and you are bigger than his inn. Press Call it!, under the clock.",
            "There is nothing left in here that fits, or that you do not. Call it! Under the clock.",
        ],
        &[],
        &[],
    ],
    generic: &[
        "You have out-grown everything in reach. The brass button under the clock says Call it!",
        "Nothing left to eat, boss. Press Call it! under the clock, or keep rolling for the joy of it.",
        "I believe that is the lot. Call it! when you are ready. The button is under the clock.",
    ],
};

/// The Free Roll drop.
pub(super) const KEEP_IT: &str = "Fine. Keep it. For NOW.";

/// The results remarks, per grade (Scraped, Respectable, Splendid, Preposterous); the first of each
/// is the line the verdict always had.
pub(super) const REMARKS: [&[&str]; 4] = [
    &[
        "It counts. Technically. Please do not tell the committee.",
        "The size, and not an inch more. You have the soul of an accountant.",
        "Just over the line. The line is relieved. I am relieved. We are all relieved.",
    ],
    &[
        "Adequate! The gears are almost impressed.",
        "Respectable. I shall write it in ink. Small ink.",
        "A good roll. Not a great roll. A good roll. I was there.",
    ],
    &[
        "Marvellous! Even the cogs are applauding.",
        "Splendid! That is a word I keep for special days and this is one.",
        "Oh, that is a BALL. That is a proper ball. Frame it. I cannot, it is round.",
    ],
    &[
        "Preposterous! Unscheduled! I am writing a new category!",
        "Preposterous! The committee has fainted. The committee is one gnome. Me.",
        "That is not a result. That is an EVENT. I shall be dining out on this.",
    ],
];

/// The Catalogue's line for a whole category (indexed like `names::Category`).
pub(super) const CATALOGUE_CATEGORY: [&str; 8] = [
    "Edible. Was edible. Is now structural.",
    "Somebody sat on this. Somebody will have to stand now.",
    "Honest goods, honestly rolled. The merchant disagrees.",
    "It is fine. It is on a ball now. Balls are a kind of home.",
    "A citizen of Azeroth, volunteered. The paperwork is retroactive.",
    "Nature. Compressed. Very efficient use of nature.",
    "Architecture is just a big object that thinks it is special.",
    "I do not know what this is either. It is in the log now. It counts.",
];

// ---------------------------------------------------------------------------------------------
// The replay layer (D33, docs/azeroll/replay-design.md §7): the parcel, the cousins, the news and
// the equip lines.
// ---------------------------------------------------------------------------------------------

/// The parcel rolled up: Fizzwick's bonus pop-in.
pub(super) const PARCEL: Pool = Pool {
    per_level: [
        &[
            "Found it! The post said 'lost'. The post LIED. It was in the FIRE.",
            "My parcel! In a fireplace! Farley has questions to answer. Later.",
            "That is my post! Unopened! Singed! A monocle, if the label is right.",
        ],
        &[
            "The parcel! In a CHICKEN COOP. The chickens have been sitting on it.",
            "Mirela's bandana, by chicken. The post office owes me an explanation.",
            "A parcel among hens. They were keeping it warm. That is not how post works.",
        ],
        &[
            "The parcel! Under a BRIDGE! In a CANAL! The post THREW it. I knew it.",
            "Fished out of the canal by a ball. Stormwind's post is a disgrace.",
            "My helm! Under the Trade bridge! Dry, mostly. I shall think in it.",
        ],
    ],
    generic: &[
        "A parcel! Addressed to ME. Opened by YOU. I shall allow it, this once.",
        "Post! Years late! Gnomeregan quality, Stormwind delivery.",
    ],
};

/// A cousin caught inside the window.
pub(super) const COUSIN_FAIR: Pool = Pool {
    per_level: [
        &["PIPPA! My cousin! She was counting rats. She may stop now."],
        &["Twinkle! Cousin! Out of the barrel at last, and onto a ball."],
        &["Grand-Uncle THRUM. A dwarf. By marriage. He owes me three silver."],
    ],
    generic: &["A RELATIVE! On the ball! Family reunions are usually worse."],
};

/// A cousin swallowed whole by a ball far past their window.
pub(super) const COUSIN_SWALLOWED: Pool = Pool {
    per_level: [
        &["Was that Pippa? That was Pippa. You did not even slow down."],
        &["Twinkle went in with the apples. She will be fine. She is small."],
        &["That was Thrum. All of him. The haggling is over, I suppose."],
    ],
    generic: &["Someone from the family went by. Never mind. They will write."],
};

/// The hub's news after a run that found one (Fizzwick cheers).
pub(super) const NEWS_PARCEL: &str =
    "The post arrived! Years late and by katamari, but it ARRIVED.";
pub(super) const NEWS_COUSIN: &str = "A cousin, found! The family grows. The family is on a ball.";
pub(super) const NEWS_ALL_PARCELS: &str =
    "ALL my post! Fireworks! I have fireworks. I always have fireworks.";
pub(super) const NEWS_ALL_COUSINS: &str =
    "The whole family, assembled. Nobody is speaking to anybody. Perfect.";

/// Typed in Fizzwick's window when the Catalogue wears an outfit or picks a pusher, by wardrobe key.
/// The empty key is the Prince himself again.
pub(super) const EQUIP: &[(&str, &str)] = &[
    (
        "monocle",
        "A monocle. On the Prince. I do not own one. I am not bitter.",
    ),
    (
        "bandana",
        "Very piratical. Mirela will want it back. Mirela can roll for it.",
    ),
    (
        "thinking-helm",
        "The Thinking Helm! Think big thoughts. Round ones, preferably.",
    ),
    (
        "pippa",
        "Pippa pushes now. She is small. She is also DETERMINED.",
    ),
    (
        "twinkle",
        "Twinkle pushes now. Mind the goggles, they are not for swimming.",
    ),
    (
        "thrum",
        "Thrum pushes now. He is a dwarf. The ball will not notice. I will.",
    ),
    (
        "",
        "The Prince is back on the ball. Where he belongs. Push, boy.",
    ),
];

/// The Catalogue name the parcel's quip lives under.
pub(super) const PARCEL_QUIP_KEY: &str = "Fizzwick's Parcel";

/// The equip line for a wardrobe key.
pub(super) fn equip_line(key: &str) -> &'static str {
    EQUIP
        .iter()
        .find(|(k, _)| *k == key)
        .map_or("Very smart. Now ROLL.", |(_, l)| l)
}

/// The Catalogue's line for a signature thing, by display name (compared ignoring case).
pub(super) const CATALOGUE_ITEM: &[(&str, &str)] = &[
    ("Muffin", "Two hundred muffins. The cook fled for a reason, and the reason was muffins."),
    ("Cheese Wheel", "The wheel is round. The ball is round. This was always going to happen."),
    ("Cheese", "Follow the cheese. The cheese led to rats. The rats led to more cheese."),
    ("Bread", "Day-old bread. Now day-old bread at speed."),
    ("Loaf of Bread", "Day-old bread. Now day-old bread at speed."),
    ("Pie", "A whole pie. Nobody had a slice. Nobody will."),
    ("Apple", "Crisp, red, and doomed."),
    ("Mug", "An empty mug, which is the saddest kind, until it joined something."),
    ("Bottle", "Vintage. The year is 'this one'."),
    ("Keg", "A keg! The regiment's. They will be along to complain. Roll them too."),
    ("Barrel", "Barrels are the universe's way of saying 'this will roll'."),
    ("Crate", "Contents unknown. Contents now also round."),
    ("Sack", "A sack of something. Everything is a sack of something, if you think about it."),
    ("Book", "Knowledge is power. Stacked knowledge is stairs."),
    ("Open Book", "Knowledge is power. Stacked knowledge is stairs."),
    ("Boots", "Boots in the hall. Someone is in their socks right now, and furious."),
    ("Pillow", "The pillow fight is over. Everyone lost. The pillows most of all."),
    ("Chair", "A chair. The regiment sat on it. Now it sits on YOU."),
    ("Stool", "Farley's stool. Farley was fond of it. Farley is fond of many things he no longer has."),
    ("Table", "A table! With the supper still on it. Efficient."),
    ("Captain's Table", "The captain's own table. He was under it. He is next."),
    ("Bed", "The honeymoon suite. The honeymoon is over."),
    ("Candle", "A candle. Not THE candle. The candle is mine."),
    ("Rat", "A rat. Pied piper. I am not paying you in rats."),
    ("Hearth Cat", "The hearth cat. It was asleep. It is still asleep. On a ball."),
    ("Chicken", "The chicken did not cross the road. The road came to it."),
    ("Sheep", "Wool, legs and a grudge."),
    ("Cow", "This is how Taurus happened, you know. Ask the King. Not that one."),
    ("Pig", "Market pork. Free range. VERY free range."),
    ("Wolf", "Elwynn's finest predator, now prey. Do not tell it."),
    ("Hogger", "HOGGER. Marshal Dughan has a poster. The poster is wrong now."),
    ("Innkeeper Farley", "Farley. He wanted his floor back. He has it; he is on it; it is round."),
    ("Brog Hamfist", "The cook. He fled from the kitchen into the kitchen. Poor planning."),
    ("Remy", "Remy, asleep under the bar. Still asleep. Impressive."),
    ("Barkeep Dobbins", "Dobbins. The card game ended badly and this is the badly."),
    ("Marshal Dughan", "The Marshal. Goldshire's law. Now Goldshire's ballast."),
    ("Stormwind Guard", "A guard stood his ground. The ground was collected first."),
    ("Ma Stonefield", "Ma Stonefield grew the pumpkin. Now she is in it, roughly."),
    ("Prize Pumpkin", "First prize! The judges awarded it to YOU. By being on you."),
    ("Pumpkin", "Not THE pumpkin. A pumpkin. Still orange, still round, still yours."),
    ("Hay Pile", "Hay. Lots of hay. Katamari rule: you will roll up more hay than you want."),
    ("Fire Wood Pile", "Firewood. It would have warmed someone. It will now warm nobody, efficiently."),
    ("Wagon", "A wagon. The horse is wondering where it went. Keep rolling, the horse is next."),
    ("Cart", "A broken-down cart, now a broken-down cart in orbit."),
    ("Anvil", "An anvil. Heavy. Round now. Smith Argus has questions."),
    ("Tree", "A tree! A whole tree! The birds have been reassigned."),
    ("Elwynn Pine", "A tree! A whole tree! The birds have been reassigned."),
    ("Sapling", "A sapling from the tree fair. It will grow up round."),
    ("House", "A house! Was anyone home? Do not answer that."),
    ("Stormwind Gate", "The gate! Stormwind is open. Stormwind is also inside you. Both."),
    ("Bridge", "A bridge. The canal is very confused."),
    ("Cathedral of Light", "The Cathedral of Light. It did not RSVP. It came anyway. In you."),
    ("Stormwind Guard Tower", "A guard tower, which saw you coming, which did not help it."),
    ("Hearth", "The hearth has gone out. It has gone out, round, with you."),
    ("Fireplace", "The hearth has gone out. It has gone out, round, with you."),
    ("Fizzwick's Parcel", "Addressed to me, opened by you, logged by both of us."),
    ("Pippa Fizzwick", "Pippa. She was counting the rats. She lost count. She IS the count now."),
    ("Twinkle Fizzwick", "Twinkle, apple-bobbing. Got the apple. Also got rolled up. Mixed day."),
    ("Grand-Uncle Thrum", "Grand-Uncle Thrum. A dwarf. By marriage. We do not discuss it. He is on the ball now, so we will have to."),
];

/// The signature quip for a display name, if it has one.
pub(super) fn item_quip(name: &str) -> Option<&'static str> {
    CATALOGUE_ITEM
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, q)| *q)
}

#[cfg(test)]
mod tests {
    use super::*;

    const POOLS: [&Pool; 10] = [
        &PARCEL,
        &COUSIN_FAIR,
        &COUSIN_SWALLOWED,
        &GOAL,
        &SIXTY_BEFORE,
        &SIXTY_AFTER,
        &PREPOSTEROUS,
        &LANDMARK,
        &TRY_AGAIN,
        &Pool {
            per_level: [&[], &[], &[]],
            generic: TIER,
        },
    ];

    fn every_line() -> Vec<&'static str> {
        let mut all: Vec<&str> = Vec::new();
        for p in POOLS {
            all.extend(p.per_level.iter().flat_map(|l| l.iter()).chain(p.generic));
        }
        all.extend(REMARKS.iter().flat_map(|r| r.iter()));
        all.extend(CATALOGUE_CATEGORY);
        all.extend(CATALOGUE_ITEM.iter().map(|(_, q)| q));
        all.push(KEEP_IT);
        all.extend([NEWS_PARCEL, NEWS_COUSIN, NEWS_ALL_PARCELS, NEWS_ALL_COUSINS]);
        all.extend(EQUIP.iter().map(|(_, l)| l));
        all
    }

    #[test]
    fn a_pop_in_line_fits_two_lines_of_the_bubble() {
        // The bubble is two or three short lines at the HUD's size.
        for p in POOLS {
            for l in p.per_level.iter().flat_map(|l| l.iter()).chain(p.generic) {
                assert!(
                    l.chars().count() <= 90,
                    "{l:?} is {} long",
                    l.chars().count()
                );
            }
        }
    }

    #[test]
    fn the_replay_lines_fit_a_line_and_every_pool_picks() {
        let mut said = Said::default();
        for pool in [&PARCEL, &COUSIN_FAIR, &COUSIN_SWALLOWED] {
            for level in 0..3 {
                assert!(!pick(pool, level, &mut said, level as u64).is_empty());
            }
            for l in pool
                .per_level
                .iter()
                .flat_map(|l| l.iter())
                .chain(pool.generic)
            {
                assert!(
                    l.chars().count() <= 76,
                    "{l:?} is {} long",
                    l.chars().count()
                );
            }
        }
        assert_eq!(equip_line("pippa"), EQUIP[3].1);
        assert_eq!(equip_line(""), EQUIP[6].1);
    }

    #[test]
    fn nothing_is_empty_or_unfinished() {
        for l in every_line() {
            assert!(!l.trim().is_empty());
            assert!(l.ends_with(['.', '!', '?', '\'']), "{l:?}");
        }
    }

    #[test]
    fn a_line_is_not_said_twice_until_the_pool_runs_dry() {
        let mut said = Said::default();
        let n = GOAL.per_level[0].len() + GOAL.generic.len();
        let mut heard = Vec::new();
        for i in 0..n {
            heard.push(pick(&GOAL, 0, &mut said, i as u64 * 7919));
        }
        let mut unique = heard.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), n, "{heard:?}");
        // And then it starts over rather than going quiet.
        assert!(!pick(&GOAL, 0, &mut said, 3).is_empty());
    }

    #[test]
    fn an_unknown_level_or_an_empty_table_never_panics() {
        let mut said = Said::default();
        assert!(!pick(&GOAL, 9, &mut said, 1).is_empty());
        let none = Pool {
            per_level: [&[], &[], &[]],
            generic: &[],
        };
        assert_eq!(pick(&none, 0, &mut said, 1), "");
        // LANDMARK has no generic lines: a level past the table says nothing.
        assert_eq!(pick(&LANDMARK, 7, &mut said, 1), "");
    }

    #[test]
    fn every_grade_has_a_pool_led_by_the_original_remark() {
        assert_eq!(REMARKS.len(), 4);
        assert!(REMARKS.iter().all(|r| r.len() == 3));
        assert_eq!(
            REMARKS[0][0],
            "It counts. Technically. Please do not tell the committee."
        );
    }

    #[test]
    fn signature_quips_resolve_ignoring_case_and_names_are_unique() {
        assert!(item_quip("cheese wheel").is_some());
        assert!(item_quip("Nothing In Particular").is_none());
        let mut names: Vec<String> = CATALOGUE_ITEM
            .iter()
            .map(|(n, _)| n.to_ascii_lowercase())
            .collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len());
    }
}
