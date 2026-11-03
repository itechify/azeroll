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

/// Stable identity of a MODD record, independent of selected-set ordinals and residency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WmoPropKey {
    pub map: u32,
    pub placement: u32,
    pub modd: u32,
}

#[derive(Debug, Clone)]
pub struct WmoPropQuery {
    /// Original MODD light provider, retained by ownership transfers.
    pub light: super::PropLight,
    pub key: WmoPropKey,
    pub model: String,
    pub transform: Transform,
}

/// Install before streaming. Each selected MODD is classified once when its root resolves.
#[derive(Resource, Default, Clone)]
pub struct WmoPropClaims {
    pub predicate: Option<std::sync::Arc<dyn Fn(&WmoPropQuery) -> bool + Send + Sync>>,
}

#[derive(Message, Debug, Clone)]
pub struct WmoPropClaimed(pub WmoPropQuery);

#[derive(Message, Debug, Clone, Copy)]
pub struct WmoPropReleased {
    pub key: WmoPropKey,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_has_three_independent_namespaces() {
        let key = WmoPropKey {
            map: 0,
            placement: 71414,
            modd: 303,
        };
        assert_ne!(key, WmoPropKey { map: 1, ..key });
        assert_ne!(
            key,
            WmoPropKey {
                placement: 1,
                ..key
            }
        );
        assert_ne!(key, WmoPropKey { modd: 1, ..key });
        assert!(WmoPropClaims::default().predicate.is_none());
    }
}
