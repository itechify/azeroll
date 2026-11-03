//! The replay layer (D33, `docs/azeroll/replay-design.md`): one hidden **parcel** and one **cousin**
//! of Grand Tinker Fizzwick per level. Rolling a parcel up unlocks an outfit for the Prince (a gnome
//! display swap); rolling a cousin up unlocks them as an alternate pusher. Both are kept in the save
//! ([`super::progress`]) and worn from the Catalogue.
//!
//! This module owns the vocabulary the rest of the game shares: the [`Wardrobe`] the Prince's
//! display is resolved from, the [`ParcelFound`] / [`CousinCaught`] moments, and the pure pieces
//! (display resolution, the cousin's size window) that the unit tests pin. The parcel itself is a
//! scene item flagged `present` ([`super::design::Present`]); the cousin is a cast row with
//! `role = "cousin"` ([`super::npcs`]); the words are `lines.rs`'s.

use bevy::prelude::*;

use super::campaign::{Campaign, CurrentLevel};
use super::design::Present;
use super::progress::{Progress, WardrobeKeys};
use super::{Attached, BallState, Collected, Phase, Run};

/// The Catalogue name of every level's parcel: the level tells them apart.
pub(super) const PARCEL_NAME: &str = "Fizzwick's Parcel";

/// Real seconds a banner holds `PARCEL!` / `COUSIN!`.
pub(super) const MOMENT_SECS: f32 = 2.6;

/// What a level's parcel unlocks (from the recipe's `[present]`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PresentInfo {
    pub id: String,
    pub title: String,
    pub display: u32,
}

/// A cousin of Fizzwick (from the cast row with a `cousin` key).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CousinInfo {
    pub id: String,
    pub name: String,
    pub display: u32,
    /// Ball diameters: `[opens, swallowed-above]`.
    pub catch: [f32; 2],
}

/// The cousin of a cast file, if it has one. The cast is data, so this reads it as plain TOML.
pub(super) fn cousin_in_cast(cast: Option<&str>) -> Option<CousinInfo> {
    let cast: toml::Value = toml::from_str(cast?).ok()?;
    cast.get("npc")?.as_array()?.iter().find_map(|n| {
        let id = n.get("cousin")?.as_str()?.to_owned();
        let catch = n.get("catch")?.as_array()?;
        Some(CousinInfo {
            id,
            name: n.get("name")?.as_str()?.to_owned(),
            display: n.get("display")?.as_integer()? as u32,
            catch: [
                catch
                    .first()?
                    .as_float()
                    .or_else(|| catch.first()?.as_integer().map(|i| i as f64))?
                    as f32,
                catch
                    .get(1)?
                    .as_float()
                    .or_else(|| catch.get(1)?.as_integer().map(|i| i as f64))?
                    as f32,
            ],
        })
    })
}

/// On the parcel and on every cousin: human content the autopilot never targets or counts, so the
/// pacing numbers do not move (`auto::Loose` excludes it).
#[derive(Component)]
pub(super) struct HumanOnly;

/// Where the cousin stands against the ball, by diameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Window {
    /// Too small to be caught: the cousin ignores the ball.
    Below,
    /// The chase: the cousin notices, waves and runs a circle.
    Inside,
    /// Too big for a chase: the cousin cowers and is swallowed at a touch.
    Above,
}

/// The window a ball of `diameter` is in for a cousin that `catch` describes.
pub(super) fn window(diameter: f32, catch: [f32; 2]) -> Window {
    if diameter < catch[0] {
        Window::Below
    } else if diameter < catch[1] {
        Window::Inside
    } else {
        Window::Above
    }
}

/// The wardrobe as displays: what `creatures` dresses the Prince in. A pusher wears their own
/// clothes, so a chosen cousin hides the outfit; clearing the pusher restores it.
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub(super) struct Wardrobe {
    pub outfit: Option<u32>,
    pub pusher: Option<u32>,
}

impl Wardrobe {
    /// The display the Prince wears and why: the pusher, else the outfit, else neither.
    pub fn prince(&self) -> Option<(u32, &'static str)> {
        self.pusher
            .map(|d| (d, "pusher"))
            .or(self.outfit.map(|d| (d, "outfit")))
    }
}

