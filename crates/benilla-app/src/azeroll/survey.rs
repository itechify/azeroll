//! Opt-in, read-only map survey. No systems or resources exist without WOW_AZEROLL_SURVEY.
//! Captures are local references; only numeric/path inventories belong in version control.
use std::{collections::BTreeMap, fmt::Write as _, path::PathBuf};

use benilla_assets::{
    coords::{bevy_to_wow, placement_rotation, wow_to_bevy},
    m2_url, wmo_url, AdtTile, M2Model, WmoModel,
};
use benilla_world::{
    schedule::WorldStage,
    terrain_stream::{ViewFocus, WorldLoadProgress},
    view::{ViewDistance, WorldCamera},
    world_point::{Subject, WorldPoint},
};
use bevy::{
    camera::ScalingMode,
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
};

use super::{level, BallState};

struct Placement {
    kind: &'static str,
    uid: u32,
    model: String,
    pos: [f32; 3],
    rotation: [f32; 3],
    scale: f32,
    m2: Option<Handle<M2Model>>,
    wmo: Option<Handle<WmoModel>>,
}

#[derive(Resource)]
struct Survey {
    dir: PathBuf,
    placements: BTreeMap<(u8, u32), Placement>,
    terrain: BTreeMap<(i32, i32), (f32, Option<f32>)>,
    step: usize,
    elapsed: f32,
    captured: bool,
    done: bool,
    started: bool,
}

pub(super) fn plugin(app: &mut App) {
    let Some(dir) = crate::webenv::var("WOW_AZEROLL_SURVEY") else {
        return;
    };
    let dir = PathBuf::from(dir);
    if let Err(e) = std::fs::create_dir_all(dir.join("img")) {
        error!("survey: cannot create output: {e}");
        return;
    }
    app.insert_resource(Survey {
        dir,
        placements: BTreeMap::new(),
        terrain: BTreeMap::new(),
        step: 0,
        elapsed: 0.0,
        captured: false,
        done: false,
        started: false,
    })
    .add_systems(
        Update,
        tick.after(WorldStage::Input).before(WorldStage::Stream),
    );
}

// North-up: rows run north to south, columns west to east. Nine 400-yd squares cover
// the entire 1200-yd bounding square of the 600-yd inventory circle.
fn shot(step: usize) -> (String, [f32; 3], bool) {
    if step < 9 {
        let row = step / 3;
        let col = step % 3;
        return (
            format!("overhead-{row}-{col}"),
            [
                level::START[0] + 400.0 - row as f32 * 400.0,
                level::START[1] + 400.0 - col as f32 * 400.0,
                60.0,
            ],
            true,
        );
    }
    let (name, x, y) = [
        ("inn", -9462.0, 16.0),
        ("blacksmith", -9455.0, 87.0),
        ("square", -9450.0, 60.0),
        ("north-road", -9300.0, -35.0),
        ("farms", -9840.0, 180.0),
        ("forest-edge", -9600.0, 260.0),
    ][step - 9];
    (name.into(), [x, y, 60.0], false)
}

