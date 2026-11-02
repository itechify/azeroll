//! Screen-space canopy window. The engine resource defaults off for every other mode.
use super::{camera::CamRig, BallState};
use benilla_world::{lighting::FoliageCutout, view::WorldCamera};
use bevy::{camera::CameraProjection, prelude::*};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        publish.after(benilla_world::schedule::WorldStage::Input),
    );
}

fn publish(
    ball: Res<BallState>,
    rig: Res<CamRig>,
    cameras: Query<(&Camera, &Transform, &Projection), With<WorldCamera>>,
    mut cutout: ResMut<FoliageCutout>,
) {
    *cutout = FoliageCutout::default();
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
    let centre = view.transform_point3(ball.pos);
    let depth = -centre.z;
    if depth <= ball.radius {
        return;
    }
    let clip = perspective.get_clip_from_view() * centre.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let size = viewport.size().as_vec2();
    let pixel = viewport.min.as_vec2() + (Vec2::new(ndc.x, -ndc.y) + Vec2::ONE) * size * 0.5;
    let projected_radius = size.y * 0.5 / (perspective.fov * 0.5).tan() * ball.radius
        / (depth * depth - ball.radius * ball.radius).sqrt();
    cutout.screen = pixel.extend(projected_radius * 1.5).extend(0.0);
    // Preserve the contact band even on foliage cards and all fragments beyond the near clump.
    cutout.limits = Vec4::new(
        depth - ball.radius * 0.3,
        ball.pos.y + ball.radius * 0.6,
        1.0,
        0.0,
    );
}