/// Resolve the saved keys against every level's recipe and cast. A key no level knows is ignored.
pub(super) fn resolve(keys: &WardrobeKeys, campaign: &Campaign) -> Wardrobe {
    let outfit = keys.outfit.as_deref().and_then(|k| {
        campaign
            .levels
            .iter()
            .filter_map(|l| l.present.as_ref())
            .find(|p| p.id == k)
            .map(|p| p.display)
    });
    let pusher = keys.pusher.as_deref().and_then(|k| {
        campaign
            .levels
            .iter()
            .filter_map(|l| l.cousin.as_ref())
            .find(|c| c.id == k)
            .map(|c| c.display)
    });
    Wardrobe { outfit, pusher }
}

/// `WOW_AZEROLL_WARDROBE=<outfit>,<pusher>`: seed the wardrobe without a save (either may be empty).
fn env_keys() -> Option<WardrobeKeys> {
    let text = crate::webenv::var("WOW_AZEROLL_WARDROBE")?;
    let (outfit, pusher) = text.split_once(',').unwrap_or((text.as_str(), ""));
    let key = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_owned());
    Some(WardrobeKeys {
        outfit: key(outfit),
        pusher: key(pusher),
    })
}

/// A cousin speaks over their head (their `caption`, once per chase).
#[derive(Message, Clone, Debug)]
pub(super) struct CousinSays {
    pub entity: Entity,
    pub text: String,
}

/// The parcel was rolled up (the first time this run).
#[derive(Message, Clone, Debug)]
pub(super) struct ParcelFound {
    pub title: String,
}

/// A cousin was rolled up. `fair`: inside their window; otherwise swallowed whole.
#[derive(Message, Clone, Debug)]
pub(super) struct CousinCaught {
    pub key: String,
    pub name: String,
    pub fair: bool,
}

/// A cousin's caption, one line over their head for [`CAPTION_SECS`]: which entity, and until when
/// (run seconds). The text lives in the node.
#[derive(Resource, Default)]
struct Caption {
    who: Option<(Entity, f32)>,
    node: Option<Entity>,
}

const CAPTION_SECS: f32 = 2.0;
/// Yards above the cousin's feet where the caption hangs.
const CAPTION_LIFT: f32 = 1.2;

#[derive(Component)]
struct CaptionNode;

/// `WOW_AZEROLL_FIND_TEST=parcel|cousin` (a capture aid, off by default): after the autopilot has
/// done its steering, point the ball at the level's parcel or cousin instead, so a fixture start
/// reaches it. The autopilot still starts the run, shoots and exits; it never targets these itself.
fn find_test() -> Option<String> {
    crate::webenv::var("WOW_AZEROLL_FIND_TEST")
}

/// Aim at the parcel (or the cousin) and press forward.
fn find_drive(
    mut ball: ResMut<BallState>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    parcels: Query<&GlobalTransform, With<Present>>,
    npcs: Query<(&GlobalTransform, &super::npcs::Npc), Without<Attached>>,
) {
    let want = find_test().unwrap_or_default();
    let target = if want == "cousin" {
        npcs.iter()
            .find(|(_, n)| n.cousin().is_some())
            .map(|(t, _)| t.translation())
    } else {
        parcels.iter().next().map(|t| t.translation())
    };
    let Some(target) = target else { return };
    let to = super::units::flat(target - ball.pos);
    if to.length() < 0.05 {
        return;
    }
    ball.heading = (-to.x).atan2(-to.z);
    keys.press(KeyCode::KeyW);
}

/// With `WOW_AZEROLL_SHOTS=<dir>` set, save a frame a moment after a parcel or cousin is found.
fn moment_shot(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut parcels: MessageReader<ParcelFound>,
    mut cousins: MessageReader<CousinCaught>,
    mut waiting: Local<Vec<(f32, String)>>,
) {
    let Some(dir) = crate::webenv::var("WOW_AZEROLL_SHOTS") else {
        return;
    };
    let now = time.elapsed_secs();
    if parcels.read().count() > 0 {
        waiting.push((now + 0.7, "moment-parcel".to_string()));
    }
    if let Some(c) = cousins.read().last() {
        waiting.push((
            now + 0.7,
            format!(
                "moment-cousin-{}",
                if c.fair { "fair" } else { "swallowed" }
            ),
        ));
    }
    waiting.retain(|(at, name)| {
        if now < *at {
            return true;
        }
        use bevy::render::view::screenshot::{save_to_disk, Screenshot};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(
                std::path::PathBuf::from(&dir).join(format!("{name}.png")),
            ));
        false
    });
}

