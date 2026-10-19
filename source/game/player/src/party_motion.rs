//! A party member's motion controller: the system object at `obj+0x158` of
//! the executable's friend object, and the ground mode's choice of the
//! locomotion motion.
//!
//! * `FUN_1403c86a0` starts a motion: the queue is emptied, then
//!   `FUN_1403c88c0` resolves the slot for the member's stance.
//! * `FUN_1403c88c0`: idle asked for while the playing slot belongs to the
//!   other stance (slots `4m + 0/1` are the battle variants, `4m + 2/3` the
//!   field ones) plays motion 6 instead, the weapon drawn or put away, with
//!   idle queued behind it.
//! * `FUN_1403c8a40`: a blend longer than 2 ticks sets the clock to 2 and
//!   holds the new motion at its start frame until the clock reaches the
//!   blend; meanwhile the shown pose turns from the one the switch left
//!   toward it (weight clock / blend).
//! * `FUN_1403c6dc0` changes the motion: nothing when the slot already
//!   plays; walk and run (1 and 2) keep the phase, starting at
//!   `fmod(len_new × (clock + 1 + blend) / len_old, len_new)`.
//! * `FUN_1403c8310` queues a motion behind the playing one (4 slots); it
//!   starts when the clock reaches the end, instead of the loop.
//! * `FUN_1403c6740` advances the clock one tick: the first step from 0
//!   jumps to 2, a finished blend continues from the start frame, and at the
//!   end the queue's next motion starts or the clock wraps to the return
//!   frame (it holds at the end when the return frame is the end).
//!
//! The stance (`obj+0x120` bit 20) is the member's: any battle-variant slot
//! that starts turns it on (`FUN_1403c8cd0`), so does the battle flag every
//! tick (`FUN_1403d68b0`); `FUN_1403ba350` turns it off at a motion's end
//! once the battle flag is down. [`Step::stance_check`] marks the ends that
//! call it.

use crate::Vec3;

/// The locomotion blend (`DAT_140623bac`, FUN_140422520) and the blend of
/// the spell acts and the item motion.
pub const BLEND: f32 = 8.;
/// The weapon draw / put-away motion of a stance switch (A010).
pub const STANCE_SWITCH: u16 = 6;
/// Motion ids the ground mode plays.
pub const IDLE: u16 = 0;
pub const WALK: u16 = 1;
pub const RUN: u16 = 2;

/// A resolved motion: its slot in the set, its length in ticks and the
/// tick its loop returns to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clip {
    pub slot: usize,
    pub length: u32,
    pub loop_top: u32,
}

impl Clip {
    /// The battle variants (`slot & 3` 0 or 1) against the field ones.
    pub fn battle(&self) -> bool {
        self.slot & 3 < 2
    }
}

/// The member's motion set (`FUN_1403c7c50`: the slot of a motion id for
/// the stance) and its range triggers.
pub trait Clips {
    /// The motion's clip in the field (`true`) or battle stance.
    fn clip(&self, motion: u16, field: bool) -> Option<Clip>;
    /// Whether range trigger `kind` (0, 1 or 2: `obj+0x18c` bits 2-4) of
    /// the slot is open at `tick`.
    fn range(&self, slot: usize, kind: u8, tick: u32) -> bool;
}

/// What one advance did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    /// The clock reached the motion's end (`+0x34` bit 1).
    pub ended: bool,
    /// The end called `FUN_1403ba350`: the stance may return to the field.
    pub stance_check: bool,
    /// A queued motion started.
    pub queued: Option<u16>,
    /// The clock spans the motion's frames covered this tick (for root
    /// motion): (motion id, field stance, from, to).
    pub spans: Vec<(u16, bool, f32, f32)>,
    /// A battle-variant slot started this tick (`FUN_1403c8cd0` sets the
    /// stance bit).
    pub battle_started: bool,
}

