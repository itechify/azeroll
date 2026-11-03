# Campaign structure, hubs and finales: research for Azeroll

Scope: how the Katamari series sequences its levels and presents them. Covers the hub and King's missions (§1), the stage table and objective types (§2), how play areas open (§3), what happens when a stage ends (§4), and the giant finales (§5). §6 proposes a hub, briefing, unlock rules, records and objectives for Azeroll's three levels. Mechanics are in [katamari-reference.md](katamari-reference.md), layout and density in [level-design-research.md](level-design-research.md), and current gaps in [katamari-gap-audit.md](katamari-gap-audit.md).

**Evidence grades** (same key as the other research docs).
- **[C]** stated by a source I fetched (link given). Fetches that went through a summarising model were cross-checked against each other and the existing docs.
- **[S]** seen only in a search-result snippet, because the page blocked fetch.
- **[F]** fan, forum or community claim.
- **[I]** my inference or design proposal.
- **[?]** looked for and not found.

**Source reliability.** Fandom wikis (`katamari.fandom.com`) returned 402 on every fetch, so the King of All Cosmos, Select Meadow, Beautiful Katamari stage list and Sun pages are snippet-only or absent. TrueAchievements, GameFAQs and StrategyWiki returned 403. Katamaripedia fetched cleanly, but many stage pages are stubs. **The biggest gap is the original game's stage-select and results screens:** no reachable source describes their layout, text or art.

---

## 1. The hub: King's requests, the sky as record, records

### 1a. Katamari Damacy (2004) and REROLL (2018)

