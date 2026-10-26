//! The launcher window `kh2fm` opens without arguments: pick the game
//! folder (found in Steam's libraries, browsed, or dropped on the window),
//! check it against the supported build, then choose a test. The choice is
//! remembered, so later launches open on the test list. The first launch
//! opens on a short quick start guide (F1 brings it back).
//!
//! Before a folder is chosen there are no game files, so the launcher is
//! drawn from plain shapes and a Windows font: a dark screen and one list
//! [launcher design, not the original's menus]. A test runs as a child
//! `kh2fm <program> ...` process; this window waits hidden and comes back
//! when the test closes.
mod install;
mod paint;
mod test_configs;

use bevy::app::AppExit;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{FileDragAndDrop, PrimaryWindow, WindowResolution};
use bevy::winit::{UpdateMode, WinitSettings};
use install::{Bound, CheckError, Settings, TESTS};
use paint::{Face, Paint, Rgba, rgba, with_alpha};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use visual::overlay::{Ui2dFrame, Ui2dPlugin};

pub use install::{
    data_dir, field_args, field_start, landing_room_config, play_args, save_start, saved_start,
};

// Palette.
const TEXT: u32 = 0xf2f5fb;
const MUTED: u32 = 0x9aa6bd;
const FAINT: u32 = 0x5d6880;
const ACCENT: u32 = 0x2f63d6;
const ACCENT_DEEP: u32 = 0x1c3f99;
const GOLD: u32 = 0xe9c45f;

const MARGIN: f32 = 96.;
const LIST_TOP: f32 = 196.;
const LIST_BOTTOM: f32 = 630.;
const LIST_WIDTH: f32 = 620.;
const GUIDE_LIST_TOP: f32 = 572.;
/// The Multiplayer screen in the pause menu's (the camp's) layout: its
/// entry boxes at the top left under the header, the help box and the
/// panel at the bottom [launcher drawing after the camp's look; the game's
/// own menu art is not loaded here].
const MP_ROW_X: f32 = 200.;
const MP_ROW_TOP: f32 = 132.;
const MP_ROW_W: f32 = 340.;
const MP_ROW_BOTTOM: f32 = 540.;
const MP_BAND: f32 = 112.;
const MP_PANEL_TOP: f32 = 560.;
const MP_HELP_X: f32 = 400.;
const MP_BLUE: u32 = 0x1530ff;
const MP_HELP: u32 = 0x48b4ff;
const MP_ORANGE: u32 = 0xff7a1a;
const MP_YELLOW: u32 = 0xffcc33;

#[derive(Clone, Debug, PartialEq)]
enum Action {
    UseFolder(PathBuf),
    /// Browse into a folder (None: the drive list).
    Browse(Option<PathBuf>),
    Pick,
    /// Browse for the Skate 3 disc (None: the drive list).
    BrowseIso(Option<PathBuf>),
    /// Remember this Skate 3 ISO or `default.xex`.
    UseIso(PathBuf),
    Run(usize),
    /// A developer viewer (index into `Launcher::viewers`).
    View(usize),
    /// Open the quick start guide.
    Guide,
    /// Leave the guide.
    CloseGuide,
    /// The LAN duel screen.
    Multiplayer,
    /// Host a duel and wait for a player.
    HostDuel,
    /// Join a host the LAN search found (index into the found list).
    JoinFound(usize),
    /// Type the host's address, then join it.
    JoinAddress,
    /// Type the name the other player sees.
    EditName,
    /// The next devices choice for duels (two windows on one PC).
    CycleInput,
    Quit,
}

/// A text field being typed into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Name,
    Address,
}

/// Longest name and address typed.
const NAME_LIMIT: usize = 24;
const ADDRESS_LIMIT: usize = 40;

#[derive(Clone)]
struct Item {
    label: String,
    detail: Option<String>,
    /// A gold tag on the right ("Steam", "KH2FM here").
    tag: Option<&'static str>,
    action: Option<Action>,
}

impl Item {
    fn new(label: impl Into<String>, action: Action) -> Self {
        Self {
            label: label.into(),
            detail: None,
            tag: None,
            action: Some(action),
        }
    }
    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
    fn tag(mut self, t: &'static str) -> Self {
        self.tag = Some(t);
        self
    }
    fn disabled(mut self) -> Self {
        self.action = None;
        self
    }
}

/// A folder check or scan running on its own thread.
struct Job {
    folder: PathBuf,
    progress: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    scanning: Arc<AtomicBool>,
    done: Arc<Mutex<Option<Result<Bound, String>>>>,
}

/// A test running as a child process.
struct Running {
    child: Child,
    name: String,
    output: Arc<Mutex<VecDeque<String>>>,
    window_open: Arc<AtomicBool>,
    since_window: Option<f32>,
    hidden: bool,
}

enum Screen {
    Boot,
    /// The quick start guide; the tests screen to return to, if any.
    Guide(Option<Bound>),
    Pick,
    Browse(Option<PathBuf>),
    /// Browsing for the Skate 3 disc; the tests screen to return to.
    BrowseIso(Option<PathBuf>, Bound),
    Check(Job),
    Tests(Bound),
    /// The LAN duel: host, join, the player's name.
    Multiplayer(Bound),
    Loading(Running, Bound),
    Problem {
        title: String,
        lines: Vec<String>,
        retry: Option<Bound>,
    },
}

#[derive(Resource)]
struct Launcher {
    dir: PathBuf,
    settings: Settings,
    supported: Option<String>,
    detected: Vec<PathBuf>,
    viewers: Vec<install::Viewer>,
    screen: Screen,
    items: Vec<Item>,
    cursor: usize,
    scroll: usize,
    /// Per-row highlight, eased towards the cursor.
    glow: Vec<f32>,
    entered: f32,
    now: f32,
    fade: f32,
    hover_drop: bool,
    repeat: Option<(i32, f32)>,
    mouse: Option<Vec2>,
    paint: Option<Paint>,
    /// The field being typed into and its text so far.
    editing: Option<(Field, String)>,
    /// The Multiplayer screen's LAN search and the hosts it found.
    browser: Option<net::Browser>,
    found: Vec<net::Found>,
    /// This executable's identity (hosts with another are not joinable).
    build: Option<[u8; 32]>,
}

impl Launcher {
    fn go(&mut self, screen: Screen) {
        // The LAN search runs only on the Multiplayer screen.
        if matches!(screen, Screen::Multiplayer(_)) {
            if self.browser.is_none() {
                let build = *self
                    .build
                    .get_or_insert_with(|| crate::test_room::build_identity().unwrap_or([0; 32]));
                self.browser = net::Browser::new(net::DEFAULT_PORT, build)
                    .inspect_err(|e| eprintln!("LAN search unavailable: {e}"))
                    .ok();
            }
        } else {
            self.browser = None;
            self.found.clear();
            self.editing = None;
        }
        self.screen = screen;
        self.items = self.build_items();
        self.cursor = self
            .items
            .iter()
            .position(|i| i.action.is_some())
            .unwrap_or(0);
        self.scroll = 0;
        self.glow = vec![0.; self.items.len()];
        self.entered = self.now;
        self.repeat = None;
    }

