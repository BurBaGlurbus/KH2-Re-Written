//! Sora's game over (task `FUN_1403fc7a0`, mode 0, created by Sora's death
//! handler `FUN_140404d60`). The sequence it runs:
//!
//! 1. The death event `re_gameover-0a` (00battle.bin, first type-22 entry):
//!    a camera pushing in on Sora, a radial blur for 21 event frames, a game
//!    speed of 0.1 from event frame 4 to 20 and a white fade of layer A that
//!    starts at frame 11 and lasts 50 event frames. The task waits (state 2,
//!    `FUN_1403fcec0`) until the event is over and the fade holds opaque.
//! 2. The field is cleared and `obj/GAMEOVER.mset` loads. The mode-8 list
//!    (pause definition 1: Continue, Load Game) opens over the `pause.2ld`
//!    backdrop group 3 (white to black in 16 frames), with Sora's model
//!    playing the set's motion for his weapon joint (`0010` for joint 1) at
//!    the menu model slot (translation z -100).
//! 3. Continue (result 2, `FUN_1403fcbe0`) fades to black over 30 ticks
//!    (`FUN_140152990` type 1), restores the party's HP and reloads the room
//!    at the last entrance (`FUN_1403a0660`'s record, `FUN_1403a5650`).
//!
//! What this does not reproduce is listed in the task record.
use super::*;
use data::path_lookup::Library;
use data::pause_definition::{self, Definition};
use event::{Timeline, event_camera};
use visual::fade::State as FadeState;
use visual::pause_definition::{self as list, Menu as List};
use visual::sprite_player::Sheet as SpriteSheet;

/// se000 sound 0x13 (`FUN_1401dd170(0x13)`) at the task's creation.
const GAME_OVER_SOUND: u32 = 0x13;
/// The death event's game-speed ops (`SeqGameSpeed` 0x20): from event frame
/// 4 the speed is 0.1, from 20 it is 1.
const SLOW: (f32, f32) = (4., 0.1);
const NORMAL: (f32, f32) = (20., 1.);
/// The event's `EndFrame`.
const EVENT_FRAMES: f32 = 21.;
/// An event frame is 2 dt units (the player's `DAT_140b64f84 / 2.0`).
const UNITS_PER_FRAME: f32 = 2.;
/// The radial blur weight (`SeqBlur` alpha 56 of 128) while the event runs.
const BLUR_WEIGHT: f32 = 56. / 128.;
/// Ticks between the white-out holding and the list opening: the task's
/// states 3 to 5 wait one frame each [inferred].
const LOAD_TICKS: u32 = 3;
/// Continue's fade (`FUN_140156ce0(…, 0xf)`): n = 15, so 30 ticks.
const LEAVE_FADE: u32 = 15;

/// The unattended `--game-over-tour`: Sora is knocked out at tick 90 and the
/// list's Continue is pressed after 150 ticks on screen.
pub const TOUR_DEFEAT: u64 = 90;
/// The room's own capture list stays empty: the field's tick stands still
/// while the screen is up, so the tour captures by ticks since the fall.
pub const CAPTURES: [u64; 0] = [];
pub const END: u64 = 240;
const SNAPS: [u32; 20] = [
    1, 30, 80, 130, 180, 230, 280, 330, 370, 390, 400, 412, 420, 440, 480, 520, 560, 600, 640, 700,
];

/// Everything read from the install.
pub struct Source {
    event: Timeline,
    /// The set's motion for Sora's weapon joint.
    motion: motion::SelectedMotion,
    /// Its length in 60 Hz ticks (the motion loops on it).
    length: f32,
    pause: Menu2ld,
    definition: Definition,
    texts: Vec<Vec<u8>>,
    font: SystemFont,
}

fn locate(
    library: &Library,
    name: &str,
) -> Result<data::path_lookup::Found, Box<dyn std::error::Error>> {
    library
        .find(name)
        .ok_or_else(|| format!("{name} absent").into())
}

