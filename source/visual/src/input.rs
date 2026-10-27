//! Bevy device state into the exe's PADDATA (`pad`): the PC version's
//! keyboard and mouse bindings and the first gamepad in XInput order
//! (FUN_140134d10).
//!
//! The bindings are the PC version's (T256): each action has two keys
//! (DirectInput DIK codes; 0x1000 + n is mouse button n), read from the
//! user's `config1525.dat` when there is one (FUN_1404e6d10 loads it,
//! falling back per slot to the defaults) and otherwise the defaults the
//! exe writes into a new config (0x14014ce50). FUN_140102ca0 turns held keys
//! into PS2 buttons and stick deflections: WASD -128 / +127, with the walk
//! key held -96 / +96 (FUN_1404e94c0(1, 0x60)); Enter also answers to the
//! numpad's Enter.
//!
//! Mouse-look (FUN_140105810, FUN_1404e51f0, FUN_14039c180, FUN_140166100):
//! DirectInput's relative motion over 200, clamped to -1..1, is read only
//! while the window is active and the cursor inside it; a frame with motion
//! puts (24 dx, 20 dy) (each clamped) on the right stick with a dead zone of
//! one byte unit, multiplies the stick's magnitude by 1 + 0.02 x the
//! config's sensitivity (20 by default), and turns the camera
//! max(10 |dx|, 1) times faster (its height, max(10 |dy|, 1)).
use crate::controller::{self, PadData, button};
use bevy::prelude::*;

/// The keys and devices this instance reads.
#[derive(Resource, Clone, Debug)]
pub struct Controls {
    pub bindings: Bindings,
    /// The assignment asked for (`--input`).
    pub wanted: Assign,
    /// The devices read now (Auto settles when a duel finds its peer).
    pub devices: Devices,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            bindings: Bindings::default(),
            wanted: Assign::Auto,
            devices: Devices::Both,
        }
    }
}

/// `--input`: which devices an instance reads [rewrite option, T256].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Assign {
    /// Both, until a duel's other player turns out to be on this PC: then
    /// the host keeps the keyboard and mouse and the joiner a controller.
    #[default]
    Auto,
    Fixed(Devices),
}

impl Assign {
    /// `auto`, `kbm`, `pad`, `pad:N` (the Nth controller from 1), `both`,
    /// `none`.
    pub fn parse(s: &str) -> Option<Assign> {
        Some(match s {
            "auto" => Assign::Auto,
            "kbm" | "keyboard" => Assign::Fixed(Devices::KeyboardMouse),
            "pad" | "controller" => Assign::Fixed(Devices::Pad(None)),
            "none" => Assign::Fixed(Devices::None),
            "both" => Assign::Fixed(Devices::Both),
            _ => {
                let n: u8 = s.strip_prefix("pad:")?.parse().ok()?;
                Assign::Fixed(Devices::Pad(Some(n.checked_sub(1)?)))
            }
        })
    }

    pub fn arg(self) -> String {
        match self {
            Assign::Auto => "auto".into(),
            Assign::Fixed(Devices::KeyboardMouse) => "kbm".into(),
            Assign::Fixed(Devices::Both) => "both".into(),
            Assign::Fixed(Devices::Pad(None)) => "pad".into(),
            Assign::Fixed(Devices::Pad(Some(n))) => format!("pad:{}", n + 1),
            Assign::Fixed(Devices::None) => "none".into(),
        }
    }
}

/// The devices an instance reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Devices {
    #[default]
    Both,
    KeyboardMouse,
    /// A controller: any, or the one at this index.
    Pad(Option<u8>),
    /// Nothing: an unattended run, which controllers in use by other
    /// programs must not reach.
    None,
}

impl Devices {
    pub fn keyboard(self) -> bool {
        !matches!(self, Devices::Pad(_) | Devices::None)
    }

    pub fn pad(self) -> bool {
        !matches!(self, Devices::KeyboardMouse | Devices::None)
    }

    /// For the window title and the lobby.
    pub fn label(self) -> String {
        match self {
            Devices::Both => "keyboard, mouse and controller".into(),
            Devices::KeyboardMouse => "keyboard and mouse".into(),
            Devices::Pad(None) => "controller".into(),
            Devices::Pad(Some(n)) => format!("controller {}", n + 1),
            Devices::None => "no devices".into(),
        }
    }
}

