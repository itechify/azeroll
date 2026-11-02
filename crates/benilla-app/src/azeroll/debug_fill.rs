//! Debug-only: a believable clump for `WOW_AZEROLL_DEBUG_RADIUS` starts.
//!
//! A debug start sets the radius and nothing else, so the ball is a bare core with no debris
//! shell and few trophies, and a capture at that size says little about real play. When the
//! variable is set this plants the two things a rolled-up ball of that size would have: the shell's
//! buried volume (what grows `ball::ClumpShell`) and a surface layer of trophies, copies of the
//! small props around the start, attached to [`BallSpin`] the way [`super::collect`] attaches them.

use bevy::prelude::*;

use super::level::COLLECT_RATIO;
use super::props::{spawn_copy, PropCopy};
use super::{Attached, BallSpin, BallState, Collectible, Kind, Phase, Rng, RunEntity};

/// Where a rolled-up thing's centre sits, as in `collect.rs` (`ATTACH_SINK`).
const ATTACH_SINK: f32 = 0.35;
/// Share of the ball's volume the shell has swallowed. In a real run most of the volume is
/// small things buried under the surface layer.
const BURIED_SHARE: f32 = 0.65;
/// The surface layer covers about this multiple of the sphere's area, capped by count.
const COVER: f32 = 1.1;
const MAX_FILL: usize = 200;
/// Candidates come from within this many yards of the ball, at least this fraction of the
/// radius across and at most what the ball can roll up.
const REACH: f32 = 90.0;
const MIN_PICKUP: f32 = 0.04;
/// Give up waiting for props to assemble after this many seconds.
const PATIENCE: f32 = 6.0;

/// Plants read as scenery, not as things a katamari has picked up.
fn is_flora(model: &str) -> bool {
    let model = model.to_lowercase();
    [
        "grass", "bush", "tree", "flower", "fern", "weed", "bramble", "plant",
    ]
    .iter()
    .any(|w| model.contains(w))
}

#[derive(Resource, Default)]
struct Fill {
    done: bool,
    waited: f32,
}

/// Where a filler trophy goes on the sphere, resolved once its model is known.
#[derive(Component)]
struct FillPlan {
    dir: Vec3,
    rot: Quat,
}

fn debug_radius() -> Option<f32> {
    crate::webenv::var("WOW_AZEROLL_DEBUG_RADIUS")?.parse().ok()
}

/// `WOW_AZEROLL_CAM_SCENE=post|lens`: stage copies of a nearby wooden post around the first frame of
/// play, to check the occlusion fade on a known layout. `post` puts one just short of the ball on
/// the lens side (close enough that the boom is not shortened for it), one beside the ball and one
/// ahead of it; `lens` puts one around the lens.
fn scene() -> Option<String> {
    crate::webenv::var("WOW_AZEROLL_CAM_SCENE")
}

pub(super) fn plugin(app: &mut App) {
    if debug_radius().is_none() && scene().is_none() {
        return;
    }
    app.init_resource::<Fill>()
        .add_systems(OnEnter(Phase::Ready), |mut fill: ResMut<Fill>| {
            *fill = Fill::default()
        })
        .add_systems(Update, (fill, settle, stage).chain());
}

