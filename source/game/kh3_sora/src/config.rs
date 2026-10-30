//! OWNER: the UI implementer (docs/sora-ui-spec.md section 10.8). THE USER CONFIGURATION BLOCK
//! of the camp menu's Config screen: 0x20 bytes at `GW+0x2E8` in the game's save data, edited by
//! the screen on a copy and written back, applied and saved when the screen is left.
//!
//! Every bit and byte below is the spec's table (10.8, [I]: load `0x14154F520` and store
//! `0x14154E660` read side by side); the defaults are the block constructor's (`0x140E84340`).
//! The port READS only what it has a reader for: the camera options (`kh-camera`'s input: auto
//! camera, the two flips, the two sensitivities), vibration (a stub: the host has no force
//! feedback yet) and Control Type (the command menu's `setLegacyMode`). The other options are
//! stored and shown, nothing reads them yet.

/// One row of the Config screen, top to bottom (the game's numbering 1 .. 26).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConfigOption {
    CameraWork = 1,
    VerticalCameraControl,
    HorizontalCameraControl,
    VerticalCameraSensitivity,
    HorizontalCameraSensitivity,
    VerticalReticleControl,
    HorizontalReticleControl,
    VerticalRideControl,
    ComboCam,
    KeybladeCinematics,
    LinkCinematics,
    AttractionCinematics,
    CommandMenus,
    CommandSelection,
    CursorPosition,
    SituationCommands,
    ControlType,
    Subtitles,
    Maps,
    MapOrientation,
    Vibration,
    MasterVolume,
    Graphics,
    DisplayArea,
    ScreenBrightness,
    Language,
}

/// Where an option lives in the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// One bit; `inverted`: the bit SET means choice 0 (Camera Work: set = Auto).
    Bit {
        byte: usize,
        bit: u8,
        inverted: bool,
    },
    /// A whole byte holding the choice index or the slider value.
    Byte { byte: usize },
    /// Not in the block on PC (platform query, own screen, or a stub): nothing is stored.
    None,
}

/// What a row offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choices {
    /// Choice texts in index order.
    List(&'static [&'static str]),
    /// A slider over `min ..= max`.
    Slider { min: u8, max: u8 },
}

/// One row's description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionInfo {
    pub option: ConfigOption,
    /// English label (`kh3_camp.config.config_*`).
    pub label: &'static str,
    pub storage: Storage,
    pub choices: Choices,
    /// The port has a reader for it.
    pub read_by_port: bool,
}

const NORMAL_FLIPPED: &[&str] = &["Normal", "Flipped"];
const OFF_ON: &[&str] = &["Off", "On"];

const fn bit(
    option: ConfigOption,
    label: &'static str,
    byte: usize,
    bit: u8,
    inverted: bool,
    choices: &'static [&'static str],
    read_by_port: bool,
) -> OptionInfo {
    OptionInfo {
        option,
        label,
        storage: Storage::Bit {
            byte,
            bit,
            inverted,
        },
        choices: Choices::List(choices),
        read_by_port,
    }
}