pub(super) fn plugin(app: &mut App) {
    if find_test().is_some() {
        app.add_systems(
            Update,
            find_drive
                .run_if(in_state(Phase::Playing))
                .in_set(benilla_world::schedule::WorldStage::Input)
                .after(super::auto::drive)
                .before(super::ball::drive_ball),
        );
    }
    app.add_systems(Update, moment_shot);
    app.init_resource::<Wardrobe>()
        .init_resource::<Caption>()
        .add_message::<ParcelFound>()
        .add_message::<CousinCaught>()
        .add_message::<CousinSays>()
        .add_systems(PreUpdate, sync_wardrobe)
        .add_systems(
            Update,
            caption.run_if(in_state(Phase::Playing).or(in_state(Phase::Paused))),
        )
        .add_systems(
            Update,
            moment
                .after(benilla_world::schedule::WorldStage::Input)
                .run_if(not(in_state(Phase::Select))),
        );
}

/// Type a cousin's one-liner over their head: a small gold line that follows them for two seconds.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn caption(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    run: Res<Run>,
    mut says: MessageReader<CousinSays>,
    mut state: ResMut<Caption>,
    cameras: Query<(&Camera, &GlobalTransform), With<benilla_world::view::WorldCamera>>,
    cousins: Query<&GlobalTransform>,
    mut node: Query<(&mut Node, &mut Text, &mut Visibility), With<CaptionNode>>,
) {
    for s in says.read() {
        state.who = Some((s.entity, run.elapsed + CAPTION_SECS));
        if let Ok((_, mut text, _)) = node.single_mut() {
            text.0.clone_from(&s.text);
        } else if state.node.is_none() {
            let font = crate::char_select::wow_font(&asset_server);
            let id = commands
                .spawn((
                    CaptionNode,
                    Text::new(s.text.clone()),
                    TextFont {
                        font,
                        font_size: 17.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.0)),
                    TextShadow {
                        offset: Vec2::splat(1.5),
                        color: Color::BLACK,
                    },
                    TextLayout::new_with_justify(Justify::Center),
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(360.0),
                        ..default()
                    },
                    GlobalZIndex(426),
                    Visibility::Hidden,
                    bevy::ui::FocusPolicy::Pass,
                ))
                .id();
            state.node = Some(id);
            return;
        }
    }
    let Ok((mut layout, _, mut vis)) = node.single_mut() else {
        return;
    };
    let on = state.who.and_then(|(e, until)| {
        (run.elapsed < until)
            .then(|| cousins.get(e).ok())
            .flatten()
            .zip(cameras.single().ok())
    });
    let Some((tf, (camera, cam_tf))) = on else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    };
    let head = tf.translation() + Vec3::Y * CAPTION_LIFT;
    match camera.world_to_viewport(cam_tf, head) {
        Ok(p) => {
            layout.left = Val::Px(p.x - 180.0);
            layout.top = Val::Px(p.y - 40.0);
            if *vis != Visibility::Inherited {
                *vis = Visibility::Inherited;
            }
        }
        Err(_) => {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    }
}

/// Keep the display wardrobe in step with the save (and the capture override).
fn sync_wardrobe(progress: Res<Progress>, campaign: Res<Campaign>, mut wardrobe: ResMut<Wardrobe>) {
    let keys = env_keys().unwrap_or_else(|| progress.wardrobe());
    let now = resolve(&keys, &campaign);
    if *wardrobe != now {
        *wardrobe = now;
    }
}