impl Source {
    /// `joint` is Sora's objentry weapon joint (`FUN_1403fd2b0` formats it as
    /// `%03d0`; the motion is the third entry of that name in the set's
    /// first collection).
    pub fn load(
        source: &ProbeSource,
        library: &mut Library,
        joint: u8,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // The death event: 00battle.bin's first type-22 entry
        // (FUN_1403f4b10(0)).
        let battle = library.read("00battle.bin")?;
        let table = bar::read_bar(&battle)?;
        let entry = table
            .entries
            .iter()
            .find(|e| e.entry_type == 22 && e.size > 0)
            .ok_or("death event absent")?;
        let script = battle
            .get(entry.offset as usize..entry.offset as usize + entry.size as usize)
            .ok_or("death event exceeds 00battle.bin")?;
        let event = Timeline::new(&data::event_script::read(script)?);

        // GAMEOVER.mset entry 0xfc (mode 0).
        let mset = library.read("obj/GAMEOVER.mset")?;
        let table = bar::read_bar(&mset)?;
        let collection = table.entries.get(0xfc).ok_or("GAMEOVER set short")?;
        let bytes = mset
            .get(collection.offset as usize..collection.offset as usize + collection.size as usize)
            .ok_or("GAMEOVER collection exceeds its set")?;
        let wanted = <[u8; 4]>::try_from(format!("{joint:03}0").as_bytes())?;
        let motion_entry = bar::read_bar(bytes)?
            .entries
            .into_iter()
            .filter(|e| e.name_bytes == wanted)
            .nth(2)
            .filter(|e| e.size > 0)
            .ok_or("GAMEOVER motion for this joint absent")?;
        let anb = bytes
            .get(
                motion_entry.offset as usize
                    ..motion_entry.offset as usize + motion_entry.size as usize,
            )
            .ok_or("GAMEOVER motion exceeds its collection")?;
        let motion = motion::read_anb(anb, 0xfc)?;
        let (start, end, fps, _) = motion.motion.frame_window()?;
        let length = ((end - start) * 60. / fps).round();

        // pause.2ld, the definition and its messages, the system font.
        let pause = locate(library, "menu/us/pause.2ld")?;
        let pause = menu_screen::load(source, pause.index, pause.ordinal, pause.name_hash)?;
        let missions = library.read("14mission.bar")?;
        let table = bar::read_bar(&missions)?;
        let slct = table
            .entries
            .iter()
            .find(|e| e.entry_type == 2)
            .ok_or("definition table absent")?;
        let definitions = pause_definition::read(
            missions
                .get(slct.offset as usize..slct.offset as usize + slct.size as usize)
                .ok_or("definition table exceeds its file")?,
        )?;
        let definition = pause_definition::find(&definitions, 1)
            .ok_or("game-over definition absent")?
            .clone();
        let sys = locate(library, "msg/us/sys.bar")?;
        let ids: Vec<u32> = definition.items.iter().map(|i| i.message as u32).collect();
        let texts = hud::load_messages(source, sys.index, sys.ordinal, sys.name_hash, &ids)?;
        let image = locate(library, "msg/us/fontimage.bar")?;
        let info = locate(library, "msg/us/fontinfo.bar")?;
        let font = hud::load_system_font(
            source,
            (image.index, image.ordinal, image.name_hash),
            (info.index, info.ordinal, info.name_hash),
        )?;
        Ok(Self {
            event,
            motion,
            length,
            pause,
            definition,
            texts,
            font,
        })
    }

    /// The `SeqGameSpeed` speed at event frame `frame`.
    fn speed_at(frame: f32) -> f32 {
        let mut speed = 1.;
        for (at, value) in [SLOW, NORMAL] {
            if frame >= at {
                speed = value;
            }
        }
        speed
    }