/// The 26 rows of the screen (spec 10.8).
pub const OPTIONS: [OptionInfo; 26] = [
    bit(
        ConfigOption::CameraWork,
        "Camera Work",
        0,
        1,
        true,
        &["Auto", "Manual"],
        true,
    ),
    bit(
        ConfigOption::VerticalCameraControl,
        "Vertical Camera Control",
        0,
        3,
        false,
        NORMAL_FLIPPED,
        true,
    ),
    bit(
        ConfigOption::HorizontalCameraControl,
        "Horizontal Camera Control",
        0,
        2,
        false,
        NORMAL_FLIPPED,
        true,
    ),
    OptionInfo {
        option: ConfigOption::VerticalCameraSensitivity,
        label: "Vertical Camera Sensitivity (Gamepad)",
        storage: Storage::Byte { byte: 6 },
        choices: Choices::Slider { min: 1, max: 100 },
        read_by_port: true,
    },
    OptionInfo {
        option: ConfigOption::HorizontalCameraSensitivity,
        label: "Horizontal Camera Sensitivity (Gamepad)",
        storage: Storage::Byte { byte: 5 },
        choices: Choices::Slider { min: 1, max: 100 },
        read_by_port: true,
    },
    bit(
        ConfigOption::VerticalReticleControl,
        "Vertical Reticle Control",
        0,
        5,
        false,
        NORMAL_FLIPPED,
        false,
    ),
    bit(
        ConfigOption::HorizontalReticleControl,
        "Horizontal Reticle Control",
        0,
        4,
        false,
        NORMAL_FLIPPED,
        false,
    ),
    bit(
        ConfigOption::VerticalRideControl,
        "Vertical Ride Control",
        2,
        3,
        false,
        NORMAL_FLIPPED,
        false,
    ),
    OptionInfo {
        option: ConfigOption::ComboCam,
        label: "Combo Cam",
        storage: Storage::Byte { byte: 3 },
        choices: Choices::List(&["Auto", "On", "Off"]),
        read_by_port: false,
    },
    bit(
        ConfigOption::KeybladeCinematics,
        "Keyblade Cinematics",
        1,
        2,
        true,
        &["Normal", "Basic"],
        false,
    ),
    bit(
        ConfigOption::LinkCinematics,
        "Link Cinematics",
        1,
        3,
        false,
        OFF_ON,
        false,
    ),
    bit(
        ConfigOption::AttractionCinematics,
        "Attraction Cinematics",
        1,
        4,
        false,
        OFF_ON,
        false,
    ),
    bit(
        ConfigOption::CommandMenus,
        "Command Menus",
        1,
        5,
        false,
        &["By World", "Classic"],
        false,
    ),
    bit(
        ConfigOption::CommandSelection,
        "Command Selection",
        1,
        6,
        false,
        &["Active", "Slow"],
        false,
    ),
    bit(
        ConfigOption::CursorPosition,
        "Cursor Position",
        1,
        7,
        false,
        &["Reset", "Lock"],
        false,
    ),
    bit(
        ConfigOption::SituationCommands,
        "Situation Commands",
        2,
        0,
        false,
        &["Prioritize Older", "Prioritize Newer"],
        false,
    ),
    bit(
        ConfigOption::ControlType,
        "Control Type",
        2,
        1,
        false,
        &["Type A", "Type B"],
        true,
    ),
    bit(
        ConfigOption::Subtitles,
        "Subtitles",
        1,
        0,
        false,
        OFF_ON,
        false,
    ),
    bit(ConfigOption::Maps, "Maps", 0, 6, false, OFF_ON, false),
    bit(
        ConfigOption::MapOrientation,
        "Map Orientation",
        0,
        7,
        true,
        &["Fixed", "Match Camera"],
        false,
    ),
    bit(
        ConfigOption::Vibration,
        "Vibration",
        0,
        0,
        false,
        OFF_ON,
        true,
    ),
    OptionInfo {
        option: ConfigOption::MasterVolume,
        label: "Master Volume",
        storage: Storage::Byte { byte: 7 },
        choices: Choices::Slider { min: 0, max: 10 },
        read_by_port: false,
    },
    OptionInfo {
        option: ConfigOption::Graphics,
        label: "Graphics",
        storage: Storage::None,
        choices: Choices::List(&["Default Mode", "Stable Mode"]),
        read_by_port: false,
    },
    OptionInfo {
        option: ConfigOption::DisplayArea,
        label: "Display Area",
        storage: Storage::None,
        choices: Choices::Slider { min: 0, max: 0 },
        read_by_port: false,
    },
    OptionInfo {
        option: ConfigOption::ScreenBrightness,
        label: "Screen Brightness",
        storage: Storage::Byte { byte: 4 },
        choices: Choices::Slider { min: 0, max: 100 },
        read_by_port: false,
    },
    OptionInfo {
        option: ConfigOption::Language,
        label: "Language",
        storage: Storage::None,
        choices: Choices::List(&["Japanese", "English"]),
        read_by_port: false,
    },
];

/// The block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserConfig {
    pub block: [u8; 0x20],
}

impl Default for UserConfig {
    /// The constructor `0x140E84340`: byte 0 = 0xC3, byte 1 = 0x1F, byte 2 = `(old & 0xE4) | 4`,
    /// bytes 3, 4 = 0, bytes 5, 6 = 50, byte 7 = 10, byte 8 = 0.
    fn default() -> UserConfig {
        let mut c = UserConfig { block: [0; 0x20] };
        c.restore_defaults();
        c
    }
}

