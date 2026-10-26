//! The KH2FM title screen from the selected Steam installation:
//! `menu/us/title.2ld` played by the translated layout player and title
//! task (`visual::title`), drawn on the GPU in the original's 2D
//! space, with the system sounds from se000 and the exe's pad/keyboard
//! mapping (pad mode 7).
//!
//! Every launch checks the selected source (executable and indexes rehashed,
//! each package byte read verified against the full-scan record). Not built:
//! the file menu, the Theater and the Game Settings overlay. Esc quits (on PC it opens Game Settings).
use crate::save_files;
use bevy::{
    app::AppExit,
    input::mouse::MouseWheel,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use data::probe_source::ProbeSource;
use data::sprite_sequence::Quad;
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use visual::controller::{self, Pad};
use visual::fade::{self, Fade};
use visual::field_command_menu::Font;
use visual::hud::{self, SystemFont};
use visual::input;
use visual::menu_screen::{self, Menu2ld};
use visual::new_game::{self, NewGame, Texts};
use visual::overlay::{Ui2dFrame, Ui2dPlugin};
use visual::sound::{self, Banks, Music};
use visual::sprite_player::Sheet;
use visual::title::{Action, Event, State, Title};
use visual::title_banner;

#[derive(Deserialize, Clone)]
struct Locator {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
}

#[derive(Deserialize)]
struct Config {
    schema_version: u32,
    source_content_identity: String,
    /// menu/us/title.2ld.
    title: Locator,
    /// se/se000.win32.scd (system sounds).
    #[serde(default)]
    system_sounds: Option<Locator>,
    /// Play the first-boot splash (scene 8: Disney, Square Enix, copyright)
    /// before the title. The exe plays it on every process start
    /// (FUN_14014ed40 → FUN_140150870, boot-01), so it defaults on.
    #[serde(default = "yes")]
    first_boot: bool,
    /// msg/us/sys.bar, msg/us/fontimage.bar and msg/us/fontinfo.bar for the
    /// New Game dialog.
    messages: Locator,
    font_image: Locator,
    font_info: Locator,
    /// vagstream/Title.win32.scd.
    #[serde(default)]
    music: Option<Locator>,
}

fn yes() -> bool {
    true
}

/// Mixer key of the title stream (FUN_140537420's single AxaSoundStream).
const BGM_KEY: u32 = 9_999_000;

/// The dialog's art: title.2ld image 0.
const DIALOG_IMAGE: usize = 0;
/// The fade quad at 16:9 (FUN_140156940: margin 85).
const FADE_RECT: [f32; 4] = [-86., -1., 598., 417.];

/// Title image 8's device-glyph alternates (FUN_1402b40d0): replacement N
/// for keyboard, Xbox, PlayStation and other pads.
const GLYPH_IMAGE: usize = 8;
const GLYPHS: [usize; 4] = [8, 9, 10, 11];

/// Unattended plan: presses by tick, captures by tick. Rows down and up,
/// New Game, Proud, vibration ON, back out of the confirm page, confirm.
const AUTOPLAY_PRESSES: &[(u64, u64)] = &[
    (700, pad_bits::DOWN),
    (740, pad_bits::UP),
    (780, pad_bits::DECIDE),
    (840, pad_bits::DOWN),
    (880, pad_bits::DECIDE),
    (940, pad_bits::DECIDE),
    (1000, pad_bits::CANCEL),
    (1060, pad_bits::DECIDE),
    (1120, pad_bits::DECIDE),
];
const AUTOPLAY_CAPTURES: &[u64] = &[
    60, 240, 450, 690, 750, 790, 800, 830, 845, 900, 970, 1010, 1080, 1125, 1135, 1200, 1300,
];
const AUTOPLAY_END: u64 = 1450;

/// Unattended plan for the Load menu (`--autoplay-files`): down to Load,
/// open it, move, page, ask to delete, back out of the question, load a slot.
const FILES_PRESSES: &[(u64, u64)] = &[
    (700, pad_bits::DOWN),
    (760, pad_bits::DECIDE),
    (900, 0x1000),
    (1000, pad_bits::CANCEL),
    (1060, pad_bits::DECIDE),
];
const FILES_CAPTURES: &[u64] = &[700, 880, 940, 1040, 1075, 1200];
const FILES_END: u64 = 1250;

/// Unattended plan for the save point menu (`--autoplay-savepoint`): the menu
/// opens at tick 720 as at a world point; down to World Map and its question,
/// back, Save, the list, the overwrite question, back out to the rows.
const SAVEPOINT_PRESSES: &[(u64, u64)] = &[
    (850, pad_bits::DOWN),
    (950, pad_bits::DECIDE),
    (1050, pad_bits::CANCEL),
    (1100, pad_bits::UP),
    (1150, pad_bits::DECIDE),
    (1300, pad_bits::DECIDE),
    (1400, pad_bits::CANCEL),
    (1450, pad_bits::CANCEL),
];
const SAVEPOINT_CAPTURES: &[u64] = &[800, 900, 1010, 1250, 1350, 1500];
const SAVEPOINT_END: u64 = 1550;
const AUTOPLAY_WARMUP_FRAMES: u64 = 30;

mod pad_bits {
    pub use visual::title::bits::*;
}

#[derive(Resource)]
struct Session {
    menu: Menu2ld,
    first_boot: bool,
    title: Option<Title>,
    dialog: Option<NewGame>,
    music_rx: Option<sound::MusicSlot>,
    music: Option<Music>,
    /// The title asked for its stream (FUN_140537420) and it has started.
    bgm_wanted: bool,
    bgm_started: bool,
    texts: Texts,
    font: SystemFont,
    font_sheet: usize,
    /// The font image's icon plane (0x09 icons).
    icons: SystemFont,
    icon_sheet: usize,
    /// This frame's 2D draw (taken before the title task's logic, as the
    /// sprite draw task runs before it).
    snapshot: Vec<(usize, Vec<Quad>)>,
    fade: Fade,
    fade_drawn: bool,
    pad: Pad,
    wheel: f32,
    keyboard_last: bool,
    glyph: usize,
    /// Sheet slot per layout image (image 8 follows the device).
    image_sheets: Vec<usize>,
    glyph_sheets: [Option<usize>; 4],
    white: usize,
    banks: Banks,
    output: Option<audio::Output>,
    handoff: Option<Action>,
    tick: u64,
    rendered_frames: u64,
    log: Option<File>,
    limit: Option<u64>,
    autoplay: Option<PathBuf>,
    captures_taken: usize,
    capture_pending: bool,
    exit_code: Option<u8>,
    /// The Load menu's art and state; None when `save.2ld` is missing.
    files: Option<save_files::Files>,
    files_plan: bool,
    savepoint_plan: bool,
    loaded_slot: Option<usize>,
}

impl Session {
    fn presses(&self) -> &'static [(u64, u64)] {
        if self.savepoint_plan {
            SAVEPOINT_PRESSES
        } else if self.files_plan {
            FILES_PRESSES
        } else {
            AUTOPLAY_PRESSES
        }
    }

    fn captures(&self) -> &'static [u64] {
        if self.savepoint_plan {
            SAVEPOINT_CAPTURES
        } else if self.files_plan {
            FILES_CAPTURES
        } else {
            AUTOPLAY_CAPTURES
        }
    }

    fn end(&self) -> u64 {
        if self.savepoint_plan {
            SAVEPOINT_END
        } else if self.files_plan {
            FILES_END
        } else {
            AUTOPLAY_END
        }
    }

    fn event(&mut self, value: serde_json::Value) {
        if let Some(log) = &mut self.log {
            serde_json::to_writer(&mut *log, &value).expect("title log write");
            writeln!(log).expect("title log newline");
            log.flush().expect("title log flush");
        }
    }

    fn play(&mut self, se: u32, cause: &str) {
        let resolved = self.banks.request(se).unwrap_or_else(|e| {
            eprintln!("Sound {se} failed to decode: {e}");
            None
        });
        let tick = self.tick;
        self.event(
            serde_json::json!({"event":"sound","tick":tick,"id":se,"cause":cause,
            "layers":resolved.as_ref().map(|r| r.layers.len())}),
        );
        if let (Some(output), Some(r)) = (self.output.as_ref(), resolved)
            && let Ok(mut mixer) = output.mixer.lock()
        {
            for layer in r.layers {
                mixer.play(layer, se, r.volume, 1.);
            }
        }
    }
}

