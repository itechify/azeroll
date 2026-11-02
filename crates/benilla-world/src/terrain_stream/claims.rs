//! Optional ownership transfer before an ADT doodad enters any render or collision lane.

use bevy::prelude::*;

/// A newly registered MDDF placement, in Bevy world space (yards).
#[derive(Debug, Clone)]
pub struct ClaimQuery {
    pub unique_id: u32,
    pub model: String,
    pub transform: Transform,
}

/// Set before streaming begins. Claims are latched until the placement's last tile releases it;
/// changing the predicate does not transfer already registered placements. With no predicate the
/// streamer keeps its ordinary rendering and collision behavior.
#[derive(Resource, Default)]
pub struct PlacementClaims {
    pub predicate: Option<Box<dyn Fn(&ClaimQuery) -> bool + Send + Sync>>,
}

/// The consumer owns this placement's visuals and collision until its matching release. Shared
/// tile references announce once; streaming out and back in announces again.
#[derive(Message, Debug, Clone)]
pub struct DoodadClaimed {
    pub unique_id: u32,
    pub model: String,
    pub transform: Transform,
}

/// The last tile referencing a claimed MDDF placement left residency.
#[derive(Message, Debug, Clone, Copy)]
pub struct DoodadReleased {
    pub unique_id: u32,
}