/// The controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ctl {
    pub motion: u16,
    /// The stance the slot was resolved for.
    pub field: bool,
    pub clip: Option<Clip>,
    /// `+0x44`: the clock in ticks.
    pub clock: f32,
    /// `+0x48`: the blend's length while one runs, else 0.
    pub blend: f32,
    /// `+0x4c`: the frame the motion continues from after its blend.
    pub blend_frame: f32,
    /// `+0x50..`: motion, blend and start frame of the queued motions (a
    /// ring of four in the exe).
    queue: [(u16, f32, f32); 4],
    queued: u8,
    /// Counts the starts, so the presentation knows when a switch happened.
    pub serial: u32,
    /// The last advance ended the motion.
    pub ended: bool,
    /// A battle-variant slot started since the last [`Ctl::take_battle`].
    battle_started: bool,
}

impl Default for Ctl {
    fn default() -> Self {
        Ctl {
            motion: IDLE,
            field: false,
            clip: None,
            clock: 0.,
            blend: 0.,
            blend_frame: 0.,
            queue: [(0, 0., 0.); 4],
            queued: 0,
            serial: 0,
            ended: false,
            battle_started: false,
        }
    }
}

impl Ctl {
    /// The motion's frame to show (the start frame while a blend runs).
    pub fn frame(&self) -> f32 {
        if self.blend > 0. {
            self.blend_frame
        } else {
            self.clock
        }
    }

    /// The share of the new motion in the shown pose while a blend runs.
    pub fn weight(&self) -> Option<f32> {
        (self.blend > 0.).then(|| (self.clock / self.blend).clamp(0., 1.))
    }

    /// Ticks left before the motion's own frames run (the blend still to
    /// count; FUN_1403c8a40 starts the clock at 2).
    pub fn hold(blend: f32) -> u32 {
        if blend > 2. { (blend - 2.) as u32 } else { 0 }
    }

    /// The slot plays a battle variant.
    pub fn battle_slot(&self) -> bool {
        self.clip.is_some_and(|c| c.battle())
    }

    /// Whether range `kind` of the playing motion is open now.
    pub fn range(&self, kind: u8, clips: &dyn Clips) -> bool {
        self.clip
            .is_some_and(|c| clips.range(c.slot, kind, self.frame().max(0.) as u32))
    }

    /// Returns and clears the "battle variant started" mark.
    pub fn take_battle(&mut self) -> bool {
        std::mem::take(&mut self.battle_started)
    }

    /// FUN_1403c8a40 / FUN_1403c8cd0: the slot begins.
    fn begin(&mut self, motion: u16, field: bool, clip: Clip, blend: f32, frame: f32) {
        self.motion = motion;
        self.field = field;
        self.clip = Some(clip);
        self.serial = self.serial.wrapping_add(1);
        self.ended = false;
        if blend <= 2. {
            self.clock = frame;
            self.blend = 0.;
        } else {
            self.blend = blend;
            self.blend_frame = frame;
            self.clock = 2.;
        }
        if clip.slot & 3 == 0 {
            self.battle_started = true;
        }
    }

    /// FUN_1403c88c0: the stance switch, then the slot.
    fn resolve(
        &mut self,
        motion: u16,
        blend: f32,
        frame: f32,
        field: bool,
        clips: &dyn Clips,
    ) -> bool {
        let Some(clip) = clips.clip(motion, field) else {
            return false;
        };
        if motion == IDLE
            && let Some(current) = self.clip
            && current.battle() != clip.battle()
            && let Some(switch) = clips.clip(STANCE_SWITCH, field)
        {
            self.begin(STANCE_SWITCH, field, switch, 0., 0.);
            self.push(IDLE, 0., 0.);
            return true;
        }
        self.begin(motion, field, clip, blend, frame);
        true
    }

    fn push(&mut self, motion: u16, blend: f32, frame: f32) {
        if (self.queued as usize) < self.queue.len() {
            self.queue[self.queued as usize] = (motion, blend, frame);
            self.queued += 1;
        }
    }

    fn pop(&mut self) -> Option<(u16, f32, f32)> {
        if self.queued == 0 {
            return None;
        }
        let first = self.queue[0];
        self.queue.copy_within(1.., 0);
        self.queued -= 1;
        Some(first)
    }

    /// FUN_1403c86a0: start `motion` with `blend`, emptying the queue.
    pub fn start(&mut self, motion: u16, blend: f32, field: bool, clips: &dyn Clips) -> bool {
        self.queued = 0;
        self.resolve(motion, blend, 0., field, clips)
    }

