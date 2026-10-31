//! The L1 shortcut menu (menu 5): the four rows FUN_1403f29e0 fills every
//! frame from the Customize slots, and FUN_1403f2470 putting it on the
//! command stack while L1 is held. `field_command_menu` draws it
//! (FUN_14015df90).
use super::{Game, deck_tour};
use visual::field_command_menu::{CommandMenu, Item, SHORTCUT_DECK, SHORTCUT_SELECTORS};

/// A slot's command as the menu shows it (FUN_1403f29e0, FUN_1403b0f20) and
/// its colour index: an item names its own slot's command (6 when no slot
/// holds it, 5 in the air: FUN_1403b6cf0); in Limit Form any other slot is that
/// form's limit (0x1405c9678); a spell shows the tier its level has reached
/// (FUN_1403b68b0, FUN_1403c5900), with FUN_140401d60's colour for its cost.
/// FUN_1403b6cb0 greys what the Drive form masks.
fn slot(game: &Game, k: usize) -> (u16, usize) {
    let Some(mg) = game.magic.as_ref() else {
        return (0, 5);
    };
    let cmd = game.shortcuts[k];
    let record = mg.src.commands.get(&cmd);
    let drive = game.drive.as_ref();
    let (shown, colour) = match record {
        None => (0, 5),
        Some(c) if c.execute == 3 => {
            let held = game
                .items
                .as_ref()
                .is_some_and(|it| it.slot_for_command(cmd).is_some());
            let colour = if !held {
                6
            } else if game.sora.player.in_air() {
                5
            } else {
                0
            };
            (cmd, colour)
        }
        Some(_) if drive.is_some_and(|d| d.limit_form()) => {
            let (mp, charging) = (game.sora.player.mp.cur, game.sora.player.mp_charging);
            let row = drive.and_then(|d| d.limit_rows(mp, charging).get(k).copied());
            row.unwrap_or((0, 5))
        }
        Some(c) if c.execute == 2 => {
            let t = c.argument as u8;
            match mg.current(t) {
                Some(r) => (r.command, mg.colour(t, &game.sora.player.mp).unwrap_or(5)),
                None => (0, 5),
            }
        }
        // Command 0 and anything else has no use from here.
        Some(_) => (cmd, 5),
    };
    let colour = if drive.is_some_and(|d| d.masked(shown)) {
        5
    } else {
        colour
    };
    (shown, colour)
}

/// The menu's four rows.
pub fn rows(game: &Game) -> Vec<Item> {
    let Some(mg) = game.magic.as_ref() else {
        return Vec::new();
    };
    (0..game.shortcuts.len())
        .map(|k| {
            let (cmd, colour) = slot(game, k);
            let names = game
                .drive
                .as_ref()
                .filter(|d| d.limit_form())
                .and_then(|d| d.src.names.get(&cmd));
            Item {
                label: names
                    .or_else(|| mg.src.names.get(&cmd))
                    .cloned()
                    .unwrap_or_default(),
                icon: mg.src.commands.get(&cmd).map_or(0, |c| c.icon),
                colour,
            }
        })
        .collect()
}

/// FUN_1403f2470: with L1 held the stack holds only menu 5, so an open list
/// closes with no sound; its release puts the deck back with the cursor on the
/// first row (FUN_1403b1b70). `free`: nothing else owns the deck.
pub fn update(game: &mut Game, menu: &mut CommandMenu, was_up: bool, free: bool) {
    let tour = game.deck_tour;
    let held = if tour {
        deck_tour::shortcut_held(game.tick)
    } else {
        game.pad.held & 0x80_0000 != 0
    };
    if tour && deck_tour::shortcut_press(game.tick) {
        game.shortcut_queued |= 1;
    }
    if held && free {
        if !was_up {
            menu.close_sub();
            game.cure_pick = None;
        }
        menu.shortcut = rows(game);
        menu.list = Some(SHORTCUT_SELECTORS.to_vec());
        menu.list_mode = SHORTCUT_DECK;
    } else if was_up {
        menu.cursor = 0;
    }
}
