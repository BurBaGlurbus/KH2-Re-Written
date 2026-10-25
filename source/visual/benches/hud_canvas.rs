//! The field HUD's software canvas (`gauge::Canvas::render`), the hottest
//! system in the test room and Twilight Town traces. Generated textures and
//! a draw list shaped like the field HUD: a command menu at the lower left,
//! the gauge with depth masks at the lower right, a line of glyphs at the
//! upper left. No game data.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use data::sprite_sequence::Quad;
use std::hint::black_box;
use visual::gauge::{Canvas, Draw, Texture};

fn texture(size: usize) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| {
            let (x, y) = (i % size, i / size);
            [
                (x * 7) as u8,
                (y * 5) as u8,
                (x ^ y) as u8,
                ((x + y) % 256) as u8,
            ]
        })
        .collect()
}

fn quad(x: f32, y: f32, w: f32, h: f32, uv: [i32; 4], blend: i32) -> Quad {
    Quad {
        corners: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]],
        uv,
        colors: [[1., 1., 1., 0.9]; 4],
        blend,
    }
}

/// A field HUD frame: about 180 quads and two masks.
fn hud_draws() -> Vec<Draw> {
    let menu = (0..12)
        .map(|i| quad(20., 300. + i as f32 * 8., 120., 14., [0, 0, 60, 14], 0))
        .collect();
    let gauge = (0..40)
        .map(|i| {
            let a = i as f32 * 0.15;
            quad(
                400. + a.cos() * 40.,
                330. + a.sin() * 40.,
                32.,
                32.,
                [64, 0, 96, 32],
                i % 2,
            )
        })
        .collect();
    let glyphs = (0..120)
        .map(|i| quad(16. + i as f32 * 4., 20., 6., 10., [0, 64, 6, 74], 0))
        .collect();
    vec![
        Draw::Quads {
            quads: menu,
            z: 0,
            sheet: 0,
        },
        Draw::Mask {
            polygons: vec![[[380., 300.], [470., 300.], [380., 400.], [470., 400.]]],
            z: 2,
        },
        Draw::Quads {
            quads: gauge,
            z: 3,
            sheet: 0,
        },
        Draw::Mask {
            polygons: vec![[[420., 340.], [450., 340.], [420., 370.], [450., 370.]]],
            z: 4,
        },
        Draw::Quads {
            quads: glyphs,
            z: 0,
            sheet: 0,
        },
    ]
}

fn render(c: &mut Criterion) {
    // Two frames that differ (the gauge moves one unit), so every render
    // in `render` draws; `unchanged` repeats one frame.
    let frames = [hud_draws(), {
        let mut d = hud_draws();
        if let Draw::Quads { quads, .. } = &mut d[2] {
            for q in quads {
                q.corners.iter_mut().for_each(|c| c[0] += 1.);
            }
        }
        d
    }];
    let mut group = c.benchmark_group("hud_canvas");
    for scale in [2u32, 4] {
        let size = 128 * scale as usize;
        let rgba = texture(size);
        let textures = [Texture {
            rgba: &rgba,
            size: [size, size],
            texel_scale: scale as f32,
        }];
        let mut canvas = Canvas::new(scale as f32);
        let mut n = 0;
        group.bench_with_input(BenchmarkId::new("render", scale), &scale, |b, _| {
            b.iter(|| {
                n += 1;
                canvas.render(black_box(&frames[n % 2]), &textures)
            })
        });
        group.bench_with_input(BenchmarkId::new("unchanged", scale), &scale, |b, _| {
            b.iter(|| canvas.render(black_box(&frames[0]), &textures))
        });
        let mut texture_data = canvas.rgba.clone();
        group.bench_with_input(BenchmarkId::new("copy", scale), &scale, |b, _| {
            b.iter(|| texture_data.copy_from_slice(black_box(&canvas.rgba)))
        });
    }
    group.finish();
}

criterion_group!(benches, render);
criterion_main!(benches);