    /// The camera at event frame `frame` in Sora's local space.
    fn view(&self, frame: f32) -> Option<event_camera::View> {
        let cut = self.event.cut_at(frame)?;
        let data = self.event.camera(cut.camera)?;
        let seconds = (frame - cut.start as f32).min((cut.end - cut.start) as f32) / 30.;
        Some(event_camera::view(data, seconds))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The death event runs (white-out included).
    Event,
    /// The white-out holds while the set loads (states 3 to 5).
    Load,
    /// The list is open.
    Screen,
    /// Continue chosen: the black fade-out.
    Leave,
    /// The room is back; the fade in plays.
    Reload,
}

/// One game over from Sora's death to the reload.
pub struct Rt {
    pub phase: Phase,
    /// Event time in dt units (2 per event frame), advanced by the speed.
    clock: f32,
    accumulator: f32,
    /// Layer A: the white fade of the event.
    pub white: Fade,
    white_started: bool,
    wait: u32,
    pub menu: Option<List>,
    /// Ticks the list has been open (the model motion's clock).
    pub screen_ticks: u32,
    pub black: Fade,
    /// Where Sora stood when he fell (scene position and yaw).
    pub fell_at: ([f32; 3], f32),
    /// Whether the field steps this tick (the speed's accumulator).
    pub steps: bool,
}

impl Rt {
    pub fn new(fell_at: ([f32; 3], f32)) -> Self {
        Self {
            phase: Phase::Event,
            clock: 0.,
            accumulator: 0.,
            white: Fade::default(),
            white_started: false,
            wait: 0,
            menu: None,
            screen_ticks: 0,
            black: Fade::default(),
            fell_at,
            steps: true,
        }
    }

    /// The event frame (30 per second at speed 1).
    pub fn frame(&self) -> f32 {
        self.clock / UNITS_PER_FRAME
    }

    /// Whether the death event still plays.
    pub fn event_running(&self) -> bool {
        self.phase == Phase::Event && self.frame() < EVENT_FRAMES
    }

    /// One tick of the death event. `fade` is the event's first fade
    /// (frame, duration).
    pub fn event_tick(&mut self, fade: Option<(f32, u32)>) {
        let speed = Source::speed_at(self.frame());
        self.accumulator += speed;
        self.steps = self.accumulator >= 1.;
        if self.steps {
            self.accumulator -= 1.;
        }
        if self.event_running() {
            self.clock += speed;
        }
        if !self.white_started
            && let Some((frame, duration)) = fade
            && self.frame() >= frame
        {
            self.white.fade_out(duration, true);
            self.white_started = true;
        }
        // The fade object steps with the unscaled frame (DAT_140717484).
        self.white.update(1.);
        if !self.event_running() && self.white.state == FadeState::Held {
            self.phase = Phase::Load;
            self.wait = LOAD_TICKS;
            self.steps = false;
        }
    }

    /// The radial blur weight while the event runs.
    pub fn blur(&self) -> Option<f32> {
        self.event_running().then_some(BLUR_WEIGHT)
    }

    pub fn load_tick(&mut self) {
        self.wait = self.wait.saturating_sub(1);
        if self.wait == 0 {
            // Opening the list clears the white layer; the backdrop's own
            // white to black takes over.
            self.white = Fade::default();
            self.phase = Phase::Screen;
        }
    }

    pub fn start_leaving(&mut self) {
        self.black.fade_out(LEAVE_FADE, false);
        self.phase = Phase::Leave;
    }

    pub fn leave_tick(&mut self) -> bool {
        self.black.update(1.);
        self.black.state == FadeState::Held
    }

    pub fn start_reload(&mut self) {
        self.phase = Phase::Reload;
        self.black.fade_in(1, false);
    }

