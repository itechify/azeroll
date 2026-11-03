//! A bounded, visual-only skin of the building kinds actually eaten (D27).
//! Shared exterior mesh/material handles instance cheaply; these copies never carry credit,
//! colliders, Attached, or Collectible, and therefore never affect support or pickup gates.
use super::*;
use std::collections::VecDeque;
const LIMIT: usize = 64;
#[derive(Resource, Default)]
pub(super) struct Skin {
    roots: VecDeque<Entity>,
    next: usize,
}
#[derive(Component)]
pub(super) struct Tile {
    index: usize,
    axes: Vec3,
    rotation: Quat,
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Skin>()
        .add_systems(OnEnter(Phase::Ready), |mut skin: ResMut<Skin>| {
            *skin = default();
        });
}
impl Skin {
    pub(super) fn attach(
        &mut self,
        commands: &mut Commands,
        root: Entity,
        spin: Entity,
        axes: Vec3,
        rotation: Quat,
    ) {
        let index = self.next;
        self.next += 1;
        commands.entity(root).insert((
            Visibility::Inherited,
            Tile {
                index,
                axes,
                rotation,
            },
            ChildOf(spin),
        ));
        self.roots.push_back(root);
        if self.roots.len() > LIMIT {
            commands
                .entity(self.roots.pop_front().unwrap())
                .try_despawn();
        }
    }
}
pub(super) fn dress(
    ball: Res<BallState>,
    design: Res<design::Design>,
    skin: Res<Skin>,
    mut tiles: Query<(&Tile, &mut Transform)>,
) {
    let shell = ball::shell_radius(design.start_radius, ball.buried_volume);
    for (tile, mut tf) in &mut tiles {
        let count = skin.roots.len().max(1);
        let slot = tile.index.saturating_sub(skin.next.saturating_sub(count));
        let y = 1. - 2. * (slot as f32 + 0.5) / count as f32;
        let a = slot as f32 * 2.399963;
        let ring = (1. - y * y).max(0.).sqrt();
        let direction = Vec3::new(ring * a.cos(), y, ring * a.sin());
        // Keep whole-house silhouettes legible on a district-sized shell. The pool is
        // spread over its current count, so a half-full pool cannot leave a bare hemisphere.
        let scale = (shell * 0.6 / tile.axes.max_element()).clamp(0.12, 1.0);
        let seat = (shell - tile.axes.y * scale * 0.05).max(0.5);
        *tf = Transform {
            translation: direction * seat,
            rotation: Quat::from_rotation_arc(Vec3::Y, direction) * tile.rotation,
            scale: Vec3::splat(scale),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn a_partial_skin_covers_both_hemispheres() {
        let mut app = App::new();
        plugin(&mut app);
        app.insert_resource(BallState {
            radius: 80.,
            buried_volume: 300000.,
            ..default()
        })
        .insert_resource(toml::from_str::<design::Design>(include_str!("stormwind.toml")).unwrap());
        app.world_mut()
            .run_system_once(|mut commands: Commands, mut skin: ResMut<Skin>| {
                let spin = commands.spawn_empty().id();
                for _ in 0..12 {
                    let root = commands
                        .spawn((Transform::IDENTITY, Visibility::Hidden))
                        .id();
                    skin.attach(&mut commands, root, spin, Vec3::splat(30.), Quat::IDENTITY);
                }
            })
            .unwrap();
        app.world_mut().run_system_once(dress).unwrap();
        let positions: Vec<_> = app
            .world_mut()
            .query_filtered::<&Transform, With<Tile>>()
            .iter(app.world())
            .map(|t| t.translation)
            .collect();
        assert!(positions.iter().any(|p| p.y > 30.));
        assert!(positions.iter().any(|p| p.y < -30.));
    }
    #[test]
    fn a_full_visual_skin_is_bounded_and_cannot_add_credit_or_support() {
        let mut app = App::new();
        plugin(&mut app);
        app.insert_resource(BallState {
            radius: 50.,
            volume: 125000.,
            buried_volume: 62500.,
            ..default()
        })
        .insert_resource(Run {
            count: 7,
            ..default()
        })
        .insert_resource(toml::from_str::<design::Design>(include_str!("stormwind.toml")).unwrap());
        app.world_mut()
            .run_system_once(|mut commands: Commands, mut skin: ResMut<Skin>| {
                let spin = commands.spawn_empty().id();
                for _ in 0..(LIMIT + 8) {
                    let root = commands
                        .spawn((RunEntity, Transform::IDENTITY, Visibility::Hidden))
                        .id();
                    skin.attach(
                        &mut commands,
                        root,
                        spin,
                        Vec3::new(30., 20., 24.),
                        Quat::IDENTITY,
                    );
                }
            })
            .unwrap();
        app.world_mut().run_system_once(dress).unwrap();
        assert_eq!(app.world().resource::<Skin>().roots.len(), LIMIT);
        assert_eq!(
            app.world_mut().query::<&Tile>().iter(app.world()).count(),
            LIMIT
        );
        assert_eq!(
            app.world_mut()
                .query::<&Attached>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .query::<&Collectible>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .query::<&avian3d::prelude::Collider>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(app.world().resource::<BallState>().volume, 125000.);
        assert_eq!(app.world().resource::<Run>().count, 7);
        for tf in app
            .world_mut()
            .query_filtered::<&Transform, With<Tile>>()
            .iter(app.world())
        {
            assert!(tf.translation.length() > 20. && tf.translation.length() < 50.);
        }
    }
}
