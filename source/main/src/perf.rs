//! Opt-in performance diagnostics, built only with `--features perf-diag`.
//! Bevy's frame time, entity count and process memory diagnostics are logged
//! every five seconds; with `--perf <new-dir>` every rendered frame is also
//! recorded (real frame time, entities, process memory, heap allocations,
//! mesh and image assets added or modified)
//! and written to `<dir>/frames.csv` when the app exits.
//! `tools/perf_report.py` turns the record into averages and percentiles.
//! Nothing here touches the simulation or the presentation.
use std::alloc::System;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    LogDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::ecs::entity::Entities;
use bevy::prelude::*;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

/// Counts heap allocations so a frame's allocation burst can be recorded.
#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// Bevy's diagnostics plus the per-frame record when `dir` is set.
pub struct PerfPlugin {
    pub dir: Option<PathBuf>,
}

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            SystemInformationDiagnosticsPlugin,
            LogDiagnosticsPlugin {
                wait_duration: Duration::from_secs(5),
                ..default()
            },
        ));
        if let Some(dir) = &self.dir {
            app.insert_resource(PerfRecord {
                dir: dir.clone(),
                started: Instant::now(),
                region: Region::new(GLOBAL),
                rows: Vec::with_capacity(1 << 16),
                written: false,
            })
            .add_systems(Last, (record_frame, write_record).chain());
        }
    }
}

struct Row {
    /// Since the plugin was built, at the end of the frame (ms).
    at_ms: f64,
    /// Real time since the previous frame (ms).
    frame_ms: f64,
    /// Virtual time is paused (unattended runs warm up paused).
    paused: bool,
    /// Fixed-timestep ticks run this frame.
    fixed_ticks: u32,
    entities: u32,
    /// Process memory (GiB), as last sampled by the sysinfo task.
    process_gib: f64,
    allocations: usize,
    allocated_bytes: usize,
    /// Mesh and image assets added and modified this frame (each one is
    /// uploaded to the GPU again).
    meshes_added: u32,
    meshes_modified: u32,
    images_added: u32,
    images_modified: u32,
}

/// Counts this frame's added and modified assets of one type.
fn count<A: Asset>(events: &mut MessageReader<AssetEvent<A>>) -> (u32, u32) {
    events.read().fold((0, 0), |(a, m), e| match e {
        AssetEvent::Added { .. } => (a + 1, m),
        AssetEvent::Modified { .. } => (a, m + 1),
        _ => (a, m),
    })
}

#[derive(Resource)]
struct PerfRecord {
    dir: PathBuf,
    started: Instant,
    region: Region<'static, System>,
    rows: Vec<Row>,
    written: bool,
}

fn record_frame(
    real: Res<Time<Real>>,
    virt: Res<Time<Virtual>>,
    fixed: Res<Time<Fixed>>,
    entities: &Entities,
    store: Res<DiagnosticsStore>,
    mut record: ResMut<PerfRecord>,
    mut last_fixed: Local<Duration>,
    mut meshes: MessageReader<AssetEvent<Mesh>>,
    mut images: MessageReader<AssetEvent<Image>>,
) {
    let elapsed = fixed.elapsed();
    let fixed_ticks =
        ((elapsed - *last_fixed).as_secs_f64() / fixed.timestep().as_secs_f64()).round() as u32;
    *last_fixed = elapsed;
    let process_gib = store
        .get(&SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE)
        .and_then(|d| d.value())
        .unwrap_or(0.);
    let stats = record.region.change_and_reset();
    let (meshes_added, meshes_modified) = count(&mut meshes);
    let (images_added, images_modified) = count(&mut images);
    let row = Row {
        at_ms: record.started.elapsed().as_secs_f64() * 1e3,
        frame_ms: real.delta_secs_f64() * 1e3,
        paused: virt.is_paused(),
        fixed_ticks,
        entities: entities.count_spawned(),
        process_gib,
        allocations: stats.allocations + stats.reallocations,
        allocated_bytes: stats.bytes_allocated + stats.bytes_reallocated.max(0) as usize,
        meshes_added,
        meshes_modified,
        images_added,
        images_modified,
    };
    record.rows.push(row);
}

fn write_record(mut exit: MessageReader<AppExit>, mut record: ResMut<PerfRecord>) {
    if exit.read().count() == 0 || record.written {
        return;
    }
    record.written = true;
    let path = record.dir.join("frames.csv");
    let file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("perf record {}: {e}", path.display());
            return;
        }
    };
    let mut out = BufWriter::new(file);
    let _ = writeln!(
        out,
        "at_ms,frame_ms,paused,fixed_ticks,entities,process_gib,allocations,allocated_bytes,\
         meshes_added,meshes_modified,images_added,images_modified"
    );
    for r in &record.rows {
        let _ = writeln!(
            out,
            "{:.3},{:.4},{},{},{},{:.4},{},{},{},{},{},{}",
            r.at_ms,
            r.frame_ms,
            u8::from(r.paused),
            r.fixed_ticks,
            r.entities,
            r.process_gib,
            r.allocations,
            r.allocated_bytes,
            r.meshes_added,
            r.meshes_modified,
            r.images_added,
            r.images_modified
        );
    }
    let _ = out.flush();
    eprintln!(
        "perf record: {} frames in {}",
        record.rows.len(),
        path.display()
    );
}
