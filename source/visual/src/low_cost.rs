//! Engine scheduling for a game that fits in one thread's frame.
//!
//! Bevy's multi-threaded executor hands each system to a pool thread, waking
//! it and putting it back to sleep, several hundred times per frame. This
//! game's systems are chained or tiny, so the wake-ups cost more than they
//! save.
use bevy::app::{App, Plugin, TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use bevy::ecs::schedule::{Schedules, SingleThreadedExecutor};
use bevy::light::cluster::GlobalClusterSettings;
use bevy::render::RenderApp;

pub struct LowCostPlugin;

fn single_threaded(schedules: &mut Schedules) {
    for (_, schedule) in schedules.iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
}

impl Plugin for LowCostPlugin {
    fn build(&self, _app: &mut App) {}

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