#[allow(clippy::too_many_arguments)]
fn tick(
    mut commands: Commands,
    mut survey: ResMut<Survey>,
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    progress: Res<WorldLoadProgress>,
    mut ball: ResMut<BallState>,
    (mut focus, mut player): (ResMut<ViewFocus>, ResMut<crate::player::Player>),
    mut distance: ResMut<ViewDistance>,
    tiles: Res<Assets<AdtTile>>,
    (models, wmos): (Res<Assets<M2Model>>, Res<Assets<WmoModel>>),
    collision: benilla_world::collision::WorldCollision,
    server: Res<AssetServer>,
    world: WorldPoint,
    mut cameras: Query<(&mut Transform, &mut Projection), With<WorldCamera>>,
    mut ui: Query<&mut Visibility, With<Node>>,
    mut exit: MessageWriter<AppExit>,
) {
    if survey.done {
        return;
    }
    if !survey.started {
        if loading.covering() {
            return;
        }
        survey.started = true;
    }
    let (name, mut at, overhead) = shot(survey.step);
    if let Some(h) = world.terrain_height_under(wow_to_bevy(at)) {
        at[2] = h;
    }
    ball.pos = wow_to_bevy(at);
    ball.vel = Vec3::ZERO;
    player.pos = ball.pos;
    *focus = ViewFocus::body(at, true);
    // 550 yd covers a square corner without retaining an entire Elwynn-sized scene.
    distance.farclip = 550.0;
    for mut v in &mut ui {
        *v = Visibility::Hidden;
    }
    let look = wow_to_bevy(at);
    for (mut tf, mut projection) in &mut cameras {
        if overhead {
            *projection = Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: 400.0,
                    height: 400.0,
                },
                near: 0.1,
                far: 2000.0,
                ..OrthographicProjection::default_3d()
            });
            *tf = Transform::from_translation(look + Vec3::Y * 360.0).looking_at(look, Vec3::NEG_Z);
        } else {
            *projection = Projection::Perspective(PerspectiveProjection {
                far: 2000.0,
                ..default()
            });
            *tf = Transform::from_translation(look + Vec3::new(65.0, 85.0, 100.0))
                .looking_at(look, Vec3::Y);
        }
    }
    // Read all resident ADTs, including placements outside the collectible claim radius.
    for (_, tile) in tiles.iter() {
        for d in &tile.doodads {
            if in_radius(d.position) {
                survey
                    .placements
                    .entry((0, d.unique_id))
                    .or_insert_with(|| Placement {
                        kind: "doodad",
                        uid: d.unique_id,
                        model: d.model.clone(),
                        pos: d.position,
                        rotation: d.rotation,
                        scale: d.scale,
                        m2: Some(server.load(m2_url(&d.model))),
                        wmo: None,
                    });
            }
        }
        for w in &tile.wmos {
            if in_radius(w.position) {
                survey
                    .placements
                    .entry((1, w.unique_id))
                    .or_insert_with(|| Placement {
                        kind: "WMO",
                        uid: w.unique_id,
                        model: w.model.clone(),
                        pos: w.position,
                        rotation: w.rotation,
                        scale: 1.0,
                        m2: None,
                        wmo: Some(server.load(wmo_url(&w.model))),
                    });
            }
        }
    }
    // Restrict each pass to the settled focus neighbourhood. Missing ground stays absent;
    // a missing water surface is meaningful only once that neighbourhood has streamed.
    survey.elapsed += time.delta_secs();
    if survey.elapsed > 15.0 && progress.presentable() {
        for ix in -60..=60 {
            for iy in -60..=60 {
                let p = [
                    level::START[0] + ix as f32 * 10.0,
                    level::START[1] + iy as f32 * 10.0,
                    500.0,
                ];
                if (p[0] - at[0]).abs() > 240.0 || (p[1] - at[1]).abs() > 240.0 {
                    continue;
                }
                if let Some(h) = world.terrain_height_under(wow_to_bevy(p)) {
                    let water = world.water_surface_at(Subject::Player, [p[0], p[1], h + 0.1]);
                    survey.terrain.insert((ix, iy), (h, water));
                }
            }
        }
    }
    // Warm-up is timed in real time (Ready deliberately keeps gameplay stationary).
    if survey.elapsed >= 28.0 && progress.presentable() && !survey.captured {
        let path = survey.dir.join("img").join(format!("{name}.png"));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        info!(
            "survey: shot {} at {:?}, placements={} terrain={}",
            name,
            at,
            survey.placements.len(),
            survey.terrain.len()
        );
        if name == "inn" {
            let mut floors = String::from("x,y,ray_start_z,hit_z\n");
            for x in -9500..=-9420 {
                for y in -30..=70 {
                    for z in [62.0, 67.0, 72.0] {
                        if let Some(hit) = collision.ray_body(
                            wow_to_bevy([x as f32, y as f32, z]),
                            Dir3::NEG_Y,
                            30.0,
                        ) {
                            let _ = writeln!(floors, "{x},{y},{z},{}", z - hit.distance);
                        }
                    }
                }
            }
            if let Err(e) = std::fs::write(survey.dir.join("inn-floor-probes.csv"), floors) {
                error!("survey: floor output: {e}");
            }
        }
        survey.captured = true;
    }
    // A 55-second cap keeps a stuck streaming lane visible instead of waiting forever.
    if (survey.captured && survey.elapsed > 32.0) || survey.elapsed > 55.0 {
        if !survey.captured {
            warn!("survey: shot {name} skipped: streaming did not settle");
        }
        if let Err(e) = dump(&survey, &models, &wmos) {
            error!("survey: output failed: {e}");
        }
        survey.step += 1;
        survey.elapsed = 0.0;
        survey.captured = false;
        if survey.step == 15 {
            info!(
                "survey: complete, {} placements, {} terrain samples",
                survey.placements.len(),
                survey.terrain.len()
            );
            survey.done = true;
            exit.write(AppExit::Success);
        }
    }
}

