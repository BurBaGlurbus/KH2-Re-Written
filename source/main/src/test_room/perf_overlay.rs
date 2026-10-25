//! The debug menu's performance overlay: a few lines at the top left with
//! the frame rate, frame times, the GPU and the entity count (and, in
//! `perf-diag` builds, process memory, CPU use and GPU pass times). It is
//! drawn by the 2D layer over everything, so the HUD canvas never changes
//! for it, and it costs a flag test per frame while it is off.
use super::*;
use bevy::diagnostic::{DiagnosticPath, DiagnosticsStore};
use bevy::ecs::entity::Entities;
use bevy::render::renderer::RenderAdapterInfo;
use std::collections::VecDeque;

/// Frame times kept for the statistics (about two seconds at 120 FPS).
const SAMPLES: usize = 240;
/// Seconds between text refreshes, so the numbers stay readable.
const REFRESH: f32 = 0.25;
/// Text size and line height (HUD units), and the top-left corner: the
/// window's left edge (x -85 is the edge of the 16:9 layout), under the
/// HUD's status line.
const SIZE: f32 = 10.;
const LINE: f32 = 12.;
const ORIGIN: [f32; 2] = [-78., 34.];
const TEXT: [f32; 4] = [1.6, 1.6, 1.6, 1.];
const BACKING: [f32; 4] = [0., 0., 0., 0.55];

#[derive(Resource, Default)]
pub struct PerfOverlay {
    pub on: bool,
    /// Real frame times (seconds), newest last.
    samples: VecDeque<f32>,
    since_refresh: f32,
    lines: Vec<String>,
    /// The system font and a white texel as 2D-layer sheets (uploaded on
    /// first use).
    sheets: Option<(usize, usize)>,
}

/// Average FPS over the last second of `samples`, the 1% low FPS (mean of
/// the slowest 1%) and the 99th percentile frame time (ms), over all.
fn stats(samples: &VecDeque<f32>) -> Option<(f32, f32, f32)> {
    if samples.is_empty() {
        return None;
    }
    let (mut sum, mut n) = (0., 0);
    for s in samples.iter().rev() {
        if sum >= 1. {
            break;
        }
        sum += s;
        n += 1;
    }
    let mut sorted: Vec<f32> = samples.iter().copied().collect();
    sorted.sort_by(f32::total_cmp);
    let worst = (sorted.len() / 100).max(1);
    let low = sorted[sorted.len() - worst..].iter().sum::<f32>() / worst as f32;
    let p99 = sorted[(sorted.len() - 1) * 99 / 100];
    Some((n as f32 / sum, 1. / low, p99 * 1e3))
}

/// The latest value of a diagnostic, when its plugin runs.
fn latest(store: Option<&DiagnosticsStore>, path: &'static str) -> Option<f64> {
    store?.get(&DiagnosticPath::const_new(path))?.value()
}