- **The premise is a commission.** The King of All Cosmos, drunk, destroyed the stars, constellations and the Moon, and sends his five-centimetre son to rebuild them ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C]. His pre-level rants are nonsensical and he speaks as "we" ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C].
- **Selection.** "Each level is presented on a stage select screen, and can be replayed multiple times after being completed" ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C]. The screen's layout and art are not described in any source I reached [?].
- **Missions are announced before the level.** "When each level begins, you are given a time limit and an objective size" ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C]. Typical missions: reach a size; collect as many items as possible in a time; collect the largest item ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C].
- **Completed levels become sky objects.** "At the end of each stage, the katamari is lifted into the night sky and becomes a constellation or a star" ([Takahashi postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. The record system is the map of rebuilt stars.
- **Per-stage collectables.** Each stage has a Royal Present and a cousin to roll up ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy), [Katamaripedia: MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) [C]. The log screen adds "lighthearted learning about Japanese food" ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy)) [C].
- **Unlocks by completion.** "If you do reach that size in time, then another level is unlocked" ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
- **Eternal (free-roll) modes.** Katamaripedia says Eternal stages "can be unlocked when reaching a certain diameter in three specific Make a Star stages" without naming the diameters ([stage list](https://katamaripedia.miraheze.org/wiki/List_of_Katamari_Damacy_Stages)) [C]. Cheat and forum sources name them: Make a Star 4 at 1 m 30 cm, Make a Star 8 at 25 m, Make the Moon at 800 m ([CheatCC](https://www.cheatcc.com/?p=92268), Steam snippet) [S]/[F]. A Steam thread adds that the game must be finished first [F]. The exact AFAP trigger is unverified [?].
- **Judging.** The King "judges the result on both size and time" ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C]. The stage grades listed in §4 are size-based.

**The Royal Rainbow.** The brief names it as the departure. The name appears only in a fan Super Smash Bros. Mercurious wiki, which describes a special move with the King appearing behind the stage ([Smash Mercurious wiki](https://ssbmercurious.fandom.com/wiki/The_Prince_of_All_Cosmos?oldid=5196), snippet) [S]. I could not confirm what it does in the original's stage exit or ending [?]. The audit's video index points (Make a Star 4 at 00:39:59 and 35:43, see [katamari-gap-audit.md](katamari-gap-audit.md)) are the next place to check. Nothing below depends on its exact form.

### 1b. We Love Katamari (2005) and REROLL+

- **The hub is a fan-facing meadow.** It is the **Select Meadow** in the original and the **Select Plaza** in REROLL+ ([TrueAchievements](https://www.trueachievements.com/game/We-Love-Katamari-REROLL-Royal-Reverie/walkthrough/2), snippet) [S]. From it you can "return to the Select Meadow to save and resume later" ([Katamaripedia: We Love Katamari](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C]. Roses there "grow taller and taller as more roses are collected" (same) [C]. A growing record on the hub.
- **Requests come from fans.** "The Prince starts by receiving a request from a fan. Completing each request brings in more fans, which open more stages" ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C]. Level structure is built "around taking requests from fictional fans" ([Wikipedia: We Love Katamari](https://en.wikipedia.org/wiki/We_Love_Katamari)) [C].
- **Fulfilled requests make planets.** "After a request is fulfilled, the Katamari is given to the King, which he uses to create a new planet in the cosmos" ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C]. In REROLL+ the cosmos is reached by moving up from the hub, and a fan menu lets the player pick levels directly ([TrueAchievements](https://www.trueachievements.com/game/We-Love-Katamari-REROLL-Royal-Reverie/walkthrough/2), snippet) [S].
- **Gates by cousins.** Roses unlock "after collecting all of the cousins and beating the 'Cousins' stage" ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C].
- **Super clears show in the summary.** In REROLL+, the level-summary planet gains a comet or meteor for a super clear ([TrueAchievements](https://www.trueachievements.com/game/We-Love-Katamari-REROLL-Royal-Reverie/walkthrough/2), snippet) [S].

### 1c. Beautiful Katamari (2007) and Katamari Forever (2009)

- **Beautiful Katamari's hub is the Princedom,** per one review ([Gamingpastime](https://gamingpastime.com/beautiful-katamari-review), snippet) [S]. Its stage select and unlocks are not described in the Wikipedia article [?].
- **Perfect scores unlock free roll.** "Eternal modes, where the player is allowed to roll without any time limits, are available once a perfect score is obtained on a level" ([Wikipedia: Beautiful Katamari](https://en.wikipedia.org/wiki/Beautiful_Katamari)) [C].
- **Rated on three things.** "The player is rated on time, size, and the number of objects meeting the King's request" (same) [C]. The King also awards "cookies" for online play (same) [C].
- **Katamari Forever's hub is the Village Square,** with "34 levels spread across the King's Cosmos and RoboKing's Cosmos" ([Katamaripedia: Katamari Forever](https://katamaripedia.miraheze.org/wiki/Katamari_Forever)) [C]. Its records are online leaderboards and PlayStation Home rewards (same) [C]. Its stage selection and unlocks are not described [?].

### 1d. What made each hub charming

- **The commission is parental and absurd.** "For an infant the literal king of all cosmos is your Dad" ([Affectionate Discourse](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)) [C]. "There's no winking irony here... the lack of posturing is a thing of beauty" (same) [C].
- **One tone across the interface.** Takahashi wanted "the movies, sounds, and interface all carry this similar, unique tone," and the game "does not rely on words" ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **The sky is the trophy case.** Finished levels become stars, planets and constellations (postmortem; [Katamaripedia: We Love Katamari](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C]. Records are visible, funny and cumulative.
- **Charm and reuse are separate problems.** Beautiful Katamari was "quirky, charming, and fun" ([GamesRadar](https://gamesradar.com/beautiful-katamari-review), snippet) [S]. IGN faulted its short length and Game Informer called its environments "dull and lifeless" ([Wikipedia: Beautiful Katamari](https://en.wikipedia.org/wiki/Beautiful_Katamari)) [C]. Katamari Forever's 34 stages are "mostly reused from earlier series entries" ([Wikipedia: Katamari Forever](https://en.wikipedia.org/wiki/Katamari_Forever)) [C]. The hub carries the charm; reuse is a stage-level risk.
- **A comic first pickup.** "I giggled like a schoolgirl when I picked up my first human" ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C]. One anecdote, not evidence about hubs generally [I].

---

## 2. Stage progression across a campaign

### 2a. The Make a Star ladder

Source: [Katamaripedia stage list](https://katamaripedia.miraheze.org/wiki/List_of_Katamari_Damacy_Stages) [C] unless marked. Times are PS2 / REROLL where both are listed. "Super" is the top grade from the stage page where listed.

| Stage | Start | Goal | Super | Time | Setting (where known) | Other listed values |
|---|---|---|---|---|---|---|
| Make a Star 1 | 5 cm | 10 cm | 15 cm | 3:00 / 4:00 | Living room | comet "1:00"; replay aim 17 cm ([MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) |
| Make a Star 2 | 5 cm | 20 cm | 35 cm | 6:00 / 7:00 | Takeda Residence: living room and park | comet "3:00"; 10 cm park fences to tunnels ([MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2)) |
| Make a Star 3 | 10 cm | 50 cm | ? | 8:00 | ? | ? |
| Make a Star 4 | 5 cm | 1 m | ? | 10:00 | House; rooftop and kitchen table | comet at 1 m in 6 min [F] ([TASVideos](https://tasvideos.org/Forum/Posts/20447), old forum); Eternal 1 gate 1 m 30 cm [S]/[F] |
| Make a Star 5 | 10 cm | 1 m 50 cm | ? | 12:00 | ? | ? |
| Make a Star 6 | 50 cm | 3 m | ? | 10:00 | ? | ? |
| Make a Star 7 | 50 cm | 6 m | ? | 10:00 | ? | ? |
| Make a Star 8 | 10 cm | 12 m | 25 m | 18:00 | ? | comet "12:00"; Eternal 2 gate 25 m [S]/[F] ([MaS8](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_8)) |
| Make a Star 9 | 50 cm | 30 m | ? | 18:00 | ? | ? |
| Make the Moon | 1 m | 300 m | ? | 25:00 | Urchin Town | Dipp at 1 m 37.5 cm; Eternal 3 gate 800 m [S]/[F] ([Make the Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon)) |

- **Starts are fixed per stage.** Each stage sets its own start size, jumping from 5 cm to 50 cm to 1 m [C]. Nothing carries over.
- **Super sits at roughly 1.5-2.1× the goal.** MaS1 15/10 = 1.5; MaS2 35/20 = 1.75; MaS8 25/12 ≈ 2.1 [C, my arithmetic]. That gives a grade ladder above the goal [I].
- **Goal-to-start ratios** run from 2× (MaS1) to 300× (the Moon), matching [katamari-reference.md §9](katamari-reference.md) [C].
- **Replays raise the bar.** MaS1's page says a return visit can aim for 17 cm [C].
- **Discrepancies.** The REROLL and PS2 time limits differ, and a second guide's MaS5-9 times conflict with the PS2 table ([existing reference doc](katamari-reference.md) notes this) [F]. I use the PS2 limits.

### 2b. Constellation and theme stages

Source: [stage list](https://katamaripedia.miraheze.org/wiki/List_of_Katamari_Damacy_Stages) [C].

| Stage | Start | Time | Objective (as written) |
|---|---|---|---|
| Make Cancer | 22 cm 2 mm | 5:00 | "Roll up as many crabs as possible." |
| Make Cygnus | 13 cm 9 mm | 6:00 | "Collect as many swans as possible." |
| Make Corona Borealis | 55 cm 5 mm | 8:00 | "Collect as many crowns as possible." |
| Make Gemini | 88 cm 8 mm | 10:00 | "Collect as many twins as possible." |
| Make Pisces | 99 cm 9 mm | 5:00 | "Collect as many fish as possible." |
| Make Virgo | 1 m 11 cm 1 mm | 10:00 | "Collect as many maidens as possible." |
| Make Ursa Major | 1 m 51 cm 5 mm | 10:00 | "Roll up a bear to make Ursa Major." |
| Make Taurus | 1 m 11 cm 1 mm | 10:00 | "Roll up a cow to make Taurus." |
| Make the North Star / Polaris | 3 m | none | "Make it to 10 meters without the starting diameter." (wording as written, ambiguous) |
| Eternal 1 / 2 / 3 | 5 cm / 5 cm / 1 m | none | "Make the biggest katamari with no time limit." |
| Credits | n/a | none | "Roll up as many countries as possible." |

- **Each constellation is a named sky object.** The stage name is the reward [C] (postmortem: stages become constellations).
- **Single-target stages start big.** Taurus and Ursa Major start at 1 m+, so a one-animal goal skips the nursery [C].
- **Untimed stages are the keep-going mode.** Polaris and the Eternals have no clock [C].

### 2c. Objective types

Each type appears in a fetched source.

1. **Reach diameter X in time.** The base mode of Make a Star and the Moon [C].
2. **Grades above the goal.** OK / Great / Super at rising diameters (MaS1: 12, 14, 15 cm) [C].
3. **Comet or time bonus.** MaS2 "3:00"; MaS4 1 m in 6 min [C for listed values, F for MaS4 meaning].
4. **Count collection.** "As many as possible" in a time, for a named kind [C]. Beautiful Katamari rates "the number of objects meeting the King's request" [C].
5. **Single named target.** One animal or thing: Taurus (cow), Ursa Major (bear), the Moon's cousin Dipp [C].
6. **Largest item.** "Collecting the largest item possible" ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C].
7. **Untimed growth.** Eternal, Polaris, Beautiful's post-perfect free roll [C].
8. **Score attack.** "Make the largest katamari possible in the allotted time" ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C].
9. **Second-run objective.** MaS1's 17 cm replay aim; We Love Katamari's post-completion objective ([Wikipedia: We Love Katamari](https://en.wikipedia.org/wiki/We_Love_Katamari)) [C].
10. **Fan requests.** We Love Katamari's missions, which open more stages [C].

The postmortem says the team tried "a specific level objective, in addition to just making the katamari bigger," and that "some of the missions don't have time limits" [C]. Specific objectives were a deliberate choice.

---

## 3. How the playable area opens

- **Each stage is a bounded set.** The stages are "houses, gardens, and towns" ([Wikipedia: Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy)) [C]. MaS2 has two areas, the living room and the park ([MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2)) [C].
- **Size gates are fences and tunnels.** MaS2's 10 cm park fences lead to tunnels with shortcuts. The page does not say whether fences open or show visually ([MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2)) [C]. I found no source for invisible boundaries [?].
- **Walls are climbed.** "Rolling against a wall allows the player katamari to 'climb' it in order to reach higher areas" ([Wikipedia: We Love Katamari](https://en.wikipedia.org/wiki/We_Love_Katamari)) [C].
- **A wall can be a size gate.** At a set size, "the King will instruct you to roll up the wall to the second level" ([TrueAchievements](https://www.trueachievements.com/game/We-Love-Katamari-REROLL-Royal-Reverie/walkthrough/2), snippet) [S]. That walkthrough is from the remaster.
- **Conflict: do We Love Katamari areas open by size?** Wikipedia, citing the Sydney Morning Herald, says "areas open or close depending on the katamari's size" ([Wikipedia: We Love Katamari](https://en.wikipedia.org/wiki/We_Love_Katamari)) [C]. Katamaripedia says "Growth only affects which objects can be picked up" ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C]. A review snippet says growth lets you "access new areas" ([Hexus review](https://m.hexus.net/gaming/reviews/ps2/4579-we-love-katamari-ps2/?page=3), snippet) [S]. Unresolved [?]. Do not design around size-gated areas until a play check settles it.
- **Object gates are the common mechanism.** "A stationary object can only be rolled up once you've reached a certain size whereas a moving object has two different size requirements" (REROLL+ snippet, [TrueAchievements](https://www.trueachievements.com/game/We-Love-Katamari-REROLL-Royal-Reverie/walkthrough/2)) [S]. This matches audit item #5.
- **Mid-stage, the tiers change, not the area.** "You can start absorbing the next level of object within a stage" ([Affectionate Discourse](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)) [C]. I found no source for a loading or "area change" moment mid-stage [?].
- **Streaming by size.** Some areas load only at a size breakpoint, such as the train in the Moon stage (speedrun route note, snippet) [S]. See [level-design-research.md §3](level-design-research.md).

---

## 4. The end of a stage: verdict, presentation, return

- **The stage ends on the goal, not the clock.** "Reaching the target in time is the only way forward" ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/), summary) [C].
- **Grades are tiers.** MaS1's final size sets one of four endings: Minimum 10 cm, OK 12 cm, Great 14 cm, Super 15 cm ([MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1)) [C].
- **Barely clearing draws criticism.** "If they only barely exceeded the required size, the king criticizes the player and tells them to do better. A larger margin earns praise" ([Wikipedia: Katamari](https://en.wikipedia.org/wiki/Katamari)) [C].
- **The best verdict may need an exact size.** A Steam thread says the top King line needs an exact size, cited as 10 m ([Steam REROLL discussion](https://steamcommunity.com/app/848350/discussions/0/3774483849431082336), snippet) [F].
- **The clump goes to the sky.** "At the end of each stage, the katamari is lifted into the night sky and becomes a constellation or a star" ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C]. How this looks on screen in the original is [?].
- **We Love Katamari's version makes a planet.** The King uses the katamari "to create a new planet in the cosmos," and a second one "can replace" or break into stardust if one already exists ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C].
- **The return.** In We Love Katamari the player returns to the hub to save and resume ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C]. The original's return path is not described [?].
- **The Royal Rainbow** is the unverified exit (§1a) [?].

---

## 5. Giant finales

### 5a. Make the Moon

- **Setup.** 1 m start, 300 m goal, 25 minutes, Urchin Town ([Make the Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon)) [C]. The designer's goal: "the ultimate goal is to grow the katamari to more than 500m in diameter" ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **It starts high.** The final stage begins at about 1 m, so the player never sees the thumbtack-scale nursery [C]. In Ethan Hein's count, "in the eighth level, you start out at the size of a thumbtack" ([Ethan Hein](https://ethanhein.com/wp/2009/katamari/)) [C].
- **What gets rolled up.** "A 741m katamari rolls up clouds, volcanoes, and a giant squid" (postmortem caption) [C]. "Out past whale or brontosaurus (both of which you pick up in the final level)" ([Ethan Hein](https://ethanhein.com/wp/2009/katamari/), excerpt) [C]. The late band runs "from individual buildings to city blocks, mountains, hexagonal bits of land" ([Affectionate Discourse](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)) [C].
- **Giants were improvised.** "I couldn't come up with any really large objects, so I just put" giant versions of ordinary things, such as a giant mushroom or giant daruma (postmortem) [C].
- **Landmarks.** Low-poly islands in the ocean stand for Takeda Residence and Pigeon Town, and one area has an airport, space station and rocket. Demo Island sits on Whale Mountain ([Make the Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon)) [C]. The size at which each becomes collectible is [?].
- **Eternal 3** returns in Katamari Forever with Ace at 1 m 46 cm 1 mm ([Make the Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon)) [C].

### 5b. We Love Katamari's Sun

- **Prerequisite: the other levels.** "You have to beat the other levels first" so there are enough planets ([Prima Games](https://primagames.com/tips/we-love-katamari-reroll-royal-reverie-how-to-get-the-platinum-trophy), snippet) [S]. A speedrun thread says all 23 others ([speedrun.com](https://www.speedrun.com/users/hotdogturtle/threads), snippet) [F].
- **The mechanic.** Planets accumulate until there are enough to roll up the Sun, with Earth as the katamari ([Katamaripedia](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari)) [C].
- **Thresholds are hidden.** The bare minimum for the Big Bang is not enough for the Sun, and planets are worth different amounts (speedrun thread, snippet) [F].
- **The only time the Sun is a katamari.** "The Sun is also the only time it appears as a katamari in the series" ([Katamari Wiki: Sun](https://katamari.fandom.com/wiki/Sun), snippet) [S].

### 5c. Avoiding the feel of a long journey

- **Gradual scale is the praised feature.** "The scale change is gradual and continuous" ([Ethan Hein](https://ethanhein.com/wp/2009/katamari/)) [C]. "The greatest strength of the game is its ability to scale the world smoothly" ([Diabolical Plots](https://www.diabolicalplots.com/tag/katamari-damacy/)) [C].
- **The climax is a crossing.** "A hugely pleasing climax," in which "the journey takes on an epic quality" ([Affectionate Discourse](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)) [C].
- **The camera carries scale.** "We made the camera zoom out automatically when the katamari grows so big that it occupies the entire screen" (postmortem) [C]. Takahashi wished for more: "we could have used the camera to add scale for the overall" (same) [C].
- **Finales start high, so no nursery.** The Moon starts at 1 m, and Taurus and Ursa Major start at 1 m+ [C]. This is the cheapest answer to "long journey" I can find [I].
- **No boss.** The climax is reached by rolling the next tier, with no enemies [C] ([existing reference doc §7](katamari-reference.md)).

---

## 6. Applying it to Azeroll

A proposal, not a decision. It respects the audit's constraints: no quest UI, no combat, a readable uninterrupted growth loop first ([katamari-gap-audit.md](katamari-gap-audit.md), verdict and items #15, #19, #20).

### 6a. Hub

| Candidate | What the player sees | For | Against |
|---|---|---|---|
| **Gnomish orrery under a dome** | A brass orrery in a gnome's study. Each level is a brass moon on a rail. Finished levels light a star. | Matches the postmortem's constellation record [C]. Gnomes tinker, so the orrery suits them. Sky-as-record without copying Katamari's UI. | Must read at small sizes. Risk of decoration unless the stars carry records. |
| **Eastern Kingdoms map table** | A worn map on a table in the Lion's Pride Inn, with pins for each level. | Shows the three-level route at a glance. | Reads as WoW's world map, an MMO UI. Drifts toward a quest board, which the audit rules out. |
| **Miniature Azeroth globe** | A small globe the gnome rolls; each finished level adds a landmass. | Strongest joke: the globe is itself a katamari. | Close to We Love Katamari's planet-making [C]. Better as the finale beat than the hub. |

**Recommendation: the orrery, with the globe as the finale beat.** A gnomish inventor (an invented commissioner, not the King, so the Warcraft cast stays distinct) hangs three brass request tags on the orrery. Picking a level means touching its tag. Finished levels become stars, named by the run's best find ("The Cheese Wheel", "The Inn's Last Stew"). The globe spins up once, in the Stormwind ending (§6f). [I]

**Avoid copying Katamari's UI.** No floating head above a menu list, no "Eternal" label, no planet-creation animation on the hub. The orrery's stars are the only record, and show only what is earned.

### 6b. Mission briefing

Katamari's briefing is the King's pre-level rant plus a time limit and an objective size [C]. For Azeroll:

- **One card, not a screen.** A brass request tag, with the commissioner's oversized portrait. It shows three things: the target diameter as a scale ring, the time as a burning candle, and one absurd sentence from the commissioner.
- **One joke line.** The audit asks for a skippable five-second vignette once (item #19). The briefing repeats the line, not the vignette.
- **Show the bonus.** A small comet icon for early finish (§6e).
- **The clock starts on movement,** as it does now (`level.rs:194–215`, per the audit).

### 6c. Unlock rules

- **L1 is open at start,** the Lion's Pride Inn.
- **L2 unlocks on any L1 completion.** This matches "reach that size in time, then another level is unlocked" [C].
- **L3 unlocks on L2 completion.** Keeps the route linear [I].
- **Free roll unlocks per level on a Super-tier finish.** Beautiful Katamari used a perfect score for this [C]. The original used hard-to-read diameter gates [S]/[F]. A tier-based gate is easier to explain [I].
- **The Stormwind ending is not gated by free roll.** It is the story's end.

### 6d. Records shown

1. **Best diameter.** Shown as the star's size or ring fill. The HUD already shows size and goal; the star reflecting the result is my proposal [I].
2. **Best time to goal.** A comet on the star, since REROLL+ shows a super clear as a comet [S].
3. **Named finds.** Counted on the star's label. This is the collection log, in sky form.
4. **Free-roll record.** Largest diameter, a ribbon on the orrery.

No running tally of every pickup on the hub, per the audit's #15.

### 6e. Objectives for the three levels

**L1: Lion's Pride Inn (tiny start, target size).**
- Mode: reach a diameter, in the Make a Star 1 pattern. Start 0.5 yd diameter (r 0.25), goal 1.0 yd (r 0.5), limit about 3 minutes. This is MaS1's 2× ratio and the reference doc's r 0.5 by 60 s [I].
- Grades: Passable at 1.0 yd (criticised as barely clearing), Pleasing at 1.2 yd, Splendid at 1.5 yd [I].
- Comet for finishing inside 90 s [I].

**L2: Goldshire farmers' market day (memorable objective).**
- Mode: count, in the Make Cancer pattern. Example: "roll up as many market cheeses and loaves as possible" in about six minutes [I].
- Optional single-target gift, in the Taurus pattern: the market's one prize item (a wheel of cheese or the merchant's barrel) for a bonus grade [I].
- The count needs audit item #5 first, so counts match what the ball can roll up [I].

**L3: Elwynn to Stormwind, ending on the Cathedral of Light (finale pattern).**
- Mode: a single named target plus a size goal, in the Make the Moon pattern. The goal is the Cathedral. The start should already be large (§5c) [I].
- Limit: about 10 minutes, inside the 8-10 minute Hogger window in audit item #3. For tuning [I].
- **Dependency.** The Cathedral is a WMO building. Building pickup is an open, large item (audit #9, Batch 3A). Until that seam lands, L3 cannot end on rolling it up. The audit warns against decorative duplicates that claim to solve this [I]. Fallback: end on a reachable structure and make the Cathedral the final beat only after the WMO work.

### 6f. Ending beat

- **Zoom out at screen fill,** with the Prince visible at the bottom ([postmortem](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)) [C].
- **A light beam from the clump,** the Cathedral's holy light, in place of the Royal Rainbow. A design proposal, not a copy of an exact REROLL animation [I].
- **The clump lifts into the night sky** as a constellation named for the best find. The orrery's star lights, closing the loop with the hub (postmortem §1a) [C/I].
- **Verdict uses the L1 grades,** plus a fourth, "Preposterous", for a finish over 2× the goal. Names are placeholders [I].
- **Skip stays immediate,** and camera and time are restored on every exit, as audit #20 asks.

### 6g. Open choices

- **Commissioner's species.** The Prince is a gnome, so a second gnome may crowd the cast. Check `goldshire-npcs.toml` and the Prince's model [I].
- **Count or timed goal in L2.** Decide after audit #3 telemetry shows how many market goods the ball rolls up in six minutes.
- **Free-roll tier.** Super or Splendid. Decide by playtest.
- **Royal Rainbow form.** Unresolved. Check the audit's video index points before copying the exit. The proposal does not depend on it.
- **Two claims need a play check:** whether We Love Katamari areas gate by size (§3), and whether the first-finish comet should be earned by time or goal size.

---

## Sources

- Wikipedia: [Katamari Damacy](https://en.wikipedia.org/wiki/Katamari_Damacy), [Katamari](https://en.wikipedia.org/wiki/Katamari), [We Love Katamari](https://en.wikipedia.org/wiki/We_Love_Katamari), [Beautiful Katamari](https://en.wikipedia.org/wiki/Beautiful_Katamari), [Katamari Forever](https://en.wikipedia.org/wiki/Katamari_Forever)
- Katamaripedia: [stage list](https://katamaripedia.miraheze.org/wiki/List_of_Katamari_Damacy_Stages), [Katamari Damacy](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy), [MaS1](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_1), [MaS2](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_2), [MaS8](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_a_Star_8), [Make the Moon](https://katamaripedia.miraheze.org/wiki/Katamari_Damacy/Make_the_Moon), [We Love Katamari](https://katamaripedia.miraheze.org/wiki/We_Love_Katamari), [Katamari Forever](https://katamaripedia.miraheze.org/wiki/Katamari_Forever)
- Takahashi postmortem, Game Developer (2004): [link](https://www.gamedeveloper.com/design/postmortem-the-singular-design-of-namco-s-katamari-damacy-2004-)
- Affectionate Discourse: [Katamari Damacy: The Definitive Review](https://affectionatediscourse.substack.com/p/katamari-damacy-the-definitive-review)
- Diabolical Plots: [Niche Games: Katamari Damacy](https://www.diabolicalplots.com/tag/katamari-damacy/)
- Ethan Hein: [Katamari](https://ethanhein.com/wp/2009/katamari/)
- TASVideos forum (old): [Katamari Damacy size attacks/speed run?](https://tasvideos.org/Forum/Posts/20447)
- Snippet only: [TrueAchievements, We Love Katamari REROLL+ Basics](https://www.trueachievements.com/game/We-Love-Katamari-REROLL-Royal-Reverie/walkthrough/2) (403 on fetch), [Prima Games](https://primagames.com/tips/we-love-katamari-reroll-royal-reverie-how-to-get-the-platinum-trophy), [speedrun.com threads](https://www.speedrun.com/users/hotdogturtle/threads), [CheatCC](https://www.cheatcc.com/?p=92268), [Smash Mercurious wiki](https://ssbmercurious.fandom.com/wiki/The_Prince_of_All_Cosmos?oldid=5196), [Gamingpastime](https://gamingpastime.com/beautiful-katamari-review), [GamesRadar](https://gamesradar.com/beautiful-katamari-review), [Hexus](https://m.hexus.net/gaming/reviews/ps2/4579-we-love-katamari-ps2/?page=3), [Katamari Wiki: Sun](https://katamari.fandom.com/wiki/Sun) (402 on fetch), [Steam REROLL discussion](https://steamcommunity.com/app/848350/discussions/0/3774483849431082336)
- Not reached: Katamari Wiki pages returning 402 (King of All Cosmos, Select Meadow, Beautiful Katamari stage list), StrategyWiki and GameFAQs (403), the GameCorner video index (see [katamari-gap-audit.md](katamari-gap-audit.md)).