fn read_limited(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err("selection exceeds 1 MiB".into());
    }
    Ok(bytes)
}

fn setup(
    mut session: ResMut<Session>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let s = &mut *session;
    let count = s.menu.image_sizes.len();
    for k in 0..count {
        let t = s.menu.replacements[k].take().expect("checked at load");
        s.image_sheets
            .push(frame.add_sheet(&mut images, t.rgba, t.size, t.texel_scale));
    }
    for (slot, n) in GLYPHS.into_iter().enumerate() {
        if n == GLYPH_IMAGE {
            s.glyph_sheets[slot] = Some(s.image_sheets[GLYPH_IMAGE]);
        } else if let Some((t, scale)) = s.menu.texture_for(GLYPH_IMAGE, n) {
            let (rgba, size) = (t.rgba.clone(), t.size);
            s.glyph_sheets[slot] = Some(frame.add_sheet(&mut images, rgba, size, scale));
        }
    }
    if let Some(f) = s.files.as_mut() {
        for k in 0..f.menu.image_sizes.len() {
            if let Some(t) = f.menu.replacements[k].take() {
                f.sheets
                    .push(frame.add_sheet(&mut images, t.rgba, t.size, t.texel_scale));
            }
        }
    }
    s.white = frame.add_sheet(&mut images, vec![255; 4], [1, 1], 1.);
    s.font_sheet = frame.add_sheet(
        &mut images,
        std::mem::take(&mut s.font.rgba),
        s.font.size,
        s.font.texel_scale as f32,
    );
    s.icon_sheet = frame.add_sheet(
        &mut images,
        std::mem::take(&mut s.icons.rgba),
        s.icons.size,
        s.icons.texel_scale as f32,
    );
    // FUN_1402b4c60: fade.out(0), near-instant black; the task then fades
    // in over 0x3c.
    s.fade.fade_out(0, false);
    while s.fade.state != fade::State::Held {
        s.fade.update(1.);
    }
    s.fade.fade_in(0x3c, false);
}