    fn build_items(&self) -> Vec<Item> {
        match &self.screen {
            Screen::Pick => {
                let mut items: Vec<Item> = self
                    .detected
                    .iter()
                    .map(|p| {
                        let current = self.settings.game_folder.as_ref() == Some(p);
                        Item::new(
                            if current {
                                "Steam install (current)"
                            } else {
                                "Steam install"
                            },
                            Action::UseFolder(p.clone()),
                        )
                        .detail(plain(p))
                        .tag("STEAM")
                    })
                    .collect();
                items.push(Item::new("Browse for the folder", Action::Browse(None)));
                if let Some(remembered) = &self.settings.game_folder
                    && !self.detected.contains(remembered)
                {
                    items.push(
                        Item::new("Current folder", Action::UseFolder(remembered.clone()))
                            .detail(plain(remembered)),
                    );
                }
                items.push(Item::new("Quick start guide", Action::Guide));
                items.push(Item::new("Quit", Action::Quit));
                items
            }
            Screen::Browse(path) => browse_items(path.as_deref()),
            Screen::BrowseIso(path, b) => browse_iso_items(path.as_deref(), b),
            Screen::Tests(_) => {
                let mut items: Vec<Item> = TESTS
                    .iter()
                    .enumerate()
                    .map(|(i, t)| Item::new(t.name, Action::Run(i)).detail(t.blurb))
                    .collect();
                items.push(
                    Item::new("Multiplayer", Action::Multiplayer)
                        .detail("A duel against another player on the same network"),
                );
                items.extend(self.viewers.iter().enumerate().map(|(i, v)| {
                    Item::new(v.label.clone(), Action::View(i))
                        .detail("Developer tool, slow to start")
                }));
                let iso = self.settings.skate3_iso.as_deref();
                items.push(
                    Item::new("Skate 3 ISO", Action::BrowseIso(None))
                        .detail(iso.map_or("Not set: needed for the J skateboard".into(), plain)),
                );
                items.push(Item::new("Change game folder", Action::Pick));
                items.push(Item::new("Quick start guide", Action::Guide));
                items.push(Item::new("Quit", Action::Quit));
                items
            }
            Screen::Multiplayer(b) => {
                let typing = |field: Field| match &self.editing {
                    Some((f, text)) if *f == field => Some(format!("{text}_")),
                    _ => None,
                };
                let name = typing(Field::Name).unwrap_or_else(|| self.player_name());
                let address = typing(Field::Address).unwrap_or_else(|| {
                    self.settings
                        .last_host
                        .clone()
                        .unwrap_or_else(|| "Type the host's IP address".into())
                });
                let mut items = vec![
                    Item::new("Host a duel", Action::HostDuel)
                        .detail("Wait for a player on this network"),
                ];
                for (i, host) in self.found.iter().enumerate() {
                    let item =
                        Item::new(format!("Join {}", host.name), Action::JoinFound(i)).tag("LAN");
                    items.push(if !host.compatible {
                        item.detail(format!(
                            "{}: another version of the launcher",
                            host.addr.ip()
                        ))
                        .disabled()
                    } else if !host.open {
                        item.detail(format!("{}: already in a duel", host.addr.ip()))
                            .disabled()
                    } else {
                        item.detail(match host.ping_ms {
                            Some(ms) => format!("{}, {ms} ms", host.addr.ip()),
                            None => host.addr.ip().to_string(),
                        })
                    });
                }
                if self.found.is_empty() && self.browser.is_some() {
                    items.push(
                        Item::new("Looking for hosts on this network...", Action::Multiplayer)
                            .disabled(),
                    );
                }
                items.extend([
                    Item::new("Join by address", Action::JoinAddress).detail(address),
                    Item::new("Name", Action::EditName).detail(name),
                    Item::new("Controls", Action::CycleInput)
                        .detail(input_label(self.duel_input())),
                    Item::new("Back", Action::UseFolder(b.folder.clone())),
                ]);
                items
            }
            Screen::Guide(_) => vec![Item::new("Continue", Action::CloseGuide)],
            Screen::Problem { retry, .. } => {
                let mut items = Vec::new();
                if let Some(b) = retry {
                    items.push(Item::new(
                        "Back to the tests",
                        Action::UseFolder(b.folder.clone()),
                    ));
                }
                items.push(Item::new("Choose another folder", Action::Pick));
                items.push(Item::new("Quit", Action::Quit));
                items
            }
            Screen::Boot | Screen::Check(_) | Screen::Loading(..) => Vec::new(),
        }
    }
}

/// A path as people type it (no `\\?\` prefix).
fn plain(p: &Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(s)
}

/// Drives, folders, `.iso` files and `default.xex` for the Skate 3 disc.
fn browse_iso_items(path: Option<&Path>, tests: &Bound) -> Vec<Item> {
    let Some(path) = path else {
        let mut items: Vec<Item> = ('A'..='Z')
            .map(|d| PathBuf::from(format!("{d}:\\")))
            .filter(|p| p.is_dir())
            .map(|p| Item::new(plain(&p), Action::BrowseIso(Some(p))))
            .collect();
        items.push(Item::new("Back", Action::UseFolder(tests.folder.clone())));
        return items;
    };
    let mut items = vec![Item::new(
        "Up one level",
        Action::BrowseIso(path.parent().map(Path::to_path_buf)),
    )];
    let mut entries: Vec<(bool, PathBuf)> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let dir = e.file_type().ok()?.is_dir();
            let p = e.path();
            let name = p.file_name()?.to_string_lossy().to_lowercase();
            (dir || name.ends_with(".iso") || name == "default.xex").then_some((dir, p))
        })
        .collect();
    entries.sort_by_key(|(dir, p)| {
        (
            !*dir,
            p.file_name().map(|n| n.to_string_lossy().to_lowercase()),
        )
    });
    for (dir, p) in entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        items.push(if dir {
            Item::new(name, Action::BrowseIso(Some(p)))
        } else {
            Item::new(name, Action::UseIso(p)).tag("DISC")
        });
    }
    items
}

fn browse_items(path: Option<&Path>) -> Vec<Item> {
    let Some(path) = path else {
        let mut items: Vec<Item> = ('A'..='Z')
            .map(|d| PathBuf::from(format!("{d}:\\")))
            .filter(|p| p.is_dir())
            .map(|p| Item::new(plain(&p), Action::Browse(Some(p))))
            .collect();
        items.push(Item::new("Back", Action::Pick));
        return items;
    };
    let mut items = Vec::new();
    let here = Item::new("Use this folder", Action::UseFolder(path.to_path_buf()));
    items.push(if install::has_executable(path) {
        here.detail("The game is here").tag("KH2FM")
    } else {
        here.detail("The game is not in this folder").disabled()
    });
    items.push(Item::new(
        "Up one level",
        Action::Browse(path.parent().map(Path::to_path_buf)),
    ));
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    for d in dirs {
        let name = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item = Item::new(name, Action::Browse(Some(d.clone())));
        items.push(if install::has_executable(&d) {
            item.tag("KH2FM")
        } else {
            item
        });
    }
    items
}

/// Words wrapped to `width`.
fn wrap(paint: &mut Paint, face: Face, size: f32, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let next = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && paint.measure(face, size, &next) > width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    lines
}

fn start_check(folder: PathBuf, dir: PathBuf, hint: Option<PathBuf>) -> Job {
    let done = Arc::new(Mutex::new(None));
    let progress = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let scanning = Arc::new(AtomicBool::new(false));
    let (p, t, s, f, tx) = (
        progress.clone(),
        total.clone(),
        scanning.clone(),
        folder.clone(),
        done.clone(),
    );
    std::thread::spawn(move || {
        let result = match install::quick_check(&dir, &f, hint.as_deref()) {
            Ok(b) => Ok(b),
            Err(CheckError::Rejected(why)) => Err(why),
            Err(CheckError::NeedsScan) => {
                t.store(validation::required_bytes(&f), Ordering::Relaxed);
                s.store(true, Ordering::Relaxed);
                install::full_scan(&dir, &f, &p)
            }
        };
        if let Ok(mut slot) = tx.lock() {
            *slot = Some(result);
        }
    });
    Job {
        folder,
        progress,
        total,
        scanning,
        done,
    }
}

fn start_test(index: usize, dir: &Path, b: &Bound) -> Result<Running, String> {
    let test = &TESTS[index];
    let args = install::test_args(dir, test.program, b)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    spawn(test.name.into(), exe, args)
}