/// Samples the frame and, a few times a second, rebuilds the text.
pub(super) fn sample(
    game: Res<Game>,
    mut overlay: ResMut<PerfOverlay>,
    real: Res<Time<Real>>,
    entities: &Entities,
    adapter: Option<Res<RenderAdapterInfo>>,
    store: Option<Res<DiagnosticsStore>>,
) {
    if overlay.on != game.perf_overlay {
        overlay.on = game.perf_overlay;
    }
    if !overlay.on {
        if !overlay.samples.is_empty() {
            overlay.samples.clear();
            overlay.lines.clear();
        }
        return;
    }
    let dt = real.delta_secs();
    if overlay.samples.len() == SAMPLES {
        overlay.samples.pop_front();
    }
    overlay.samples.push_back(dt);
    overlay.since_refresh += dt;
    if overlay.since_refresh < REFRESH && !overlay.lines.is_empty() {
        return;
    }
    overlay.since_refresh = 0.;
    let Some((fps, low, p99)) = stats(&overlay.samples) else {
        return;
    };
    let mut lines = vec![
        format!("FPS {fps:.0}   1% low {low:.0}"),
        format!(
            "Frame {:.2} ms   p99 {p99:.2} ms",
            1e3 / fps.max(f32::EPSILON)
        ),
    ];
    if let Some(a) = adapter {
        lines.push(format!("GPU {} - {:?}", a.name, a.backend));
    }
    let store = store.as_deref();
    // Bevy's render diagnostics (perf-diag builds): the passes' GPU times.
    if let Some(store) = store {
        let gpu: f64 = store
            .iter()
            .filter(|d| d.path().as_str().ends_with("/elapsed_gpu"))
            .filter_map(|d| d.value())
            .sum();
        if gpu > 0. {
            lines.push(format!("GPU passes {gpu:.2} ms"));
        }
    }
    let mut last = format!("Entities {}", entities.count_spawned());
    if let Some(mem) = latest(store, "process/mem_usage") {
        last += &format!("   Memory {mem:.2} GiB");
    }
    if let Some(cpu) = latest(store, "process/cpu_usage") {
        last += &format!("   CPU {cpu:.0}%");
    }
    lines.push(last);
    overlay.lines = lines;
}

/// Draws the text over everything (after the HUD and the menus).
pub(super) fn present(
    mut overlay: ResMut<PerfOverlay>,
    hud: Option<Res<Hud>>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    if !overlay.on || overlay.lines.is_empty() {
        return;
    }
    let Some(font) = hud.as_ref().and_then(|h| h.menu.as_ref()).map(|m| &m.font) else {
        return;
    };
    let (sheet, white) = *overlay.sheets.get_or_insert_with(|| {
        (
            frame.add_sheet(
                &mut images,
                font.rgba.clone(),
                font.size,
                font.texel_scale as f32,
            ),
            frame.add_sheet(&mut images, vec![255; 16], [2, 2], 1.),
        )
    });
    let f = field_command_menu::Font {
        widths: &font.widths,
        pad: 1,
    };
    let encoded: Vec<Vec<u8>> = overlay
        .lines
        .iter()
        .map(|l| pause_menu::encode_text(l))
        .collect();
    // A dark backing keeps the text readable over any picture.
    let width = encoded
        .iter()
        .map(|l| field_command_menu::text_width(l, &f, SIZE))
        .fold(0., f32::max);
    let [x, y] = ORIGIN;
    let (x0, y0, x1, y1) = (
        x - 4.,
        y - 2.,
        x + width + 4.,
        y + LINE * encoded.len() as f32 + 2.,
    );
    frame.draws.push((
        white,
        vec![data::sprite_sequence::Quad {
            corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
            uv: [0, 0, 1, 1],
            colors: [BACKING; 4],
            blend: 0,
        }],
    ));
    let mut quads = Vec::new();
    for (n, line) in encoded.iter().enumerate() {
        let origin = [ORIGIN[0], ORIGIN[1] + n as f32 * LINE];
        quads.extend(field_command_menu::text_quads_sized(
            line, origin, TEXT, &f, SIZE,
        ));
    }
    frame.draws.push((sheet, quads));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_report_rate_lows_and_p99() {
        let mut s: VecDeque<f32> = std::iter::repeat_n(0.01, 198).collect();
        s.push_back(0.05);
        s.push_back(0.03);
        let (fps, low, p99) = stats(&s).unwrap();
        // The last second: the two slow frames and 92 of 10 ms.
        assert!((fps - 94.).abs() < 0.5, "{fps}");
        // The slowest 1% (2 frames) average 40 ms.
        assert!((low - 25.).abs() < 0.01, "{low}");
        // Two slow frames of 200 are the slowest 1%: the 99th percentile
        // is still a 10 ms frame.
        assert!((p99 - 10.).abs() < 0.01, "{p99}");
        assert!(stats(&VecDeque::new()).is_none());
    }
}