/// The binding slots, in `config1525.dat`'s order (two keys each).
pub const WALK: usize = 0;
const BUTTON_SLOTS: [(usize, u16); 16] = [
    (1, button::CROSS),
    (2, button::CIRCLE),
    (3, button::TRIANGLE),
    (4, button::SQUARE),
    (5, button::L1),
    (6, button::R1),
    (7, button::L2),
    (8, button::R2),
    (9, button::L3),
    (10, button::R3),
    (11, button::RIGHT),
    (12, button::DOWN),
    (13, button::UP),
    (14, button::LEFT),
    (15, button::SELECT),
    (16, button::START),
];
/// Left stick up, down, left, right.
const MOVE_SLOTS: [usize; 4] = [17, 18, 19, 20];
const SLOTS: usize = 21;
/// Where the slots start in the file, and the camera's four codes after them.
const SLOT_AT: usize = 0x3c;
const CAMERA_AT: usize = SLOT_AT + 8 * SLOTS;
/// The file's first bytes and least size.
const MAGIC: &[u8; 4] = b"SET-";
const FILE_LEAST: usize = CAMERA_AT + 16;

/// Mouse codes (0x1000 + button); 0x1005..=0x1008 in the camera slots are
/// the mouse's own motion (left, right, up, down).
const MOUSE: u32 = 0x1000;
const MOUSE_MOTION: [u32; 4] = [0x1005, 0x1006, 0x1007, 0x1008];
/// [I] The two codes the D-pad up/down slots name after their arrows: the
/// wheel (the exe turns a scroll into one D-pad press, FUN_1404ebd30(1)).
const WHEEL_UP: u32 = 0x1009;
const WHEEL_DOWN: u32 = 0x100a;

/// The PC version's key bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bindings {
    pub slots: [[u32; 2]; SLOTS],
    /// Camera left, right, up, down.
    pub camera: [u32; 4],
    /// The config's mouse sensitivity (20 by default).
    pub sensitivity: u16,
    /// Japanese confirm: Circle decides and Cross goes back (the config's
    /// option; the decide and back keys swap with it).
    pub swap_confirm: bool,
    /// Where they came from (for the log): "defaults" or "config".
    pub source: &'static str,
}

impl Default for Bindings {
    /// 0x14014ce50: what a new config holds.
    fn default() -> Self {
        Bindings {
            slots: [
                [0x38, 0],      // walk: Left Alt
                [0x1000, 0x1c], // Cross: left click, Enter
                [0x39, 0x0e],   // Circle: Space, Backspace
                [0x21, 0x9d],   // Triangle: F, Right Ctrl
                [0x1001, 0x36], // Square: right click, Right Shift
                [0x2a, 0],      // L1: Left Shift
                [0x13, 0],      // R1: R
                [0x2e, 0],      // L2: C
                [0x12, 0],      // R2: E
                [0x2d, 0],      // L3: X
                [0x1002, 0xb8], // R3: middle click, Right Alt
                [0xcd, 0x1004], // right: Right arrow, mouse forward
                [0xd0, 0x100a], // down: Down arrow, wheel down
                [0xc8, 0x1009], // up: Up arrow, wheel up
                [0xcb, 0x1003], // left: Left arrow, mouse back
                [0x31, 0x04],   // Select: N, 3
                [0x32, 0x05],   // Start: M, 4
                [0x11, 0],      // stick up: W
                [0x1f, 0],      // stick down: S
                [0x1e, 0],      // stick left: A
                [0x20, 0],      // stick right: D
            ],
            camera: MOUSE_MOTION,
            sensitivity: 20,
            swap_confirm: false,
            source: "defaults",
        }
    }
}