/// A viewer runs inside the launcher executable like a test: its program
/// name, then folder, manifest, selection and a new log.
fn start_viewer(v: &install::Viewer, b: &Bound) -> Result<Running, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let log = install::viewer_log(v.program)?;
    let mut args = vec![v.program.to_owned()];
    args.extend(
        [&b.folder, &b.manifest, &v.selection, &log].map(|p| p.to_string_lossy().into_owned()),
    );
    spawn(v.label.clone(), exe, args)
}

fn spawn(name: String, exe: PathBuf, args: Vec<String>) -> Result<Running, String> {
    let mut command = Command::new(exe);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // A child run gets no console window.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("{name} could not start: {e}"))?;
    let output = Arc::new(Mutex::new(VecDeque::new()));
    let window_open = Arc::new(AtomicBool::new(false));
    let pipes: [Box<dyn std::io::Read + Send>; 2] = [
        Box::new(child.stdout.take().ok_or("no test output")?),
        Box::new(child.stderr.take().ok_or("no test output")?),
    ];
    for pipe in pipes {
        let (output, window_open) = (output.clone(), window_open.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                if line.contains("Creating new window") {
                    window_open.store(true, Ordering::Relaxed);
                }
                if let Ok(mut o) = output.lock() {
                    if o.len() >= 80 {
                        o.pop_front();
                    }
                    o.push_back(line);
                }
            }
        });
    }
    Ok(Running {
        child,
        name,
        output,
        window_open,
        since_window: None,
        hidden: false,
    })
}

/// The lines worth showing from a failed test's output: its own messages,
/// not the engine's log.
fn failure_lines(output: &VecDeque<String>) -> Vec<String> {
    let own: Vec<String> = output
        .iter()
        .filter(|l| !l.contains(" INFO ") && !l.contains(" WARN ") && !l.trim().is_empty())
        .cloned()
        .collect();
    let tail = own.len().saturating_sub(6);
    own[tail..].to_vec()
}

#[derive(Default)]
struct Pressed {
    up: bool,
    down: bool,
    page: i32,
    accept: bool,
    back: bool,
    change: bool,
    guide: bool,
}

fn read_input(
    l: &mut Launcher,
    keys: &ButtonInput<KeyCode>,
    pads: &Query<&Gamepad>,
    wheel: f32,
) -> Pressed {
    let mut p = Pressed {
        accept: keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]),
        back: keys.any_just_pressed([KeyCode::Escape, KeyCode::Backspace]),
        change: keys.just_pressed(KeyCode::F2),
        guide: keys.just_pressed(KeyCode::F1),
        page: keys.just_pressed(KeyCode::PageDown) as i32
            - keys.just_pressed(KeyCode::PageUp) as i32,
        ..default()
    };
    let mut held = keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) as i32
        - keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) as i32;
    for g in pads.iter() {
        p.accept |= g.just_pressed(GamepadButton::South);
        p.back |= g.just_pressed(GamepadButton::East);
        p.change |= g.just_pressed(GamepadButton::North);
        p.guide |= g.just_pressed(GamepadButton::Select);
        let stick = g.left_stick().y;
        let dir = g.pressed(GamepadButton::DPadDown) as i32
            - g.pressed(GamepadButton::DPadUp) as i32
            + (stick < -0.5) as i32
            - (stick > 0.5) as i32;
        if held == 0 {
            held = dir.signum();
        }
        p.page += g.just_pressed(GamepadButton::RightTrigger) as i32
            - g.just_pressed(GamepadButton::LeftTrigger) as i32;
    }
    // Held directions repeat after a pause.
    let step = match (held, l.repeat) {
        (0, _) => {
            l.repeat = None;
            0
        }
        (d, Some((was, at))) if was == d => {
            if l.now >= at {
                l.repeat = Some((d, l.now + 0.075));
                d
            } else {
                0
            }
        }
        (d, _) => {
            l.repeat = Some((d, l.now + 0.38));
            d
        }
    };
    // A tap shorter than a frame still moves once.
    let tap = keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) as i32
        - keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) as i32;
    let step = if step == 0 { tap } else { step };
    let wheel_step = if wheel > 0. {
        -1
    } else if wheel < 0. {
        1
    } else {
        0
    };
    p.up = step < 0 || wheel_step < 0;
    p.down = step > 0 || wheel_step > 0;
    p
}

/// Rows of the current list: height, gap, label size.
fn row_metrics(l: &Launcher) -> (f32, f32, f32) {
    match l.screen {
        Screen::Multiplayer(_) => (44., 4., 24.),
        Screen::Browse(_) | Screen::BrowseIso(..) => (40., 4., 20.),
        _ if l.items.iter().any(|i| i.detail.is_some()) => (66., 6., 24.),
        _ => (48., 6., 24.),
    }
}

fn visible_rows(l: &Launcher) -> usize {
    let (h, gap, _) = row_metrics(l);
    let bottom = if matches!(l.screen, Screen::Multiplayer(_)) {
        MP_ROW_BOTTOM
    } else {
        LIST_BOTTOM
    };
    (((bottom - list_top(l) + gap) / (h + gap)) as usize).max(1)
}

fn row_rect(l: &Launcher, i: usize) -> [f32; 4] {
    let (h, gap, _) = row_metrics(l);
    let k = i as f32 - l.scroll as f32;
    let (x, w) = if matches!(l.screen, Screen::Multiplayer(_)) {
        (MP_ROW_X, MP_ROW_W)
    } else {
        (MARGIN, LIST_WIDTH)
    };
    [x, list_top(l) + k * (h + gap), w, h]
}

fn move_cursor(l: &mut Launcher, by: i32) {
    let n = l.items.len() as i32;
    if n == 0 {
        return;
    }
    let mut c = l.cursor as i32;
    for _ in 0..n {
        c = (c + by.signum()).rem_euclid(n);
        if l.items[c as usize].action.is_some() {
            break;
        }
    }
    l.cursor = c as usize;
    keep_visible(l);
}

fn keep_visible(l: &mut Launcher) {
    let rows = visible_rows(l);
    if l.cursor < l.scroll {
        l.scroll = l.cursor;
    } else if l.cursor >= l.scroll + rows {
        l.scroll = l.cursor + 1 - rows;
    }
}

