//! The Config page's settings in the room (visual::settings): the
//! window, the 3D scene, the mixer and the menu language follow them while
//! the camp is open. They are kept beside the room's configuration.
use super::*;
use bevy::window::{MonitorSelection, PresentMode, VideoModeSelection, WindowMode};
use visual::frame_limit::FrameLimit;
use visual::settings::{LANGUAGES, Settings};
use visual::smoothing::Smoothing;

/// The KH2 package indexes a language's files may sit in.
const INDEXES: [&str; 6] = [
    "Image/dt/kh2_first.hed",
    "Image/dt/kh2_second.hed",
    "Image/dt/kh2_third.hed",
    "Image/dt/kh2_fourth.hed",
    "Image/dt/kh2_fifth.hed",
    "Image/dt/kh2_sixth.hed",
];

/// Language-bearing paths (`{l}` = the language code); a configured file
/// that matches one is swapped for the same path in the new language.
const TEMPLATES: [&str; 10] = [
    "menu/{l}/camp.2ld",
    "msg/{l}/sys.bar",
    "msg/{l}/fontimage.bar",
    "msg/{l}/fontinfo.bar",
    "menu/{l}/jmemo.2ld",
    "msg/{l}/jm.bar",
    "menu/{l}/pause.2ld",
    "menu/{l}/jiminy.bar",
    "msg/{l}/jiminy.bar",
    "menu/{l}/jm_world.2ld",
];

/// World codes of the field command layouts (field2d/<lang>/<world><n>command.2dd).
const WORLDS: [&str; 19] = [
    "al", "bb", "ca", "dc", "di", "eh", "es", "gm", "hb", "he", "lk", "lm", "mu", "nm", "po", "tr",
    "tt", "wi", "wm",
];

/// The files the menus are built from, as configured (for a language swap).
#[derive(Clone)]
pub(super) struct LanguageSources {
    pub camp: Locator,
    pub menu: MenuSources,
    pub journal: Option<JournalConfig>,
    pub battle: Locator,
    pub system: Locator,
}

#[derive(Resource)]
pub(super) struct SettingsRes {
    pub current: Settings,
    saved: Settings,
    /// None: an unattended run, which uses the defaults and saves nothing.
    path: Option<PathBuf>,
    applied: Option<(Settings, bool)>,
    /// Codes of the languages the install has (empty: no swap possible).
    pub languages: Vec<String>,
    /// The language the menus are loaded in.
    loaded: String,
    sources: Option<LanguageSources>,
    /// Every KH2 file's name hash: (index, ordinal).
    names: HashMap<[u8; 16], (&'static str, usize)>,
    source: Arc<ProbeSource>,
}

fn hash(path: &str) -> [u8; 16] {
    data::name_hash::of_path(path)
}

impl SettingsRes {
    /// The saved settings at `path` (or defaults) and the languages the
    /// selected source holds; the configured menus fix the language they
    /// start in.
    pub fn new(
        source: Arc<ProbeSource>,
        path: Option<PathBuf>,
        sources: Option<LanguageSources>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut names = HashMap::new();
        if sources.is_some() {
            for index in INDEXES {
                let Ok(checked) = source.open_index(index) else {
                    continue;
                };
                for (i, e) in checked.inventory.entries.iter().enumerate() {
                    names.insert(e.name_hash, (index, i));
                }
            }
        }
        let loaded = sources
            .as_ref()
            .and_then(|s| {
                LANGUAGES
                    .iter()
                    .find(|(c, _)| hash(&format!("msg/{c}/sys.bar")) == s.menu.messages.name_hash)
            })
            .map(|(c, _)| c.to_string());
        let languages: Vec<String> = match &loaded {
            Some(_) => LANGUAGES
                .iter()
                .filter(|(c, _)| {
                    [
                        "msg/{l}/sys.bar",
                        "msg/{l}/fontimage.bar",
                        "msg/{l}/fontinfo.bar",
                        "menu/{l}/camp.2ld",
                    ]
                    .iter()
                    .all(|t| names.contains_key(&hash(&t.replace("{l}", c))))
                })
                .map(|(c, _)| c.to_string())
                .collect(),
            None => Vec::new(),
        };
        let loaded = loaded.unwrap_or_default();
        let mut current = path.as_deref().map(Settings::load).unwrap_or_default();
        if !languages.contains(&current.language) {
            current.language = loaded.clone();
        }
        Ok(Self {
            saved: current.clone(),
            current,
            path,
            applied: None,
            languages,
            loaded,
            sources,
            names,
            source,
        })
    }

    /// The language the menus were loaded in (`fallback` when unknown).
    pub fn loaded_or(&self, fallback: &str) -> String {
        if self.loaded.is_empty() {
            fallback.to_owned()
        } else {
            self.loaded.clone()
        }
    }