    pub fn reload_tick(&mut self) -> bool {
        self.black.update(1.);
        self.black.state == FadeState::Idle
    }
}

/// The resource the systems share.
#[derive(Resource)]
pub struct GameOverRes {
    src: Source,
    art: usize,
    font: usize,
    /// A white texel for the fades.
    fill: usize,
    pub rt: Option<Rt>,
    /// The row the list last had (the exe's `DAT_140744cf0`).
    last: Option<usize>,
    /// Sora's start; arena rooms put him back there on Continue.
    start: ([f32; 3], f32),
    /// An unattended run presses Continue by itself.
    tour: bool,
    /// Ticks since the fall (the unattended capture clock) and the capture
    /// due this tick.
    real: u32,
    snap: Option<usize>,
}

impl GameOverRes {
    pub fn new(src: Source, start: ([f32; 3], f32), tour: bool) -> Self {
        Self {
            src,
            art: 0,
            font: 0,
            fill: 0,
            rt: None,
            last: None,
            start,
            tour,
            real: 0,
            snap: None,
        }
    }

    /// The field's own tick runs unless the game over holds the screen or
    /// the speed skips it.
    pub fn field_runs(&self) -> bool {
        self.rt.as_ref().is_none_or(|r| r.steps)
    }

    /// The HUD and the camp stay away from the death event on.
    pub fn hides_hud(&self) -> bool {
        self.rt.is_some()
    }

    /// The list screen is up (it owns the pad).
    pub fn menu_up(&self) -> bool {
        self.rt.as_ref().is_some_and(|r| r.phase == Phase::Screen)
    }

    pub fn blur(&self) -> Option<f32> {
        self.rt.as_ref().and_then(Rt::blur)
    }

    /// The model's motion time in ticks while the list is up.
    pub fn model_time(&self) -> Option<f32> {
        let r = self.rt.as_ref().filter(|r| r.phase == Phase::Screen)?;
        let length = self.src.length.max(1.);
        // t += dt each frame, wrapping on the motion's length.
        let t = r.screen_ticks as f32 % length;
        Some(if t == 0. { length } else { t })
    }
}

/// Uploads the art and the font as 2D sheets.
pub fn setup(
    res: Option<ResMut<GameOverRes>>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut res) = res else { return };
    let res = &mut *res;
    if let Some((t, scale)) = res.src.pause.texture_for(0, 0) {
        let (rgba, size) = (t.rgba.clone(), t.size);
        res.art = frame.add_sheet(&mut images, rgba, size, scale);
    }
    res.fill = frame.add_sheet(&mut images, vec![255; 16], [2, 2], 1.);
    res.font = frame.add_sheet(
        &mut images,
        std::mem::take(&mut res.src.font.rgba),
        res.src.font.size,
        res.src.font.texel_scale as f32,
    );
}

