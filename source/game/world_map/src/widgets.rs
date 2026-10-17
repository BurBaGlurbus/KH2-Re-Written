//! The world map events' character scenes: up to four worldmap::ObjWidget
//! (FUN_140393920 over menu::ObjWidget, FUN_140535040) in the event
//! object's slots (+0x50), each an object model drawn by its own fixed
//! camera (FUN_14019dc50: focal 512 on a 640 × 480 screen, near 1, far
//! 4 000 000, at the origin looking along +Z) through the shared widget
//! view matrix (`DAT_140bf2c50`, ops 0x10b / 0x10c). While any widget
//! exists the map is hidden (FUN_1401625f0); when the last goes it shows
//! again and the objects refresh (FUN_140162460, FUN_1403f0b70,
//! FUN_140394050).
//!
//! Matrices are the exe's: row vectors, `v' = v · M`, engine axes (Y down).
use crate::events::Op;

pub const SLOTS: usize = 4;

/// The widget camera's projection (FUN_140535040's FUN_14019dc50 arguments
/// `DAT_1405b1390` and `DAT_140585c7c`: 512 / 0.8 = 640 wide).
pub const FOCAL: f32 = 512.;
pub const NEAR: f32 = 1.;
pub const FAR: f32 = 4_000_000.;

/// The horizontal field of view of the widget camera (FUN_14019dc50:
/// `2 atan(512 / 0.8 × 0.5 / 512)`).
pub fn horizontal_fov() -> f32 {
    2. * (320. / FOCAL).atan()
}

pub type Matrix = [[f32; 4]; 4];

pub const IDENTITY: Matrix = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

/// FUN_14013f850(b, out, a): every row of `a` times `b`, so `a · b`.
pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.; 4]; 4];
    for (r, row) in a.iter().enumerate() {
        for c in 0..4 {
            out[r][c] = (0..4).map(|k| row[k] * b[k][c]).sum();
        }
    }
    out
}

/// FUN_1401411b0 / FUN_140141270 / FUN_140141320: the rotations about X,
/// Y and Z.
fn rotation(axis: usize, a: f32) -> Matrix {
    let (s, c) = a.sin_cos();
    let mut m = IDENTITY;
    match axis {
        0 => {
            m[1] = [0., c, s, 0.];
            m[2] = [0., -s, c, 0.];
        }
        1 => {
            m[0] = [c, 0., -s, 0.];
            m[2] = [s, 0., c, 0.];
        }
        _ => {
            m[0] = [c, s, 0., 0.];
            m[1] = [-s, c, 0., 0.];
        }
    }
    m
}

/// FUN_1401aae60: X, then Y, then Z; a zero angle leaves its axis out.
pub fn rotate_xyz(mut m: Matrix, r: [f32; 3]) -> Matrix {
    for (axis, &a) in r.iter().enumerate() {
        if a != 0. {
            m = multiply(&m, &rotation(axis, a));
        }
    }
    m
}

/// FUN_1403b7410: the rotation, then the translation row with w = 1.
pub fn rotate_translate(r: [f32; 3], t: [f32; 3]) -> Matrix {
    let mut m = rotate_xyz(IDENTITY, r);
    m[3] = [t[0], t[1], t[2], 1.];
    m
}

/// What a widget's motion entry plays: its length in ticks (the motion's
/// vtable +8, header word 1), where it loops back to (+0x30:
/// `(frameReturn − frameStart) × 60 / fps`), and the sounds of its frame
/// triggers (type 8: `bank × 1000 + number`, LAB_1405359b0) at their
/// frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    pub length: f32,
    pub loop_start: f32,
    pub sounds: Vec<(f32, u32)>,
}

/// FUN_14040e8a0's test: frames in (from, to], or exactly `from` when `to`
/// is negative.
fn fire(clip: &Clip, from: f32, to: f32, out: &mut Vec<u32>) {
    for &(frame, sound) in &clip.sounds {
        if (from < frame && frame <= to) || (to < 0. && frame == from) {
            out.push(sound);
        }
    }
}

/// menu::ObjWidget's motion player (+0x298, FUN_140535970): the playing
/// archive entry, its clock and a ring of four queued entries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    /// +0x10: the playing entry of the event archive.
    pub entry: Option<u32>,
    /// +0x20, in ticks.
    pub time: f32,
    /// +0x24.
    length: f32,
    /// +0x30: {entry, start time}; +0x28 the write and +0x2c the read
    /// index.
    queue: [(u32, f32); 4],
    write: usize,
    read: usize,
}

impl Track {
    /// FUN_1405359d0: the entry starts at `start` and its triggers at that
    /// frame fire.
    fn set(&mut self, entry: u32, start: f32, clips: &[Option<Clip>], out: &mut Vec<u32>) {
        let Some(Some(clip)) = clips.get(entry as usize) else {
            // Not a motion archive: the exe would read garbage here; the
            // widget holds still instead.
            self.entry = None;
            return;
        };
        self.entry = Some(entry);
        self.time = start;
        self.length = clip.length;
        fire(clip, start, -1., out);
    }

