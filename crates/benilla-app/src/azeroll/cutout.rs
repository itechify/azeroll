//! Screen-space see-through window around the ball. The engine resource defaults off for every
//! other mode.
//!
//! The camera never dodges the world (`camera.rs`), so the lens often sits behind or inside a
//! tree, a wall or a roof. Instead, the shaders discard any world fragment inside a screen circle
//! around the ball. The ordinary depth plane preserves its floor; a stricter foreground plane
//! also clears low fragments well between the lens and clump. What stands beside or ahead of
//! the ball stays solid, with a dithered rim at the window edge.
use super::{camera::CamRig, Attached, BallState};
use benilla_world::{
    lighting::FoliageCutout, view::WorldCamera, wmo_portal::WmoRevealPoint, world_point::WorldPoint,
};
use bevy::{camera::CameraProjection, prelude::*};

/// The window's radius, as a multiple of the clump's projected radius, in the open and indoors
/// (where the surroundings are what the walls would hide).
const WINDOW_OPEN: f32 = 1.55;
const WINDOW_INDOORS: f32 = 2.1;
/// The clump counts as at most this many radii wide for the window's size.
const CLUMP_WIDTH: f32 = 1.6;
/// Fragments must be nearer than the clump's near side by this share of its extent...
const DEPTH_MARGIN: f32 = 0.1;
/// The height floor sits this share of a radius under the ball's centre: the ground, steps and
/// the lowest contact band stay solid, everything above is eligible.
const FLOOR_BELOW: f32 = 0.5;
/// The depth margin also has this many yards, which keeps the Prince (he stands at the clump's
/// back on the lens side, hands on its surface) solid: units are not told apart from scenery in
/// the shader.
const DEPTH_MARGIN_ABS: f32 = 0.7;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        publish
            .after(benilla_world::schedule::WorldStage::Input)
            .after(super::finale::animate)
            .before(benilla_world::wmo_portal::WmoPvsSet),
    );
}

pub(super) fn publish(
    time: Res<Time<Real>>,
    mut trace_at: Local<f32>,
    ball: Res<BallState>,
    rig: Res<CamRig>,
    flow: Res<super::progress::Flow>,
    phase: Res<State<super::Phase>>,
    finale: Res<super::finale::Finale>,
    ritual: Res<super::ritual::Ritual>,
    trophies: Query<&Attached>,
    world: WorldPoint,
    cameras: Query<(&Camera, &Transform, &Projection), With<WorldCamera>>,
    mut cutout: ResMut<FoliageCutout>,
    mut reveal: ResMut<WmoRevealPoint>,
) {
    *cutout = FoliageCutout::default();
    // The lens sits outside walls now, so the building the ball is in draws every room, not just
    // what the camera's portal flood reaches.
    // D26: the lens frames a remote city while the body stays on the safe approach.
    // Reveal the city around the Cathedral; the terminal render mask is applied afterward.
    reveal.0 = Some(
        if *phase.get() == super::Phase::Playing && flow.finale.is_some() {
            finale.reveal_point().unwrap_or(ball.pos)
        } else {
            ball.pos
        },
    );
    // `WOW_AZEROLL_NO_CUTOUT=1` leaves the window off, for an A/B against the raw view.
    if crate::webenv::var("WOW_AZEROLL_NO_CUTOUT").is_some() {
        return;
    }
    let Ok((camera, transform, projection)) = cameras.single() else {
        return;
    };
    if rig.eye == Vec3::ZERO {
        return;
    }
    let Projection::Perspective(perspective) = projection else {
        return;
    };
    let Some(viewport) = camera.physical_viewport_rect() else {
        return;
    };
    let view = transform.compute_affine().inverse();
    // The send-off lifts the clump (a render offset): the window follows it.
    let lifted = ball.pos + Vec3::Y * ritual.lift;
    let centre = view.transform_point3(lifted);
    let depth = -centre.z;
    // The clump reaches this far from the ball's centre: attached items must never be cut.
    let extent = trophies
        .iter()
        .map(|a| a.radial + a.size)
        .fold(ball.radius, f32::max);
    if depth <= extent {
        return;
    }
    let clip = perspective.get_clip_from_view() * centre.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let size = viewport.size().as_vec2();
    let pixel = viewport.min.as_vec2() + (Vec2::new(ndc.x, -ndc.y) + Vec2::ONE) * size * 0.5;
    let indoors = world.indoors_at(ball.pos - Vec3::Y * ball.radius);
    // The fixed-size Prince must fit in the opening even beside a crumb-sized core.
    let seen = extent
        .clamp(ball.radius, ball.radius * CLUMP_WIDTH)
        .max(if indoors { 0.25 } else { 0.0 });
    let projected_radius = size.y * 0.5 / (perspective.fov * 0.5).tan() * seen
        / (depth * depth - seen * seen).max(1.0e-4).sqrt();
    let celebration = flow.finale.map_or(0.0, |t| {
        let k = (t / 0.8).clamp(0.0, 1.0);
        k * k * (3.0 - 2.0 * k)
    });
    let window = if indoors {
        WINDOW_INDOORS * (1.0 + 0.3 * celebration)
    } else {
        WINDOW_OPEN
    } * if ritual.active() { 1.25 } else { 1.0 };
    cutout.screen = pixel
        .extend(projected_radius * window)
        .extend(ball.pos.y - 0.98 * ball.radius);
    // `WOW_AZEROLL_CAM_TRACE=1`: once a second, the framing and the window.
    *trace_at += time.delta_secs();
    if *trace_at >= 1.0 && crate::webenv::var("WOW_AZEROLL_CAM_TRACE").is_some() {
        *trace_at = 0.0;
        info!(
            "azeroll cam: r={:.2} extent={extent:.2} boom={:.2} depth={depth:.2} pitch={:.1}deg window={:.0}px at ({:.0},{:.0}) indoors={indoors}",
            ball.radius,
            rig.eye.distance(rig.look),
            (rig.eye - rig.look).y.atan2((rig.eye - rig.look).xz().length()).to_degrees(),
            projected_radius * window,
            pixel.x,
            pixel.y,
        );
    }
    // Cut what is nearer than the clump and above the floor; the rest stays solid.
    cutout.limits = Vec4::new(
        depth - extent * (1.0 + DEPTH_MARGIN) - DEPTH_MARGIN_ABS,
        ball.pos.y - ball.radius * FLOOR_BELOW,
        1.0,
        // A second, stricter near-side plane bypasses the contact-band floor. It clears
        // chair rails, legs and foreground ground between lens and clump, while keeping
        // trophies and the fixed-scale Prince inside a protected depth envelope.
        depth - (1.5 * ball.radius + 0.20).max(extent + 0.22),
    );
}
