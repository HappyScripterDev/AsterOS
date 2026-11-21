//! Time-based animation primitives for Aster OS.
//!
//! Everything here is deterministic and driven by an explicit millisecond
//! timestamp. There are no sleeps, frame counters or hidden clocks, so an
//! animation completes correctly even when the compositor drops frames.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::vec::Vec;

/// Easing curves used by the desktop and its applications.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
    Smoothstep,
    Spring,
}

impl Easing {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0) * (-2.0 * t + 2.0) / 2.0
                }
            }
            Easing::CubicIn => t * t * t,
            Easing::CubicOut => 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t),
            Easing::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let u = -2.0 * t + 2.0;
                    1.0 - u * u * u / 2.0
                }
            }
            Easing::Smoothstep => t * t * (3.0 - 2.0 * t),
            // A slight overshoot then settle. The result is still clamped by
            // callers that need a strict range; opacity does, position does not.
            Easing::Spring => {
                let c = 1.70158;
                let c3 = c + 1.0;
                let u = t - 1.0;
                1.0 + c3 * u * u * u + c * u * u
            }
        }
    }
}

/// Linear interpolation shared by rect and scalar animations.
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// A one-shot scalar animation.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start: u64,
    pub duration: u64,
    pub easing: Easing,
}

impl Tween {
    pub fn new(from: f32, to: f32, start: u64, duration: u64, easing: Easing) -> Self {
        Self {
            from,
            to,
            start,
            duration: duration.max(1),
            easing,
        }
    }

    pub fn progress(&self, now: u64) -> f32 {
        let elapsed = now.saturating_sub(self.start);
        (elapsed as f32 / self.duration as f32).clamp(0.0, 1.0)
    }

    pub fn value(&self, now: u64) -> f32 {
        let t = self.progress(now);
        lerp(self.from, self.to, self.easing.apply(t))
    }

    /// Like [`value`] but clamped to the endpoints, for opacity and scale.
    pub fn clamped(&self, now: u64) -> f32 {
        self.value(now)
            .clamp(self.from.min(self.to), self.from.max(self.to))
    }

    pub fn finished(&self, now: u64) -> bool {
        now.saturating_sub(self.start) >= self.duration
    }

    pub fn retarget(&mut self, from: f32, to: f32, now: u64, duration: u64) {
        self.from = from;
        self.to = to;
        self.start = now;
        self.duration = duration.max(1);
    }
}