impl UserConfig {
    /// Triangle + Yes on the screen: the constructor's values (byte 2 keeps its bits 2, 5, 6, 7).
    pub fn restore_defaults(&mut self) {
        let b = &mut self.block;
        b[0] = 0xC3;
        b[1] = 0x1F;
        b[2] = (b[2] & 0xE4) | 4;
        b[3] = 0;
        b[4] = 0;
        b[5] = 50;
        b[6] = 50;
        b[7] = 10;
        b[8] = 0;
    }

    pub fn info(option: ConfigOption) -> &'static OptionInfo {
        &OPTIONS[option as usize - 1]
    }

    /// The choice index (or slider value) of a row; 0 for a row that is not stored.
    pub fn get(&self, option: ConfigOption) -> u8 {
        match Self::info(option).storage {
            Storage::Bit {
                byte,
                bit,
                inverted,
            } => (((self.block[byte] >> bit) & 1) != 0) as u8 ^ inverted as u8,
            Storage::Byte { byte } => self.block[byte],
            Storage::None => 0,
        }
    }

    /// Sets a row's choice index (or slider value), clamped to what the row offers.
    pub fn set(&mut self, option: ConfigOption, value: u8) {
        let info = Self::info(option);
        let value = match info.choices {
            Choices::List(list) => value.min(list.len().saturating_sub(1) as u8),
            Choices::Slider { min, max } => value.clamp(min, max),
        };
        match info.storage {
            Storage::Bit {
                byte,
                bit,
                inverted,
            } => {
                let on = (value != 0) ^ inverted;
                self.block[byte] = (self.block[byte] & !(1 << bit)) | ((on as u8) << bit);
            }
            Storage::Byte { byte } => self.block[byte] = value,
            Storage::None => {}
        }
    }

    // ---- what the port reads

    /// Camera Work: Auto (the camera follows by itself).
    pub fn camera_auto(&self) -> bool {
        self.get(ConfigOption::CameraWork) == 0
    }

    /// Vertical / horizontal camera control flipped.
    pub fn camera_flip(&self) -> (bool, bool) {
        (
            self.get(ConfigOption::VerticalCameraControl) != 0,
            self.get(ConfigOption::HorizontalCameraControl) != 0,
        )
    }

    /// Vertical / horizontal camera sensitivity, 1 ..= 100 (the game uses value - 1).
    pub fn camera_sensitivity(&self) -> (u8, u8) {
        (
            self.get(ConfigOption::VerticalCameraSensitivity),
            self.get(ConfigOption::HorizontalCameraSensitivity),
        )
    }

    pub fn vibration(&self) -> bool {
        self.get(ConfigOption::Vibration) != 0
    }

    /// Control Type B: the command menu's legacy mode (`setLegacyMode(true)`).
    pub fn control_type_b(&self) -> bool {
        self.get(ConfigOption::ControlType) != 0
    }

    /// Master Volume as a gain 0 ..= 1 (the slider is 0 ..= 10, default 10). LOCAL mapping: the
    /// game's curve from the slider to the SEAD master volume was not read; linear is taken.
    pub fn master_volume(&self) -> f32 {
        self.get(ConfigOption::MasterVolume).min(10) as f32 / 10.0
    }

    // ---- persistence (integration pass). The game keeps the block in its save data; the port's
    // hosts keep it as a small text file. The core reads and writes no files: these are the text.

    /// The block as text: a header line and the 0x20 bytes in hex.
    pub fn to_text(&self) -> String {
        let hex: Vec<String> = self.block.iter().map(|b| format!("{b:02X}")).collect();
        format!("{CONFIG_TEXT_HEADER}\n{}\n", hex.join(" "))
    }

    /// The block of a text written by `to_text`; `None` when it is not one (the caller keeps the
    /// defaults). Slider bytes are clamped to their ranges.
    pub fn from_text(text: &str) -> Option<UserConfig> {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        if lines.next()? != CONFIG_TEXT_HEADER {
            return None;
        }
        let bytes: Vec<u8> = lines
            .next()?
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).ok())
            .collect::<Option<_>>()?;
        let mut config = UserConfig {
            block: bytes.try_into().ok()?,
        };
        for info in OPTIONS.iter() {
            if let Choices::Slider { .. } = info.choices {
                config.set(info.option, config.get(info.option));
            }
        }
        Some(config)
    }
}

/// First line of the configuration text file.
pub const CONFIG_TEXT_HEADER: &str = "kh3-user-config 1 (TresUserConfig block, 0x20 bytes)";