    /// FUN_1403c6dc0: like start, but nothing when the slot already plays;
    /// walk and run keep the phase.
    pub fn change(&mut self, motion: u16, blend: f32, field: bool, clips: &dyn Clips) {
        self.queued = 0;
        let Some(clip) = clips.clip(motion, field) else {
            return;
        };
        if self.clip.is_some_and(|c| c.slot == clip.slot) {
            return;
        }
        let mut frame = 0.;
        if let Some(old) = self.clip
            && matches!(self.motion, WALK | RUN)
            && matches!(motion, WALK | RUN)
            && old.length > 0
            && clip.length > 0
        {
            let new = clip.length as f32;
            frame = (new * ((1. + self.clock + blend) / old.length as f32)) % new;
        }
        self.resolve(motion, blend, frame, field, clips);
    }

    /// FUN_1403c8310: play `motion` with `blend` when the current one ends.
    pub fn queue(&mut self, motion: u16, blend: f32) {
        self.push(motion, blend, 0.);
    }

    /// The motions queued, in order.
    pub fn queued(&self) -> impl Iterator<Item = u16> + '_ {
        self.queue[..self.queued as usize].iter().map(|q| q.0)
    }

    /// FUN_1403c6740: one tick. `field` is the stance a queued motion
    /// resolves in.
    pub fn advance(&mut self, field: bool, clips: &dyn Clips) -> Step {
        let mut step = Step::default();
        self.ended = false;
        let Some(clip) = self.clip else {
            return step;
        };
        let old = self.clock;
        let mut clock = old + 1.;
        if clock == 1. && old == 0. {
            clock = 2.;
        }
        self.clock = clock;
        if self.blend > 0. {
            if clock >= self.blend {
                self.clock = clock - self.blend + self.blend_frame;
                self.blend = 0.;
                step.spans
                    .push((self.motion, self.field, self.blend_frame, self.clock));
            }
            step.battle_started = self.take_battle();
            return step;
        }
        let length = clip.length as f32;
        if clock < length {
            step.spans.push((self.motion, self.field, old, clock));
            step.battle_started = self.take_battle();
            return step;
        }
        step.ended = true;
        self.ended = true;
        step.spans.push((self.motion, self.field, old, length));
        let left = clock - length;
        if self.queued == 0 {
            let wrapped = clip.loop_top as f32 + left;
            if wrapped <= length {
                self.clock = wrapped;
                step.spans
                    .push((self.motion, self.field, clip.loop_top as f32, wrapped));
            } else {
                self.clock = length;
            }
            step.stance_check = true;
        } else {
            // `+0x30` (the variant that ended) other than 0 checks the
            // stance before the queued motion starts.
            step.stance_check = clip.slot & 3 != 0;
            let (motion, blend, frame) = self.pop().expect("queued above");
            if self.resolve(motion, blend, frame, field, clips) {
                step.queued = Some(motion);
                // The new motion's ended flag is the end pulse of this tick.
                self.ended = true;
            }
        }
        step.battle_started = self.take_battle();
        step
    }
}

/// FUN_140422520 (the ground mode): idle at no speed, run from
/// `(walk + run) / 2`, walk below.
pub fn ground_motion(speed: f32, walk: f32, run: f32) -> u16 {
    if speed <= 0. {
        IDLE
    } else if (run + walk) * 0.5 <= speed {
        RUN
    } else {
        WALK
    }
}

/// FUN_1401b1b20: the think's wish (direction `obj+0xe78`, speed `+0xe84`)
/// smoothed over the last six ticks, and the speed snapped to none, walk or
/// run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wish {
    history: [Vec3; 6],
    /// `obj+0xf60`: the next history slot (-1 before the first tick).
    next: i32,
}

impl Default for Wish {
    fn default() -> Self {
        Wish {
            history: [[0.; 3]; 6],
            next: -1,
        }
    }
}