    /// The selected source the menus are read from.
    pub fn source(&self) -> &ProbeSource {
        &self.source
    }

    /// The chosen language differs from the one the menus are in.
    pub fn language_pending(&self) -> bool {
        !self.languages.is_empty() && self.current.language != self.loaded
    }

    pub fn save_if_changed(&mut self) {
        if let Some(path) = &self.path
            && self.current != self.saved
        {
            match self.current.save(path) {
                Ok(()) => self.saved = self.current.clone(),
                Err(e) => eprintln!("Settings not saved to {}: {e}", path.display()),
            }
        }
    }

    /// `l` in language `to` when it is one of a language's files (`extra`
    /// adds paths to try); otherwise `l` unchanged. A file the install
    /// lacks in `to` stays as configured.
    fn relocate(&self, l: &Locator, to: &str, extra: &[String]) -> Locator {
        let mut templates: Vec<String> = TEMPLATES.iter().map(|t| t.to_string()).collect();
        templates.extend(extra.iter().cloned());
        for w in WORLDS {
            for n in 0..10 {
                templates.push(format!("field2d/{{l}}/{w}{n}command.2dd"));
            }
        }
        for t in &templates {
            for (code, _) in LANGUAGES.iter().chain(&[("uk", "")]) {
                if hash(&t.replace("{l}", code)) != l.name_hash {
                    continue;
                }
                let name_hash = hash(&t.replace("{l}", to));
                return match self.names.get(&name_hash) {
                    Some(&(index, ordinal)) => Locator {
                        index: index.to_owned(),
                        ordinal,
                        name_hash,
                    },
                    None => l.clone(),
                };
            }
        }
        l.clone()
    }

    /// Reloads the camp's (and journal's) files, sheets and the field
    /// command menu in the chosen language.
    pub fn switch_language(
        &mut self,
        c: &mut CampRes,
        hud: Option<&mut Hud>,
        frame: &mut Ui2dFrame,
        images: &mut Assets<Image>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ls = self.sources.as_ref().ok_or("no language sources")?;
        let to = self.current.language.clone();
        let source = &*self.source;
        let m = MenuSources {
            command: self.relocate(&ls.menu.command, &to, &[]),
            font_image: self.relocate(&ls.menu.font_image, &to, &[]),
            font_info: self.relocate(&ls.menu.font_info, &to, &[]),
            messages: self.relocate(&ls.menu.messages, &to, &[]),
        };
        let battle = read_archive(source, &ls.battle)?;
        let system = read_archive(source, &ls.system)?;
        let mut camp = load_camp_source(
            source,
            &self.relocate(&ls.camp, &to, &[]),
            &m,
            &battle,
            &system,
        )?;
        let journal = match &ls.journal {
            Some(j) => {
                let tops = j
                    .tops
                    .iter()
                    .map(|(code, l)| {
                        let extra = [format!("menu/{{l}}/jm_top_{code}.2ld")];
                        (code.clone(), self.relocate(l, &to, &extra))
                    })
                    .collect();
                let j = JournalConfig {
                    book: self.relocate(&j.book, &to, &[]),
                    tables: self.relocate(&j.tables, &to, &[]),
                    messages: self.relocate(&j.messages, &to, &[]),
                    worlds: self.relocate(&j.worlds, &to, &[]),
                    tops,
                    curtain: self.relocate(&j.curtain, &to, &[]),
                    ..j.clone()
                };
                Some(load_journal(source, &j, &m.messages, &system)?)
            }
            None => None,
        };
        let command = match hud.as_ref().and_then(|h| h.menu.as_ref()) {
            Some(_) => Some((
                hud::load(
                    source,
                    &m.command.index,
                    m.command.ordinal,
                    m.command.name_hash,
                    b"comw",
                )?,
                hud::load_system_font(
                    source,
                    (
                        &m.font_image.index,
                        m.font_image.ordinal,
                        m.font_image.name_hash,
                    ),
                    (
                        &m.font_info.index,
                        m.font_info.ordinal,
                        m.font_info.name_hash,
                    ),
                )?,
                hud::load_messages(
                    source,
                    &m.messages.index,
                    m.messages.ordinal,
                    m.messages.name_hash,
                    &MENU_MESSAGES,
                )?,
            )),
            None => None,
        };
        // Everything loaded: swap. Level-ups changed the Status numbers.
        camp.numbers = c.source.numbers;
        let s = &c.journal_sheets;
        let old: Vec<usize> = c
            .art
            .iter()
            .flatten()
            .copied()
            .chain([c.font_sheet, c.icon_sheet])
            .chain(c.journal_source.as_ref().map_or_else(Vec::new, |_| {
                let mut v = vec![s.book, s.curtain];
                v.extend(&s.worlds);
                v.extend(s.tops.values().flatten());
                v.extend(s.photos.values());
                v
            }))
            .chain(c.notice_sheets.values().copied())
            .collect();
        for slot in old {
            images.remove(&frame.sheets[slot].image);
        }
        c.source = camp;
        if journal.is_some() {
            c.journal_source = journal;
        }
        c.art = [None; 4];
        c.journal_sheets = JournalSheets {
            faces: std::mem::take(&mut c.journal_sheets.faces),
            ..Default::default()
        };
        c.notice_sheets.clear();
        c.lang = to.clone();
        upload_camp_sheets(c, frame, images);
        if let (Some(h), Some((sheet, font, labels))) = (hud, command)
            && let Some(ms) = h.menu.as_mut()
        {
            for (item, label) in ms.menu.items.iter_mut().zip(labels) {
                item.label = label;
            }
            ms.sheet = sheet;
            ms.font = font;
            // The world layout is read again in the new language.
            ms.menu.reload(&ms.sheet.sequence);
            ms.layout = None;
            ms.language = to.clone();
            h.canvas.forget();
            h.updated = true;
        }
        self.loaded = to;
        Ok(())
    }

