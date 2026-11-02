//! Local units, built the way the net layer builds streamed ones: a [`Guid`], a [`NetEntity`] and an
//! [`ObjectStore`] are all the attach and animation systems need. A trimmed copy of
//! `survivors::units`: creatures only, no weapons, auras or spell visuals.

use benilla_protocol::messages::ObjectFields;
use bevy::prelude::*;

use super::RunEntity;
use crate::net::{Guid, NetEntity, ObjectStore};

const FIELD_HEALTH: u16 = 22;
const FIELD_MAX_HEALTH: u16 = 28;
const FIELD_LEVEL: u16 = 34;
const FIELD_FACTION: u16 = 35;
const FIELD_BYTES_0: u16 = 36;
const FIELD_BYTES_2: u16 = 164;

/// `FactionTemplate.dbc`: friendly to players, and the hostile monster template.
pub(super) const FACTION_FRIENDLY: u32 = 35;
pub(super) const FACTION_MONSTER: u32 = 14;

const HIGH_CREATURE: u64 = 0xF130 << 48;

/// WoW (x north, y west, z up) to Bevy (x east, y up, z south) yards.
pub(super) fn wow_to_bevy(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

/// The inverse of [`wow_to_bevy`].
pub(super) fn bevy_to_wow(p: Vec3) -> [f32; 3] {
    [-p.z, -p.x, p.y]
}

/// The Bevy yaw that faces a model along `dir` (its XZ part).
pub(super) fn yaw_toward(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

/// `v` with its height dropped: the ground-plane part.
pub(super) fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// What a local creature is dressed as.
pub(super) struct UnitSpec {
    /// A serial unique across the whole session ([`super::Run::guid`]).
    pub serial: u64,
    /// `CreatureDisplayInfo` row.
    pub display: u32,
    /// The final model scale: the attach writes it straight into the root [`Transform`], so the
    /// caller folds in the display's own DBC scale.
    pub scale: f32,
    pub level: u32,
    pub faction: u32,
    /// `UNIT_FIELD_BYTES_0`: race, class, gender, power type.
    pub bytes0: u32,
}

/// Spawn a unit the attach builds a body for, tagged [`RunEntity`]; the caller adds its own
/// components.
pub(super) fn spawn_unit(commands: &mut Commands, spec: &UnitSpec, pos: Vec3, yaw: f32) -> Entity {
    let guid = HIGH_CREATURE | spec.serial;
    let pairs: [(u16, u32); 10] = [
        (0, guid as u32),
        (1, (guid >> 32) as u32),
        (2, 0x9),
        (4, spec.scale.to_bits()),
        (FIELD_HEALTH, 100),
        (FIELD_MAX_HEALTH, 100),
        (FIELD_LEVEL, spec.level),
        (FIELD_FACTION, spec.faction),
        (FIELD_BYTES_0, spec.bytes0),
        (FIELD_BYTES_2, 0),
    ];
    commands
        .spawn((
            Guid(guid),
            NetEntity {
                kind: benilla_protocol::EntityKind::Unit,
                display_id: Some(spec.display),
                scale: spec.scale,
            },
            ObjectStore(ObjectFields::from_pairs(&pairs)),
            crate::creature_anim::MovementState::default(),
            crate::net::ScriptedFacing,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RunEntity,
        ))
        .id()
}
