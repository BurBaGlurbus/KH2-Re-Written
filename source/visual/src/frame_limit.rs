//! A frame cap. The main app sleeps at the very end of each update until the
//! next frame is due, so winit reads the input that arrived during the wait
//! just before the next update starts. Deadlines advance by whole intervals,
//! so the average rate holds the cap exactly; a late frame moves the schedule
//! instead of bunching up the frames after it.
//!
//! While frames arrive on the tick's interval, each one runs exactly one
//! simulation tick, as the original's frame loop does: scheduling jitter would
//! otherwise show as frames of none and of two ticks. The clock stays within
//! `TICK_LOCK` of real time; past that it catches up by the real time passed.
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::time::{Duration, Instant};

/// How far the clock may run from real time while frames hold one tick each.
const TICK_LOCK: Duration = Duration::from_millis(4);

/// The original's frame rate: one simulation tick per frame.
pub const ORIGINAL_FPS: u16 = 60;

#[derive(Resource, Default)]
pub struct FrameLimit {
    /// Frames per second to hold; None or 0 = no limit.
    pub fps: Option<u16>,
    next: Option<Instant>,
}

impl FrameLimit {
    pub fn new(fps: u16) -> Self {
        Self {
            fps: Some(fps),
            next: None,
        }
    }
}

/// Runs after every other system of the app's `Last` schedule.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameLimitSet;

pub struct FrameLimitPlugin;

impl Plugin for FrameLimitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameLimit>()
            .add_systems(Last, (wait, lock_ticks).chain().in_set(FrameLimitSet));
    }
}

/// When the frame after one ending at `now` is due, given the schedule's
/// `next` deadline: the deadline itself while it is ahead or less than one
/// interval behind, otherwise `now` (the schedule restarts).
fn due(next: Option<Instant>, now: Instant, interval: Duration) -> Instant {
    match next {
        Some(at) if at + interval > now => at,
        _ => now,
    }
}

fn wait(mut limit: ResMut<FrameLimit>) {
    let Some(fps) = limit.fps.filter(|&f| f > 0) else {
        limit.next = None;
        return;
    };
    let interval = Duration::from_secs_f64(1. / f64::from(fps));
    let at = due(limit.next, Instant::now(), interval);
    // A wake a little late stays on the schedule, so nothing here spins.
    let left = at.saturating_duration_since(Instant::now());
    if !left.is_zero() {
        std::thread::sleep(left);
    }
    limit.next = Some(at + interval);
}

/// Whether the next frame, starting at `now`, is a tick after the clock's
/// `last` update, within `TICK_LOCK`.
fn on_tick(last: Instant, tick: Duration, now: Instant) -> bool {
    let next = last + tick;
    now.saturating_duration_since(next) < TICK_LOCK
        && next.saturating_duration_since(now) < TICK_LOCK
}

/// Sets how the next frame's clock advances: by one tick, or to real time.
fn lock_ticks(
    real: Option<Res<Time<Real>>>,
    fixed: Option<Res<Time<Fixed>>>,
    strategy: Option<ResMut<TimeUpdateStrategy>>,
) {
    let (Some(real), Some(fixed), Some(mut strategy)) = (real, fixed, strategy) else {
        return;
    };
    let tick = fixed.timestep();
    let locked = real
        .last_update()
        .is_some_and(|last| on_tick(last, tick, Instant::now()));
    *strategy = if locked {
        TimeUpdateStrategy::ManualDuration(tick)
    } else {
        TimeUpdateStrategy::Automatic
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadlines_advance_by_whole_intervals() {
        let t = Instant::now();
        let i = Duration::from_millis(10);
        assert_eq!(due(None, t, i), t, "the first frame does not wait");
        let next = t + i;
        assert_eq!(due(Some(next), t + Duration::from_millis(4), i), next);
        // A little late: keep the schedule, so the next frame catches up.
        assert_eq!(due(Some(next), t + Duration::from_millis(15), i), next);
        // More than an interval late: restart from now.
        let late = t + Duration::from_millis(25);
        assert_eq!(due(Some(next), late, i), late);
    }

    #[test]
    fn the_app_holds_the_cap() {
        let mut app = App::new();
        app.add_plugins(FrameLimitPlugin);
        app.world_mut().resource_mut::<FrameLimit>().fps = Some(200);
        let start = Instant::now();
        for _ in 0..21 {
            app.update();
        }
        // 20 waits of 5 ms after the first frame.
        let took = start.elapsed();
        assert!(took >= Duration::from_millis(100), "{took:?}");
        assert!(took < Duration::from_millis(200), "{took:?}");
    }

    #[test]
    fn a_frame_on_the_tick_runs_one_tick() {
        let t = Instant::now();
        let tick = Duration::from_micros(16_667);
        assert!(on_tick(t, tick, t + tick));
        assert!(on_tick(t, tick, t + tick + Duration::from_millis(3)));
        assert!(on_tick(t, tick, t + tick - Duration::from_millis(3)));
        // Late or uncapped: the clock follows real time.
        assert!(!on_tick(t, tick, t + tick + Duration::from_millis(5)));
        assert!(!on_tick(t, tick, t + Duration::from_millis(4)));
    }

    #[test]
    fn locked_frames_run_one_tick_each_and_stay_on_real_time() {
        let mut app = App::new();
        app.add_plugins((bevy::time::TimePlugin, FrameLimitPlugin));
        // The game's tick, as its apps set it; Bevy's default is 64 Hz.
        app.insert_resource(Time::<Fixed>::from_hz(f64::from(ORIGINAL_FPS)));
        app.world_mut().resource_mut::<FrameLimit>().fps = Some(ORIGINAL_FPS);
        app.init_resource::<Ticks>()
            .add_systems(FixedUpdate, |mut t: ResMut<Ticks>| t.0 += 1);
        let start = Instant::now();
        for _ in 0..60 {
            app.update();
        }
        let ticks = app.world().resource::<Ticks>().0;
        let real = start.elapsed().as_secs_f64() * 60.;
        assert!(
            (f64::from(ticks) - real).abs() < 2.,
            "{ticks} ticks in {real} tick times"
        );
    }

    #[derive(Resource, Default)]
    struct Ticks(u32);
}
