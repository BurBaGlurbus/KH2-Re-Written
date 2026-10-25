//! A frame cap without V-Sync. The main app waits at the very end of each
//! update until the next frame is due, so winit reads the input that arrived
//! during the wait just before the next update starts. Deadlines advance by
//! whole intervals, so the average rate holds the cap exactly; a late frame
//! moves the schedule instead of bunching up the frames after it.
use bevy::prelude::*;
use std::time::{Duration, Instant};

/// The last stretch before a deadline is spun rather than slept: Windows
/// wakes a sleeping thread up to about a millisecond late.
const SPIN: Duration = Duration::from_micros(1500);

#[derive(Resource, Default)]
pub struct FrameLimit {
    /// Frames per second to hold; None or 0 = no limit.
    pub fps: Option<u16>,
    next: Option<Instant>,
}

/// Runs after every other system of the app's `Last` schedule.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameLimitSet;

pub struct FrameLimitPlugin;

impl Plugin for FrameLimitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameLimit>()
            .add_systems(Last, wait.in_set(FrameLimitSet));
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
    loop {
        let left = at.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        if left > SPIN {
            std::thread::sleep(left - SPIN);
        } else {
            std::thread::yield_now();
        }
    }
    limit.next = Some(at + interval);
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
}