/// The moment: a parcel or a cousin was rolled up. Sets the run's flags (the save merge reads them
/// on pause, restart, leaving and results), the banner, and tells the HUD, Fizzwick and the sound.
fn moment(
    mut collected: MessageReader<Collected>,
    presents: Query<&Present>,
    npcs: Query<&super::npcs::Npc>,
    ball: Res<BallState>,
    current: Res<CurrentLevel>,
    campaign: Res<Campaign>,
    mut run: ResMut<Run>,
    mut parcels: MessageWriter<ParcelFound>,
    mut cousins: MessageWriter<CousinCaught>,
) {
    for c in collected.read() {
        if presents.contains(c.entity) && !run.parcel_found {
            run.parcel_found = true;
            run.banner = Some(("PARCEL!".to_string(), MOMENT_SECS));
            let title = campaign.levels[current.0]
                .present
                .as_ref()
                .map_or_else(|| PARCEL_NAME.to_owned(), |p| p.title.clone());
            info!(
                "azeroll replay: PARCEL found ({title}) at t={:.1}s D={:.2}",
                run.elapsed,
                ball.diameter()
            );
            parcels.write(ParcelFound { title });
        }
        if let Some(cousin) = npcs.get(c.entity).ok().and_then(|n| n.cousin()) {
            if run.cousin.is_none() {
                let fair = window(ball.diameter(), cousin.catch) != Window::Above;
                run.cousin = Some((cousin.key.to_owned(), fair));
                run.banner = Some(("COUSIN!".to_string(), MOMENT_SECS));
                info!(
                    "azeroll replay: COUSIN {} {} at t={:.1}s D={:.2}",
                    cousin.name,
                    if fair { "caught fair" } else { "swallowed" },
                    run.elapsed,
                    ball.diameter()
                );
                cousins.write(CousinCaught {
                    key: cousin.key.to_owned(),
                    name: cousin.name.to_owned(),
                    fair,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_has_three_sizes() {
        let catch = [1.33, 2.0];
        assert_eq!(window(1.2, catch), Window::Below);
        assert_eq!(window(1.33, catch), Window::Inside);
        assert_eq!(window(1.99, catch), Window::Inside);
        assert_eq!(window(2.0, catch), Window::Above);
        assert_eq!(window(5.0, catch), Window::Above);
    }

    #[test]
    fn a_pusher_wins_over_an_outfit_and_nothing_is_the_prince() {
        let w = Wardrobe {
            outfit: Some(5377),
            pusher: Some(3121),
        };
        assert_eq!(w.prince(), Some((3121, "pusher")));
        let w = Wardrobe {
            outfit: Some(5377),
            pusher: None,
        };
        assert_eq!(w.prince(), Some((5377, "outfit")));
        assert_eq!(Wardrobe::default().prince(), None);
    }

    #[test]
    fn saved_keys_resolve_against_the_recipes_and_an_unknown_key_is_ignored() {
        let campaign = Campaign::load(usize::MAX);
        let monocle = campaign
            .levels
            .iter()
            .find_map(|l| l.present.as_ref().filter(|p| p.id == "monocle"))
            .map(|p| p.display);
        assert_eq!(monocle, Some(5377));
        let w = resolve(
            &WardrobeKeys {
                outfit: Some("monocle".into()),
                pusher: Some("nonesuch".into()),
            },
            &campaign,
        );
        assert_eq!((w.outfit, w.pusher), (Some(5377), None));
        // An unknown outfit is the default Prince, no error.
        let w = resolve(
            &WardrobeKeys {
                outfit: Some("nonesuch".into()),
                pusher: None,
            },
            &campaign,
        );
        assert_eq!(w.prince(), None);
        // The shipped pusher resolves to the cousin's display.
        let w = resolve(
            &WardrobeKeys {
                outfit: Some("monocle".into()),
                pusher: Some("pippa".into()),
            },
            &campaign,
        );
        assert_eq!(w.prince(), Some((3121, "pusher")));
    }

    #[test]
    fn the_cousin_of_a_cast_reads_from_its_row() {
        let cast = r#"
            [[npc]]
            name = "Farley"
            role = "villager"
            [[npc]]
            name = "Pippa Fizzwick"
            role = "cousin"
            cousin = "pippa"
            display = 3121
            catch = [1.33, 2.0]
        "#;
        let c = cousin_in_cast(Some(cast)).unwrap();
        assert_eq!(
            (c.id.as_str(), c.display, c.catch),
            ("pippa", 3121, [1.33, 2.0])
        );
        assert_eq!(cousin_in_cast(Some("[[npc]]\nname = \"Farley\"\n")), None);
        assert_eq!(cousin_in_cast(None), None);
    }
}
