//! Optional outdoor decomposition of a shared WMO into independently retired structural parts.
//! The source asset stays immutable. Streets/foundations keep their own static collider.
use super::*;
use benilla_assets::coords::wow_to_bevy;
use benilla_assets::{ModelSubmesh, WmoGroupNav};
use benilla_formats::CollisionMesh;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct WmoPartRegion {
    pub groups: Vec<u16>,
    pub min: [f32; 3],
    pub max: [f32; 3],
}
#[derive(Clone, Debug)]
pub struct WmoPartSpec {
    pub id: u16,
    pub groups: Vec<u16>,
    pub regions: Vec<WmoPartRegion>,
    /// Lowest floor retained as permanent support, in raw WMO coordinates.
    pub floor: f32,
    /// A permanent foundation quad covering this raw-XY rectangle; seals basement stair holes.
    pub foundation: Option<[f32; 4]>,
}
impl WmoPartSpec {
    fn owns(&self, g: u16, p: [f32; 3]) -> bool {
        self.groups.contains(&g)
            || self.regions.iter().any(|r| {
                r.groups.contains(&g) && (0..3).all(|a| p[a] >= r.min[a] && p[a] <= r.max[a])
            })
    }
}
#[derive(Clone, Debug)]
pub struct WmoPartitionSpec {
    pub expected_groups: usize,
    pub fingerprint: u64,
    pub exterior_only: bool,
    /// Outdoor mode structures may promote camera-only DETAIL faces to solid walking faces.
    pub solid_parts: bool,
    /// Optional visible skin for permanent walking faces in authored raw-XYZ bounds.
    /// Uses a floor material from `groups`; never adds or changes collision faces.
    pub support_surfaces: Vec<WmoPartRegion>,
    /// Decorative backdrop groups that become invalid when structural pieces disappear.
    pub omit_render_groups: Vec<u16>,
    pub parts: Vec<WmoPartSpec>,
}