fn collect_wheel(mut wheel: MessageReader<MouseWheel>, mut session: ResMut<Session>) {
    for w in wheel.read() {
        session.wheel += w.y;
    }
}

/// One 60 Hz title frame: input, the title task, layouts and fade.
fn tick(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    mut session: ResMut<Session>,
) {
    let s = &mut *session;
    s.tick += 1;
    let tick = s.tick;
    let (data, keyboard, pad_glyph) = input::read_devices(&keys, &mouse, s.wheel, &pads);
    let pad_glyph = pad_glyph.map(usize::from);
    s.wheel = 0.;
    let mut logical = controller::logical(controller::raw(&data), controller::MODE_TITLE);
    if s.autoplay.is_some() {
        logical = s
            .presses()
            .iter()
            .filter(|(at, _)| *at == tick)
            .fold(0, |acc, (_, b)| acc | b);
    }
    s.pad.update(logical, 1.);
    let file_open = s.files.as_ref().is_some_and(|f| f.ui.is_some());
    let trig = if file_open { 0 } else { s.pad.pressed };
    if s.pad.pressed != 0 {
        s.event(serde_json::json!({"event":"input","tick":tick,"trig":trig}));
    }
    // FUN_1402b40d0: keyboard activity selects the keyboard sheet.
    if keyboard {
        s.keyboard_last = true;
    } else if pad_glyph.is_some() {
        s.keyboard_last = false;
    }
    let glyph = if s.keyboard_last {
        0
    } else {
        pad_glyph.unwrap_or(s.glyph)
    };
    if glyph != s.glyph && s.glyph_sheets[glyph].is_some() {
        s.glyph = glyph;
        s.event(
            serde_json::json!({"event":"device_glyph","tick":tick,"replacement":GLYPHS[glyph]}),
        );
    }
    s.image_sheets[GLYPH_IMAGE] = s.glyph_sheets[s.glyph].expect("glyph sheet");

    // Sprite and layout tasks (priority ~0x7532) run first, then the 2D
    // draw (103000), then the title task (300000), then the fade (330000).
    if let Some(t) = s.title.as_mut() {
        t.update_layouts(&s.menu.layout, 1.);
    }
    let dialog_seq = &s.menu.layout.sequences[0];
    if let Some(d) = s.dialog.as_mut() {
        d.update_sprites(dialog_seq);
    }
    s.snapshot.clear();
    if let Some(t) = &s.title {
        for task in std::iter::once(&t.main).chain(t.cursor_layout.as_ref()) {
            for b in menu_screen::draw(&s.menu, task, [0, 0]) {
                s.snapshot.push((s.image_sheets[b.image], b.quads));
            }
        }
    }
    if let Some(f) = &s.files
        && let Some(ui) = &f.ui
    {
        let font = Font {
            widths: &s.font.widths,
            pad: 1,
        };
        let d = ui.draw(&f.menu, &font);
        for (image, quads) in d.art {
            s.snapshot.push((f.sheets[image], quads));
        }
        for (i, (sheet, quads)) in d.sheets.into_iter().enumerate() {
            if let Some((at, [x0, y0, x1, y1])) = d.panel
                && at == i
            {
                s.snapshot.push((
                    s.white,
                    vec![Quad {
                        corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
                        uv: [0, 0, 1, 1],
                        colors: [[0., 0., 0., 0.8]; 4],
                        blend: 0,
                    }],
                ));
            }
            let slot = match sheet {
                Sheet::Art(image) => f.sheets[image],
                Sheet::Font => s.font_sheet,
                Sheet::Icon => s.icon_sheet,
            };
            s.snapshot.push((slot, quads));
        }
    }
    if let Some(d) = &s.dialog {
        let font = Font {
            widths: &s.font.widths,
            pad: 1,
        };
        for (sheet, quads) in d.sprites.draw(dialog_seq, &font) {
            let slot = match sheet {
                Sheet::Art(image) => s.image_sheets[image],
                Sheet::Font => s.font_sheet,
                Sheet::Icon => s.icon_sheet,
            };
            s.snapshot.push((slot, quads));
        }
    }

    // Music: collect the background decode, then start the stream the
    // frame after the title asked for it (only when nothing plays).
    if let Some(slot) = &s.music_rx
        && let Some(result) = slot.lock().ok().and_then(|mut r| r.take())
    {
        s.music_rx = None;
        match result {
            Ok(m) => {
                let stats = m.stats.clone();
                s.music = Some(m);
                s.event(serde_json::json!({"event":"music_decoded","tick":tick,"stats":stats}));
            }
            Err(e) => s.event(serde_json::json!({"event":"music_failed","tick":tick,"error":e})),
        }
    }
    let mut started = false;
    if s.bgm_wanted
        && !s.bgm_started
        && let (Some(m), Some(o)) = (&s.music, &s.output)
        && let Ok(mut mixer) = o.mixer.lock()
    {
        mixer.play(m.clip.clone(), BGM_KEY, m.volume, 1.);
        started = true;
    }
    if started {
        s.bgm_started = true;
        s.event(serde_json::json!({"event":"bgm_start","tick":tick}));
    }
    // FUN_1402b4590: the layouts start once the opening fade-in is done.
    // The decode runs during the opening black; the title waits for it so
    // the stream starts with the layouts [rebuild: the exe streams].
    let music_pending = s.music_rx.is_some();
    if s.title.is_none()
        && s.handoff.is_none()
        && s.fade.state == fade::State::Idle
        && !music_pending
    {
        let t = Title::new(&s.menu.layout, s.first_boot, false);
        let scene = t.main.scene;
        s.title = Some(t);
        s.event(serde_json::json!({"event":"title_start","tick":tick,"scene":scene}));
        // The stream starts one yield after the main layout (FUN_1402b4590).
        s.bgm_wanted = true;
    }
    if let Some(mut t) = s.title.take() {
        let before = (
            t.main.scene,
            t.cursor_layout.as_ref().map(|c| c.scene),
            t.state,
        );
        let mut events = Vec::new();
        if t.state == State::NewGameDialog {
            // FUN_1402b7420 runs inside the title task: the menu frame
            // pauses until the dialog returns.
            match s.dialog.as_mut() {
                None => {
                    // The modal starts the frame after the decide.
                    let mode = 1;
                    let mut d = NewGame::new(s.texts.clone(), DIALOG_IMAGE, mode, true);
                    let seq = &s.menu.layout.sequences[0];
                    for n in d.frame(seq, s.pad.pressed, s.pad.repeat) {
                        events.push(Event::Sound(n));
                    }
                    s.dialog = Some(d);
                }
                Some(d) => {
                    let seq = &s.menu.layout.sequences[0];
                    for n in d.frame(seq, s.pad.pressed, s.pad.repeat) {
                        events.push(Event::Sound(n));
                    }
                    if d.finished {
                        let (confirmed, values) = (d.confirmed, d.values);
                        s.dialog = None;
                        s.event(serde_json::json!({"event":"new_game_dialog","tick":tick,
                            "confirmed":confirmed,"mode":values[0],"vibration_on":values[1] == 0}));
                        events.extend(t.new_game_result(confirmed));
                    }
                }
            }
        } else {
            events = t.frame(&s.menu.layout, trig, 1.);
        }
        if file_open {
            // The Load menu holds the title: its idle timer does not run.
            t.timer = 0.;
        }
        let mut loaded = None;
        let widths = s.font.widths.clone();
        if s.savepoint_plan
            && tick == 720
            && let Some(f) = s.files.as_mut()
            && f.ui.is_none()
        {
            let menu = save::menu::Menu::new(
                save::menu::Kind::SavePoint { world_map: true },
                &f.backend,
                f.last_slot,
            );
            f.ui = Some(visual::file_menu::FileMenu::new(
                menu,
                f.texts.clone(),
                &f.menu.layout,
                widths.clone(),
            ));
        }
        let mut file_sounds = Vec::new();
        if let Some(f) = s.files.as_mut()
            && let Some(ui) = f.ui.as_mut()
        {
            let pressed = save_files::inputs(s.pad.pressed, s.pad.repeat);
            let layout = &f.menu.layout;
            for e in ui.frame(layout, &mut f.backend, &pressed) {
                match e {
                    save::menu::Event::Sound(n) => file_sounds.push(n as u32),
                    save::menu::Event::Loaded { slot, data } => {
                        f.last_slot = Some(slot);
                        loaded = Some((slot, data));
                    }
                    save::menu::Event::Closed => ui.close(layout),
                    _ => {}
                }
            }
            if ui.is_finished() {
                f.ui = None;
            }
        }
        for n in file_sounds {
            s.play(n, "file menu");
        }
        if let Some((slot, _data)) = loaded {
            s.event(serde_json::json!({"event":"file_loaded","tick":tick,"slot":slot}));
            s.loaded_slot = Some(slot);
            t.action = Some(Action::Load);
            t.leave(&mut events);
        }
        for e in events {
            match e {
                Event::OpenFileMenu => {
                    if let Some(f) = s.files.as_mut()
                        && f.ui.is_none()
                    {
                        let menu =
                            save::menu::Menu::new(save::menu::Kind::Title, &f.backend, f.last_slot);
                        f.ui = Some(visual::file_menu::FileMenu::new(
                            menu,
                            f.texts.clone(),
                            &f.menu.layout,
                            widths.clone(),
                        ));
                        s.play(11, "file menu open");
                    }
                }
                Event::Sound(n) => s.play(n, "title"),
                Event::FadeOut(n) => s.fade.fade_out(n, false),
                Event::FadeBgm(units) => {
                    // FUN_1405373e0(720) [inferred: milliseconds].
                    if let Some(o) = &s.output
                        && let Ok(mut m) = o.mixer.lock()
                    {
                        m.fade(BGM_KEY, 0., units as f32 / 1000., audio::FadeEnd::Stop);
                    }
                    s.bgm_wanted = false;
                    s.event(serde_json::json!({"event":"bgm_fade","tick":tick,"units":units}));
                }
                Event::Leave(a) => {
                    s.handoff = Some(a);
                    s.event(serde_json::json!({"event":"leave","tick":tick,"action":format!("{a:?}")}));
                }
                other => s.event(serde_json::json!({"event":"title_event","tick":tick,"what":format!("{other:?}")})),
            }
        }
        let after = (
            t.main.scene,
            t.cursor_layout.as_ref().map(|c| c.scene),
            t.state,
        );
        if after != before {
            s.event(
                serde_json::json!({"event":"scene","tick":tick,"main":after.0,"cursor":after.1,
                "state":format!("{:?}",after.2),"cursor_row":t.cursor}),
            );
        }
        // Back (state 3) waits for black, then returns to the launcher.
        let black = s.fade.state == fade::State::Held;
        let back_done = t.state == State::Back && black;
        let left = match s.handoff {
            Some(Action::Demo) => black,
            Some(_) => t.main.dead,
            None => false,
        };
        if back_done || left {
            let a = s.handoff.unwrap_or(Action::Back);
            s.event(
                serde_json::json!({"event":"handoff","tick":tick,"action":format!("{a:?}"),
                "note":"next screen not built"}),
            );
            s.handoff = Some(a);
        } else {
            s.title = Some(t);
        }
    }
    s.fade_drawn = s.fade.update(1.);
}