impl Bindings {
    /// A `config1525.dat`'s bindings (FUN_1404e6d10): each code the exe
    /// would take (a key, a mouse button) replaces the default; others
    /// leave it. None when the bytes are no such file.
    pub fn from_config(bytes: &[u8]) -> Option<Bindings> {
        if bytes.len() < FILE_LEAST || &bytes[..4] != MAGIC {
            return None;
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
        let half = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let mut b = Bindings::default();
        for (i, slot) in b.slots.iter_mut().enumerate() {
            for (k, code) in slot.iter_mut().enumerate() {
                let c = word(SLOT_AT + 8 * i + 4 * k);
                // A second key may be none; a first stays the default.
                if valid(c) {
                    *code = c;
                } else if k == 1 {
                    *code = 0;
                }
            }
        }
        for (i, code) in b.camera.iter_mut().enumerate() {
            let c = word(CAMERA_AT + 4 * i);
            if valid(c) {
                *code = c;
            }
        }
        b.swap_confirm = half(0x32) != 0;
        b.sensitivity = half(0x38);
        b.source = "config";
        Some(b)
    }

    /// The decide and back keys as the confirm option has them.
    fn slot(&self, i: usize) -> [u32; 2] {
        match (self.swap_confirm, i) {
            (true, 1) => self.slots[2],
            (true, 2) => self.slots[1],
            _ => self.slots[i],
        }
    }

    /// The right stick's multiplier for mouse-look (FUN_14039c180).
    pub fn mouse_gain(&self) -> f32 {
        1. + f32::from(self.sensitivity) * 0.05 * 0.4
    }
}

fn valid(code: u32) -> bool {
    dik_key(code).is_some() || (MOUSE..=WHEEL_DOWN).contains(&code)
}

/// A DirectInput key code as the key at the same place on the keyboard.
pub fn dik_key(code: u32) -> Option<KeyCode> {
    use KeyCode::*;
    const DIGITS: [KeyCode; 10] = [
        Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, Digit0,
    ];
    const F: [KeyCode; 10] = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10];
    Some(match code {
        0x01 => Escape,
        0x02..=0x0b => DIGITS[code as usize - 0x02],
        0x0c => Minus,
        0x0d => Equal,
        0x0e => Backspace,
        0x0f => Tab,
        0x10 => KeyQ,
        0x11 => KeyW,
        0x12 => KeyE,
        0x13 => KeyR,
        0x14 => KeyT,
        0x15 => KeyY,
        0x16 => KeyU,
        0x17 => KeyI,
        0x18 => KeyO,
        0x19 => KeyP,
        0x1a => BracketLeft,
        0x1b => BracketRight,
        0x1c => Enter,
        0x1d => ControlLeft,
        0x1e => KeyA,
        0x1f => KeyS,
        0x20 => KeyD,
        0x21 => KeyF,
        0x22 => KeyG,
        0x23 => KeyH,
        0x24 => KeyJ,
        0x25 => KeyK,
        0x26 => KeyL,
        0x27 => Semicolon,
        0x28 => Quote,
        0x29 => Backquote,
        0x2a => ShiftLeft,
        0x2b => Backslash,
        0x2c => KeyZ,
        0x2d => KeyX,
        0x2e => KeyC,
        0x2f => KeyV,
        0x30 => KeyB,
        0x31 => KeyN,
        0x32 => KeyM,
        0x33 => Comma,
        0x34 => Period,
        0x35 => Slash,
        0x36 => ShiftRight,
        0x37 => NumpadMultiply,
        0x38 => AltLeft,
        0x39 => Space,
        0x3a => CapsLock,
        0x3b..=0x44 => F[code as usize - 0x3b],
        0x45 => NumLock,
        0x46 => ScrollLock,
        0x47 => Numpad7,
        0x48 => Numpad8,
        0x49 => Numpad9,
        0x4a => NumpadSubtract,
        0x4b => Numpad4,
        0x4c => Numpad5,
        0x4d => Numpad6,
        0x4e => NumpadAdd,
        0x4f => Numpad1,
        0x50 => Numpad2,
        0x51 => Numpad3,
        0x52 => Numpad0,
        0x53 => NumpadDecimal,
        0x56 => IntlBackslash,
        0x57 => F11,
        0x58 => F12,
        0x9c => NumpadEnter,
        0x9d => ControlRight,
        0xb5 => NumpadDivide,
        0xb7 => PrintScreen,
        0xb8 => AltRight,
        0xc5 => Pause,
        0xc7 => Home,
        0xc8 => ArrowUp,
        0xc9 => PageUp,
        0xcb => ArrowLeft,
        0xcd => ArrowRight,
        0xcf => End,
        0xd0 => ArrowDown,
        0xd1 => PageDown,
        0xd2 => Insert,
        0xd3 => Delete,
        0xdb => SuperLeft,
        0xdc => SuperRight,
        0xdd => ContextMenu,
        _ => return None,
    })
}