pub struct PartitionedWmo {
    pub model: Handle<WmoModel>,
    /// Appended synthetic group IDs, keyed by stable recipe part ID.
    pub groups: HashMap<u16, u16>,
    pub bounds: HashMap<u16, (Vec3, Vec3)>,
    prop_groups: Vec<u16>,
}
#[derive(Resource, Default)]
pub struct WmoPartitions {
    pub plans: HashMap<(u32, u32), WmoPartitionSpec>,
    pub ready: HashMap<(u32, u32), PartitionedWmo>,
    retired: HashSet<(u32, u32, u16)>,
    failed: HashSet<(u32, u32)>,
    instances: HashMap<Entity, (u32, u32)>,
    scope_map: Option<u32>,
}
impl WmoPartitions {
    pub fn hidden(&self, instance: Entity, groups: &[u16]) -> bool {
        let Some(key) = self.instances.get(&instance) else {
            return false;
        };
        let Some(ready) = self.ready.get(key) else {
            return false;
        };
        !groups.is_empty()
            && groups.iter().all(|g| {
                ready.groups.iter().any(|(part, group)| {
                    group == g && self.retired.contains(&(key.0, key.1, *part))
                })
            })
    }
    pub fn enabled(&self, instance: Entity) -> bool {
        self.instances.contains_key(&instance)
    }
    pub(super) fn removable(&self, map: u32, uid: u32, groups: &[u16]) -> bool {
        self.plans
            .get(&(map, uid))
            .is_some_and(|s| groups.iter().any(|g| *g as usize >= s.expected_groups))
    }
    pub(super) fn removed_group(&self, map: u32, uid: u32, g: u16) -> bool {
        self.ready.get(&(map, uid)).is_some_and(|r| {
            r.groups
                .iter()
                .any(|(part, group)| *group == g && self.removed(map, uid, *part))
        })
    }
    pub fn removed_count(&self, map: u32, uid: u32) -> usize {
        self.retired
            .iter()
            .filter(|(m, u, _)| *m == map && *u == uid)
            .count()
    }
    pub fn removed(&self, map: u32, uid: u32, part: u16) -> bool {
        self.retired.contains(&(map, uid, part))
    }
    pub(super) fn effective<'a>(
        &'a self,
        map: u32,
        uid: u32,
        original: &'a Handle<WmoModel>,
    ) -> Option<&'a Handle<WmoModel>> {
        if self.failed.contains(&(map, uid)) {
            return Some(original);
        }
        if self.plans.contains_key(&(map, uid)) {
            self.ready.get(&(map, uid)).map(|r| &r.model)
        } else {
            Some(original)
        }
    }
}
#[derive(Component, Clone, Copy)]
pub(super) struct PieceProduct {
    pub map: u32,
    pub uid: u32,
    pub group: u16,
}
#[derive(Message)]
pub struct RetireWmoPart {
    pub map: u32,
    pub uid: u32,
    pub part: u16,
}
#[derive(Message)]
pub struct WmoPartRetired {
    pub map: u32,
    pub uid: u32,
    pub part: u16,
    pub accepted: bool,
}
#[derive(Clone)]
pub struct WmoPartSnapshot {
    pub map: u32,
    pub uid: u32,
    pub part: u16,
    pub group: u16,
    pub model: Handle<WmoModel>,
    pub original: Handle<WmoModel>,
    pub instance: Entity,
    pub transform: Transform,
    pub min: Vec3,
    pub max: Vec3,
    pub colliders: Vec<Entity>,
    /// Permanent structural hulls belonging to this placement; never retired by a part.
    pub support_colliders: Vec<Entity>,
    pub ready: bool,
}
#[derive(Resource, Default)]
pub struct WmoParts(pub Vec<WmoPartSnapshot>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<WmoPartitions>()
        .init_resource::<WmoParts>()
        .add_message::<RetireWmoPart>()
        .add_message::<WmoPartRetired>();
}
fn owner(spec: &WmoPartitionSpec, g: u16, t: &[[f32; 3]; 3]) -> Option<usize> {
    let p = std::array::from_fn(|a| (t[0][a] + t[1][a] + t[2][a]) / 3.0);
    spec.parts
        .iter()
        .position(|part| part.owns(g, p) && t.iter().any(|v| v[2] > part.floor + 0.05))
}
fn floor_material<'a>(
    source: &'a WmoModel,
    groups: &[u16],
    height: f32,
) -> Option<&'a ModelSubmesh> {
    let candidates = || {
        source
            .submeshes
            .iter()
            .zip(&source.submesh_group)
            .filter(|(_, g)| groups.contains(g))
            .map(|(s, _)| s)
    };
    candidates()
        .filter_map(|s| {
            if s.blend != benilla_formats::ModelBlend::Opaque || s.env_map || s.texture.is_none() {
                return None;
            }
            let area = s
                .geometry
                .indices
                .chunks_exact(3)
                .map(|t| {
                    let [a, b, c] = std::array::from_fn(|i| {
                        Vec3::from_array(s.geometry.positions[t[i] as usize])
                    });
                    let normal = (b - a).cross(c - a);
                    if normal.z <= 0.6 * normal.length() {
                        return 0.;
                    }
                    normal.z / (1. + ((a.z + b.z + c.z) / 3. - height).abs()).powi(2)
                })
                .sum::<f32>();
            (area > 0.).then_some((s, area))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| s)
        .or_else(|| candidates().next())
}
/// The source floor's affine XY -> UV mapping, including its authored texel density.
fn floor_mapping(s: &ModelSubmesh) -> (Vec2, Vec2, Mat2) {
    let g = &s.geometry;
    let face = g
        .indices
        .chunks_exact(3)
        .filter_map(|t| {
            let [a, b, c] = std::array::from_fn(|i| Vec3::from_array(g.positions[t[i] as usize]));
            let normal = (b - a).cross(c - a);
            (normal.z > 0.6 * normal.length()).then_some((t, normal.z))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((t, _)) = face {
        let [a, b, c] = std::array::from_fn(|i| {
            Vec2::from_array([g.positions[t[i] as usize][0], g.positions[t[i] as usize][1]])
        });
        let [u, v, w] = std::array::from_fn(|i| Vec2::from_array(g.uvs[t[i] as usize]));
        let xy = Mat2::from_cols(b - a, c - a);
        let uv = Mat2::from_cols(v - u, w - u);
        if xy.determinant().abs() > 0.0001 && uv.determinant().abs() > 0.0001 {
            return (a, u, uv * xy.inverse());
        }
    }
    (
        Vec2::ZERO,
        Vec2::ZERO,
        Mat2::from_diagonal(Vec2::splat(0.25)),
    )
}
fn mapped_uv(mapping: &(Vec2, Vec2, Mat2), p: [f32; 3]) -> [f32; 2] {
    (mapping.1 + mapping.2 * (Vec2::new(p[0], p[1]) - mapping.0)).to_array()
}
/// Clip render-only support to its authored volume. Collision meshes often contain
/// enormous padding faces: centroid admission would turn one into an endless floor.
fn clipped_surface(tri: [[f32; 3]; 3], region: &WmoPartRegion) -> Vec<[[f32; 3]; 3]> {
    let lo = Vec3::from_array(region.min);
    let hi = Vec3::from_array(region.max);
    if (0..3).any(|axis| {
        tri.iter().all(|p| p[axis] < lo[axis]) || tri.iter().all(|p| p[axis] > hi[axis])
    }) {
        return vec![];
    }
    let mut polygon: Vec<Vec3> = tri.into_iter().map(Vec3::from_array).collect();
    for axis in 0..3 {
        for (bound, lower) in [(lo[axis], true), (hi[axis], false)] {
            let mut output = Vec::new();
            let Some(mut previous) = polygon.last().copied() else {
                return vec![];
            };
            let inside = |p: Vec3| {
                if lower {
                    p[axis] >= bound
                } else {
                    p[axis] <= bound
                }
            };
            for current in polygon {
                if inside(current) != inside(previous) {
                    let t = (bound - previous[axis]) / (current[axis] - previous[axis]);
                    let mut edge = previous.lerp(current, t);
                    edge[axis] = bound;
                    output.push(edge);
                }
                if inside(current) {
                    output.push(current);
                }
                previous = current;
            }
            polygon = output;
        }
    }
    polygon.iter_mut().for_each(|p| *p = p.clamp(lo, hi));
    (1..polygon.len().saturating_sub(1))
        .filter_map(|i| {
            let t = [polygon[0], polygon[i], polygon[i + 1]];
            ((t[1] - t[0]).cross(t[2] - t[0]).length_squared() > 1e-10)
                .then_some(t.map(|p| p.to_array()))
        })
        .collect()
}
fn subset(s: &ModelSubmesh, indices: &[u32]) -> ModelSubmesh {
    if indices.len() == s.geometry.indices.len() {
        return s.clone();
    }
    let mut out = s.clone();
    let src = &s.geometry;
    let mut geo = (**src).clone();
    let mut map = HashMap::new();
    let mut old = Vec::new();
    geo.indices = indices
        .iter()
        .map(|&v| {
            *map.entry(v).or_insert_with(|| {
                old.push(v as usize);
                (old.len() - 1) as u32
            })
        })
        .collect();
    geo.positions = old.iter().map(|&i| src.positions[i]).collect();
    geo.normals = if src.normals.is_empty() {
        vec![]
    } else {
        old.iter().map(|&i| src.normals[i]).collect()
    };
    geo.uvs = old.iter().map(|&i| src.uvs[i]).collect();
    geo.vertex_colors = if src.vertex_colors.is_empty() {
        vec![]
    } else {
        old.iter().map(|&i| src.vertex_colors[i]).collect()
    };
    geo.joints.clear();
    geo.weights.clear();
    out.geometry = Arc::new(geo);
    out
}
fn hull(tris: impl Iterator<Item = [[f32; 3]; 3]>) -> Option<CollisionMesh> {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for tri in tris {
        for p in tri {
            indices.push(positions.len() as u32);
            positions.push(p)
        }
    }
    (!indices.is_empty()).then_some(CollisionMesh { positions, indices })
}
/// Partitions every triangle exactly once. All unselected faces remain permanent.
pub fn partition(
    source: &WmoModel,
    spec: &WmoPartitionSpec,
) -> Result<(WmoModel, HashMap<u16, u16>, HashMap<u16, (Vec3, Vec3)>), String> {
    if source.source_fingerprint != spec.fingerprint
        || source.group_nav.len() != spec.expected_groups
    {
        return Err("WMO group fingerprint mismatch".into());
    }
    let mut ids = HashSet::new();
    if spec.parts.iter().any(|p| !ids.insert(p.id)) {
        return Err("duplicate part id".into());
    }
    let base = source.group_nav.len();
    let n = base + spec.parts.len();
    if n > u16::MAX as usize {
        return Err("too many WMO parts".into());
    }
    let groups: HashMap<_, _> = spec
        .parts
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id, (base + i) as u16))
        .collect();
    let mut out = WmoModel {
        wmo_id: source.wmo_id,
        source_fingerprint: source.source_fingerprint,
        group_nav: source.group_nav.clone(),
        group_bounds: source.group_bounds.clone(),
        doodads: source.doodads.clone(),
        doodad_sets: source.doodad_sets.clone(),
        doodad_base: source.doodad_base.clone(),
        doodad_owner: source.doodad_owner.clone(),
        doodad_groups: source.doodad_groups.clone(),
        group_liquids: source.group_liquids.clone(),
        group_light_refs: source.group_light_refs.clone(),
        material_ground_type: source.material_ground_type.clone(),
        material_diff_color: source.material_diff_color.clone(),
        ..Default::default()
    };
    out.portal_vertices.clear();
    out.portal_infos.clear();
    out.portal_refs.clear();
    out.skybox = None;
    out.group_nav.iter_mut().for_each(|g| {
        g.flags = 8;
        g.ref_count = 0;
        g.ref_start = 0;
    });
    out.group_bounds.iter_mut().for_each(|g| g.interior = false);
    out.group_collision_tris = vec![vec![]; n];
    out.group_camera_only_tris = vec![vec![]; n];
    for (sub, &g) in source.submeshes.iter().zip(&source.submesh_group) {
        if (spec.exterior_only && sub.interior) || spec.omit_render_groups.contains(&g) {
            continue;
        }
        let mut buckets: HashMap<u16, Vec<u32>> = HashMap::new();
        for t in sub.geometry.indices.chunks_exact(3) {
            let tri = std::array::from_fn(|a| sub.geometry.positions[t[a] as usize]);
            let dest = owner(spec, g, &tri).map_or(g, |i| (base + i) as u16);
            buckets.entry(dest).or_default().extend_from_slice(t);
        }
        let mut buckets: Vec<_> = buckets.into_iter().collect();
        buckets.sort_by_key(|b| b.0);
        for (dest, indices) in buckets {
            out.submeshes.push(subset(sub, &indices));
            out.submesh_group.push(dest);
        }
    }
    for (is_camera, input) in [
        (false, &source.group_collision_tris),
        (true, &source.group_camera_only_tris),
    ] {
        for (g, tris) in input.iter().enumerate() {
            for t in tris {
                let dest = owner(spec, g as u16, t).map_or(g, |i| base + i);
                if is_camera {
                    out.group_camera_only_tris[dest].push(*t)
                } else {
                    out.group_collision_tris[dest].push(*t)
                }
            }
        }
    }
    if spec.solid_parts {
        for g in base..n {
            let faces = std::mem::take(&mut out.group_camera_only_tris[g]);
            out.group_collision_tris[g].extend(faces);
        }
    }
    // Native source-isolation capture: leave physical support untouched.
    #[cfg(not(target_arch = "wasm32"))]
    let draw_support = std::env::var_os("WOW_WMO_NO_SUPPORT_SKIN").is_none();
    #[cfg(target_arch = "wasm32")]
    let draw_support = true;
    for surface in &spec.support_surfaces {
        let template = floor_material(source, &surface.groups, surface.min[2])
            .ok_or("support surface has no floor material")?;
        let faces: Vec<_> = out.group_collision_tris[..base]
            .iter()
            .flatten()
            .filter(|t| {
                let [a, b, c] = t.map(Vec3::from_array);
                let normal = (b - a).cross(c - a);
                normal.z > 0.6 * normal.length()
            })
            .flat_map(|t| clipped_surface(*t, surface))
            .collect();
        if faces.is_empty() {
            continue;
        }
        let mut s = template.clone();
        let mapping = floor_mapping(template);
        let mut geo = (*s.geometry).clone();
        geo.positions = faces
            .iter()
            .flat_map(|t| t.iter().map(|v| [v[0], v[1], v[2] - 0.035]))
            .collect();
        geo.normals = vec![[0., 0., 1.]; geo.positions.len()];
        geo.uvs = geo
            .positions
            .iter()
            .map(|v| mapped_uv(&mapping, *v))
            .collect();
        geo.indices = (0..geo.positions.len() as u32).collect();
        geo.vertex_colors = vec![[1.; 4]; geo.positions.len()];
        geo.interior = false;
        geo.joints.clear();
        geo.weights.clear();
        s.geometry = Arc::new(geo);
        s.interior = false;
        s.emissive = false;
        s.window = false;
        s.sidn = None;
        s.wmo_batch = None;
        s.ground_quad = None;
        if draw_support {
            out.submeshes.push(s);
            out.submesh_group.push(0);
        }
    }
    // Foundations use the source floor's material and sit just below the retained base floor.
    for (i, part) in spec.parts.iter().enumerate() {
        if let Some([x0, y0, x1, y1]) = part.foundation {
            // Batch order often begins with a room's ceiling or a crate. Choose an
            // opaque upward floor near this foundation, preserving the city's real
            // floor texture instead of spreading an arbitrary prop material over it.
            let material_groups: Vec<_> = part
                .groups
                .iter()
                .copied()
                .chain(part.regions.iter().flat_map(|r| r.groups.iter().copied()))
                .collect();
            let template = floor_material(source, &material_groups, part.floor)
                .ok_or("foundation has no material")?;
            let z = part.floor - 0.1;
            let vs = [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]];
            let mut s = template.clone();
            let mapping = floor_mapping(template);
            let mut geo = (*s.geometry).clone();
            geo.positions = vs.to_vec();
            geo.normals = vec![[0., 0., 1.]; 4];
            geo.uvs = vs.iter().map(|v| mapped_uv(&mapping, *v)).collect();
            geo.indices = vec![0, 1, 2, 0, 2, 3];
            geo.vertex_colors = vec![[1.; 4]; 4];
            geo.interior = false;
            geo.joints.clear();
            geo.weights.clear();
            s.geometry = Arc::new(geo);
            s.interior = false;
            s.wmo_batch = None;
            s.emissive = false;
            s.window = false;
            s.sidn = None;
            s.env_map = false;
            s.additive = false;
            s.blend = benilla_formats::ModelBlend::Opaque;
            s.no_depth_write = false;
            s.no_depth_test = false;
            s.ground_quad = None;
            if draw_support {
                out.submeshes.push(s);
                out.submesh_group.push(0);
            }
            out.group_collision_tris[0].extend([[vs[0], vs[1], vs[2]], [vs[0], vs[2], vs[3]]]);
        }
        out.group_nav.push(WmoGroupNav {
            flags: 8,
            bbox_min: [0.; 3],
            bbox_max: [0.; 3],
            ref_start: 0,
            ref_count: 0,
            area_table_id: 0,
            fog_indices: [0; 4],
            group_liquid: benilla_formats::NO_GROUP_LIQUID,
        });
        let _ = i;
    }
    out.group_liquids.resize(n, None);
    out.group_light_refs.resize(n, vec![]);
    out.group_footprints = vec![None; n];
    out.group_footprint_bounds = vec![None; n];
    out.group_footprint_grids = vec![None; n];
    // This mode representation is deliberately outdoor: removed rooms can never own a subject.
    out.group_collision_bounds = vec![None; n];
    out.group_collision_grids = vec![None; n];
    out.collision_bounds = None;
    // Per-piece colliders are built directly from the group faces; no redundant city-wide mesh.
    let mut bounds: HashMap<u16, (Vec3, Vec3)> = HashMap::new();
    for (s, &g) in out.submeshes.iter().zip(&out.submesh_group) {
        if g as usize >= base {
            let id = spec.parts[g as usize - base].id;
            let b = bounds
                .entry(id)
                .or_insert((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)));
            for &v in &s.geometry.indices {
                let p = wow_to_bevy(s.geometry.positions[v as usize]);
                b.0 = b.0.min(p);
                b.1 = b.1.max(p);
            }
        }
    }
    for part in &spec.parts {
        if !bounds.get(&part.id).is_some_and(|(lo, hi)| {
            lo.is_finite() && hi.is_finite() && (*hi - *lo).min_element() > 0.01
        }) {
            return Err(format!(
                "part {} has no complete three-dimensional exterior",
                part.id
            ));
        }
    }
    for (di, d) in out.doodads.iter().enumerate() {
        let group = spec
            .parts
            .iter()
            .position(|part| {
                source
                    .doodad_groups
                    .get(di)
                    .is_some_and(|gs| gs.iter().any(|g| part.owns(*g, d.position)))
            })
            .map(|i| (base + i) as u16)
            .unwrap_or(0);
        out.doodad_groups[di] = Arc::from([group]);
        out.doodad_owner[di] = Some(group);
    }
    Ok((out, groups, bounds))
}
pub(super) fn prepare(
    mut partitions: ResMut<WmoPartitions>,
    placements: Res<Placements>,
    mut models: ResMut<Assets<WmoModel>>,
) {
    if partitions.plans.is_empty() {
        return;
    }
    for (&uid, p) in &placements.by_id {
        let key = (p.map, uid);
        // The old map's placements still exist until stream_terrain performs the swap later
        // in this chain. Never re-create an evicted old-map variant during that interval.
        if partitions.scope_map != Some(p.map) {
            continue;
        }
        if partitions.ready.contains_key(&key) || partitions.failed.contains(&key) {
            continue;
        }
        let Some(spec) = partitions.plans.get(&key) else {
            continue;
        };
        let ModelHandle::Wmo(h) = &p.model else {
            continue;
        };
        let Some(m) = models.get(h) else { continue };
        let started = bevy::platform::time::Instant::now();
        match partition(m, spec) {
            Ok((m, groups, bounds)) => {
                info!(
                    "WMO partition uid={uid}: {} parts, {} batches, {:.1}ms",
                    groups.len(),
                    m.submeshes.len(),
                    started.elapsed().as_secs_f64() * 1000.
                );
                audit(&m, spec, &groups, &bounds);
                let prop_groups = m
                    .doodad_groups
                    .iter()
                    .map(|g| g.first().copied().unwrap_or(0))
                    .collect();
                let model = models.add(m);
                partitions.ready.insert(
                    key,
                    PartitionedWmo {
                        model,
                        groups,
                        bounds,
                        prop_groups,
                    },
                );
            }
            Err(e) => {
                warn!("WMO partition uid={uid} rejected, keeping original placement: {e}");
                partitions.failed.insert(key);
            }
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn audit(
    m: &WmoModel,
    spec: &WmoPartitionSpec,
    groups: &HashMap<u16, u16>,
    bounds: &HashMap<u16, (Vec3, Vec3)>,
) {
    let Ok(path) = std::env::var("WOW_WMO_PART_AUDIT") else {
        return;
    };
    use std::fmt::Write;
    let mut report = format!(
        "version = 1\nroot_fingerprint = \"{:016x}\"\nsource_groups = {}\n",
        spec.fingerprint, spec.expected_groups
    );
    let mut ids: Vec<_> = groups.keys().copied().collect();
    ids.sort();
    for id in ids {
        let group = groups[&id];
        let Some(&(lo, hi)) = bounds.get(&id) else {
            continue;
        };
        let axes = hi - lo;
        let render: usize = m
            .submeshes
            .iter()
            .zip(&m.submesh_group)
            .filter(|(_, g)| **g == group)
            .map(|(s, _)| s.geometry.indices.len() / 3)
            .sum();
        let props = m
            .doodad_groups
            .iter()
            .filter(|g| g.contains(&group))
            .count();
        let _ = writeln!(
            report,
            "\n[[parts]]\nid = {id}\ngroup = {group}\nmin = {:?}\nmax = {:?}\nmax_axis = {}\nbox_volume = {}\nrender_triangles = {render}\nwalk_triangles = {}\nmodds = {props}",
            lo.to_array(),
            hi.to_array(),
            axes.max_element(),
            axes.x * axes.y * axes.z,
            m.group_collision_tris[group as usize].len()
        );
    }
    if let Err(e) = std::fs::write(path, report) {
        warn!("WMO partition audit: {e}");
    }
}
#[cfg(target_arch = "wasm32")]
fn audit(
    _: &WmoModel,
    _: &WmoPartitionSpec,
    _: &HashMap<u16, u16>,
    _: &HashMap<u16, (Vec3, Vec3)>,
) {
}
pub(super) fn transactions(
    mut commands: Commands,
    mut requests: MessageReader<RetireWmoPart>,
    mut replies: MessageWriter<WmoPartRetired>,
    mut restore: MessageReader<RestoreWmos>,
    mut partitions: ResMut<WmoPartitions>,
    mut placements: ResMut<Placements>,
    products: Query<(Entity, &PieceProduct)>,
    mut released: MessageWriter<WmoPropReleased>,
    mut welds: ResMut<HullWelds>,
    mut merge: ResMut<StaticMerge>,
    mut residency: ResMut<WmoResidency>,
) {
    if restore.read().count() > 0 {
        // Rebuild a changed placement from its immutable source/partition asset, including every MODD.
        let changed: HashSet<_> = partitions.retired.drain().map(|(m, u, _)| (m, u)).collect();
        for (map, uid) in changed {
            if let Some(p) = placements
                .by_id
                .get_mut(&uid)
                .filter(|p| p.map == map && p.spawned && !p.suppressed)
            {
                let pending = p.doodads.iter().filter(|d| !d.spawned).count();
                for e in p.entities.drain(..) {
                    commands.entity(e).try_despawn();
                }
                for key in p.claimed_props.drain(..) {
                    released.write(WmoPropReleased { key });
                }
                p.doodads.clear();
                p.portal_instance = None;
                p.spawned = false;
                welds.retire(uid);
                merge.retire(uid);
                residency.invalidate_placements();
                placements.pending_spawns = placements.pending_spawns.saturating_sub(pending) + 1;
            }
        }
        requests.clear();
        return;
    }
    for r in requests.read() {
        let key = (r.map, r.uid);
        let group = partitions
            .ready
            .get(&key)
            .and_then(|p| p.groups.get(&r.part))
            .copied();
        let accepted = group.is_some()
            && !partitions.retired.contains(&(r.map, r.uid, r.part))
            && placements
                .by_id
                .get(&r.uid)
                .is_some_and(|p| p.map == r.map && p.spawned && !p.suppressed);
        if accepted {
            let g = group.unwrap();
            partitions.retired.insert((r.map, r.uid, r.part));
            for (e, p) in &products {
                if p.map == r.map && p.uid == r.uid && p.group == g {
                    commands.entity(e).try_despawn();
                }
            }
            if let Some(p) = placements.by_id.get_mut(&r.uid) {
                let mut cancelled = 0;
                for d in &mut p.doodads {
                    if d.groups.contains(&g) && !d.spawned {
                        d.spawned = true;
                        cancelled += 1;
                    }
                }
                let owned = &partitions.ready[&key].prop_groups;
                p.claimed_props.retain(|key| {
                    if owned.get(key.modd as usize) == Some(&g) {
                        released.write(WmoPropReleased { key: *key });
                        false
                    } else {
                        true
                    }
                });
                placements.pending_spawns = placements.pending_spawns.saturating_sub(cancelled);
            }
        }
        replies.write(WmoPartRetired {
            map: r.map,
            uid: r.uid,
            part: r.part,
            accepted,
        });
    }
}
/// Ungated, before the live stream chain: neither logout nor a cross-map swap may leave
/// strong model handles in mode-facing caches. Authoring and run tombstones survive.
pub(super) fn release_scope(
    live: Res<crate::schedule::WorldLive>,
    focus: Res<ViewFocus>,
    current: Option<Res<CurrentMap>>,
    mut partitions: ResMut<WmoPartitions>,
    mut parts: ResMut<WmoParts>,
    mut whole: ResMut<retirement::WmoPlacements>,
    mut previous: Local<Option<u32>>,
) {
    let map = live.0.then(|| focus.map(current.as_ref().map(|m| m.0)));
    partitions.bypass_change_detection().scope_map = map;
    if *previous == map {
        return;
    }
    partitions.ready.retain(|(m, _), _| Some(*m) == map);
    partitions.failed.retain(|(m, _)| Some(*m) == map);
    partitions.instances.clear();
    parts.0.clear();
    whole.placements.clear();
    *previous = map;
}

#[allow(clippy::type_complexity)]
pub(super) fn snapshots(
    placements: Res<Placements>,
    mut partitions: ResMut<WmoPartitions>,
    mut out: ResMut<WmoParts>,
    products: Query<
        (
            Entity,
            &PieceProduct,
            Has<PendingCollider>,
            Has<avian3d::prelude::Collider>,
        ),
        Or<(With<PendingCollider>, With<avian3d::prelude::Collider>)>,
    >,
    changed: Query<
        (),
        (
            Or<(With<PendingCollider>, With<avian3d::prelude::Collider>)>,
            Or<(
                Changed<PieceProduct>,
                Added<PendingCollider>,
                Added<avian3d::prelude::Collider>,
            )>,
        ),
    >,
    mut removed_products: RemovedComponents<PieceProduct>,
    mut removed_pending: RemovedComponents<PendingCollider>,
    mut removed_colliders: RemovedComponents<avian3d::prelude::Collider>,
) {
    if partitions.plans.is_empty() {
        return;
    }
    let removed = removed_products.read().count()
        + removed_pending.read().count()
        + removed_colliders.read().count()
        > 0;
    // Placements is mutably borrowed by the streaming driver even on a settled frame.
    // Compare its small residency index rather than relying on the resource change tick.
    let resident = placements.by_id.iter().filter(|(uid, p)| {
        p.spawned
            && !p.suppressed
            && p.portal_instance.is_some()
            && partitions.ready.contains_key(&(p.map, **uid))
    });
    let residency_changed = resident.clone().count() != partitions.instances.len()
        || resident.clone().any(|(&uid, p)| {
            partitions.instances.get(&p.portal_instance.unwrap()) != Some(&(p.map, uid))
                || out
                    .0
                    .iter()
                    .any(|s| s.uid == uid && s.map == p.map && s.transform != p.transform)
        });
    if !removed && changed.is_empty() && !residency_changed {
        return;
    }
    // One pass over collider owners, never a mesh/light scan for each authored part.
    #[derive(Default)]
    struct Hulls {
        pending: bool,
        entities: Vec<Entity>,
    }
    let mut index = HashMap::<(u32, u32, u16), Hulls>::new();
    for (entity, p, pending, solid) in &products {
        let row = index.entry((p.map, p.uid, p.group)).or_default();
        row.pending |= pending;
        if solid {
            row.entities.push(entity);
        }
    }
    out.0.clear();
    let mut instances = HashMap::new();
    for (&uid, p) in resident {
        let instance = p.portal_instance.unwrap();
        let key = (p.map, uid);
        let r = &partitions.ready[&key];
        let ModelHandle::Wmo(original) = &p.model else {
            continue;
        };
        let support = index.get(&(p.map, uid, 0));
        for (&part, &group) in &r.groups {
            let Some(&(min, max)) = r.bounds.get(&part) else {
                continue;
            };
            let hulls = index.get(&(p.map, uid, group));
            out.0.push(WmoPartSnapshot {
                map: p.map,
                uid,
                part,
                group,
                model: r.model.clone(),
                original: original.clone(),
                instance,
                transform: p.transform,
                min,
                max,
                colliders: hulls.map_or_else(Vec::new, |h| h.entities.clone()),
                support_colliders: support.map_or_else(Vec::new, |h| h.entities.clone()),
                ready: hulls.is_none_or(|h| !h.pending),
            });
        }
        instances.insert(instance, key);
    }
    if partitions.instances != instances {
        partitions.instances = instances;
    }
}

/// The same walking/camera face audiences as the ordinary whole-placement hull.
pub(super) fn collider_groups(
    m: &WmoModel,
    groups: std::ops::Range<usize>,
) -> (Option<CollisionMesh>, Option<CollisionMesh>) {
    let walk = hull(
        groups
            .clone()
            .flat_map(|g| m.group_collision_tris[g].iter().copied()),
    );
    let camera = hull(groups.flat_map(|g| {
        m.group_collision_tris[g]
            .iter()
            .chain(m.group_camera_only_tris[g].iter())
            .copied()
    }));
    (walk, camera)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    fn plan() -> WmoPartitionSpec {
        WmoPartitionSpec {
            expected_groups: 1,
            fingerprint: 7,
            exterior_only: true,
            solid_parts: false,
            support_surfaces: vec![],
            omit_render_groups: vec![],
            parts: vec![WmoPartSpec {
                id: 12,
                groups: vec![],
                regions: vec![WmoPartRegion {
                    groups: vec![0],
                    min: [0., 0., 0.],
                    max: [10.; 3],
                }],
                floor: 0.,
                foundation: None,
            }],
        }
    }
    fn render(positions: Vec<[f32; 3]>) -> ModelSubmesh {
        let count = positions.len();
        ModelSubmesh {
            geometry: Arc::new(benilla_formats::RenderSubmesh {
                positions,
                indices: (0..count as u32).collect(),
                uvs: vec![[0.; 2]; count],
                ..default()
            }),
            texture: None,
            skin_slot: None,
            geoset_id: 0,
            char_slot: None,
            blend: benilla_formats::ModelBlend::Opaque,
            two_sided: true,
            interior: false,
            emissive: false,
            icon_slot: false,
            sidn: None,
            window: false,
            additive: false,
            env_map: false,
            no_depth_write: false,
            no_depth_test: false,
            fog_policy: benilla_formats::FogPolicy::Scene,
            billboard: None,
            alpha_anim: None,
            uv_anim: None,
            uv_seq: None,
            uv_rot_seq: None,
            uv_scale_seq: None,
            rgb_anim: None,
            rgb_seq: None,
            wmo_batch: None,
            ground_quad: None,
        }
    }
    #[test]
    fn foundations_use_a_floor_material_instead_of_the_first_ceiling_batch() {
        let mut images = Assets::<Image>::default();
        let ceiling_texture = images.add(Image::default());
        let floor_texture = images.add(Image::default());
        let mut ceiling = render(vec![[0., 0., 5.], [0., 5., 5.], [5., 0., 5.]]);
        ceiling.texture = Some(ceiling_texture);
        let mut floor = render(vec![[0., 0., 0.], [5., 0., 0.], [0., 5., 0.]]);
        floor.texture = Some(floor_texture.clone());
        Arc::make_mut(&mut floor.geometry).uvs = vec![[0.2, 0.4], [2.7, 0.4], [0.2, 1.65]];
        let wall = render(vec![[0., 0., 0.], [0., 5., 0.], [0., 0., 5.]]);
        let source = WmoModel {
            submeshes: vec![ceiling, floor, wall],
            submesh_group: vec![0; 3],
            source_fingerprint: 7,
            group_nav: vec![WmoGroupNav {
                flags: 8,
                bbox_min: [0.; 3],
                bbox_max: [5.; 3],
                ref_start: 0,
                ref_count: 0,
                area_table_id: 0,
                fog_indices: [0; 4],
                group_liquid: benilla_formats::NO_GROUP_LIQUID,
            }],
            ..default()
        };
        let mut spec = plan();
        spec.parts[0].foundation = Some([0., 0., 5., 5.]);
        let (out, _, _) = partition(&source, &spec).unwrap();
        let foundation = out.submeshes.last().unwrap();
        assert_eq!(foundation.texture, Some(floor_texture));
        for (actual, expected) in
            foundation
                .geometry
                .uvs
                .iter()
                .zip([[0.2, 0.4], [2.7, 0.4], [2.7, 1.65], [0.2, 1.65]])
        {
            assert!(Vec2::from_array(*actual).distance(Vec2::from_array(expected)) < 0.001);
        }
        assert!(!foundation.interior && !foundation.no_depth_write);
    }
    #[test]
    fn mixed_group_partition_preserves_floor_and_every_collision_triangle() {
        let floor = [[0., 0., 0.], [5., 0., 0.], [0., 5., 0.]];
        let wall = [[0., 0., 0.], [0., 5., 0.], [0., 0., 5.]];
        let roof = [[0., 0., 5.], [5., 0., 5.], [0., 5., 5.]];
        let source = WmoModel {
            submeshes: vec![render(floor.into_iter().chain(wall).chain(roof).collect())],
            submesh_group: vec![0],
            source_fingerprint: 7,
            group_nav: vec![WmoGroupNav {
                flags: 8,
                bbox_min: [0.; 3],
                bbox_max: [10.; 3],
                ref_start: 0,
                ref_count: 0,
                area_table_id: 0,
                fog_indices: [0; 4],
                group_liquid: benilla_formats::NO_GROUP_LIQUID,
            }],
            group_collision_tris: vec![vec![floor, wall]],
            group_camera_only_tris: vec![vec![roof]],
            ..default()
        };
        let (m, groups, _) = partition(&source, &plan()).unwrap();
        assert_eq!(groups[&12], 1);
        assert_eq!(m.group_collision_tris[0], vec![floor]);
        assert_eq!(m.group_collision_tris[1], vec![wall]);
        assert_eq!(m.group_camera_only_tris[1], vec![roof]);
        assert_eq!(source.group_collision_tris[0].len(), 2);
        assert_eq!(
            m.submeshes
                .iter()
                .map(|s| s.geometry.indices.len())
                .sum::<usize>(),
            9
        );
        assert_eq!(m.submesh_group, vec![0, 1]);
        let mut solid = plan();
        solid.solid_parts = true;
        let (solid, _, _) = partition(&source, &solid).unwrap();
        assert_eq!(solid.group_collision_tris[0], vec![floor]);
        assert_eq!(solid.group_collision_tris[1], vec![wall, roof]);
        assert!(solid.group_camera_only_tris[1].is_empty());
        let mut surfaced = plan();
        surfaced.support_surfaces = vec![WmoPartRegion {
            groups: vec![0],
            min: [-1.; 3],
            max: [6.; 3],
        }];
        let (surfaced, _, _) = partition(&source, &surfaced).unwrap();
        let skin = surfaced.submeshes.last().unwrap();
        assert_eq!(skin.geometry.positions.len(), 3);
        assert!(skin
            .geometry
            .positions
            .iter()
            .all(|p| (p[2] + 0.035).abs() < 0.001));
        assert_eq!(surfaced.submesh_group.last(), Some(&0));
        assert_eq!(surfaced.group_collision_tris, m.group_collision_tris);
        // Collision-only support can be a map-sized triangle. Its centroid is not a
        // render bound: only the intersection with the authored footprint may draw.
        let mut bounded = plan();
        bounded.support_surfaces = vec![WmoPartRegion {
            groups: vec![0],
            min: [0., 0., -1.],
            max: [2., 2., 1.],
        }];
        let (bounded, _, _) = partition(&source, &bounded).unwrap();
        let skin = bounded.submeshes.last().unwrap();
        assert!(skin
            .geometry
            .positions
            .iter()
            .all(|p| { p[0] >= 0. && p[0] <= 2. && p[1] >= 0. && p[1] <= 2. }));
        assert_eq!(bounded.group_collision_tris, m.group_collision_tris);
        assert!(m.collision_bounds.is_none() && m.portal_refs.is_empty());
        let mut stale = plan();
        stale.fingerprint = 8;
        assert!(partition(&source, &stale).is_err());
    }
    #[test]
    fn world_scope_releases_handles_but_keeps_plans_and_run_tombstones() {
        fn refs(h: &Handle<WmoModel>) -> usize {
            match h {
                Handle::Strong(h) => Arc::strong_count(h),
                _ => 0,
            }
        }
        let mut app = App::new();
        plugin(&mut app);
        retirement::plugin(&mut app);
        app.init_resource::<ViewFocus>()
            .insert_resource(crate::schedule::WorldLive(true))
            .add_systems(Update, release_scope);
        let mut assets = Assets::<WmoModel>::default();
        let original = assets.add(WmoModel::default());
        let derived = assets.add(WmoModel::default());
        let baseline = (refs(&original), refs(&derived));
        app.update(); // enter map0
        let instance = app.world_mut().spawn_empty().id();
        {
            let mut p = app.world_mut().resource_mut::<WmoPartitions>();
            p.plans.insert((0, 42), plan());
            p.retired.insert((0, 42, 12));
            p.ready.insert(
                (0, 42),
                PartitionedWmo {
                    model: derived.clone(),
                    groups: HashMap::new(),
                    bounds: HashMap::new(),
                    prop_groups: vec![],
                },
            );
            p.instances.insert(instance, (0, 42));
        }
        app.world_mut()
            .resource_mut::<WmoParts>()
            .0
            .push(WmoPartSnapshot {
                map: 0,
                uid: 42,
                part: 12,
                group: 1,
                model: derived.clone(),
                original: original.clone(),
                instance,
                transform: Transform::IDENTITY,
                min: Vec3::ZERO,
                max: Vec3::ONE,
                colliders: vec![],
                support_colliders: vec![],
                ready: true,
            });
        app.world_mut()
            .resource_mut::<WmoPlacements>()
            .placements
            .push(WmoPlacementSnapshot {
                map: 0,
                uid: 42,
                model: original.clone(),
                transform: Transform::IDENTITY,
                instance: Some(instance),
            });
        assert_eq!(
            (refs(&original), refs(&derived)),
            (baseline.0 + 2, baseline.1 + 2)
        );
        app.insert_resource(CurrentMap(1));
        app.update(); // cross-map cleanup runs even before the destination streams
        assert_eq!((refs(&original), refs(&derived)), baseline);
        assert!(app.world().resource::<WmoParts>().0.is_empty());
        assert!(app
            .world()
            .resource::<WmoPlacements>()
            .placements
            .is_empty());
        assert!(app.world().resource::<WmoPartitions>().instances.is_empty());
        assert!(app
            .world()
            .resource::<WmoPartitions>()
            .plans
            .contains_key(&(0, 42)));
        assert!(app.world().resource::<WmoPartitions>().removed(0, 42, 12));
        app.insert_resource(CurrentMap(0));
        app.update(); // reentry can install a fresh derived handle, retaining the tombstone
        app.world_mut()
            .resource_mut::<WmoPartitions>()
            .ready
            .insert(
                (0, 42),
                PartitionedWmo {
                    model: derived.clone(),
                    groups: HashMap::new(),
                    bounds: HashMap::new(),
                    prop_groups: vec![],
                },
            );
        app.world_mut()
            .resource_mut::<crate::schedule::WorldLive>()
            .0 = false;
        app.update(); // logout publishes empty caches without world_is_live
        assert_eq!((refs(&original), refs(&derived)), baseline);
        assert!(app.world().resource::<WmoPartitions>().ready.is_empty());
        assert!(app.world().resource::<WmoPartitions>().removed(0, 42, 12));
    }

    #[test]
    fn part_removal_is_scoped_and_ready_rebuilds_from_source() {
        let mut app = App::new();
        plugin(&mut app);
        app.init_resource::<Placements>()
            .init_resource::<HullWelds>()
            .init_resource::<StaticMerge>()
            .init_resource::<WmoResidency>()
            .add_message::<WmoPropReleased>()
            .add_message::<RestoreWmos>();
        let permanent = app.world_mut().spawn_empty().id();
        let removed = app
            .world_mut()
            .spawn(PieceProduct {
                map: 0,
                uid: 42,
                group: 1,
            })
            .id();
        let instance = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<Placements>().by_id.insert(
            42,
            Placement {
                model: ModelHandle::Wmo(Handle::default()),
                map: 0,
                claimed_props: vec![],
                prop_claims: WmoPropClaims::default(),
                claimed: false,
                suppressed: false,
                transform: Transform::IDENTITY,
                entities: vec![permanent, removed, instance],
                spawned: true,
                doodad_set: 0,
                name_set: 0,
                doodads: vec![],
                portal_instance: Some(instance),
                refs: 2,
                owner: (0, 0),
            },
        );
        app.world_mut()
            .resource_mut::<WmoPartitions>()
            .ready
            .insert(
                (0, 42),
                PartitionedWmo {
                    model: Handle::default(),
                    groups: HashMap::from([(12, 1)]),
                    bounds: HashMap::new(),
                    prop_groups: vec![],
                },
            );
        for _ in 0..2 {
            app.world_mut().write_message(RetireWmoPart {
                map: 0,
                uid: 42,
                part: 12,
            });
        }
        app.world_mut().run_system_once(transactions).unwrap();
        assert!(app.world().get_entity(removed).is_err());
        assert!(app.world().get_entity(permanent).is_ok());
        assert!(app.world().resource::<WmoPartitions>().removed(0, 42, 12));
        let replies = app.world().resource::<Messages<WmoPartRetired>>();
        let mut cursor = replies.get_cursor();
        assert_eq!(
            cursor.read(replies).map(|r| r.accepted).collect::<Vec<_>>(),
            [true, false]
        );
        app.world_mut().write_message(RestoreWmos);
        app.world_mut().run_system_once(transactions).unwrap();
        assert!(!app.world().resource::<WmoPartitions>().removed(0, 42, 12));
        let p = &app.world().resource::<Placements>().by_id[&42];
        assert!(!p.spawned && p.entities.is_empty());
        assert_eq!(p.refs, 2);
        assert_eq!(app.world().resource::<Placements>().pending_spawns, 1);
    }

    #[test]
    fn settled_part_snapshots_do_not_publish_again_until_hulls_change() {
        #[derive(Resource, Default)]
        struct Publications(usize);
        let mut app = App::new();
        plugin(&mut app);
        app.init_resource::<Placements>()
            .init_resource::<Publications>()
            .add_systems(
                Update,
                (
                    snapshots,
                    |parts: Res<WmoParts>, mut n: ResMut<Publications>| {
                        if parts.is_changed() {
                            n.0 += 1;
                        }
                    },
                )
                    .chain(),
            );
        let instance = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<Placements>().by_id.insert(
            42,
            Placement {
                model: ModelHandle::Wmo(Handle::default()),
                map: 0,
                claimed_props: vec![],
                prop_claims: WmoPropClaims::default(),
                claimed: false,
                suppressed: false,
                transform: Transform::IDENTITY,
                entities: vec![],
                spawned: true,
                doodad_set: 0,
                name_set: 0,
                doodads: vec![],
                portal_instance: Some(instance),
                refs: 2,
                owner: (0, 0),
            },
        );
        {
            let mut p = app.world_mut().resource_mut::<WmoPartitions>();
            p.plans.insert((0, 42), plan());
            p.ready.insert(
                (0, 42),
                PartitionedWmo {
                    model: Handle::default(),
                    groups: HashMap::from([(12, 1)]),
                    bounds: HashMap::from([(12, (Vec3::ZERO, Vec3::ONE))]),
                    prop_groups: vec![],
                },
            );
        }
        let hull = app
            .world_mut()
            .spawn((
                PieceProduct {
                    map: 0,
                    uid: 42,
                    group: 1,
                },
                avian3d::prelude::Collider::sphere(1.),
            ))
            .id();
        app.update();
        assert_eq!(app.world().resource::<WmoParts>().0[0].colliders, [hull]);
        assert_eq!(app.world().resource::<Publications>().0, 1);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Publications>().0, 1);
        app.world_mut().entity_mut(hull).despawn();
        app.update();
        assert_eq!(app.world().resource::<Publications>().0, 2);
        assert!(app.world().resource::<WmoParts>().0[0].colliders.is_empty());
    }

    #[test]
    fn whole_restore_owns_a_previously_part_retired_placement() {
        let mut app = App::new();
        plugin(&mut app);
        retirement::plugin(&mut app);
        app.init_resource::<Placements>()
            .init_resource::<HullWelds>()
            .init_resource::<StaticMerge>()
            .init_resource::<WmoResidency>()
            .add_message::<WmoPropReleased>();
        app.world_mut().resource_mut::<Placements>().by_id.insert(
            42,
            Placement {
                model: ModelHandle::Wmo(Handle::default()),
                map: 0,
                claimed_props: vec![],
                prop_claims: WmoPropClaims::default(),
                claimed: false,
                suppressed: false,
                transform: Transform::IDENTITY,
                entities: vec![],
                spawned: true,
                doodad_set: 0,
                name_set: 0,
                doodads: vec![],
                portal_instance: None,
                refs: 2,
                owner: (0, 0),
            },
        );
        app.world_mut()
            .resource_mut::<WmoPartitions>()
            .ready
            .insert(
                (0, 42),
                PartitionedWmo {
                    model: Handle::default(),
                    groups: HashMap::from([(12, 1), (13, 2)]),
                    bounds: HashMap::new(),
                    prop_groups: vec![],
                },
            );
        app.world_mut().write_message(RetireWmoPart {
            map: 0,
            uid: 42,
            part: 12,
        });
        app.world_mut().run_system_once(transactions).unwrap();
        app.world_mut()
            .write_message(retirement::RetireWmo { map: 0, uid: 42 });
        app.world_mut()
            .run_system_once(retirement::retire_wmos)
            .unwrap();
        app.world_mut().write_message(RetireWmoPart {
            map: 0,
            uid: 42,
            part: 13,
        });
        app.world_mut().run_system_once(transactions).unwrap();
        assert!(!app.world().resource::<WmoPartitions>().removed(0, 42, 13));
        app.world_mut().write_message(RestoreWmos);
        app.world_mut().run_system_once(transactions).unwrap();
        assert_eq!(app.world().resource::<Placements>().pending_spawns, 0);
        app.world_mut()
            .run_system_once(retirement::retire_wmos)
            .unwrap();
        let p = app.world().resource::<Placements>();
        assert_eq!(p.pending_spawns, 1);
        assert!(!p.by_id[&42].spawned && !p.by_id[&42].suppressed);
        assert_eq!(p.by_id[&42].refs, 2);
        assert!(!app.world().resource::<WmoPartitions>().removed(0, 42, 12));
    }
}