impl Wish {
    /// One tick: `dir` is the unit direction the think wants and `speed`
    /// its speed (0, walk or run). `hold` is `obj+0xe18` bit 4 (the member
    /// steps aside at walking speed). Returns the smoothed direction (not
    /// unit length) and the snapped speed.
    pub fn smooth(
        &mut self,
        dir: Vec3,
        speed: f32,
        walk: f32,
        run: f32,
        hold: bool,
    ) -> (Vec3, f32) {
        let v = if hold {
            [0.; 3]
        } else {
            dir.map(|c| c * speed)
        };
        if self.next < 0 {
            self.history[..5].fill(v);
            self.next = 5;
        }
        self.history[self.next as usize] = v;
        self.next += 1;
        if self.next > 5 {
            self.next = 0;
        }
        let sum = self
            .history
            .iter()
            .fold([0.; 3], |a, h| [a[0] + h[0], a[1] + h[1], a[2] + h[2]]);
        let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
        let unit = if len != 0. { sum.map(|c| c / len) } else { sum };
        let average = len / 6.;
        let out_dir = [
            (dir[0] + unit[0]) * 0.5,
            (dir[1] + unit[1]) * 0.5,
            (dir[2] + unit[2]) * 0.5,
        ];
        let mut out = (speed + average) * 0.5;
        if out < run {
            out = if walk * 0.5 <= out {
                if (run + walk) * 0.5 <= out { run } else { walk }
            } else {
                0.
            };
        }
        if hold {
            out = walk;
        }
        (out_dir, out)
    }
}

/// The follow distances of the personality header (`FUN_1401cc890` enter,
/// `FUN_1401cc8c0` exit), by goal: out of battle, the leader in battle and
/// the target in battle; each by follow state 0 (stand), 1 (walk), 2 (run).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    pub enter: [[f32; 3]; 3],
    pub exit: [[f32; 3]; 3],
}

/// Donald's (personality 0, 0x14071d850) and Goofy's (personality 1,
/// 0x14071dd50) header: the same numbers.
pub const FOLLOW: Follow = Follow {
    enter: [[200., 240., 1000.], [200., 240., 1000.], [0., 240., 1000.]],
    exit: [[0., 160., 200.], [0., 160., 200.], [0., 160., 200.]],
};

/// Which table of [`Follow`] applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Field,
    Leader,
    Target,
}