/// This frame's layouts and dialog, then the fade on top.
fn draw(session: Res<Session>, mut frame: ResMut<Ui2dFrame>) {
    frame.draws.extend(session.snapshot.iter().cloned());
    if session.fade_drawn {
        let ([r, g, b, a], additive) = session.fade.colour();
        let [x0, y0, x1, y1] = FADE_RECT;
        frame.draws.push((
            session.white,
            vec![Quad {
                corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
                uv: [0, 0, 1, 1],
                colors: [[r, g, b, a]; 4],
                blend: additive as i32,
            }],
        ));
    }
}
fn finish(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut time: ResMut<Time<Virtual>>,
    mut closed: MessageReader<bevy::window::WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) {
    session.rendered_frames += 1;
    if session.autoplay.is_some()
        && time.is_paused()
        && !session.capture_pending
        && session.rendered_frames >= AUTOPLAY_WARMUP_FRAMES
    {
        time.unpause();
    }
    if let Some(dir) = session.autoplay.clone()
        && !session.capture_pending
        && let Some(&at) = session.captures().get(session.captures_taken)
        && session.tick >= at
    {
        session.capture_pending = true;
        time.pause();
        let index = session.captures_taken;
        let path = dir.join(format!("title-{index:02}.png"));
        let tick = session.tick;
        commands.spawn(Screenshot::primary_window()).observe(
            move |event: On<ScreenshotCaptured>,
                  mut session: ResMut<Session>,
                  mut time: ResMut<Time<Virtual>>| {
                let image = event
                    .image
                    .clone()
                    .try_into_dynamic()
                    .expect("title capture");
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .expect("new private capture");
                image
                    .write_to(&mut file, image::ImageFormat::Png)
                    .expect("capture encode");
                file.sync_all().expect("capture flush");
                session.capture_pending = false;
                session.captures_taken += 1;
                time.unpause();
                session.event(serde_json::json!({"event":"capture","index":index,"tick":tick}));
            },
        );
    }
    let autoplay_done = session.autoplay.is_some()
        && session.tick >= session.end()
        && session.captures_taken == session.captures().len();
    let window_closed = closed.read().count() > 0;
    let handed_off =
        session.handoff.is_some() && session.title.is_none() && session.autoplay.is_none();
    if keys.just_pressed(KeyCode::Escape)
        || window_closed
        || autoplay_done
        || handed_off
        || session.limit.is_some_and(|n| session.tick >= n)
    {
        let code = u8::from(session.autoplay.is_some() && !autoplay_done);
        if session.exit_code.is_none() {
            session.exit_code = Some(code);
            let tick = session.tick;
            session.event(serde_json::json!({"event":"exit","tick":tick,"code":code}));
            // For the launcher: New Game continues in the test room.
            if handed_off && let Some(a) = session.handoff {
                println!("kh2fm-handoff {a:?}");
            }
            exit.write(if code == 0 {
                AppExit::Success
            } else {
                AppExit::Error(std::num::NonZeroU8::new(code).unwrap())
            });
        }
    }
}