fn mouse_button(n: u32) -> MouseButton {
    match n {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        n => MouseButton::Other(n as u16),
    }
}

/// Whether a code's key or button is held (`wheel`: this frame's scroll).
fn held(
    code: u32,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    wheel: f32,
) -> bool {
    match code {
        0 => false,
        WHEEL_UP => wheel > 0.,
        WHEEL_DOWN => wheel < 0.,
        MOUSE..WHEEL_UP => mouse.pressed(mouse_button(code - MOUSE)),
        // Enter's binding takes the numpad's Enter too (FUN_1404eb6d0).
        0x1c => keys.pressed(KeyCode::Enter) || keys.pressed(KeyCode::NumpadEnter),
        c => dik_key(c).is_some_and(|k| keys.pressed(k)),
    }
}

/// PADDATA from the devices `controls` allows: keyboard and mouse buttons
/// (wheel = D-pad up/down for one frame) and a gamepad; also whether the
/// keyboard/mouse was used and the active pad's glyph family (1 Xbox,
/// 2 PlayStation, 3 other).
pub fn read_devices(
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    wheel: f32,
    pads: &Query<(&Gamepad, Option<&Name>)>,
    controls: &Controls,
) -> (PadData, bool, Option<u8>) {
    let mut p = PadData::default();
    let mut keyboard = false;
    if controls.devices.keyboard() {
        let b = &controls.bindings;
        let on = |i: usize| b.slot(i).iter().any(|&c| held(c, keys, mouse, wheel));
        for (i, bit) in BUTTON_SLOTS {
            if on(i) {
                p.buttons |= bit;
            }
        }
        // Each held direction adds its deflection (FUN_1404e94c0): -128 /
        // +127, or -96 / +96 with the walk key held.
        let walk = on(WALK);
        let push = |neg: bool| match (walk, neg) {
            (true, true) => -96,
            (true, false) => 96,
            (false, true) => -128,
            (false, false) => 127,
        };
        let [up, down, left, right] = MOVE_SLOTS.map(on);
        let axis = |neg: bool, pos: bool| {
            let v = 0x80 + if neg { push(true) } else { 0 } + if pos { push(false) } else { 0 };
            v.clamp(0, 255) as u8
        };
        p.left = [axis(left, right), axis(up, down)];
        // Keys bound to the camera move the right stick (FUN_1404eb6d0);
        // the mouse's own motion is mouse-look (`mouse_look`).
        let cam = b
            .camera
            .map(|c| !MOUSE_MOTION.contains(&c) && held(c, keys, mouse, wheel));
        if cam.iter().any(|&c| c) {
            let axis = |neg: bool, pos: bool| {
                (0x80 + if neg { -128 } else { 0 } + if pos { 127 } else { 0 }).clamp(0, 255) as u8
            };
            p.right = [axis(cam[0], cam[1]), axis(cam[2], cam[3])];
        }
        keyboard = p.buttons != 0 || p.left != [0x80; 2] || p.right != [0x80; 2];
    }
    let mut glyph = None;
    let pad = match controls.devices {
        Devices::KeyboardMouse | Devices::None => None,
        Devices::Pad(Some(n)) => pads.iter().nth(usize::from(n)),
        Devices::Pad(None) | Devices::Both => pads.iter().next(),
    };
    if let Some((g, name)) = pad {
        let map = [
            (GamepadButton::DPadUp, button::UP),
            (GamepadButton::DPadDown, button::DOWN),
            (GamepadButton::DPadLeft, button::LEFT),
            (GamepadButton::DPadRight, button::RIGHT),
            (GamepadButton::Start, button::START),
            (GamepadButton::Select, button::SELECT),
            (GamepadButton::LeftThumb, button::L3),
            (GamepadButton::RightThumb, button::R3),
            (GamepadButton::LeftTrigger, button::L1),
            (GamepadButton::RightTrigger, button::R1),
            (GamepadButton::South, button::CROSS),
            (GamepadButton::East, button::CIRCLE),
            (GamepadButton::West, button::SQUARE),
            (GamepadButton::North, button::TRIANGLE),
        ];
        let mut b = 0;
        for (gb, bit) in map {
            if g.pressed(gb) {
                b |= bit;
            }
        }
        // Triggers count as L2/R2 past 127/255.
        for (gb, bit) in [
            (GamepadButton::LeftTrigger2, button::L2),
            (GamepadButton::RightTrigger2, button::R2),
        ] {
            if g.get(gb).unwrap_or(0.) > 127. / 255. {
                b |= bit;
            }
        }
        // Gamepad Y is up; PADDATA Y grows down.
        let (l, r) = (g.left_stick(), g.right_stick());
        let (ls, rs) = (
            controller::stick_bytes(l.x, -l.y),
            controller::stick_bytes(r.x, -r.y),
        );
        let active = b != 0 || ls != [0x80; 2] || rs != [0x80; 2];
        p.buttons |= b;
        if ls != [0x80; 2] {
            p.left = ls;
        }
        if rs != [0x80; 2] || p.right == [0x80; 2] {
            p.right = rs;
        }
        if active {
            glyph = Some(family(name));
        }
    }
    (p, keyboard, glyph)
}