fn act(l: &mut Launcher, action: Action, exit: &mut MessageWriter<AppExit>) {
    match action {
        Action::Quit => {
            exit.write(AppExit::Success);
        }
        Action::Pick => l.go(Screen::Pick),
        Action::Guide => {
            let tests = match &l.screen {
                Screen::Tests(b) => Some(b.clone()),
                _ => None,
            };
            l.go(Screen::Guide(tests));
        }
        Action::CloseGuide => {
            let Screen::Guide(tests) = &l.screen else {
                return;
            };
            // The fewest steps on: back to the tests, the remembered
            // folder, the one Steam install found, or the folder list.
            let next = match (tests, &l.settings.game_folder, l.detected.as_slice()) {
                (Some(b), ..) => return l.go(Screen::Tests(b.clone())),
                (None, Some(folder), _) => Action::UseFolder(folder.clone()),
                (None, None, [only]) => Action::UseFolder(only.clone()),
                _ => Action::Pick,
            };
            act(l, next, exit);
        }
        Action::Browse(path) => {
            let from = match &l.screen {
                Screen::Browse(Some(p)) => Some(p.clone()),
                _ => None,
            };
            l.go(Screen::Browse(path));
            // Going up lands on the folder just left.
            if let Some(from) = from
                && let Some(i) = l
                    .items
                    .iter()
                    .position(|it| it.action == Some(Action::Browse(Some(from.clone()))))
            {
                l.cursor = i;
                keep_visible(l);
            }
        }
        Action::BrowseIso(path) => {
            let tests = match &l.screen {
                Screen::Tests(b) | Screen::BrowseIso(_, b) => b.clone(),
                _ => return,
            };
            l.go(Screen::BrowseIso(path, tests));
        }
        Action::UseIso(path) => {
            let Screen::BrowseIso(_, b) = &l.screen else {
                return;
            };
            let b = b.clone();
            match skate_source::admit(Some(&path)) {
                Ok(_) => {
                    l.settings.skate3_iso = Some(path);
                    if let Err(e) = install::save_settings(&l.dir, &l.settings) {
                        eprintln!("Launcher settings not saved: {e}");
                    }
                    l.go(Screen::Tests(b));
                }
                Err(e) => l.go(Screen::Problem {
                    title: "That is not a Skate 3 disc".into(),
                    lines: vec![e.to_string()],
                    retry: Some(b),
                }),
            }
        }
        Action::UseFolder(folder) => {
            // A library or `common` folder picked by mistake: use the game
            // folder inside it.
            let folder = if install::has_executable(&folder) {
                folder
            } else {
                install::find_game_below(&folder).unwrap_or(folder)
            };
            let hint = (l.settings.game_folder.as_deref() == Some(folder.as_path()))
                .then(|| l.settings.manifest.clone())
                .flatten();
            let job = start_check(folder, l.dir.clone(), hint);
            l.go(Screen::Check(job));
        }
        Action::Run(_) | Action::View(_) => {
            if let Screen::Tests(b) = &l.screen {
                let b = b.clone();
                launch(l, &action, b);
            }
        }
        Action::Multiplayer => {
            if let Screen::Tests(b) = &l.screen {
                let b = b.clone();
                l.go(Screen::Multiplayer(b));
            }
        }
        Action::HostDuel => {
            let name = l.player_name();
            duel(l, vec!["--host".into(), "--name".into(), name]);
        }
        Action::JoinFound(i) => {
            if let Some(host) = l.found.get(i) {
                let address = host.addr.to_string();
                let name = l.player_name();
                duel(l, vec!["--join".into(), address, "--name".into(), name]);
            }
        }
        Action::JoinAddress => {
            let text = l.settings.last_host.clone().unwrap_or_default();
            l.edit(Field::Address, text);
        }
        Action::EditName => {
            let text = l.player_name();
            l.edit(Field::Name, text);
        }
        Action::CycleInput => {
            let at = INPUT_CHOICES
                .iter()
                .position(|&c| c == l.duel_input())
                .unwrap_or(0);
            l.settings.duel_input = Some(INPUT_CHOICES[(at + 1) % INPUT_CHOICES.len()].into());
            if let Err(e) = install::save_settings(&l.dir, &l.settings) {
                eprintln!("Launcher settings not saved: {e}");
            }
            l.refresh();
        }
    }
}

/// The duel's device choices (the room's `--input`), in the order Controls
/// cycles them.
const INPUT_CHOICES: [&str; 4] = ["auto", "kbm", "pad", "both"];

fn input_label(choice: &str) -> &'static str {
    match choice {
        "kbm" => "Keyboard and mouse",
        "pad" => "Controller",
        "both" => "Keyboard, mouse and controller",
        _ => "Auto: on one PC, the host keyboard and the joiner a controller",
    }
}

impl Launcher {
    /// The name the other player sees: the saved one, else the Windows
    /// user name.
    /// The duel's devices as saved (auto by default).
    fn duel_input(&self) -> &str {
        self.settings
            .duel_input
            .as_deref()
            .filter(|c| INPUT_CHOICES.contains(c))
            .unwrap_or("auto")
    }

    fn player_name(&self) -> String {
        self.settings
            .player_name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .or_else(|| std::env::var("USERNAME").ok())
            .unwrap_or_else(|| "Player".into())
    }

    fn edit(&mut self, field: Field, text: String) {
        self.editing = Some((field, text));
        self.refresh();
    }

    /// Rebuilds the rows in place (the cursor stays).
    fn refresh(&mut self) {
        self.items = self.build_items();
        self.glow.resize(self.items.len(), 0.);
        self.cursor = self.cursor.min(self.items.len().saturating_sub(1));
    }
}

/// Typing into a field: characters, Backspace, Enter to keep, Esc to drop.
/// Returns true while the field takes the keys.
fn type_text(
    l: &mut Launcher,
    keys: &ButtonInput<KeyCode>,
    typed: &mut MessageReader<KeyboardInput>,
) -> bool {
    let Some((field, mut text)) = l.editing.take() else {
        typed.clear();
        return false;
    };
    let limit = match field {
        Field::Name => NAME_LIMIT,
        Field::Address => ADDRESS_LIMIT,
    };
    for event in typed.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Character(chars) => {
                for c in chars.chars().filter(|c| !c.is_control()) {
                    let allowed = match field {
                        Field::Name => c != '"',
                        Field::Address => c.is_ascii_alphanumeric() || ".:-".contains(c),
                    };
                    if allowed && text.chars().count() < limit {
                        text.push(c);
                    }
                }
            }
            Key::Space if field == Field::Name && text.chars().count() < limit => {
                text.push(' ');
            }
            Key::Backspace => {
                text.pop();
            }
            _ => {}
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        l.refresh();
        return true;
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        let text = text.trim().to_owned();
        match field {
            Field::Name => {
                if !text.is_empty() {
                    l.settings.player_name = Some(text);
                }
                if let Err(e) = install::save_settings(&l.dir, &l.settings) {
                    eprintln!("Launcher settings not saved: {e}");
                }
                l.refresh();
            }
            Field::Address if !text.is_empty() => {
                l.settings.last_host = Some(text.clone());
                if let Err(e) = install::save_settings(&l.dir, &l.settings) {
                    eprintln!("Launcher settings not saved: {e}");
                }
                let name = l.player_name();
                duel(l, vec!["--join".into(), text, "--name".into(), name]);
            }
            Field::Address => l.refresh(),
        }
        return true;
    }
    l.editing = Some((field, text));
    l.refresh();
    true
}

/// Starts the test room as a duel host or joiner with `session` options.
fn duel(l: &mut Launcher, session: Vec<String>) {
    let Screen::Multiplayer(b) = &l.screen else {
        return;
    };
    let b = b.clone();
    let mut session = session;
    session.extend(["--input".into(), l.duel_input().to_owned()]);
    let started = install::duel_args(&l.dir, &b, &session).and_then(|args| {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        spawn("Multiplayer".into(), exe, args)
    });
    match started {
        Ok(running) => l.go(Screen::Loading(running, b)),
        Err(why) => l.go(Screen::Problem {
            title: "The duel could not start".into(),
            lines: vec![why],
            retry: Some(b),
        }),
    }
}

/// Starts a test or viewer on `b` and waits on the loading screen.
fn launch(l: &mut Launcher, which: &Action, b: Bound) {
    let (name, started) = match which {
        Action::Run(i) => (TESTS[*i].name.to_owned(), start_test(*i, &l.dir, &b)),
        Action::View(i) => (
            l.viewers[*i].label.clone(),
            start_viewer(&l.viewers[*i], &b),
        ),
        _ => return,
    };
    match started {
        Ok(running) => l.go(Screen::Loading(running, b)),
        Err(why) => l.go(Screen::Problem {
            title: format!("{name} could not start"),
            lines: vec![why],
            retry: Some(b),
        }),
    }
}