    /// FUN_140535b00: the queue empties and the entry starts.
    pub fn play(&mut self, entry: u32, start: f32, clips: &[Option<Clip>], out: &mut Vec<u32>) {
        self.write = 0;
        self.read = 0;
        self.set(entry, start, clips, out);
    }

    /// FUN_140535ac0: after the playing entry (the ring overwrites when
    /// full), or at once when nothing plays.
    pub fn queue(&mut self, entry: u32, start: f32, clips: &[Option<Clip>], out: &mut Vec<u32>) {
        if self.entry.is_none() {
            self.set(entry, start, clips, out);
            return;
        }
        self.queue[self.write] = (entry, start);
        self.write = (self.write + 1) & 3;
    }

    /// FUN_140535b10: the clock advances and the triggers passed fire; at
    /// the end the next queued entry starts, else the motion loops.
    pub fn update(&mut self, dt: f32, clips: &[Option<Clip>], out: &mut Vec<u32>) {
        let Some(entry) = self.entry else { return };
        let Some(Some(clip)) = clips.get(entry as usize) else {
            return;
        };
        let from = self.time;
        self.time += dt;
        fire(clip, from, self.time, out);
        if self.time < self.length {
            return;
        }
        if self.read != self.write {
            let (next, start) = self.queue[self.read];
            self.read = (self.read + 1) & 3;
            self.set(next, start, clips, out);
        } else {
            self.time = self.time - self.length + clip.loop_start;
            fire(clip, clip.loop_start, self.time, out);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    /// The object entry (created with bit 30 set, FUN_14038f270 op 0x102).
    pub object: u32,
    /// +0x318 (w = 1, op 0x109) and +0x308 (radians, op 0x10a).
    pub position: [f32; 4],
    pub rotation: [f32; 3],
    pub track: Track,
}

/// What the map does when the widgets come and go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// FUN_1401625f0: the map's draw groups and objects hide.
    HideMap,
    /// FUN_140162460, FUN_1403f0b70 and FUN_140394050: they show again,
    /// the draw group set applies and the objects re-read their looks.
    ShowMap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub slots: [Option<Widget>; SLOTS],
    /// `DAT_140748898` / `DAT_1407488a8` and the matrix built from them.
    view_position: [f32; 3],
    view_rotation: [f32; 3],
    view: Matrix,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            slots: Default::default(),
            view_position: [0.; 3],
            view_rotation: [0.; 3],
            view: IDENTITY,
        }
    }
}

impl Scene {
    pub fn shown(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }

    /// FUN_14038f1f0: the slot's widget goes; with none left the map shows.
    pub fn remove(&mut self, slot: usize) -> Vec<Change> {
        if let Some(s) = self.slots.get_mut(slot) {
            *s = None;
        }
        if self.shown() {
            Vec::new()
        } else {
            vec![Change::ShowMap]
        }
    }

    /// FUN_14038f150 (op 0x112 and the event's end, FUN_14038eda0): every
    /// widget goes. The exe repeats the map's return after each slot; it
    /// changes nothing the first did not.
    pub fn clear(&mut self) -> Vec<Change> {
        self.slots = Default::default();
        vec![Change::ShowMap]
    }

    /// Op 0x102 (FUN_14038ee70's body): the slot is emptied, the first
    /// widget hides the map, and the new one stands at the origin.
    pub fn create(&mut self, slot: usize, object: u32) -> Vec<Change> {
        let mut out = self.remove(slot);
        if !self.shown() {
            out.push(Change::HideMap);
        }
        if let Some(s) = self.slots.get_mut(slot) {
            *s = Some(Widget {
                object,
                position: [0., 0., 0., 1.],
                rotation: [0.; 3],
                track: Track::default(),
            });
        }
        out
    }

