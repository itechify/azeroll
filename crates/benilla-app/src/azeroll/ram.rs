//! Fair rams: a hostile's bump may sting, but it must never pin a small ball.
//!
//! Every ram goes through [`Rammer`], whoever throws it (a guard's shoulder, a gnoll, Hogger's
//! pass). Three rules, from a run where Marshal Dughan and a Stormwind Guard bumped a 1 yd ball
//! twice in a row at the guard post until the player had to Unstuck:
//!
//! 1. *A global grace.* After any ram, no other attacker's ram lands for [`GRACE`] seconds, so two
//!    attackers cannot chain one knock into the next.
//! 2. *A per-attacker cooldown.* A guard bumps again no sooner than [`NPC_COOLDOWN`] seconds
//!    ([`BACK_OFF`] of which he spends giving ground, see `npcs`), so he is never standing on the
//!    ball's way out.
//! 3. *The knock goes where the ball can go.* The direction is the attacker's push turned aside
//!    (up to 120°) until a ray from the ball finds [`RUN`] yards of room; a ball pinned
//!    against a wall is knocked along it, and with no room anywhere the ram is not thrown at all.

use benilla_world::collision::WorldCollision;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::collect::Ram;
use super::{BallState, Run};

/// No ram lands within this many seconds of the last one, from anybody.
pub(super) const GRACE: f32 = 1.75;
/// One guard bumps no more often than this, seconds.
pub(super) const NPC_COOLDOWN: f32 = 6.0;
/// A guard gives ground for this long after bumping, seconds.
pub(super) const BACK_OFF: f32 = 3.0;
/// Room the knock needs beyond the ball's radius: the 0.35 s stun at 7 yd/s, rounded up.
const RUN: f32 = 3.0;
/// Rooms smaller than this (yards beyond the radius) are not worth knocking into.
const MIN_RUN: f32 = 1.2;
/// The knock may be turned aside from "away from the attacker" in steps of 40°, up to 120°.
const STEP_TURN: f32 = std::f32::consts::PI * 2.0 / 9.0;
const TURNS: u32 = 3;

/// When the last ram landed (`Run::elapsed`).
#[derive(Resource)]
pub(super) struct RamGate {
    last: f32,
}
impl Default for RamGate {
    fn default() -> Self {
        Self {
            last: f32::NEG_INFINITY,
        }
    }
}
impl RamGate {
    /// Whether a ram may land at `now`.
    pub(super) fn ready(&self, now: f32) -> bool {
        // A restarted run rewinds the clock.
        now < self.last || now - self.last >= GRACE
    }
    /// Take the grace window if it is free.
    pub(super) fn admit(&mut self, now: f32) -> bool {
        let ok = self.ready(now);
        if ok {
            self.last = now;
        }
        ok
    }
}

/// The knock direction: `away` itself when `free(dir)` (yards before the first wall, `None` for
/// open) shows [`RUN`] yards of room, otherwise the nearest turn that does, otherwise the roomiest
/// direction if it has [`MIN_RUN`], otherwise `None`.
pub(super) fn pick_knock(away: Vec3, free: impl Fn(Vec3) -> Option<f32>) -> Option<Vec3> {
    // The ball dead centre on its attacker has no "away"; `collect` would send it east.
    let away = Vec3::new(away.x, 0.0, away.z).normalize_or(Vec3::X);
    let mut best: Option<(f32, Vec3)> = None;
    for step in 0..=TURNS {
        for sign in [1.0, -1.0] {
            if step == 0 && sign < 0.0 {
                continue;
            }
            let dir = Quat::from_rotation_y(step as f32 * STEP_TURN * sign) * away;
            let room = free(dir).unwrap_or(f32::INFINITY);
            if room >= RUN {
                return Some(dir);
            }
            if best.is_none_or(|(r, _)| room > r) {
                best = Some((room, dir));
            }
        }
    }
    best.filter(|(room, _)| *room >= MIN_RUN).map(|(_, d)| d)
}

/// The writer every attacker uses.
#[derive(SystemParam)]
pub(super) struct Rammer<'w> {
    rams: MessageWriter<'w, Ram>,
    gate: ResMut<'w, RamGate>,
    run: Res<'w, Run>,
}
impl Rammer<'_> {
    /// Whether the grace window is open: a cheap check before the rays of [`Self::ram`].
    pub(super) fn ready(&self) -> bool {
        self.gate.ready(self.run.elapsed)
    }
    /// Throw a ram from `entity` pushing the ball along `away`, bent round walls. `false` (nothing
    /// written) when the grace window is shut or the ball has nowhere to go.
    pub(super) fn ram(
        &mut self,
        collision: &WorldCollision,
        ball: &BallState,
        entity: Entity,
        name: std::borrow::Cow<'static, str>,
        away: Vec3,
    ) -> bool {
        if !self.ready() {
            return false;
        }
        let r = ball.radius;
        let dir = pick_knock(away, |d| {
            let line = Dir3::new(d).ok()?;
            collision
                .ray_body(ball.pos, line, r + RUN)
                // Floors and slopes are not in the way.
                .filter(|hit| hit.normal.y.abs() < 0.6)
                .map(|hit| (hit.distance - r).max(0.0))
        });
        let Some(dir) = dir else {
            return false;
        };
        if !self.gate.admit(self.run.elapsed) {
            return false;
        }
        self.rams.write(Ram { entity, name, dir });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grace_blocks_a_second_attacker_then_opens() {
        let mut gate = RamGate::default();
        assert!(gate.admit(10.0));
        assert!(!gate.admit(10.0), "two attackers in the same frame");
        assert!(!gate.admit(10.0 + GRACE - 0.05));
        assert!(gate.admit(10.0 + GRACE + 0.01));
        assert!(GRACE >= 1.5 && NPC_COOLDOWN >= 4.0 && NPC_COOLDOWN > BACK_OFF);
    }

    #[test]
    fn a_restarted_run_does_not_inherit_the_grace() {
        let mut gate = RamGate::default();
        assert!(gate.admit(300.0));
        assert!(gate.admit(1.0), "the clock went back to zero");
    }

    #[test]
    fn open_ground_knocks_straight_away() {
        let away = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(pick_knock(away, |_| None), Some(away));
    }

    #[test]
    fn a_wall_behind_the_ball_turns_the_knock_along_it() {
        // A wall across -z, 0.5 yd off: only directions with some +x/-x component or +z have room.
        let away = Vec3::new(0.0, 0.0, -1.0);
        let free = |d: Vec3| (d.z < -0.1).then(|| 0.5 / -d.z);
        let dir = pick_knock(away, free).expect("a way out along the wall");
        assert!(dir.z > -0.3, "not into the wall: {dir:?}");
        assert!((dir.length() - 1.0).abs() < 1e-4 && dir.y == 0.0);
    }

    #[test]
    fn a_pocket_gets_no_ram() {
        assert_eq!(pick_knock(Vec3::X, |_| Some(0.4)), None);
        assert_eq!(pick_knock(Vec3::ZERO, |_| None), Some(Vec3::X));
    }

    #[test]
    fn the_roomiest_way_is_taken_when_none_is_roomy_enough() {
        // Everything is 2 yd of room except one direction with 2.6: still short of RUN.
        let away = Vec3::X;
        let best = Quat::from_rotation_y(TURNS as f32 * STEP_TURN) * away;
        let free = |d: Vec3| Some(if d.distance(best) < 1e-3 { 2.6 } else { 2.0 });
        let got = pick_knock(away, free).unwrap();
        assert!(got.distance(best) < 1e-3);
    }
}