    /// Back to the loaded language (after a failed swap).
    pub fn keep_loaded_language(&mut self) {
        self.current.language = self.loaded.clone();
    }
}

/// The frame cap while the window is in the background (also minimised).
const UNFOCUSED_FPS: u16 = 30;

/// The frame cap for `s`. In the background the app runs at most
/// `UNFOCUSED_FPS`, except in unattended runs, which must not depend on
/// focus.
fn frame_limit(s: &Settings, focused: bool, unattended: bool) -> Option<u16> {
    let cap = s.frame_limit();
    if focused || unattended {
        cap
    } else {
        Some(cap.map_or(UNFOCUSED_FPS, |c| c.min(UNFOCUSED_FPS)))
    }
}

/// The window, the frame rate, the scene camera's multisampling and the
/// mixer's volumes.
pub(super) fn apply_settings(
    mut settings: ResMut<SettingsRes>,
    mut windows: Query<&mut Window>,
    mut limit: ResMut<FrameLimit>,
    mut smoothing: ResMut<Smoothing>,
    mut cameras: Query<(&Camera, &mut Msaa), With<Camera3d>>,
    audio: Res<Audio>,
) {
    let focused = windows.single().map(|w| w.focused).unwrap_or(true);
    let s = settings.current.clone();
    let key = (s.clone(), focused);
    if settings.applied.as_ref() == Some(&key) {
        return;
    }
    if let Ok(mut w) = windows.single_mut() {
        let mode = match s.display {
            1 => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            2 => WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current),
            _ => WindowMode::Windowed,
        };
        if w.mode != mode {
            w.mode = mode;
        }
        let present = if s.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
        if w.present_mode != present {
            w.present_mode = present;
        }
    }
    limit.fps = frame_limit(&s, focused, settings.path.is_none());
    smoothing.on = s.motion_smoothing;
    // The scene camera (order -2); the menu model and snapshot stay plain.
    for (camera, mut msaa) in &mut cameras {
        let want = if s.anti_aliasing {
            Msaa::Sample4
        } else {
            Msaa::Off
        };
        if camera.order == -2 && *msaa != want {
            *msaa = want;
        }
    }
    if let Some(output) = &audio.output
        && let Ok(mut mixer) = output.mixer.lock()
    {
        let quiet = !focused && !s.background_audio;
        let (music, effects) = if quiet {
            (0., 0.)
        } else {
            (s.music_gain(), s.effects_gain())
        };
        mixer.set_gains(music, effects, &MUSIC_KEYS);
    }
    settings.applied = Some(key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_caps_follow_settings_and_focus() {
        let mut s = Settings::default();
        assert_eq!(frame_limit(&s, true, false), None, "V-Sync paces itself");
        assert_eq!(frame_limit(&s, false, false), Some(UNFOCUSED_FPS));
        assert_eq!(
            frame_limit(&s, false, true),
            None,
            "unattended ignores focus"
        );
        s.vsync = false;
        s.frame_cap = 144;
        assert_eq!(frame_limit(&s, true, false), Some(144));
        assert_eq!(frame_limit(&s, false, false), Some(UNFOCUSED_FPS));
        s.frame_cap = 0;
        assert_eq!(frame_limit(&s, true, true), None, "Unlimited");
        s.frame_cap = 30;
        s.custom_frame_cap = true;
        s.custom_cap = 20;
        assert_eq!(frame_limit(&s, false, false), Some(20));
    }
}