fn in_radius(p: [f32; 3]) -> bool {
    (p[0] - level::START[0]).powi(2) + (p[1] - level::START[1]).powi(2) <= 600.0_f32.powi(2)
}

fn quote(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}
fn csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn dump(s: &Survey, models: &Assets<M2Model>, wmos: &Assets<WmoModel>) -> std::io::Result<()> {
    let mut json = String::from("[\n");
    let mut rows = String::from(
        "kind,uid,model,name,x,y,z,yaw_deg,scale,radius,footprint_x,footprint_y,min_z,max_z\n",
    );
    for (i, p) in s.placements.values().enumerate() {
        let mut radius = None;
        let mut bounds = None;
        if let Some(m) =
            p.m2.as_ref()
                .and_then(|h| models.get(h))
                .and_then(|m| m.bounds)
        {
            radius = Some(m.sphere_radius * p.scale);
        }
        if let Some(w) = p.wmo.as_ref().and_then(|h| wmos.get(h)) {
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            let mut local_lo = lo;
            let mut local_hi = hi;
            for g in &w.group_bounds {
                local_lo = local_lo.min(Vec3::from_array(g.bbox_min));
                local_hi = local_hi.max(Vec3::from_array(g.bbox_max));
                for x in [g.bbox_min[0], g.bbox_max[0]] {
                    for y in [g.bbox_min[1], g.bbox_max[1]] {
                        for z in [g.bbox_min[2], g.bbox_max[2]] {
                            let b = bevy_to_wow(
                                wow_to_bevy(p.pos)
                                    + placement_rotation(p.rotation) * wow_to_bevy([x, y, z]),
                            );
                            lo = lo.min(Vec3::from_array(b));
                            hi = hi.max(Vec3::from_array(b));
                        }
                    }
                }
            }
            if lo.is_finite() {
                radius = Some((local_hi - local_lo).length() * 0.5);
                bounds = Some((lo, hi));
            }
        }
        let r = radius.map_or("null".into(), |v| v.to_string());
        let b = bounds.map_or("null".into(), |(lo, hi)| {
            format!(
                "{{\"min\":{:?},\"max\":{:?}}}",
                lo.to_array(),
                hi.to_array()
            )
        });
        let name = super::names::display_name(&p.model);
        if i > 0 {
            json.push_str(",\n");
        }
        let _ = write!(
            json,
            "{{\"kind\":{},\"uid\":{},\"model\":{},\"name\":{},\"position\":{:?},\"rotation_deg\":{:?},\"yaw_deg\":{},\"scale\":{},\"radius\":{},\"world_aabb\":{}}}",
            quote(p.kind),
            p.uid,
            quote(&p.model),
            quote(&name),
            p.pos,
            p.rotation,
            p.rotation[1],
            p.scale,
            r,
            b
        );
        let extra = bounds.map_or(",,,".into(), |(lo, hi)| {
            format!("{},{},{},{}", hi.x - lo.x, hi.y - lo.y, lo.z, hi.z)
        });
        let _ = writeln!(
            rows,
            "{},{},{},{},{},{},{},{},{},{},{}",
            p.kind,
            p.uid,
            csv(&p.model),
            csv(&name),
            p.pos[0],
            p.pos[1],
            p.pos[2],
            p.rotation[1],
            p.scale,
            radius.map_or(String::new(), |v| v.to_string()),
            extra
        );
    }
    json.push_str("\n]\n");
    std::fs::write(s.dir.join("placements.json"), json)?;
    std::fs::write(s.dir.join("placements.csv"), rows)?;
    let mut terrain = String::from("x,y,height,water,water_height\n");
    for (&(ix, iy), &(h, w)) in &s.terrain {
        let _ = writeln!(
            terrain,
            "{},{},{},{},{}",
            level::START[0] + ix as f32 * 10.0,
            level::START[1] + iy as f32 * 10.0,
            h,
            u8::from(w.is_some()),
            w.map_or(String::new(), |v| v.to_string())
        );
    }
    std::fs::write(s.dir.join("terrain.csv"), terrain)
}
