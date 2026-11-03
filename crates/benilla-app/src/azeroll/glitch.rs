//! Debug: the per-frame movement trace behind the glitch polish pass.
//!
//! `WOW_AZEROLL_GLITCH=<csv>` writes one row per played frame: the ball's centre after each
//! stage that may move it (the mover, collection's solid push-outs, Unstuck, the city deck
//! guard, and the post-Stream building settles seen as the next frame's start), its velocity,
//! the support that settled it (terrain, an engine floor classified as a permanent city deck,
//! a live city piece or another WMO/doodad, or a mode prop top), the mover's step lift and
//! wall closing, the camera's eye and look point, and the real frame time. Discontinuities are
//! classified offline from the stage deltas. Off by default: no system runs without the lever.
//! `WOW_AZEROLL_GLITCH_SHOTS=<dir>:<t0>:<t1>:<every>` saves a frame sequence between two run times.
use super::*;
use benilla_world::terrain_stream::WmoParts;
use std::io::Write;

/// Stage marks within a frame, in schedule order.
const START: usize = 0;
const DRIVE: usize = 1;
const COLLECT: usize = 2;
const RECOVER: usize = 3;
const GUARD: usize = 4;

#[derive(Resource)]
struct Trace {
    out: std::io::BufWriter<std::fs::File>,
    marks: [Vec3; 5],
    frame: u64,
}

fn enabled() -> bool {
    crate::webenv::var("WOW_AZEROLL_GLITCH").is_some()
}

pub(super) fn plugin(app: &mut App) {
    if shot_window().is_some() {
        app.add_systems(Update, shots.run_if(playing));
    }
    let Some(path) = crate::webenv::var("WOW_AZEROLL_GLITCH") else {
        return;
    };
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            warn!("azeroll glitch: {path}: {e}");
            return;
        }
    };
    let mut out = std::io::BufWriter::new(file);
    let _ = writeln!(
        out,
        "frame,t,dt,r,grounded,input,x0,y0,z0,x1,y1,z1,x2,y2,z2,x3,y3,z3,x4,y4,z4,vx,vy,vz,\
         lift,support,floor,closing,eye_x,eye_y,eye_z,look_x,look_y,look_z,dist,wow_x,wow_y,wow_z,\
         raw_x,raw_y,raw_z,deck,heading,shown"
    );
    app.insert_resource(Trace {
        out,
        marks: [Vec3::ZERO; 5],
        frame: 0,
    })
    .add_systems(
        Update,
        (
            mark::<START>.before(ball::drive_ball),
            mark::<DRIVE>
                .after(ball::drive_ball)
                .before(collect::collect),
            mark::<COLLECT>
                .after(collect::collect)
                .before(unstuck::Recovery),
            mark::<RECOVER>
                .after(unstuck::Recovery)
                .before(city_support::guard),
            mark::<GUARD>
                .after(city_support::guard)
                .before(ball::publish_focus),
            row.after(camera::place_camera),
        )
            .in_set(WorldStage::Input)
            .run_if(enabled)
            .run_if(playing),
    );
}

fn mark<const S: usize>(ball: Res<BallState>, mut trace: ResMut<Trace>) {
    trace.marks[S] = ball.pos;
}

#[allow(clippy::too_many_arguments)]
fn row(
    real: Res<Time<Real>>,
    run: Res<Run>,
    ball: Res<BallState>,
    stall: Res<ball::StallTrace>,
    rig: Res<camera::CamRig>,
    parts: Res<WmoParts>,
    deck: Res<city_support::CityDeck>,
    shown: Res<ball::Shown>,
    mut trace: ResMut<Trace>,
) {
    trace.frame += 1;
    let floor = match stall.floor {
        None => "-".to_string(),
        Some(e) => parts
            .0
            .iter()
            .find_map(|p| {
                if p.support_colliders.contains(&e) {
                    Some("deck".to_string())
                } else if p.colliders.contains(&e) {
                    Some(format!("p{}", p.part))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "wmo".to_string()),
    };
    let wow = units::bevy_to_wow(ball.pos);
    let raw = parts
        .0
        .iter()
        .find(|p| p.uid == 10047)
        .map(|p| {
            benilla_assets::coords::bevy_to_wow(
                p.transform
                    .compute_affine()
                    .inverse()
                    .transform_point3(ball.pos),
            )
        })
        .unwrap_or([f32::NAN; 3]);
    let m = trace.marks;
    let frame = trace.frame;
    let cols = [
        frame.to_string(),
        format!("{:.4}", run.elapsed),
        format!("{:.5}", real.delta_secs()),
        format!("{:.4}", ball.radius),
        (ball.grounded as u8).to_string(),
        (stall.input as u8).to_string(),
        v3(m[START]),
        v3(m[DRIVE]),
        v3(m[COLLECT]),
        v3(m[RECOVER]),
        v3(m[GUARD]),
        v3(ball.vel),
        format!("{:.4}", stall.lift),
        format!("{:?}", stall.support),
        floor,
        format!("{:.3}", stall.closing),
        v3(rig.eye),
        v3(rig.look),
        format!("{:.3}", rig.dist),
        format!("{:.2},{:.2},{:.2}", wow[0], wow[1], wow[2]),
        format!("{:.2},{:.2},{:.2}", raw[0], raw[1], raw[2]),
        (deck.entered as u8).to_string(),
        format!("{:.4}", ball.heading),
        format!("{:.4}", shown.offset),
    ];
    let _ = writeln!(trace.out, "{}", cols.join(","));
    if frame % 120 == 0 {
        let _ = trace.out.flush();
    }
}

fn v3(v: Vec3) -> String {
    format!("{:.4},{:.4},{:.4}", v.x, v.y, v.z)
}

/// `WOW_AZEROLL_GLITCH_SHOTS=<dir>:<t0>:<t1>:<every>`.
fn shot_window() -> Option<(String, f32, f32, f32)> {
    let spec = crate::webenv::var("WOW_AZEROLL_GLITCH_SHOTS")?;
    let mut parts = spec.rsplitn(4, ':');
    let every = parts.next()?.parse().ok()?;
    let t1 = parts.next()?.parse().ok()?;
    let t0 = parts.next()?.parse().ok()?;
    Some((parts.next()?.to_string(), t0, t1, every))
}

fn shots(mut commands: Commands, run: Res<Run>, mut next: Local<Option<f32>>) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let Some((dir, t0, t1, every)) = shot_window() else {
        return;
    };
    let due = next.get_or_insert(t0);
    if run.elapsed < *due || run.elapsed > t1 {
        return;
    }
    *due = run.elapsed + every;
    let _ = std::fs::create_dir_all(&dir);
    let path = std::path::PathBuf::from(dir).join(format!("t{:07.2}.png", run.elapsed));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}