impl Follow {
    /// FUN_1401b41a0's state step: up a state beyond the state's enter
    /// distance, down one inside its exit distance.
    pub fn next(&self, state: u8, distance: f32, goal: Goal) -> u8 {
        let g = goal as usize;
        let s = state.min(2) as usize;
        if distance <= self.enter[g][s] {
            if distance < self.exit[g][s] && state > 0 {
                return state - 1;
            }
            state
        } else if state < 2 {
            state + 1
        } else {
            state
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Idle (slots 0 battle, 2 field), walk (4, 7), run (8, 11), the stance
    /// switch (24, 27) and an attack (604) with range 0 on the locomotion.
    struct Set;
    impl Clips for Set {
        fn clip(&self, motion: u16, field: bool) -> Option<Clip> {
            let (slot, length, loop_top) = match (motion, field) {
                (0, false) => (0, 400, 0),
                (0, true) => (2, 300, 0),
                (1, false) => (4, 52, 0),
                (1, true) => (7, 48, 0),
                (2, false) => (8, 36, 0),
                (2, true) => (11, 36, 0),
                (6, false) => (24, 26, 26),
                (6, true) => (27, 40, 40),
                (151, _) => (604, 64, 64),
                (4, _) => (16, 30, 0),
                _ => return None,
            };
            Some(Clip {
                slot,
                length,
                loop_top,
            })
        }
        fn range(&self, slot: usize, kind: u8, _: u32) -> bool {
            kind == 0 && slot < 12
        }
    }

    #[test]
    fn the_first_step_jumps_from_0_to_2_and_a_loop_wraps_to_its_return_frame() {
        let mut c = Ctl::default();
        c.start(1, 0., false, &Set);
        c.advance(false, &Set);
        assert_eq!(c.clock, 2.);
        for _ in 0..49 {
            c.advance(false, &Set);
        }
        assert_eq!(c.clock, 51.);
        let s = c.advance(false, &Set);
        assert!(s.ended && s.stance_check && s.queued.is_none());
        assert_eq!(c.clock, 0.);
    }

    #[test]
    fn a_blend_holds_the_start_frame_until_the_clock_reaches_it() {
        let mut c = Ctl::default();
        c.start(0, 0., false, &Set);
        c.start(151, BLEND, false, &Set);
        assert_eq!(c.clock, 2.);
        let mut weights = Vec::new();
        for _ in 0..5 {
            c.advance(false, &Set);
            weights.push(c.weight().unwrap());
            assert_eq!(c.frame(), 0.);
        }
        assert_eq!(weights, vec![3. / 8., 4. / 8., 5. / 8., 6. / 8., 7. / 8.]);
        c.advance(false, &Set);
        assert_eq!((c.weight(), c.clock), (None, 0.));
        assert_eq!(Ctl::hold(BLEND), 6);
    }

    #[test]
    fn an_attack_ends_into_the_queued_idle_and_keeps_the_battle_stance() {
        let mut c = Ctl::default();
        c.start(151, 0., false, &Set);
        c.queue(0, 0.);
        let mut ticks = 1;
        let step = loop {
            let s = c.advance(false, &Set);
            if s.ended {
                break s;
            }
            ticks += 1;
        };
        assert_eq!(ticks, 63);
        assert_eq!(step.queued, Some(0));
        assert!(!step.stance_check);
        assert_eq!((c.motion, c.clock), (0, 0.));
    }

    #[test]
    fn a_change_to_the_playing_slot_does_nothing_and_walk_to_run_keeps_the_phase() {
        let mut c = Ctl::default();
        c.start(1, 0., false, &Set);
        for _ in 0..10 {
            c.advance(false, &Set);
        }
        let serial = c.serial;
        c.change(1, BLEND, false, &Set);
        assert_eq!(c.serial, serial);
        assert_eq!(c.clock, 11.);
        c.change(2, BLEND, false, &Set);
        // fmod(36 × (11 + 1 + 8) / 52, 36) = 13.85; held through the blend.
        assert!((c.blend_frame - 36. * 20. / 52.).abs() < 1e-4);
        assert_eq!((c.motion, c.clock), (2, 2.));
    }

    #[test]
    fn idle_in_the_other_stance_plays_the_switch_motion_first() {
        let mut c = Ctl::default();
        c.start(0, 0., false, &Set);
        c.change(0, BLEND, true, &Set);
        assert_eq!(c.motion, STANCE_SWITCH);
        assert_eq!(c.clip.unwrap().slot, 27);
        assert_eq!(c.queued().collect::<Vec<_>>(), vec![0]);
        // The switch has no blend and ends into the field idle.
        let mut n = 0;
        while c.motion == STANCE_SWITCH {
            c.advance(true, &Set);
            n += 1;
        }
        assert_eq!(n, 39);
        assert_eq!(c.clip.unwrap().slot, 2);
    }

    #[test]
    fn the_wish_walks_in_and_out_of_a_run() {
        let mut w = Wish::default();
        let speeds: Vec<f32> = (0..8)
            .map(|_| w.smooth([0., 0., 1.], 8., 2., 8., false).1)
            .collect();
        // The first wish fills the history: the run starts at once.
        assert_eq!(speeds[0], 8.);
        // Stopping walks four ticks first; starting walks one.
        let stops: Vec<f32> = (0..6)
            .map(|_| w.smooth([0., 0., 1.], 0., 2., 8., false).1)
            .collect();
        assert_eq!(&stops, &[2., 2., 2., 2., 0., 0.]);
        let starts: Vec<f32> = (0..3)
            .map(|_| w.smooth([0., 0., 1.], 8., 2., 8., false).1)
            .collect();
        assert_eq!(&starts, &[2., 8., 8.]);
    }

    #[test]
    fn follow_states_step_up_past_enter_and_down_inside_exit() {
        assert_eq!(FOLLOW.next(0, 150., Goal::Field), 0);
        assert_eq!(FOLLOW.next(0, 201., Goal::Field), 1);
        assert_eq!(FOLLOW.next(1, 241., Goal::Field), 2);
        assert_eq!(FOLLOW.next(2, 199., Goal::Field), 1);
        assert_eq!(FOLLOW.next(1, 159., Goal::Field), 0);
        // Toward an enemy, standing ends as soon as there is any distance.
        assert_eq!(FOLLOW.next(0, 1., Goal::Target), 1);
        assert_eq!(ground_motion(2., 2., 8.), WALK);
        assert_eq!(ground_motion(8., 2., 8.), RUN);
    }
}