/// One fixed tick: starts the game over when Sora falls, runs its phases
/// and ends it with the reload.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    res: Option<ResMut<GameOverRes>>,
    mut game: ResMut<Game>,
    area: Option<Res<AreaRes>>,
    mut rig: ResMut<CameraRig>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
) {
    let Some(mut res) = res else { return };
    // A duel has no game over: a knocked-out Sora stands up again.
    if game.versus {
        return;
    }
    let res = &mut *res;
    // The capture clock runs from the fall on, past the reload.
    if res.rt.is_some() || res.real > 0 {
        res.real += 1;
        if res.tour
            && let Some(i) = SNAPS.iter().position(|&t| t == res.real)
        {
            res.snap = Some(i);
        }
    }
    let res_real = res.real;
    if res.tour && game.tour_defeat_due() {
        let Game {
            sora, clips, enemy, ..
        } = &mut *game;
        if enemy.is_some() {
            sora.vitals.hp = 0;
            sora.player.die(clips);
        }
    }
    if res.rt.is_none() {
        // Sora's HP is 0, his DEAD motion plays and no Drive form catches
        // the fall (a form's KO is a different task, FUN_14040caa0).
        let hp = game.enemy.as_ref().map(|_| game.sora.vitals.hp);
        let in_form = game.drive.as_ref().is_some_and(|d| d.shown != 0);
        if hp == Some(0) && game.sora.player.dead() && !in_form {
            res.rt = Some(Rt::new((game.sora.player.position, game.sora.player.yaw)));
            let tick = game.tick;
            game.event(serde_json::json!({"event":"game_over_start","tick":tick}));
            game.sound_queue.push((GAME_OVER_SOUND, "game_over"));
        }
        return;
    }
    // The room's own KO refill and revive must not stand Sora up.
    if game.enemy.is_some() {
        game.sora.vitals.ko_ticks = 0;
    }
    let GameOverRes {
        src,
        rt,
        last,
        start,
        tour,
        ..
    } = res;
    let Some(r) = rt.as_mut() else { return };
    let phase_before = r.phase;
    let pad = if *tour {
        // Unattended: Continue once the list has been up for two seconds.
        (u64::from(r.screen_ticks == 150) * 0x4, 0)
    } else {
        (game.pad.pressed, game.pad.repeat)
    };
    match r.phase {
        Phase::Event => {
            let fade = src
                .event
                .fades
                .first()
                .map(|f| (f.frame as f32, f.duration as u32));
            r.event_tick(fade);
        }
        Phase::Load => r.load_tick(),
        Phase::Screen => {
            let seq = &src.pause.layout.sequences[0];
            let menu = r.menu.get_or_insert_with(|| {
                List::new(&src.definition, src.texts.clone(), 0, true, *last)
            });
            r.screen_ticks += 1;
            menu.update_sprites(seq);
            let out = menu.frame(seq, pad.0, pad.1);
            *last = Some(menu.cursor());
            for sound in out.sounds {
                game.sound_queue.push((sound, "game_over_menu"));
            }
            // Result 2 is Continue (FUN_1403fcbe0); result 1 is Load Game,
            // which needs the save screens (issue #32).
            if out.picked == Some(2) {
                r.start_leaving();
            }
        }
        Phase::Leave => {
            if r.leave_tick() {
                let (position, yaw) = area.as_ref().map_or(*start, |a| {
                    (a.current.player.0.to_array(), a.current.player.1)
                });
                continue_reload(&mut game, &mut rig, &mut gallery, (position, yaw));
                r.start_reload();
            }
        }
        Phase::Reload => {
            if r.reload_tick() {
                *rt = None;
                let tick = game.tick;
                game.event(serde_json::json!({"event":"game_over_end","tick":tick}));
            }
        }
    }
    if let Some(r) = rt.as_ref()
        && r.phase != phase_before
    {
        let (real, frame) = (res_real, r.frame());
        game.event(serde_json::json!({"event":"game_over_phase","phase":format!("{:?}",r.phase),"real":real,"event_frame":frame}));
    }
}

/// Continue: `FUN_1403a5650` restores every member's HP and the room loads
/// again at the last entrance.
fn continue_reload(
    game: &mut Game,
    rig: &mut CameraRig,
    gallery: &mut enemy_gallery::EnemyGallery,
    (position, yaw): ([f32; 3], f32),
) {
    if let Some(rt) = game.enemy.as_mut() {
        let v = &mut game.sora.vitals;
        v.hp = v.target.max_hp;
        v.ko_ticks = 0;
        rt.enemy.respawn();
    }
    gallery.respawn_all();
    // He stands at the entrance with the abilities, MP and Drive he had.
    game.sora.player.revive();
    game.sora.player.position = position;
    game.sora.player.yaw = yaw;
    rig.camera = FollowCamera::new(
        follow_camera::OUTDOOR,
        &camera_input(
            &game.sora.player,
            game.sora.player.position,
            game.params.run_speed,
        ),
    );
    rig.previous_position = game.sora.player.position;
    game.action_started = game.tick;
    let tick = game.tick;
    game.event(serde_json::json!({"event":"game_over_continue","tick":tick}));
}