fn back(l: &mut Launcher, exit: &mut MessageWriter<AppExit>) {
    let to = match &l.screen {
        Screen::Browse(Some(p)) => Some(Action::Browse(p.parent().map(Path::to_path_buf))),
        Screen::Browse(None) => Some(Action::Pick),
        Screen::BrowseIso(Some(p), _) => Some(Action::BrowseIso(p.parent().map(Path::to_path_buf))),
        Screen::BrowseIso(None, b) => Some(Action::UseFolder(b.folder.clone())),
        Screen::Pick => l.settings.game_folder.clone().map(Action::UseFolder),
        Screen::Problem { retry: Some(b), .. } => Some(Action::UseFolder(b.folder.clone())),
        Screen::Tests(_) => Some(Action::Quit),
        Screen::Multiplayer(b) => Some(Action::UseFolder(b.folder.clone())),
        Screen::Guide(_) => Some(Action::CloseGuide),
        _ => None,
    };
    if let Some(a) = to {
        act(l, a, exit);
    }
}

/// A finished check: keep the folder if it is the supported build.
fn checked(l: &mut Launcher, result: Result<Bound, String>, folder: &Path) {
    let b = match result {
        Ok(b) => b,
        Err(why) => {
            return l.go(Screen::Problem {
                title: "This folder can't be used".into(),
                lines: vec![plain(folder), why],
                retry: None,
            });
        }
    };
    if let Some(want) = &l.supported
        && *want != b.identity
    {
        return l.go(Screen::Problem {
            title: "Unsupported game version".into(),
            lines: vec![
                plain(&b.folder),
                format!(
                    "The tests only run on the supported Steam version ({}…). This folder has {}….",
                    &want[..12.min(want.len())],
                    &b.identity[..12.min(b.identity.len())]
                ),
            ],
            retry: None,
        });
    }
    l.settings.game_folder = Some(b.folder.clone());
    l.settings.manifest = Some(b.manifest.clone());
    if let Err(e) = install::save_settings(&l.dir, &l.settings) {
        eprintln!("Launcher settings not saved: {e}");
    }
    // The viewers' selections are made from the install with the tests'
    // configs, so a new data folder lists them too.
    if let Err(e) = install::make_configs(&l.dir, &b) {
        eprintln!("Viewer selections not made: {e}");
    }
    l.viewers = install::viewers(&l.dir);
    l.go(Screen::Tests(b));
}

/// Developer check: with `KH2RW_LAUNCHER_SHOT` naming a PNG, F12 saves a
/// picture of the launcher there (a window capture of the GPU surface comes
/// out black).
fn screenshot(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::F12)
        && let Some(path) = std::env::var_os("KH2RW_LAUNCHER_SHOT")
    {
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(PathBuf::from(
                path,
            )));
    }
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut l: ResMut<Launcher>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut wheel: MessageReader<MouseWheel>,
    mut drops: MessageReader<FileDragAndDrop>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut winit: ResMut<WinitSettings>,
    mut typed: MessageReader<KeyboardInput>,
    mut exit: MessageWriter<AppExit>,
) {
    let l = &mut *l;
    let dt = time.delta_secs().min(0.1);
    l.now += dt;
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if matches!(l.screen, Screen::Boot) {
        l.supported = install::supported_identity(&l.dir);
        l.detected = install::detect_installs();
        l.viewers = install::viewers(&l.dir);
        if !l.settings.guide_seen {
            l.settings.guide_seen = true;
            if let Err(e) = install::save_settings(&l.dir, &l.settings) {
                eprintln!("Launcher settings not saved: {e}");
            }
            l.go(Screen::Guide(None));
        } else {
            match l.settings.game_folder.clone() {
                Some(folder) => act(l, Action::UseFolder(folder), &mut exit),
                None => l.go(Screen::Pick),
            }
        }
    }
    let mut lines = 0.;
    for w in wheel.read() {
        lines += match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y / 40.,
        };
    }
    for d in drops.read() {
        match d {
            FileDragAndDrop::HoveredFile { .. } => l.hover_drop = true,
            FileDragAndDrop::HoveredFileCanceled { .. } => l.hover_drop = false,
            FileDragAndDrop::DroppedFile { path_buf, .. } => {
                l.hover_drop = false;
                // A Skate 3 ISO dropped on the tests screen sets the disc.
                let iso = path_buf
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("iso"));
                if iso && let Screen::Tests(b) = &l.screen {
                    let b = b.clone();
                    l.screen = Screen::BrowseIso(None, b);
                    act(l, Action::UseIso(path_buf.clone()), &mut exit);
                    continue;
                }
                let folder = if path_buf.is_dir() {
                    path_buf.clone()
                } else {
                    path_buf.parent().map(Path::to_path_buf).unwrap_or_default()
                };
                if matches!(
                    l.screen,
                    Screen::Pick
                        | Screen::Browse(_)
                        | Screen::Problem { .. }
                        | Screen::Tests(_)
                        | Screen::Guide(_)
                ) {
                    act(l, Action::UseFolder(folder), &mut exit);
                }
            }
        }
    }

    // Background work.
    let (mut finished, mut ended) = (None, None);
    match &mut l.screen {
        Screen::Check(job) => {
            if let Some(result) = job.done.lock().ok().and_then(|mut d| d.take()) {
                finished = Some((result, job.folder.clone()));
            }
        }
        Screen::Loading(run, b) => {
            if run.window_open.load(Ordering::Relaxed) && run.since_window.is_none() {
                run.since_window = Some(l.now);
            }
            // Hide once the test's window has had a moment to draw.
            if !run.hidden && run.since_window.is_some_and(|t| l.now - t > 0.6) {
                run.hidden = true;
                window.visible = false;
                winit.unfocused_mode = UpdateMode::reactive_low_power(Duration::from_millis(100));
            }
            if let Ok(Some(status)) = run.child.try_wait() {
                let (lines, new_game) = run
                    .output
                    .lock()
                    .map(|o| {
                        let new_game = o.iter().any(|l| l.trim() == "kh2fm-handoff NewGame");
                        (failure_lines(&o), new_game)
                    })
                    .unwrap_or_default();
                ended = Some((status, b.clone(), run.name.clone(), lines, new_game));
            }
        }
        _ => {}
    }
    if let Some((status, b, name, lines, new_game)) = ended {
        let room = TESTS.iter().position(|t| t.program == "play");
        if let (true, true, Some(room)) = (status.success(), new_game, room) {
            // New Game on the title continues straight into the test room.
            launch(l, &Action::Run(room), b);
        } else if status.success() && name == "Multiplayer" {
            // A duel left from its result screen (or closed) comes back to
            // Multiplayer: the hosts on the network, Host, Join.
            l.go(Screen::Multiplayer(b));
        } else if status.success() {
            let cursor = l.cursor;
            l.go(Screen::Tests(b));
            l.cursor = cursor;
        } else {
            l.go(Screen::Problem {
                title: format!("{name} stopped with an error"),
                lines: if lines.is_empty() {
                    vec![format!("Exit status: {status}")]
                } else {
                    lines
                },
                retry: Some(b),
            });
        }
        if !matches!(l.screen, Screen::Loading(..)) {
            window.visible = true;
            window.focused = true;
            winit.unfocused_mode =
                UpdateMode::reactive_low_power(Duration::from_secs_f64(1. / 60.));
            l.fade = 1.;
        }
    }
    if let Some((result, folder)) = finished {
        checked(l, result, &folder);
    }

    // The LAN search: new or vanished hosts rebuild the list.
    if let Some(browser) = l.browser.as_mut() {
        browser.update();
        // (When each host was last heard is not part of the rows; its ping
        // is, to 5 ms.)
        let rows = |f: &[net::Found]| -> Vec<_> {
            f.iter()
                .map(|h| {
                    let ping = h.ping_ms.map(|ms| ms.div_ceil(5));
                    (h.name.clone(), h.addr, h.compatible, h.open, ping)
                })
                .collect()
        };
        if rows(browser.found()) != rows(&l.found) {
            l.found = browser.found().to_vec();
            l.refresh();
        }
    }
    // A field being typed into takes the keyboard.
    if type_text(l, &keys, &mut typed) {
        return;
    }
    // Input on lists.
    if l.items.is_empty() {
        return;
    }
    let p = read_input(l, &keys, &pads, lines);
    if p.up {
        move_cursor(l, -1);
    }
    if p.down {
        move_cursor(l, 1);
    }
    if p.page != 0 {
        let rows = visible_rows(l) as i32;
        let n = l.items.len() as i32;
        l.cursor = (l.cursor as i32 + p.page * rows).clamp(0, n - 1) as usize;
        keep_visible(l);
    }
    // Mouse: hover picks (only while it moves, so keys win otherwise),
    // click decides.
    let mut clicked = false;
    let pos = window.cursor_position();
    let moved = l.mouse.is_some() && pos.is_some() && pos != l.mouse;
    l.mouse = pos;
    if let Some(pos) = pos {
        let (x, y) = (
            pos.x * paint::W / window.width(),
            pos.y * paint::H / window.height(),
        );
        let rows = visible_rows(l);
        for i in l.scroll..(l.scroll + rows).min(l.items.len()) {
            let [rx, ry, rw, rh] = row_rect(l, i);
            let inside = x >= rx && x <= rx + rw + 20. && y >= ry && y <= ry + rh;
            if inside && l.items[i].action.is_some() {
                if moved {
                    l.cursor = i;
                }
                if mouse.just_pressed(MouseButton::Left) {
                    l.cursor = i;
                    clicked = true;
                }
            }
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        back(l, &mut exit);
        return;
    }
    if p.accept || clicked {
        if let Some(a) = l.items.get(l.cursor).and_then(|i| i.action.clone()) {
            act(l, a, &mut exit);
        }
    } else if p.back {
        back(l, &mut exit);
    } else if p.change && matches!(l.screen, Screen::Tests(_)) {
        act(l, Action::Pick, &mut exit);
    } else if p.guide && matches!(l.screen, Screen::Tests(_) | Screen::Pick) {
        act(l, Action::Guide, &mut exit);
    }
}