/// A pad's glyph family by its name: 1 Xbox, 2 PlayStation, 3 other.
fn family(name: Option<&Name>) -> u8 {
    let n = name
        .map(|n| n.as_str().to_ascii_lowercase())
        .unwrap_or_default();
    if [
        "dualsense",
        "dualshock",
        "ps4",
        "ps5",
        "wireless controller",
    ]
    .iter()
    .any(|k| n.contains(k))
    {
        2
    } else if n.contains("xbox") || n.contains("xinput") {
        1
    } else {
        3
    }
}

/// The glyph family a window given a controller alone starts with (its
/// pad's, before any button; Xbox's with none attached).
pub fn pad_family(pads: &Query<(&Gamepad, Option<&Name>)>, controls: &Controls) -> Option<u8> {
    let n = match controls.devices {
        Devices::Pad(Some(n)) => usize::from(n),
        Devices::Pad(None) => 0,
        _ => return None,
    };
    Some(pads.iter().nth(n).map_or(1, |(_, name)| family(name)))
}

/// One game frame of mouse-look: the camera's turn and height inputs
/// (-1..1 directions times the stick's magnitude and the mouse's speed-up),
/// from the frame's relative motion in mouse counts. None without motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseLook {
    /// Right is positive (the right stick's right bit).
    pub turn: f32,
    /// Down is positive (the right stick's down bit).
    pub height: f32,
}

