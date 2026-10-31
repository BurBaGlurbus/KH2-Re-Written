//! The GPU device and queue created on a thread of their own while the game
//! data decodes. Bevy otherwise creates them in `RenderPlugin`'s build, about
//! a second spent after the decode, before the first frame.
//!
//! Bevy's own settings (`WgpuSettings::default()`, which reads the `WGPU_*`
//! environment overrides), allocating for memory use rather than speed: this
//! game's GPU data is small and rarely changes. The adapter is requested
//! without the window's surface, which does not exist yet. If the thread
//! fails, `RenderPlugin` creates the device itself as before.
use bevy::render::{
    RenderPlugin,
    renderer::initialize_renderer,
    settings::{MemoryHints, RenderCreation, RenderResources, WgpuSettings},
};
use std::thread::JoinHandle;

pub struct EarlyRenderer(JoinHandle<Option<RenderResources>>);

impl EarlyRenderer {
    pub fn start() -> Self {
        Self(std::thread::spawn(|| {
            let settings = WgpuSettings {
                memory_hints: MemoryHints::MemoryUsage,
                ..Default::default()
            };
            let backends = settings.backends?;
            Some(bevy::tasks::block_on(initialize_renderer(
                backends, None, &settings,
            )))
        }))
    }

    /// The render plugin using the early device, or creating its own.
    pub fn plugin(self) -> RenderPlugin {
        match self.0.join() {
            Ok(Some(resources)) => {
                let info = &resources.2;
                eprintln!(
                    "GPU: {} ({:?}, created during the load)",
                    info.name, info.backend
                );
                RenderPlugin {
                    render_creation: RenderCreation::Manual(resources),
                    ..Default::default()
                }
            }
            _ => RenderPlugin::default(),
        }
    }
}