fn draw(
    mut l: ResMut<Launcher>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    let l = &mut *l;
    if l.paint.is_none() {
        match Paint::new(&mut images, &mut frame) {
            Ok(p) => l.paint = Some(p),
            Err(e) => {
                eprintln!("Launcher drawing unavailable: {e}");
                return;
            }
        }
    }
    let dt = time.delta_secs().min(0.1);
    l.fade = (l.fade - dt * 4.).max(0.);
    for (i, g) in l.glow.iter_mut().enumerate() {
        let want = (i == l.cursor) as u8 as f32;
        *g += (want - *g) * (1. - (-dt * 18.).exp());
    }
    let mut paint = l.paint.take().expect("paint");
    let (p, f) = (&mut paint, &mut *frame);
    if matches!(l.screen, Screen::Multiplayer(_)) {
        multiplayer(l, p, f);
        if l.fade > 0. {
            p.rect(
                f,
                [0., 0., paint::W, paint::H],
                [rgba(0x000000, l.fade); 4],
                false,
            );
        }
        p.flush(&mut images, f);
        l.paint = Some(paint);
        return;
    }
    let (top, bottom) = (rgba(0x0b0f1a, 1.), rgba(0x05070c, 1.));
    p.rect(
        f,
        [0., 0., paint::W, paint::H],
        [top, top, bottom, bottom],
        false,
    );
    p.text(
        f,
        Face::Heading,
        20.,
        [MARGIN, 56.],
        0.,
        "KINGDOM HEARTS II FINAL MIX",
        [rgba(MUTED, 1.); 2],
        0.,
    );
    let heading = match &l.screen {
        Screen::Boot => String::new(),
        Screen::Guide(_) => "Quick start".into(),
        Screen::Pick => "Select the game folder".into(),
        Screen::Browse(path) => path
            .as_deref()
            .map_or_else(|| "Choose a drive".into(), plain),
        Screen::BrowseIso(path, _) => path
            .as_deref()
            .map_or_else(|| "Find your Skate 3 ISO".into(), plain),
        Screen::Check(_) => "Checking the game folder".into(),
        Screen::Tests(_) => "Choose a test".into(),
        Screen::Multiplayer(_) => "Multiplayer".into(),
        Screen::Loading(run, _) => format!("Loading {}", run.name),
        Screen::Problem { title, .. } => title.clone(),
    };
    let heading = p.fit(Face::Heading, 34., &heading, paint::W - 2. * MARGIN);
    p.text(
        f,
        Face::Heading,
        34.,
        [MARGIN, 96.],
        0.,
        &heading,
        [rgba(TEXT, 1.); 2],
        0.,
    );
    match &l.screen {
        Screen::Check(job) => check_screen(job, p, f),
        Screen::Loading(..) => {
            let dots = ".".repeat(1 + (l.now * 3.) as usize % 3);
            p.text(
                f,
                Face::Body,
                19.,
                [MARGIN, 160.],
                0.,
                &dots,
                [rgba(MUTED, 1.); 2],
                0.,
            );
        }
        Screen::Boot => {}
        _ => {
            if let Screen::Guide(_) = &l.screen {
                guide(p, f);
            }
            if let Screen::Problem { lines, .. } = &l.screen {
                let mut y = 152.;
                for para in lines {
                    for line in wrap(p, Face::Body, 18., para, paint::W - 2. * MARGIN) {
                        p.text(
                            f,
                            Face::Body,
                            18.,
                            [MARGIN, y],
                            0.,
                            &line,
                            [rgba(MUTED, 1.); 2],
                            0.,
                        );
                        y += 26.;
                    }
                }
            }
            list(l, p, f);
            footer(l, p, f);
        }
    }
    if l.hover_drop {
        p.rect(
            f,
            [0., 0., paint::W, paint::H],
            [rgba(0x000000, 0.7); 4],
            false,
        );
        p.text(
            f,
            Face::Heading,
            30.,
            [paint::W / 2., 340.],
            0.5,
            "Drop to use this game folder",
            [rgba(TEXT, 1.); 2],
            0.,
        );
    }
    if l.fade > 0. {
        p.rect(
            f,
            [0., 0., paint::W, paint::H],
            [rgba(0x000000, l.fade); 4],
            false,
        );
    }
    p.flush(&mut images, f);
    l.paint = Some(paint);
}

fn list_top(l: &Launcher) -> f32 {
    match &l.screen {
        Screen::Problem { lines, .. } => LIST_TOP + 26. * lines.len() as f32 + 24.,
        Screen::Guide(_) => GUIDE_LIST_TOP,
        Screen::Multiplayer(_) => MP_ROW_TOP,
        _ => LIST_TOP,
    }
}

