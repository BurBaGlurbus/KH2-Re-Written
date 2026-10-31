//! The command menu's layout follows the world (FUN_140153ee0): each room
//! names `field2d/<lang>/<world><n>command.2dd` through its arif digit, and
//! a new name loads that file's 'comw' sheet and reloads the menu without
//! its intro (FUN_14015db50). The `--command-deck-tour` steps through every
//! installed layout instead.
use super::*;
use data::path_lookup::Library;

/// Loads a layout's sheet.
fn load(source: &ProbeSource, found: &data::path_lookup::Found) -> Result<Sheet, String> {
    hud::load(source, found.index, found.ordinal, found.name_hash, b"comw")
        .map_err(|e| e.to_string())
}

fn install(game: &mut Game, m: &mut MenuState, name: String, sheet: Result<Sheet, String>) {
    match sheet {
        Ok(sheet) => {
            m.sheet = sheet;
            m.menu.reload(&m.sheet.sequence);
            game.event(
                serde_json::json!({"event":"command_layout","tick":game.tick,
                "file":name,"groups":m.sheet.sequence.groups.len()}),
            );
        }
        Err(e) => {
            game.event(
                serde_json::json!({"event":"command_layout_error","tick":game.tick,
                "file":name,"error":e}),
            );
        }
    }
    // FUN_140153ee0 -> FUN_1401534b0: the world menus (config bit 6 clear,
    // FUN_1402e5380) keep the layout's style for the reaction-style labels.
    m.menu.layout_style = (game.config_flags >> 6 & 1 == 0)
        .then(|| visual::field_command_menu::layout_style(&name, true));
    m.layout = Some(name);
}

pub fn follow(
    mut game: ResMut<Game>,
    area: Option<Res<AreaRes>>,
    hud: Option<ResMut<Hud>>,
    settings: Option<Res<settings_room::SettingsRes>>,
    mut library: Local<Option<Library>>,
) {
    let Some(mut hud) = hud else { return };
    let Some(m) = hud.menu.as_mut() else { return };
    if game.deck_tour {
        // The tour's own layouts, from the selected source.
        let Some(code) = deck_tour::layout_at(game.tick) else {
            return;
        };
        let Some(settings) = settings else { return };
        let source = settings.source();
        if library.is_none() {
            *library = Library::open(source).ok();
        }
        let name = format!("field2d/{}/{code}command.2dd", m.language);
        let sheet = match library.as_ref().and_then(|l| l.find(&name)) {
            Some(found) => load(source, &found),
            None => Err("not installed".into()),
        };
        install(&mut game, m, name, sheet);
        hud.updated = true;
        return;
    }
    let Some(area) = area else { return };
    let (world, number) = (area.current.world, area.current.area);
    let name = area
        .source
        .command_layout(world, number, &m.language, false);
    if m.layout.as_deref() == Some(name.as_str()) {
        return;
    }
    let sheet = match area.source.find(&name) {
        Some((found, source)) => load(source, &found),
        None => Err("not installed".into()),
    };
    install(&mut game, m, name, sheet);
    hud.updated = true;
}
