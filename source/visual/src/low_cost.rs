//! Engine scheduling for a game that fits in one thread's frame.
//!
//! Bevy's multi-threaded executor hands each system to a pool thread, waking
//! it and putting it back to sleep, several hundred times per frame. This
//! game's systems are chained or tiny, so the wake-ups cost more than they
//! save.
use bevy::app::{
    App, Last, Plugin, TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy,
};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{Schedules, SingleThreadedExecutor};
use bevy::ecs::system::ResMut;
use bevy::light::cluster::GlobalClusterSettings;
use bevy::render::RenderApp;

pub struct LowCostPlugin;

/// Frames from the first one to the trim after the start's load: past the
/// first frame's GPU work, and early, since the trim stalls one frame.
const TRIM_AFTER_START: u32 = 30;

/// When to hand the load's pages back (`audio::memory`): a load touches far
/// more memory than the game reads again.
#[derive(Resource)]
pub struct WorkingSetTrim {
    in_frames: Option<u32>,
}

impl WorkingSetTrim {
    /// A load has just ended: trim while the screen is still black.
    pub fn after_load(&mut self) {
        self.in_frames = Some(1);
    }
}

fn trim_when_due(mut trim: ResMut<WorkingSetTrim>) {
    match trim.in_frames {
        Some(0) => {
            trim.in_frames = None;
            audio::memory::trim_working_set();
        }
        Some(n) => trim.in_frames = Some(n - 1),
        None => {}
    }
}

fn single_threaded(schedules: &mut Schedules) {
    for (_, schedule) in schedules.iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
}

impl Plugin for LowCostPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorkingSetTrim {
            in_frames: Some(TRIM_AFTER_START),
        })
        .add_systems(Last, trim_when_due);
    }

    // (In `finish`: pipelined rendering takes the render app away in its
    // `cleanup`.)
    fn finish(&self, app: &mut App) {
        // Everything drawn is unlit: no lights to sort into clusters on the
        // GPU (the settings are copied to the render world each frame).
        if let Some(mut clusters) = app.world_mut().get_resource_mut::<GlobalClusterSettings>() {
            clusters.gpu_clustering = None;
        }
        if let Some(mut schedules) = app.world_mut().get_resource_mut::<Schedules>() {
            single_threaded(&mut schedules);
        }
        if let Some(render) = app.get_sub_app_mut(RenderApp)
            && let Some(mut schedules) = render.world_mut().get_resource_mut::<Schedules>()
        {
            single_threaded(&mut schedules);
        }
    }
}

/// One thread each for the engine's task pools: what still runs on them (a
/// parallel query, an asset or pipeline task) is a small part of a frame.
pub fn task_pools() -> TaskPoolPlugin {
    let one = || TaskPoolThreadAssignmentPolicy {
        min_threads: 1,
        max_threads: 1,
        percent: 1.0,
        on_thread_spawn: None,
        on_thread_destroy: None,
    };
    TaskPoolPlugin {
        task_pool_options: TaskPoolOptions {
            io: one(),
            async_compute: one(),
            compute: one(),
            ..TaskPoolOptions::default()
        },
    }
}