    /// The widget ops of FUN_14038f270 (0x102, 0x103, 0x109..0x10d, 0x111,
    /// 0x112). `clips` holds the event archive's entries (None where an
    /// entry is no motion). Sound ids the motions' triggers request are
    /// added to `sounds`.
    pub fn apply(&mut self, op: &Op, clips: &[Option<Clip>], sounds: &mut Vec<u32>) -> Vec<Change> {
        match op {
            Op::Widget(s, object) => return self.create(slot(*s), *object),
            Op::RemoveWidget(s) => return self.remove(slot(*s)),
            Op::ClearWidgets => return self.clear(),
            Op::WidgetMotion(s, entries) => {
                if let Some(Some(w)) = self.slots.get_mut(slot(*s))
                    && let Some((first, rest)) = entries.split_first()
                {
                    // FUN_140535440 (start 0), then FUN_140535420.
                    w.track.play(*first, 0., clips, sounds);
                    for &e in rest {
                        w.track.queue(e, 0., clips, sounds);
                    }
                }
            }
            Op::WidgetQueue(s, entries) => {
                if let Some(Some(w)) = self.slots.get_mut(slot(*s)) {
                    for &e in entries {
                        w.track.queue(e, 0., clips, sounds);
                    }
                }
            }
            Op::WidgetPosition(s, p) => {
                if let Some(Some(w)) = self.slots.get_mut(slot(*s)) {
                    w.position = [p[0], p[1], p[2], 1.];
                }
            }
            Op::WidgetRotation(s, r) => {
                if let Some(Some(w)) = self.slots.get_mut(slot(*s)) {
                    w.rotation = *r;
                }
            }
            // FUN_140393ad0 / FUN_140393b10.
            Op::WidgetViewPosition(p) => {
                self.view_position = *p;
                self.view = rotate_translate(self.view_rotation, self.view_position);
            }
            Op::WidgetViewRotation(r) => {
                self.view_rotation = *r;
                self.view = rotate_translate(self.view_rotation, self.view_position);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Each widget's motion clock (FUN_140535450 → FUN_140535b10, one tick:
    /// `DAT_140717480` = 1).
    pub fn update(&mut self, clips: &[Option<Clip>], sounds: &mut Vec<u32>) {
        for w in self.slots.iter_mut().flatten() {
            w.track.update(1., clips, sounds);
        }
    }

    /// FUN_1403939f0: the widget's rotation and position, then the view;
    /// the result maps model space into the widget camera's space.
    pub fn matrix(&self, slot: usize) -> Option<Matrix> {
        let w = self.slots.get(slot)?.as_ref()?;
        let mut local = rotate_xyz(IDENTITY, w.rotation);
        local[3] = w.position;
        Some(multiply(&local, &self.view))
    }
}

/// Script slot words are small indices; anything past the four slots is
/// ignored (the exe would write past its array).
fn slot(s: u32) -> usize {
    if (s as usize) < SLOTS {
        s as usize
    } else {
        usize::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(length: f32, loop_start: f32, sounds: &[(f32, u32)]) -> Option<Clip> {
        Some(Clip {
            length,
            loop_start,
            sounds: sounds.to_vec(),
        })
    }

    #[test]
    fn a_motion_plays_its_queue_then_loops_the_last() {
        let clips = [
            clip(3., 0., &[(0., 1001), (2., 1002)]),
            clip(4., 2., &[(3., 2001)]),
        ];
        let mut sounds = Vec::new();
        let mut t = Track::default();
        t.play(0, 0., &clips, &mut sounds);
        t.queue(1, 0., &clips, &mut sounds);
        // The start fires frame 0's trigger.
        assert_eq!(sounds, [1001]);
        sounds.clear();
        for _ in 0..3 {
            t.update(1., &clips, &mut sounds);
        }
        // Frame 2's trigger, then entry 1 starts at its end.
        assert_eq!(sounds, [1002]);
        assert_eq!((t.entry, t.time), (Some(1), 0.));
        for _ in 0..4 {
            t.update(1., &clips, &mut sounds);
        }
        // Entry 1 has nothing queued after it: it loops back to 2.
        assert_eq!((t.entry, t.time), (Some(1), 2.));
        assert_eq!(sounds, [1002, 2001]);
    }

    #[test]
    fn the_first_widget_hides_the_map_and_the_last_brings_it_back() {
        let mut s = Scene::default();
        let mut sounds = Vec::new();
        // Creating into an empty scene first empties the slot, which finds
        // no widget: the map shows (and refreshes), then hides.
        assert_eq!(
            s.apply(&Op::Widget(0, 0x863), &[], &mut sounds),
            [Change::ShowMap, Change::HideMap]
        );
        assert!(s.apply(&Op::Widget(1, 0x895), &[], &mut sounds).is_empty());
        assert!(s.apply(&Op::RemoveWidget(0), &[], &mut sounds).is_empty());
        assert_eq!(
            s.apply(&Op::RemoveWidget(1), &[], &mut sounds),
            [Change::ShowMap]
        );
        assert!(!s.shown());
    }

    #[test]
    fn the_view_moves_and_turns_every_widget() {
        let mut s = Scene::default();
        let mut sounds = Vec::new();
        s.apply(&Op::Widget(1, 0x895), &[], &mut sounds);
        s.apply(&Op::WidgetPosition(1, [-33., 0., -20.]), &[], &mut sounds);
        s.apply(&Op::WidgetViewPosition([2., 38., 192.]), &[], &mut sounds);
        let m = s.matrix(1).unwrap();
        assert_eq!(m[3], [-31., 38., 172., 1.]);
        // A quarter turn about Y takes +X to −Z (D3DX rows).
        s.apply(
            &Op::WidgetViewRotation([0., std::f32::consts::FRAC_PI_2, 0.]),
            &[],
            &mut sounds,
        );
        let m = s.matrix(1).unwrap();
        assert!((m[0][2] - -1.).abs() < 1e-6 && m[0][0].abs() < 1e-6);
        // The position turns too, then the view's translation adds.
        assert!((m[3][0] - (-20. + 2.)).abs() < 1e-4 && (m[3][2] - (33. + 192.)).abs() < 1e-4);
    }
}