/// The death event's camera over the field: its keys are in Sora's local
/// space (`FUN_140406a90` copies his matrix into the event), the scene's
/// x mirrored against the game's.
pub fn camera(
    res: Option<Res<GameOverRes>>,
    mut camera: Query<(&mut Transform, &mut Projection), CameraOnly>,
) {
    let Some(res) = res else { return };
    let Some(r) = res.rt.as_ref().filter(|r| r.event_running()) else {
        return;
    };
    let Some(view) = res.src.view(r.frame()) else {
        return;
    };
    let (at, yaw) = r.fell_at;
    let turn = Quat::from_rotation_y(yaw);
    let local = |p: [f32; 3]| Vec3::from_array(at) + turn * Vec3::new(-p[0], p[1], p[2]);
    if let Ok((mut transform, mut projection)) = camera.single_mut() {
        *transform =
            Transform::from_translation(local(view.eye)).looking_at(local(view.target), Vec3::Y);
        transform.rotate_local_z(view.roll.to_radians());
        if let Projection::Perspective(p) = projection.as_mut() {
            p.fov = view.fov.to_radians();
        }
    }
}

/// An additive white fade quad over the whole screen.
fn white_quad(alpha: f32) -> data::sprite_sequence::Quad {
    let (l, r) = (
        256. - visual::overlay::WIDE_WIDTH,
        256. + visual::overlay::WIDE_WIDTH,
    );
    let h = visual::overlay::HEIGHT;
    data::sprite_sequence::Quad {
        corners: [[l, -h], [r, -h], [l, 2. * h], [r, 2. * h]],
        uv: [0, 0, 1, 1],
        colors: [[255. / 128., 255. / 128., 255. / 128., alpha]; 4],
        blend: 1,
    }
}

/// Draws the game over's layers over the HUD-less scene: the white layer,
/// then the list's backdrop, Sora's model, the list, and Continue's fade.
pub fn present(
    res: Option<Res<GameOverRes>>,
    model: Option<Res<MenuModel>>,
    mut frame: ResMut<Ui2dFrame>,
) {
    let Some(res) = res else { return };
    let Some(r) = res.rt.as_ref() else { return };
    let white = res.fill;
    if r.white.state != FadeState::Idle {
        frame
            .draws
            .push((white, vec![white_quad(r.white.alpha as f32 / 128.)]));
    }
    if matches!(r.phase, Phase::Screen | Phase::Leave | Phase::Reload)
        && let Some(menu) = r.menu.as_ref()
    {
        let seq = &res.src.pause.layout.sequences[0];
        let font = field_command_menu::Font {
            widths: &res.src.font.widths,
            pad: 1,
        };
        let draws = menu.draw(seq, &font);
        let slot = |sheet: SpriteSheet| match sheet {
            SpriteSheet::Art(_) => Some(res.art),
            SpriteSheet::Font => Some(res.font),
            SpriteSheet::Icon => None,
        };
        for (sheet, quads) in draws.below {
            if let Some(s) = slot(sheet) {
                frame.draws.push((s, quads));
            }
        }
        if let Some(m) = model.as_ref() {
            let mut quads = list::model_quad(MODEL_OFFSET, pause_menu::MODEL_TINT, 1.);
            for q in &mut quads {
                q.uv = [0, 0, MENU_TARGET[0] as i32, MENU_TARGET[1] as i32];
            }
            frame.draws.push((m.sheet, quads));
        }
        for (sheet, quads) in draws.above {
            if let Some(s) = slot(sheet) {
                frame.draws.push((s, quads));
            }
        }
    }
    if r.black.state != FadeState::Idle {
        frame.fill(white, [0., 0., 0., r.black.alpha as f32 / 128.]);
    }
}

/// Sora's model at the menu slot, posed on the game-over motion at `time`
/// ticks (called from the menu model's presenter).
pub fn pose(res: &GameOverRes, rig: &SoraRig, time: f32) -> Option<Vec<[f32; 3]>> {
    let (pose, _) = rig
        .skinning
        .sample_evaluated(&res.src.motion.motion, time)
        .ok()?;
    Some(
        pose.position_bits
            .iter()
            .map(|p| p.map(f32::from_bits))
            .collect(),
    )
}