/// Plant the `WOW_AZEROLL_CAM_SCENE` posts, once the camera has been placed.
#[allow(clippy::type_complexity)]
fn stage(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    assets: Res<AssetServer>,
    rig: Res<super::camera::CamRig>,
    ball: Res<BallState>,
    mut done: Local<bool>,
    sources: Query<(&PropCopy, &Transform), (Without<Attached>, With<Collectible>)>,
) {
    let Some(which) = scene() else {
        return;
    };
    if *done || *phase.get() != Phase::Playing || rig.eye == Vec3::ZERO {
        return;
    }
    let post = ["post", "fence"].into_iter().find_map(|want| {
        sources
            .iter()
            .filter(|(p, _)| p.model.to_lowercase().contains(want))
            .min_by(|a, b| {
                a.1.translation
                    .distance(ball.pos)
                    .total_cmp(&b.1.translation.distance(ball.pos))
            })
    });
    let Some((post, source)) = post else {
        return;
    };
    *done = true;
    let r = ball.radius;
    let ground = ball.pos.y - r;
    let back = Vec3::new(rig.eye.x - rig.look.x, 0.0, rig.eye.z - rig.look.z).normalize_or(Vec3::Z);
    let right = back.cross(Vec3::Y);
    let boom = rig.eye.distance(rig.look);
    let at = |p: Vec3| Vec3::new(p.x, ground, p.z);
    let spots: Vec<Vec3> = if which == "lens" {
        vec![at(ball.pos + back * boom * 0.97)]
    } else {
        vec![
            at(ball.pos + back * r * 3.2),
            at(ball.pos + right * (r * 2.0 + 0.35)),
            at(ball.pos - back * (r * 2.0 + 0.5)),
        ]
    };
    for translation in spots {
        spawn_copy(
            &mut commands,
            &assets,
            &post.model,
            Transform {
                translation,
                scale: source.scale,
                ..default()
            },
            None,
        );
    }
    info!(
        "azeroll cam scene: {which} with {} (r={r:.2}, boom={boom:.2})",
        post.model
    );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fill(
    mut commands: Commands,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    assets: Res<AssetServer>,
    mut state: ResMut<Fill>,
    mut ball: ResMut<BallState>,
    mut rng: ResMut<Rng>,
    spin: Query<Entity, With<BallSpin>>,
    sources: Query<
        (&PropCopy, &Collectible, &Transform),
        (Without<Attached>, Without<super::design::Gate>),
    >,
) {
    if state.done || *phase.get() != Phase::Playing {
        return;
    }
    let (Some(r), Ok(spin)) = (debug_radius(), spin.single()) else {
        return;
    };
    // The boost lands on entering Ready; wait for it rather than fill a start-size ball.
    if ball.radius < r * 0.99 {
        return;
    }
    state.waited += time.delta_secs();
    let mut candidates: Vec<(&str, Vec3, f32)> = sources
        .iter()
        .filter(|(p, c, tf)| {
            c.kind == Kind::Prop
                && !is_flora(&p.model)
                && c.pickup >= MIN_PICKUP * r
                && c.pickup * (r / 4.0).powf(0.7).clamp(1.0, 2.0) <= COLLECT_RATIO * r
                && tf.translation.distance(ball.pos) < REACH
        })
        // Long thin things (fences, poles) bristle out of a ball far past their share of it.
        .filter(|(_, c, tf)| {
            let half = c.half_extents * tf.scale.abs();
            half.max_element() < 3.0 * half.min_element().max(1.0e-3)
        })
        .map(|(p, c, tf)| (p.model.as_str(), tf.scale, c.pickup))
        .collect();
    if candidates.len() < 8 && state.waited < PATIENCE {
        return;
    }
    state.done = true;
    // One entry per model, the biggest copy of each, so the draw below mixes kinds.
    candidates.sort_by(|a, b| b.2.total_cmp(&a.2));
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.0));
    candidates.truncate(48);
    if candidates.is_empty() {
        warn!("azeroll debug fill: no props near the start to copy");
        return;
    }

    ball.buried_volume = BURIED_SHARE * r * r * r;
    let area = 4.0 * std::f32::consts::PI * r * r * COVER;
    // What a ball this size has picked up is bigger than the start's barrels and mugs: scale the
    // copies up past r = 4, never beyond what the ball can roll up.
    let grow = (r / 4.0).powf(0.7).clamp(1.0, 2.0);
    let mut picks = Vec::new();
    let mut covered = 0.0_f32;
    while covered < area && picks.len() < MAX_FILL {
        let pick = (rng.f32() * candidates.len() as f32) as usize;
        let (model, scale, pickup) = candidates[pick.min(candidates.len() - 1)];
        covered += (pickup * grow) * (pickup * grow) * 0.6;
        picks.push((model, scale * grow, pickup));
    }
    // Fibonacci points keep the layer even; the draw order keeps sizes mixed.
    let count = picks.len();
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    for (i, (model, scale, _)) in picks.into_iter().enumerate() {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let ring = (1.0 - y * y).max(0.0).sqrt();
        let angle = golden * i as f32;
        let dir = Vec3::new(ring * angle.cos(), y, ring * angle.sin());
        let rot = Quat::from_euler(
            EulerRot::YXZ,
            rng.range(0.0, std::f32::consts::TAU),
            rng.range(-0.5, 0.5),
            rng.range(-0.5, 0.5),
        );
        let entity = spawn_copy(
            &mut commands,
            &assets,
            model,
            Transform {
                translation: dir * r,
                rotation: rot,
                scale,
            },
            None,
        );
        commands.entity(entity).insert((
            ChildOf(spin),
            // Hidden until `settle` knows its true centre; harmless numbers until then.
            Visibility::Hidden,
            Attached {
                radial: r,
                volume: 0.0,
                size: 0.0,
                at: 0.0,
            },
            FillPlan { dir, rot },
            RunEntity,
        ));
    }
    info!(
        "azeroll debug fill: r={r:.2} buried_volume={:.1} trophies={count} from {} models",
        ball.buried_volume,
        candidates.len()
    );
}

/// Once a filler's model has loaded, seat it on the surface exactly as `collect` would have.
fn settle(
    ball: Res<BallState>,
    mut fresh: Query<
        (
            &Collectible,
            &FillPlan,
            &mut Transform,
            &mut Attached,
            &mut Visibility,
        ),
        Added<Collectible>,
    >,
) {
    for (thing, plan, mut tf, mut attached, mut vis) in &mut fresh {
        let radial = (ball.radius - ATTACH_SINK * thing.size).max(0.0);
        let offset = plan.rot * (tf.scale * thing.center);
        tf.translation = plan.dir * radial - offset;
        tf.rotation = plan.rot;
        *attached = Attached {
            radial,
            volume: thing.volume(tf.scale),
            size: thing.size,
            at: 0.0,
        };
        *vis = Visibility::Inherited;
    }
}