fn pause_for_autoplay(session: Res<Session>, mut time: ResMut<Time<Virtual>>) {
    if session.autoplay.is_some() {
        time.pause();
    }
}

fn launch_file_args() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    #[derive(Deserialize)]
    struct Launch {
        steam_root: PathBuf,
        manifest: PathBuf,
        config: PathBuf,
        #[serde(default)]
        options: Vec<String>,
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for up in exe.ancestors().skip(1).take(4) {
            candidates.push(up.join(".local/test-room/launch-title.json"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("KH2FinalRewrite/test-room-launch-title.json"));
    }
    let path = candidates
        .iter()
        .find(|p| p.is_file())
        .ok_or("no arguments and no launch-title.json launch file")?;
    let launch: Launch = serde_json::from_slice(&read_limited(path)?)?;
    let mut args = vec![launch.steam_root, launch.manifest, launch.config];
    args.extend(launch.options.into_iter().map(PathBuf::from));
    Ok(args)
}

fn run(mut args: Vec<PathBuf>) -> Result<AppExit, Box<dyn std::error::Error>> {
    if args.is_empty() {
        args = launch_file_args()?;
    }
    if args.len() < 3 {
        return Err("usage: title <Steam-root> <manifest> <title-config> [--log new.jsonl] [--frames N] [--autoplay <new-capture-dir>]".into());
    }
    // The GPU device starts while the data decodes (used by the app below).
    let early_renderer = crate::early_renderer::EarlyRenderer::start();
    let (mut log_path, mut limit, mut autoplay) = (None, None, None);
    let mut files_plan = false;
    let mut savepoint_plan = false;
    let mut options = args[3..].iter();
    while let Some(option) = options.next() {
        match option.to_str() {
            Some("--log") if log_path.is_none() => log_path = options.next().cloned(),
            Some("--frames") if limit.is_none() => {
                limit = Some(
                    options
                        .next()
                        .and_then(|v| v.to_str())
                        .ok_or("frame limit absent")?
                        .parse::<u64>()?,
                )
            }
            Some("--autoplay") if autoplay.is_none() => autoplay = options.next().cloned(),
            Some("--autoplay-savepoint") if autoplay.is_none() => {
                files_plan = true;
                savepoint_plan = true;
                autoplay = options.next().cloned();
            }
            Some("--autoplay-files") if autoplay.is_none() => {
                files_plan = true;
                autoplay = options.next().cloned();
            }
            _ => return Err("unknown/duplicate title option".into()),
        }
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    let log_path = match log_path {
        Some(path) => {
            let path = source.output_path(&path)?;
            if path.exists() {
                return Err("title log must be new".into());
            }
            Some(path)
        }
        None => None,
    };
    if let Some(dir) = &autoplay {
        source.output_path(&dir.join("title-00.png"))?;
        if !dir.is_dir() || std::fs::read_dir(dir)?.next().is_some() {
            return Err("autoplay capture directory must exist and be empty".into());
        }
    }
    eprintln!("Validating the selected KH2FM source, then decoding the title...");
    let config: Config = serde_json::from_slice(&read_limited(&args[2])?)?;
    if config.schema_version != 1 || config.source_content_identity != source.content_identity() {
        return Err("title config does not match the selected source".into());
    }
    source.verify_session()?;
    let l = &config.title;
    let mut menu = menu_screen::load(&source, &l.index, l.ordinal, l.name_hash)?;
    // The owner's edits [rebuild addition]: a shorter copyright hold and the
    // "Final Re:WRITE" banner.
    let copyright_cut = title_banner::shorten_copyright(&mut menu.layout);
    let banner = match menu.replacements.get_mut(title_banner::LOGO_IMAGE) {
        Some(Some(t)) => title_banner::final_rewrite_banner(t)?,
        _ => false,
    };
    if !copyright_cut || !banner {
        eprintln!("Title edits not applied: copyright cut {copyright_cut}, banner {banner}.");
    }
    let mut banks = Banks::default();
    if let Some(l) = &config.system_sounds {
        banks.insert(sound::load_bank(
            &source,
            &l.index,
            l.ordinal,
            l.name_hash,
            None,
        )?)?;
    }
    let m = &config.messages;
    let texts = Texts::from_messages(hud::load_messages(
        &source,
        &m.index,
        m.ordinal,
        m.name_hash,
        &new_game::message_ids(),
    )?);
    let backend = if autoplay.is_some() {
        match std::env::var_os("KH2RW_TEST_SAVE") {
            Some(path) => save_files::FileBackend::from_test_file(Path::new(&path))?,
            None => save_files::FileBackend::in_memory(save_files::synthetic_container()),
        }
    } else {
        save_files::FileBackend::open(&crate::launcher::data_dir(), Path::new(&args[0]))?
    };
    let files = match save_files::Files::load(
        &source,
        (&m.index, m.ordinal, m.name_hash),
        backend,
        usize::from(!savepoint_plan),
    ) {
        Ok(f) => Some(f),
        Err(e) => {
            eprintln!("Load menu unavailable: {e}");
            None
        }
    };
    let (fi, fo) = (&config.font_image, &config.font_info);
    let font = hud::load_font(
        &source,
        (&fi.index, fi.ordinal, fi.name_hash),
        (&fo.index, fo.ordinal, fo.name_hash),
        b"sys\0",
    )?;
    let icons = hud::load_font(
        &source,
        (&fi.index, fi.ordinal, fi.name_hash),
        (&fo.index, fo.ordinal, fo.name_hash),
        b"icon",
    )?;
    let music_rx = match &config.music {
        Some(l) => {
            let bytes = sound::load_bank(&source, &l.index, l.ordinal, l.name_hash, None)?;
            Some(sound::decode_music_in_background(bytes))
        }
        None => None,
    };
    let mut log = match log_path {
        Some(path) => Some(OpenOptions::new().write(true).create_new(true).open(path)?),
        None => None,
    };
    if let Some(log) = &mut log {
        let summary = serde_json::json!({"event":"loaded","props":menu.layout.props.len(),
            "scenes":menu.layout.scenes.len(),"sequences":menu.layout.sequences.len(),
            "images":menu.image_sizes,"replacements":menu.replacements.iter()
                .map(|r| r.as_ref().map(|t| t.size)).collect::<Vec<_>>(),
            "banks":banks.numbers(),"copyright_cut":copyright_cut,"banner":banner});
        serde_json::to_writer(&mut *log, &summary)?;
        writeln!(log)?;
    }
    let output = match audio::Output::open() {
        Ok(o) => Some(o),
        Err(e) => {
            eprintln!("Sound output unavailable: {e}");
            None
        }
    };
    // Unattended plans press at fixed ticks from the scene-17 start, so they
    // skip the boot splash.
    let first_boot = config.first_boot && autoplay.is_none();
    let session = Session {
        menu,
        first_boot,
        title: None,
        dialog: None,
        music_rx,
        music: None,
        bgm_wanted: false,
        bgm_started: false,
        texts,
        font,
        font_sheet: 0,
        icons,
        icon_sheet: 0,
        snapshot: Vec::new(),
        fade: Fade::default(),
        fade_drawn: false,
        pad: Pad::default(),
        wheel: 0.,
        keyboard_last: true,
        glyph: 0,
        image_sheets: Vec::new(),
        glyph_sheets: [None; 4],
        white: 0,
        banks,
        output,
        handoff: None,
        tick: 0,
        rendered_frames: 0,
        log,
        limit,
        autoplay,
        captures_taken: 0,
        capture_pending: false,
        exit_code: None,
        files,
        files_plan,
        savepoint_plan,
        loaded_slot: None,
    };
    eprintln!(
        "Title ready. Arrows/W/S or the D-pad move, Enter/left-click/A decides, Space/Backspace/B backs out, M/Start skips the intro, Esc quits."
    );
    Ok(App::new()
        .insert_resource(session)
        .insert_resource(Time::<Fixed>::from_hz(60.))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Kingdom Hearts II Re:Written (title; quarantined source)".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    close_when_requested: false,
                    ..default()
                })
                .set(early_renderer.plugin()),
        )
        .add_plugins(Ui2dPlugin {
            order: 0,
            overlay: false,
        })
        .add_systems(Startup, (setup, pause_for_autoplay))
        .add_systems(PreUpdate, collect_wheel.after(bevy::input::InputSystems))
        .add_systems(FixedUpdate, tick)
        .add_systems(Update, (draw, finish).chain())
        .run())
}

/// Runs this program with its arguments; `interactive` = started from the
/// launcher menu (keep the console open on an error).
pub fn main(args: Vec<PathBuf>, interactive: bool) -> std::process::ExitCode {
    match run(args) {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("Title rejected: {e}");
            if interactive {
                eprintln!("Press Enter to close.");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            std::process::ExitCode::FAILURE
        }
    }
}