/// The placeholder sprite sits at its base (0, 0), so the model lands at the
/// projection centre.
const MODEL_OFFSET: [f32; 2] = [0., 0.];

/// The unattended tour's captures by ticks since the fall.
pub fn snap(mut commands: Commands, res: Option<ResMut<GameOverRes>>, game: Res<Game>) {
    let Some(mut res) = res else { return };
    let (Some(i), Some(dir)) = (res.snap.take(), game.autoplay.clone()) else {
        return;
    };
    let path = dir.join(format!("game-over-{i:02}.png"));
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>| {
            let image = captured
                .image
                .clone()
                .try_into_dynamic()
                .expect("game over capture");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .expect("new private capture");
            image
                .write_to(&mut file, image::ImageFormat::Png)
                .expect("capture encode");
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_event(r: &mut Rt) -> (u32, u32, Vec<u32>) {
        let (mut steps, mut ticks) = (0, 0);
        let mut phases = Vec::new();
        while r.phase == Phase::Event && ticks < 2000 {
            r.event_tick(Some((11., 50)));
            ticks += 1;
            steps += u32::from(r.steps);
            if phases.last().is_none() && r.event_running() && r.blur().is_none() {
                phases.push(ticks);
            }
        }
        (ticks, steps, phases)
    }

    #[test]
    fn the_speed_steps_the_field_on_an_accumulator() {
        assert_eq!(Source::speed_at(0.), 1.);
        assert_eq!(Source::speed_at(3.9), 1.);
        assert_eq!(Source::speed_at(4.), 0.1);
        assert_eq!(Source::speed_at(19.9), 0.1);
        assert_eq!(Source::speed_at(20.), 1.);
        let mut r = Rt::new(([0.; 3], 0.));
        // Event frames are two dt units: frame 4 is reached after 8 ticks.
        let mut stepped = 0;
        for _ in 0..8 {
            r.event_tick(Some((11., 50)));
            stepped += u32::from(r.steps);
        }
        assert_eq!((stepped, r.frame()), (8, 4.));
        // Then one field tick in ten.
        stepped = 0;
        for _ in 0..100 {
            r.event_tick(Some((11., 50)));
            stepped += u32::from(r.steps);
        }
        assert_eq!(stepped, 10);
    }

    #[test]
    fn the_event_ends_then_the_white_out_holds_into_the_load() {
        let mut r = Rt::new(([0.; 3], 0.));
        let (ticks, steps, _) = run_event(&mut r);
        assert_eq!(r.phase, Phase::Load);
        // 8 ticks at speed 1, 320 at 0.1 to event frame 20, then to 21.
        assert!((329..=333).contains(&ticks), "{ticks}");
        assert!(steps < ticks / 2);
        assert_eq!(r.white.state, FadeState::Held);
        for _ in 0..LOAD_TICKS {
            r.load_tick();
        }
        assert_eq!(r.phase, Phase::Screen);
        // The list's backdrop takes over from the white layer.
        assert_eq!(r.white.state, FadeState::Idle);
        assert!(r.blur().is_none());
    }

    #[test]
    fn the_blur_runs_with_the_event_only() {
        let mut r = Rt::new(([0.; 3], 0.));
        assert_eq!(r.blur(), Some(BLUR_WEIGHT));
        run_event(&mut r);
        assert_eq!(r.blur(), None);
    }

    #[test]
    fn continue_fades_to_black_over_thirty_ticks_then_back_in() {
        let mut r = Rt::new(([0.; 3], 0.));
        r.start_leaving();
        let mut ticks = 1;
        while !r.leave_tick() {
            ticks += 1;
        }
        assert_eq!(ticks, 2 * LEAVE_FADE);
        r.start_reload();
        assert_eq!(r.phase, Phase::Reload);
        let mut guard = 0;
        while !r.reload_tick() && guard < 50 {
            guard += 1;
        }
        assert!(guard < 50);
    }
}
