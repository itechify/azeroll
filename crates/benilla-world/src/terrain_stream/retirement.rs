//! Optional placement lifecycle seam. Suppression retains MODF identity/refcounts, not products.
use super::*;

#[derive(Clone)]
pub struct WmoPlacementSnapshot {
    pub map: u32,
    pub uid: u32,
    pub model: Handle<WmoModel>,
    pub transform: Transform,
    pub instance: Option<Entity>,
}
#[derive(Resource, Default)]
pub struct WmoPlacements {
    pub enabled: bool,
    pub placements: Vec<WmoPlacementSnapshot>,
}

/// Caller must prepare its visual replacement and safe support before requesting retirement.
#[derive(Message)]
pub struct RetireWmo {
    pub map: u32,
    pub uid: u32,
}
/// Acknowledged only after all placement products have been queued for removal in Stream.
#[derive(Message)]
pub struct WmoRetired {
    pub map: u32,
    pub uid: u32,
    pub accepted: bool,
}
#[derive(Message)]
pub struct RestoreWmos;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<WmoPlacements>()
        .add_message::<RetireWmo>()
        .add_message::<WmoRetired>()
        .add_message::<RestoreWmos>();
}

pub(super) fn snapshots(p: Res<Placements>, mut out: ResMut<WmoPlacements>) {
    if !out.enabled {
        return;
    }
    out.placements.clear();
    for (&uid, p) in &p.by_id {
        if p.suppressed || !p.spawned {
            continue;
        }
        if let ModelHandle::Wmo(model) = &p.model {
            out.placements.push(WmoPlacementSnapshot {
                map: p.map,
                uid,
                model: model.clone(),
                transform: p.transform,
                instance: p.portal_instance,
            });
        }
    }
}

pub(super) fn retire_wmos(
    mut commands: Commands,
    mut requests: MessageReader<RetireWmo>,
    mut restores: MessageReader<RestoreWmos>,
    mut replies: MessageWriter<WmoRetired>,
    mut placements: ResMut<Placements>,
    mut welds: ResMut<HullWelds>,
    mut merge: ResMut<StaticMerge>,
    mut residency: ResMut<WmoResidency>,
) {
    if restores.read().count() > 0 {
        placements.suppressed.clear();
        let mut pending = 0;
        for p in placements.by_id.values_mut().filter(|p| p.suppressed) {
            p.suppressed = false;
            p.spawned = false;
            pending += 1;
        }
        placements.pending_spawns += pending;
        residency.invalidate_placements();
        // A restart wins over an old in-flight request.
        requests.clear();
        return;
    }
    for request in requests.read() {
        let accepted = placements.by_id.get(&request.uid).is_some_and(|p| {
            p.map == request.map
                && matches!(p.model, ModelHandle::Wmo(_))
                && !p.suppressed
                && p.spawned
        });
        if accepted {
            let p = placements.by_id.get_mut(&request.uid).unwrap();
            p.suppressed = true;
            let pending = p.doodads.iter().filter(|d| !d.spawned).count();
            p.doodads.clear();
            for key in p.claimed_props.drain(..) {
                commands.queue(move |world: &mut World| {
                    world.write_message(WmoPropReleased { key });
                });
            }
            // Includes portal instance, meshes, props, lights, liquids, pending/active hull owners.
            for e in p.entities.drain(..) {
                commands.entity(e).try_despawn();
            }
            p.portal_instance = None;
            placements.pending_spawns -= pending;
            placements.suppressed.insert((request.map, request.uid));
            welds.retire(request.uid);
            merge.retire(request.uid);
            residency.invalidate_placements();
        }
        replies.write(WmoRetired {
            map: request.map,
            uid: request.uid,
            accepted,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    fn setup() -> App {
        let mut app = App::new();
        plugin(&mut app);
        app.init_resource::<Placements>()
            .init_resource::<HullWelds>()
            .init_resource::<StaticMerge>()
            .init_resource::<WmoResidency>()
            .add_message::<WmoPropReleased>();
        let owner = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<Placements>().by_id.insert(
            42,
            Placement {
                model: ModelHandle::Wmo(Handle::default()),
                map: 0,
                claimed_props: vec![WmoPropKey {
                    map: 0,
                    placement: 42,
                    modd: 7,
                }],
                prop_claims: WmoPropClaims::default(),
                claimed: false,
                suppressed: false,
                transform: Transform::IDENTITY,
                entities: vec![owner],
                spawned: true,
                doodad_set: 0,
                name_set: 0,
                doodads: vec![],
                portal_instance: Some(owner),
                refs: 2,
                owner: (0, 0),
            },
        );
        app
    }
    #[test]
    fn retirement_preserves_references_removes_products_and_restores() {
        let mut app = setup();
        let old = app.world().resource::<Placements>().by_id[&42].entities[0];
        app.world_mut()
            .resource_mut::<HullWelds>()
            .add_prop(42, vec![], vec![]);
        app.world_mut().resource_mut::<Placements>().pending_spawns = 1;
        app.world_mut()
            .resource_mut::<Placements>()
            .by_id
            .get_mut(&42)
            .unwrap()
            .doodads
            .push(WmoDoodadInst {
                key: WmoPropKey {
                    map: 0,
                    placement: 42,
                    modd: 8,
                },
                handle: Handle::default(),
                transform: Transform::IDENTITY,
                groups: std::sync::Arc::from([]),
                light: spawn::prop_light::PropLight::Exterior,
                spawned: false,
            });
        app.world_mut().write_message(RetireWmo { map: 0, uid: 42 });
        app.world_mut().run_system_once(retire_wmos).unwrap();
        let placements = app.world().resource::<Placements>();
        let p = &placements.by_id[&42];
        assert!(p.suppressed && p.spawned);
        assert_eq!(p.refs, 2);
        assert!(p.entities.is_empty() && p.claimed_props.is_empty() && p.portal_instance.is_none());
        assert_eq!(placements.pending_spawns, 0);
        assert!(placements.suppressed.contains(&(0, 42)));
        assert!(app.world().get_entity(old).is_err());
        assert_eq!(app.world().resource::<HullWelds>().unflushed(), 0);
        assert_eq!(app.world().resource::<Messages<WmoPropReleased>>().len(), 1);
        app.world_mut().write_message(RestoreWmos);
        app.world_mut().run_system_once(retire_wmos).unwrap();
        let placements = app.world().resource::<Placements>();
        assert!(placements.suppressed.is_empty());
        assert!(!placements.by_id[&42].suppressed && !placements.by_id[&42].spawned);
        assert_eq!(placements.pending_spawns, 1);
    }
    #[test]
    fn wrong_map_and_duplicate_requests_cannot_double_credit() {
        let mut app = setup();
        for map in [1, 0, 0] {
            app.world_mut().write_message(RetireWmo { map, uid: 42 });
        }
        app.world_mut().run_system_once(retire_wmos).unwrap();
        let messages = app.world().resource::<Messages<WmoRetired>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(
            cursor
                .read(messages)
                .map(|m| m.accepted)
                .collect::<Vec<_>>(),
            vec![false, true, false]
        );
    }
}