fn list(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame) {
    let (h, _, size) = row_metrics(l);
    let rows = visible_rows(l);
    for i in l.scroll..(l.scroll + rows).min(l.items.len()) {
        let item = &l.items[i];
        let g = l.glow.get(i).copied().unwrap_or(0.);
        let [x, y, w, _] = row_rect(l, i);
        if g > 0.01 {
            let (a, b) = (rgba(ACCENT, g), rgba(ACCENT_DEEP, g));
            p.plate(f, [x - 18., y, w + 18., h], 7., 0., [a, b, a, b], false);
        }
        let (ink, sub) = match (item.action.is_some(), g > 0.5) {
            (false, _) => (rgba(FAINT, 1.), rgba(FAINT, 1.)),
            (true, true) => (rgba(TEXT, 1.), rgba(0xc9d6f2, 1.)),
            (true, false) => (rgba(0xc2cad9, 1.), rgba(FAINT, 1.)),
        };
        let tag_w = item.tag.map_or(0., |t| p.measure(Face::Body, 15., t) + 24.);
        let detail = item.detail.as_ref().filter(|_| h > 48.);
        let label_y = if detail.is_some() {
            y + 5.
        } else {
            y + (h - size * 1.33) / 2.
        };
        let label = p.fit(Face::Body, size, &item.label, w - 20. - tag_w);
        p.text(f, Face::Body, size, [x, label_y], 0., &label, [ink; 2], 0.);
        if let Some(d) = detail {
            let d = p.fit(Face::Body, 15., d, w - 20. - tag_w);
            p.text(f, Face::Body, 15., [x, y + 39.], 0., &d, [sub; 2], 0.);
        }
        if let Some(tag) = item.tag {
            let ty = y + (h - 20.) / 2.;
            p.text(
                f,
                Face::Body,
                15.,
                [x + w - 16., ty],
                1.,
                tag,
                [rgba(GOLD, 1.); 2],
                0.,
            );
        }
    }
    let c = rgba(MUTED, 0.8);
    let mid = MARGIN + LIST_WIDTH / 2.;
    if l.scroll > 0 {
        let y = list_top(l) - 14.;
        triangle(
            f,
            p,
            [[mid - 9., y + 4.], [mid + 9., y + 4.], [mid, y - 5.]],
            c,
        );
    }
    if l.scroll + rows < l.items.len() {
        let y = LIST_BOTTOM + 10.;
        triangle(
            f,
            p,
            [[mid - 9., y - 4.], [mid + 9., y - 4.], [mid, y + 5.]],
            c,
        );
    }
}

/// The Multiplayer screen in the camp's look: the header band and its
/// MENU-style title, the entries as the camp's boxes (red for the cursor's,
/// the pointer beside it), the bottom panel with the player's name and the
/// hosts found, and the help box with the cursor's entry's line.
fn multiplayer(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame) {
    let blue = rgba(MP_BLUE, 1.);
    let (top, bottom) = (rgba(0x030a2e, 1.), rgba(0x01030f, 1.));
    p.rect(
        f,
        [0., 0., paint::W, paint::H],
        [top, top, bottom, bottom],
        false,
    );
    // The header band, its frame lines and the title's plate.
    let (band_top, band_bottom) = (rgba(0x020a3a, 1.), rgba(0x010628, 1.));
    p.rect(
        f,
        [0., 0., paint::W, MP_BAND],
        [band_top, band_top, band_bottom, band_bottom],
        false,
    );
    p.rect(f, [0., MP_BAND, paint::W, 4.], [blue; 4], false);
    for x in [218., 236.] {
        p.rect(f, [x, 0., 3., MP_BAND], [blue; 4], false);
    }
    let (pill_top, pill_bottom) = (rgba(0x2040e0, 1.), rgba(0x0a1890, 1.));
    p.plate(
        f,
        [128., 14., 272., 44.],
        22.,
        0.,
        [pill_top, pill_top, pill_bottom, pill_bottom],
        false,
    );
    p.text(
        f,
        Face::Heading,
        30.,
        [264., 16.],
        0.5,
        "MULTIPLAYER",
        [rgba(TEXT, 1.); 2],
        0.,
    );
    // The entries.
    let (h, _, size) = row_metrics(l);
    let rows = visible_rows(l);
    for i in l.scroll..(l.scroll + rows).min(l.items.len()) {
        let item = &l.items[i];
        let on = i == l.cursor;
        let [x, y, w, _] = row_rect(l, i);
        let (edge, fill_top, fill_bottom, ink) = match (on, item.action.is_some()) {
            (true, _) => (0xff2a1a, 0x7a0a08, 0x220000, TEXT),
            (false, true) => (0x4a5060, 0x2a2e36, 0x0e1014, 0xb4b8c0),
            (false, false) => (0x353a46, 0x1c1f26, 0x0a0b0e, FAINT),
        };
        p.plate(
            f,
            [x - 2., y - 2., w + 4., h + 4.],
            12.,
            0.,
            [rgba(edge, 1.); 4],
            false,
        );
        let (a, b) = (rgba(fill_top, 1.), rgba(fill_bottom, 1.));
        p.plate(f, [x, y, w, h], 10., 0., [a, a, b, b], false);
        let tag_w = item.tag.map_or(0., |t| p.measure(Face::Body, 15., t) + 20.);
        let label = p.fit(Face::Body, size, &item.label, w - 32. - tag_w);
        let label_y = y + (h - size * 1.33) / 2.;
        p.text(
            f,
            Face::Body,
            size,
            [x + 22., label_y],
            0.,
            &label,
            [rgba(ink, 1.); 2],
            0.,
        );
        if let Some(tag) = item.tag {
            p.text(
                f,
                Face::Body,
                15.,
                [x + w - 14., y + (h - 20.) / 2.],
                1.,
                tag,
                [rgba(GOLD, 1.); 2],
                0.,
            );
        }
        if on {
            let cy = y + h / 2.;
            triangle(
                f,
                p,
                [[x - 34., cy - 11.], [x - 34., cy + 11.], [x - 10., cy]],
                rgba(TEXT, 1.),
            );
        }
    }
    // The bottom panels: the player's name and the hosts found, then the
    // help box.
    let (panel_top, panel_bottom) = (rgba(0x000528, 1.), rgba(0x000214, 1.));
    let fill = [panel_top, panel_top, panel_bottom, panel_bottom];
    let panel_h = paint::H - MP_PANEL_TOP;
    p.rect(f, [0., MP_PANEL_TOP, MP_HELP_X - 16., panel_h], fill, false);
    p.rect(
        f,
        [MP_HELP_X, MP_PANEL_TOP, paint::W - MP_HELP_X, panel_h],
        fill,
        false,
    );
    p.rect(f, [0., MP_PANEL_TOP, MP_HELP_X - 16., 4.], [blue; 4], false);
    p.rect(
        f,
        [MP_HELP_X - 19., MP_PANEL_TOP, 3., panel_h],
        [blue; 4],
        false,
    );
    p.rect(
        f,
        [MP_HELP_X, MP_PANEL_TOP, paint::W - MP_HELP_X, 4.],
        [blue; 4],
        false,
    );
    p.rect(f, [MP_HELP_X, MP_PANEL_TOP, 3., panel_h], [blue; 4], false);
    let name = p.fit(Face::Body, 26., &l.player_name(), 300.);
    let hosts = l.found.iter().filter(|h| h.compatible && h.open).count();
    for (label, value, colour, y) in [
        ("NAME", name, MP_ORANGE, MP_PANEL_TOP + 16.),
        ("HOSTS", hosts.to_string(), 0x2ad4ff, MP_PANEL_TOP + 80.),
    ] {
        p.text(
            f,
            Face::Heading,
            22.,
            [40., y],
            0.,
            label,
            [rgba(colour, 1.); 2],
            0.,
        );
        p.text(
            f,
            Face::Body,
            26.,
            [MP_HELP_X - 50., y + 26.],
            1.,
            &value,
            [rgba(MP_YELLOW, 1.); 2],
            0.,
        );
    }
    let help = l
        .items
        .get(l.cursor)
        .map(|i| {
            i.detail
                .clone()
                .unwrap_or_else(|| multiplayer_help(&i.label))
        })
        .unwrap_or_default();
    let mut y = MP_PANEL_TOP + 18.;
    for line in wrap(p, Face::Body, 26., &help, paint::W - MP_HELP_X - 60.)
        .into_iter()
        .take(2)
    {
        p.text(
            f,
            Face::Body,
            26.,
            [MP_HELP_X + 25., y],
            0.,
            &line,
            [rgba(MP_HELP, 1.); 2],
            0.,
        );
        y += 36.;
    }
    let hints = if l.editing.is_some() {
        "Type, then Enter to keep      Esc  cancel"
    } else {
        "Enter / A  select      Esc / B  back"
    };
    p.text(
        f,
        Face::Body,
        15.,
        [MP_HELP_X + 25., paint::H - 34.],
        0.,
        hints,
        [rgba(FAINT, 1.); 2],
        0.,
    );
}

