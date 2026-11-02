//! Units whose local pose belongs to a moving parent.

use bevy::prelude::*;

/// This unit's `Transform` is parent-local: world-space motion, ground seating, terrain conform
/// and scale interpolation must leave it alone. Its model's local animation can still advance.
#[derive(Component)]
pub(crate) struct Carried;