/// Critically damped spring, integrated in seconds.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub position: f32,
    pub velocity: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    pub fn new(position: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            position,
            velocity: 0.0,
            target: position,
            stiffness,
            damping,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Advance by `dt_ms`. Returns the new position.
    pub fn step(&mut self, dt_ms: f32) -> f32 {
        if dt_ms <= 0.0 {
            return self.position;
        }
        // Fixed sub-steps keep the spring stable after long frames or when the
        // scheduler was busy.
        let mut remaining = dt_ms.min(100.0) / 1000.0;
        while remaining > 0.0 {
            let step = remaining.min(0.008);
            let force = (self.target - self.position) * self.stiffness;
            self.velocity += force * step;
            self.velocity *= (1.0 - self.damping * step).clamp(0.0, 1.0);
            self.position += self.velocity * step;
            if (self.target - self.position).abs() < 0.001 && self.velocity.abs() < 0.001 {
                self.position = self.target;
                self.velocity = 0.0;
                break;
            }
            remaining -= step;
        }
        self.position
    }

    pub fn settled(&self) -> bool {
        (self.target - self.position).abs() < 0.01 && self.velocity.abs() < 0.01
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackKey {
    pub owner: u64,
    pub channel: u16,
}

#[derive(Clone, Copy, Debug)]
struct Track {
    key: TrackKey,
    tween: Tween,
    remove_at: u64,
}

/// Shared hub for named scalar animations.
///
/// UI code starts a track once and queries `value` each frame. Tracks are
/// removed when they finish, so a long-running desktop does not accumulate
/// animation state. Allocations happen only on `start`, never per frame.
pub struct MotionSystem {
    tracks: Vec<Track>,
}

impl MotionSystem {
    pub fn new() -> Self {
        Self { tracks: Vec::new() }
    }

    pub fn start(
        &mut self,
        owner: u64,
        channel: u16,
        from: f32,
        to: f32,
        now: u64,
        duration: u64,
        easing: Easing,
    ) {
        let key = TrackKey { owner, channel };
        let tween = Tween::new(from, to, now, duration, easing);
        if let Some(track) = self.tracks.iter_mut().find(|track| track.key == key) {
            track.tween = tween;
            track.remove_at = now + duration.max(1) + 1;
            return;
        }
        self.tracks.push(Track {
            key,
            tween,
            remove_at: now + duration.max(1) + 1,
        });
    }

    pub fn value(&self, owner: u64, channel: u16, now: u64, default: f32) -> f32 {
        let key = TrackKey { owner, channel };
        self.tracks
            .iter()
            .find(|track| track.key == key)
            .map(|track| track.tween.value(now))
            .unwrap_or(default)
    }

    pub fn clamped(&self, owner: u64, channel: u16, now: u64, default: f32) -> f32 {
        let key = TrackKey { owner, channel };
        self.tracks
            .iter()
            .find(|track| track.key == key)
            .map(|track| track.tween.clamped(now))
            .unwrap_or(default)
    }

    pub fn running(&self, owner: u64, channel: u16, now: u64) -> bool {
        let key = TrackKey { owner, channel };
        self.tracks
            .iter()
            .find(|track| track.key == key)
            .map(|track| !track.tween.finished(now))
            .unwrap_or(false)
    }

    pub fn active(&self, now: u64) -> bool {
        self.tracks.iter().any(|track| !track.tween.finished(now))
    }

    pub fn cancel(&mut self, owner: u64) {
        self.tracks.retain(|track| track.key.owner != owner);
    }

    pub fn cancel_channel(&mut self, owner: u64, channel: u16) {
        self.tracks
            .retain(|track| track.key.owner != owner || track.key.channel != channel);
    }

    /// Drop finished tracks. Call once per rendered frame.
    pub fn collect(&mut self, now: u64) {
        self.tracks.retain(|track| now < track.remove_at);
    }

    pub fn count(&self) -> usize {
        self.tracks.len()
    }
}

impl Default for MotionSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple sequence of durations. Each step is evaluated from the same start
/// time, so a skipped frame does not desynchronise the sequence.
#[derive(Clone, Debug)]
pub struct Sequence {
    durations: Vec<u64>,
    start: u64,
}

impl Sequence {
    pub fn new(durations: &[u64], start: u64) -> Self {
        Self {
            durations: durations.to_vec(),
            start,
        }
    }

    pub fn total(&self) -> u64 {
        self.durations.iter().sum()
    }

    /// Returns `(index, local_progress)` for the active step.
    pub fn at(&self, now: u64) -> Option<(usize, f32)> {
        let elapsed = now.saturating_sub(self.start);
        let mut cursor = 0u64;
        for (index, duration) in self.durations.iter().enumerate() {
            let duration = (*duration).max(1);
            if elapsed < cursor + duration {
                return Some((index, (elapsed - cursor) as f32 / duration as f32));
            }
            cursor += duration;
        }
        None
    }

    pub fn finished(&self, now: u64) -> bool {
        now.saturating_sub(self.start) >= self.total()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.001, "{a} != {b}");
    }

    #[test]
    fn easing_endpoints_are_stable() {
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
            Easing::Smoothstep,
            Easing::Spring,
        ] {
            close(easing.apply(0.0), 0.0);
            close(easing.apply(1.0), 1.0);
        }
    }

    #[test]
    fn easing_is_monotonic_except_spring() {
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
            Easing::Smoothstep,
        ] {
            let mut previous = 0.0;
            for step in 0..=100 {
                let value = easing.apply(step as f32 / 100.0);
                assert!(value >= previous - 0.0001);
                previous = value;
            }
        }
    }

    #[test]
    fn tween_completes_and_survives_skipped_frames() {
        let tween = Tween::new(10.0, 20.0, 100, 200, Easing::Linear);
        assert!(tween.value(100) == 10.0);
        assert!(tween.value(300) == 20.0);
        close(tween.value(200), 15.0);
        assert!(tween.finished(300));
        assert!(tween.finished(10_000));
    }

    #[test]
    fn tween_retarget_restarts_from_now() {
        let mut tween = Tween::new(0.0, 1.0, 0, 100, Easing::Linear);
        tween.retarget(1.0, 0.0, 50, 100);
        close(tween.value(50), 1.0);
        close(tween.value(100), 0.5);
        close(tween.value(150), 0.0);
    }

    #[test]
    fn spring_converges() {
        let mut spring = Spring::new(0.0, 120.0, 20.0);
        spring.set_target(100.0);
        for _ in 0..240 {
            spring.step(16.0);
        }
        assert!(spring.settled(), "spring did not settle: {:?}", spring);
        close(spring.position, 100.0);
    }

    #[test]
    fn motion_system_replaces_and_collects() {
        let mut motion = MotionSystem::new();
        motion.start(7, 1, 0.0, 1.0, 0, 100, Easing::Linear);
        close(motion.value(7, 1, 50, 9.0), 0.5);
        motion.start(7, 1, 1.0, 0.0, 100, 100, Easing::Linear);
        close(motion.value(7, 1, 100, 9.0), 1.0);
        assert_eq!(motion.count(), 1);
        motion.collect(1000);
        assert_eq!(motion.count(), 0);
        close(motion.value(7, 1, 1000, 9.0), 9.0);
    }

    #[test]
    fn cancel_removes_owner() {
        let mut motion = MotionSystem::new();
        motion.start(1, 0, 0.0, 1.0, 0, 100, Easing::Linear);
        motion.start(2, 0, 0.0, 1.0, 0, 100, Easing::Linear);
        motion.cancel(1);
        assert_eq!(motion.count(), 1);
        assert!(!motion.running(1, 0, 10));
        assert!(motion.running(2, 0, 10));
    }

    #[test]
    fn sequence_uses_absolute_time() {
        let sequence = Sequence::new(&[100, 50], 1000);
        assert_eq!(sequence.at(1050), Some((0, 0.5)));
        assert_eq!(sequence.at(1100), Some((1, 0.0)));
        assert!(sequence.at(1150).is_none());
        assert!(sequence.finished(1150));
    }
}