/// `gain`: the stick magnitude's multiplier (`Bindings::mouse_gain`).
pub fn mouse_look(counts: Vec2, gain: f32) -> Option<MouseLook> {
    // FUN_140105810: DirectInput's lX / lY over 200, clamped.
    let dx = (counts.x / 200.).clamp(-1., 1.);
    let dy = (counts.y / 200.).clamp(-1., 1.);
    if dx == 0. && dy == 0. {
        return None;
    }
    // FUN_14039c180: the right stick's bytes, a one-unit dead zone, the
    // magnitude with the sensitivity.
    let byte = |v: f32| (v.clamp(-1., 1.) * 127.5 + 127.5) as i32 as f32;
    let (bx, by) = (byte(dx * 20. * 1.2), byte(dy * 20.));
    let part = |b: f32| {
        let d = b - 127.5;
        d.signum() * (d.abs() - 1.).max(0.) / (127.5 - 1.)
    };
    let (vx, vy) = (part(bx), part(by));
    let magnitude = vx.hypot(vy).min(1.) * gain;
    let bit = |b: f32| {
        let d = b - 127.5;
        if d >= 1. {
            1.
        } else if d <= -1. {
            -1.
        } else {
            0.
        }
    };
    // FUN_140166100: the turn and height speeds grow with the motion.
    Some(MouseLook {
        turn: bit(bx) * magnitude * (10. * dx.abs()).max(1.),
        height: bit(by) * magnitude * (10. * dy.abs()).max(1.),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(edit: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut b = vec![0u8; 0x1fc];
        b[..4].copy_from_slice(MAGIC);
        let d = Bindings::default();
        for (i, slot) in d.slots.iter().enumerate() {
            for (k, c) in slot.iter().enumerate() {
                let at = SLOT_AT + 8 * i + 4 * k;
                b[at..at + 4].copy_from_slice(&c.to_le_bytes());
            }
        }
        for (i, c) in d.camera.iter().enumerate() {
            let at = CAMERA_AT + 4 * i;
            b[at..at + 4].copy_from_slice(&c.to_le_bytes());
        }
        b[0x38] = 20;
        edit(&mut b);
        b
    }

    #[test]
    fn a_default_config_reads_as_the_defaults() {
        let b = Bindings::from_config(&file(|_| {})).unwrap();
        assert_eq!(b.slots, Bindings::default().slots);
        assert_eq!((b.sensitivity, b.swap_confirm), (20, false));
        assert_eq!(Bindings::from_config(b"nope"), None);
    }

    #[test]
    fn a_config_rebinds_and_bad_codes_keep_the_defaults() {
        let b = Bindings::from_config(&file(|f| {
            // Jump on J; a nonsense first key for Triangle; Square's second
            // cleared.
            f[SLOT_AT + 16..SLOT_AT + 20].copy_from_slice(&0x24u32.to_le_bytes());
            f[SLOT_AT + 24..SLOT_AT + 28].copy_from_slice(&0x7777u32.to_le_bytes());
            f[SLOT_AT + 36..SLOT_AT + 40].copy_from_slice(&0u32.to_le_bytes());
            f[0x32] = 1;
            f[0x38] = 0;
        }))
        .unwrap();
        assert_eq!(b.slots[2], [0x24, 0x0e]);
        assert_eq!(b.slots[3], [0x21, 0x9d]);
        assert_eq!(b.slots[4], [0x1001, 0]);
        assert!(b.swap_confirm);
        assert_eq!(b.slot(1), [0x24, 0x0e]);
        assert_eq!(b.mouse_gain(), 1.);
    }

    #[test]
    fn every_default_code_is_a_key_or_a_mouse_code() {
        for c in Bindings::default().slots.iter().flatten() {
            assert!(*c == 0 || valid(*c), "{c:#x}");
        }
        assert_eq!(dik_key(0x11), Some(KeyCode::KeyW));
        assert_eq!(dik_key(0x04), Some(KeyCode::Digit3));
        assert_eq!(dik_key(0x3c), Some(KeyCode::F2));
    }

    #[test]
    fn mouse_look_follows_the_exe() {
        let b = Bindings::default().mouse_gain();
        assert_eq!(mouse_look(Vec2::ZERO, b), None);
        // 10 counts right: dx 0.05, the stick full right, 1.4 x the
        // magnitude, the turn's speed-up 10 x 0.05 under 1.
        let m = mouse_look(Vec2::new(10., 0.), b).unwrap();
        assert!((m.turn - 1.4).abs() < 1e-4, "{m:?}");
        assert_eq!(m.height, 0.);
        // 100 counts: dx 0.5, five times as fast.
        let m = mouse_look(Vec2::new(100., 0.), b).unwrap();
        assert!((m.turn - 7.).abs() < 1e-3, "{m:?}");
        // One count up: a small turn upward, at the base speed.
        let m = mouse_look(Vec2::new(0., -1.), b).unwrap();
        assert!(m.height < 0. && m.height > -0.2, "{m:?}");
    }

    #[test]
    fn assignments_parse() {
        assert_eq!(
            Assign::parse("pad:2"),
            Some(Assign::Fixed(Devices::Pad(Some(1))))
        );
        assert_eq!(Assign::parse("pad:0"), None);
        assert_eq!(
            Assign::parse("kbm"),
            Some(Assign::Fixed(Devices::KeyboardMouse))
        );
        for a in ["auto", "kbm", "pad", "pad:3", "both"] {
            assert_eq!(Assign::parse(a).unwrap().arg(), a);
        }
        assert!(!Devices::Pad(None).keyboard() && !Devices::KeyboardMouse.pad());
    }
}