/// The help line of a Multiplayer entry with no detail of its own.
fn multiplayer_help(label: &str) -> String {
    match label {
        "Back" => "Back to the test list.".into(),
        _ => String::new(),
    }
}

fn triangle(f: &mut Ui2dFrame, p: &Paint, pts: [[f32; 2]; 3], c: Rgba) {
    let at = |[x, y]: [f32; 2]| paint::to_layout(x, y);
    f.draws.push((
        p.white,
        vec![data::sprite_sequence::Quad {
            corners: [at(pts[0]), at(pts[1]), at(pts[2]), at(pts[1])],
            uv: [1, 1, 3, 3],
            colors: [c; 4],
            blend: 0,
        }],
    ));
}

fn footer(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame) {
    let y = 668.;
    let hints = match l.screen {
        Screen::Tests(_) => {
            "Enter / A  start      F2 / Y  change folder      F1 / Back  guide      Esc / B  quit"
        }
        Screen::Pick => "Enter / A  select      F1 / Back  guide      Esc / B  back",
        Screen::Guide(_) => "Enter / A  continue      F1 / Back  open this guide again later",
        Screen::Multiplayer(_) if l.editing.is_some() => {
            "Type, then Enter to keep      Esc  cancel"
        }
        _ => "Enter / A  select      Esc / B  back",
    };
    p.text(
        f,
        Face::Body,
        16.,
        [MARGIN, y],
        0.,
        hints,
        [rgba(FAINT, 1.); 2],
        0.,
    );
    if let Screen::Tests(b) = &l.screen {
        let build = b
            .build_id
            .as_deref()
            .map_or_else(String::new, |id| format!("   build {id}"));
        let status = p.fit(
            Face::Body,
            16.,
            &format!("{}{build}", plain(&b.folder)),
            320.,
        );
        p.text(
            f,
            Face::Body,
            16.,
            [paint::W - MARGIN, y],
            1.,
            &status,
            [rgba(FAINT, 1.); 2],
            0.,
        );
    }
}

/// The quick start steps [launcher text, not the original's].
const GUIDE_STEPS: [&str; 4] = [
    "You need your own Steam copy of KINGDOM HEARTS -HD 1.5+2.5 ReMIX-, installed. Only Kingdom Hearts II Final Mix is used.",
    "Choose the game folder. A Steam install is found for you. If it is not, browse to it or drop the folder on this window.",
    "Optional: on the test list, set your Skate 3 disc (an Xbox 360 ISO) to ride a skateboard with J.",
    "Choose Title screen and pick New Game, or Test room to start playing straight away.",
];

/// Action, keyboard, controller (the room's bindings, `input::read_devices`).
const GUIDE_CONTROLS: [[&str; 3]; 10] = [
    ["Move", "WASD", "Left stick"],
    ["Walk", "Hold Left Alt", "Tilt slightly"],
    ["Camera", "Mouse", "Right stick"],
    ["Attack", "Enter, left click", "A"],
    ["Jump", "Space", "B"],
    ["Guard", "Right Shift, right click", "X"],
    ["Talk", "F", "Y"],
    ["Shortcuts", "Hold Left Shift", "LB"],
    ["Skateboard", "J", ""],
    ["Leave the test", "Esc", ""],
];

fn guide(p: &mut Paint, f: &mut Ui2dFrame) {
    let mut y = 152.;
    for (i, step) in GUIDE_STEPS.iter().enumerate() {
        p.text(
            f,
            Face::Body,
            18.,
            [MARGIN, y],
            0.,
            &format!("{}.", i + 1),
            [rgba(TEXT, 1.); 2],
            0.,
        );
        for line in wrap(p, Face::Body, 18., step, 470.) {
            p.text(
                f,
                Face::Body,
                18.,
                [MARGIN + 28., y],
                0.,
                &line,
                [rgba(MUTED, 1.); 2],
                0.,
            );
            y += 26.;
        }
        y += 16.;
    }
    let columns = [680., 870., 1080.];
    for (x, head) in columns.iter().zip(["Controls", "Keyboard", "Controller"]) {
        p.text(
            f,
            Face::Body,
            16.,
            [*x, 152.],
            0.,
            head,
            [rgba(FAINT, 1.); 2],
            0.,
        );
    }
    for (row, cells) in GUIDE_CONTROLS.iter().enumerate() {
        let y = 184. + row as f32 * 32.;
        for ((x, cell), ink) in columns.iter().zip(cells).zip([TEXT, MUTED, MUTED]) {
            p.text(
                f,
                Face::Body,
                17.,
                [*x, y],
                0.,
                cell,
                [rgba(ink, 1.); 2],
                0.,
            );
        }
    }
}

fn check_screen(job: &Job, p: &mut Paint, f: &mut Ui2dFrame) {
    let path = p.fit(Face::Body, 18., &plain(&job.folder), paint::W - 2. * MARGIN);
    p.text(
        f,
        Face::Body,
        18.,
        [MARGIN, 152.],
        0.,
        &path,
        [rgba(MUTED, 1.); 2],
        0.,
    );
    if !job.scanning.load(Ordering::Relaxed) {
        return;
    }
    let (done, total) = (
        job.progress.load(Ordering::Relaxed),
        job.total.load(Ordering::Relaxed).max(1),
    );
    let t = (done as f32 / total as f32).clamp(0., 1.);
    let bar = [MARGIN, 210., paint::W - 2. * MARGIN, 6.];
    p.rect(f, bar, [rgba(0x1a2234, 1.); 4], false);
    p.rect(
        f,
        [bar[0], bar[1], bar[2] * t, bar[3]],
        [rgba(ACCENT, 1.); 4],
        false,
    );
    let gb = |b: u64| b as f64 / (1u64 << 30) as f64;
    let line = format!(
        "Reading the game files once: {:.0}%  ({:.1} of {:.1} GB)",
        t * 100.,
        gb(done),
        gb(total)
    );
    p.text(
        f,
        Face::Body,
        17.,
        [MARGIN, 232.],
        0.,
        &line,
        [with_alpha(rgba(MUTED, 1.), 1.); 2],
        0.,
    );
}

/// Opens the launcher window. Returns when it closes.
/// The Skate 3 disc the launcher remembers (an ISO, or an extracted
/// folder's `default.xex`), for the test room's J key.
pub fn skate3_iso() -> Option<std::path::PathBuf> {
    install::load_settings(&install::data_dir()).skate3_iso
}

/// Where the riding data prepared from the Skate 3 disc is kept.
#[cfg(feature = "skate3")]
pub fn skate3_cache() -> std::path::PathBuf {
    install::data_dir().join("skate3")
}

pub fn main() -> std::process::ExitCode {
    let dir = install::data_dir();
    let settings = install::load_settings(&dir);
    App::new()
        .insert_resource(Launcher {
            dir,
            settings,
            supported: None,
            detected: Vec::new(),
            viewers: Vec::new(),
            screen: Screen::Boot,
            items: Vec::new(),
            cursor: 0,
            scroll: 0,
            glow: Vec::new(),
            entered: 0.,
            now: 0.,
            fade: 1.,
            hover_drop: false,
            repeat: None,
            mouse: None,
            paint: None,
            editing: None,
            browser: None,
            found: Vec::new(),
            build: None,
        })
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WinitSettings::game())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Kingdom Hearts II Re:Written {}", env!("CARGO_PKG_VERSION")),
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(Ui2dPlugin {
            order: 0,
            overlay: false,
        })
        .add_systems(Update, ((update, draw).chain(), screenshot))
        .run();
    std::process::ExitCode::SUCCESS
}
