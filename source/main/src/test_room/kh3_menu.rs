//! The KH3 ruleset's ability menu [T279]: KH3's abilities with their AP
//! (`player::kh3::ability`), on the camp's Config screen as the duel's own
//! pages are, seven pages of them (movement, actions, attacks, combo and
//! so on) so
//! each fits the screen. The lobby shows them while the builds are made
//! (duel_lobby_room.rs); Test KH3 mode opens them on START. What is
//! equipped gates the moves at once (the layer reads `Kh3::abilities`).

use super::duel_net::DuelNet;
use super::*;
use player::kh3::ability::{ALL, AP_MAX, Set};
use visual::pause_menu::{DuelPages, DuelRow};

/// The camp screen id (duel_camp.rs's LOBBY, VOTE, RESULT, FORFEIT are 1-4).
pub(crate) const SCREEN: u8 = 5;
/// The top level's entries: the pages (the Config entry's id) and Close.
const ENTRY_PAGES: u8 = 9;
const ENTRY_CLOSE: u8 = pause_menu::CALLER_ENTRY + 1;
/// se000: open, refused.
const SE_OPEN: u32 = 3;
const SE_ERROR: u32 = 5;
/// How long a refusal stays in the help line.
const NOTE_TICKS: u64 = 300;

/// The pages: name and how many of `ALL` (in its order) each lists after
/// the AP row (eight fit the Config screen's panel).
pub(crate) const PAGES: [(&str, usize); 9] = [
    ("KH3 Movement", 8),
    ("KH3 Actions", 8),
    ("KH3 Attacks", 7),
    ("KH3 Combo", 5),
    ("KH3 Swings", 8),
    ("KH3 Counters", 4),
    ("KH3 Magic", 6),
    ("KH3 Survival", 8),
    ("KH3 System", 8),
];

/// Where page `page` starts in `ALL`.
fn first(page: usize) -> usize {
    PAGES[..page].iter().map(|p| p.1).sum()
}

/// The pages: names and rows (each: the AP used, then its abilities with
/// their copies).
pub(crate) fn pages(set: Set, note: Option<&str>) -> Vec<(Vec<u8>, Vec<DuelRow>)> {
    let text = pause_menu::encode_text;
    let help = |h: &str| duel_camp::help_text(note.unwrap_or(h));
    PAGES
        .iter()
        .enumerate()
        .map(|(p, &(name, n))| {
            let mut rows = vec![DuelRow {
                label: text("AP"),
                choices: vec![text(&format!("{} / {AP_MAX}", set.ap()))],
                selected: 0,
                help: help("The AP your KH3 abilities use. Left and Right equip one; R2 and L2 turn the page."),
                locked: true,
            }];
            for i in &ALL[first(p)..first(p) + n] {
                let choices = if i.most == 1 {
                    vec![text("Off"), text("On")]
                } else {
                    std::iter::once(text("Off"))
                        .chain((1..=i.most).map(|c| text(&format!("x{c}"))))
                        .collect()
                };
                let each = if i.most > 1 { " each" } else { "" };
                rows.push(DuelRow {
                    label: text(i.name),
                    choices,
                    selected: usize::from(set.count(i.ability)),
                    help: help(&format!("{} {} AP{each}.", i.help, i.ap)),
                    locked: false,
                });
            }
            (text(name), rows)
        })
        .collect()
}

/// Left / Right on row `row` of page `page` (row 0 is the AP): None for a
/// row that is not an ability, else whether the AP allowed it.
pub(crate) fn step(
    set: &mut Set,
    page: usize,
    row: usize,
    forward: bool,
) -> Option<Result<(), String>> {
    let (_, n) = *PAGES.get(page)?;
    let r = row.checked_sub(1).filter(|&r| r < n)?;
    let info = &ALL[first(page) + r];
    Some(if set.step(info.ability, forward, AP_MAX) {
        Ok(())
    } else {
        Err(format!(
            "Not enough AP for {}: unequip something first.",
            info.name
        ))
    })
}

/// The note shown in the help lines, while it is fresh.
fn fresh(note: &Option<(String, u64)>, now: u64) -> Option<String> {
    note.as_ref()
        .filter(|(_, at)| now.saturating_sub(*at) < NOTE_TICKS)
        .map(|(n, _)| n.clone())
}

/// Test KH3 mode's menu: START opens it on the KH3 pages; Close, or Cancel
/// on its top level, shuts it. Not in a LAN duel (the lobby has the pages).
pub(crate) fn tick(
    net: Option<Res<DuelNet>>,
    camp: Option<ResMut<CampRes>>,
    mut note: Local<Option<(String, u64)>>,
    mut script_at: Local<usize>,
    mut game: ResMut<Game>,
) {
    if net.is_some() || !duel_kh3::on(&game) {
        return;
    }
    let Some(mut camp) = camp else {
        return;
    };
    let game = &mut *game;
    script(game, &mut script_at);
    if std::mem::take(&mut game.duel_start) && game.kh3_menu.is_none() {
        game.kh3_menu = Some(game.tick as u32);
        game.sound_queue.push((SE_OPEN, "kh3_menu"));
        let tick = game.tick;
        game.event(serde_json::json!({"event":"kh3_menu","tick":tick,"open":true}));
    }
    let set = game.sora.kh3.abilities;
    let (names, rows): (Vec<_>, Vec<_>) = pages(set, fresh(&note, game.tick).as_deref())
        .into_iter()
        .unzip();
    let want = game.kh3_menu;
    let menu = duel_camp::hold(&mut camp, game, SCREEN, want, || duel_camp::Request {
        entries: vec![
            duel_camp::entry(
                ENTRY_PAGES,
                "KH3 Abilities",
                "Equip KH3 abilities: they take effect at once.",
            ),
            duel_camp::entry(ENTRY_CLOSE, "Close", "Back to the test."),
        ],
        on_config: true,
        pages: DuelPages {
            names,
            rows,
            ..DuelPages::default()
        },
    });
    let Some(menu) = menu else {
        return;
    };
    let close = duel_camp::picked(menu) == Some(ENTRY_CLOSE)
        || menu
            .data
            .duel
            .as_mut()
            .is_some_and(|d| std::mem::take(&mut d.cancel));
    let steps = menu
        .data
        .duel
        .as_mut()
        .map(|d| std::mem::take(&mut d.steps))
        .unwrap_or_default();
    let mut set = game.sora.kh3.abilities;
    for (page, row, forward) in steps {
        match step(&mut set, page, row, forward) {
            Some(Err(why)) => {
                *note = Some((why, game.tick));
                game.sound_queue.push((SE_ERROR, "kh3_menu"));
            }
            Some(Ok(())) => *note = None,
            None => {}
        }
    }
    if set != game.sora.kh3.abilities {
        game.sora.kh3.abilities = set;
        let tick = game.tick;
        game.event(abilities_event(set, tick));
    }
    if let Some(d) = menu.data.duel.as_mut() {
        d.rows = pages(set, fresh(&note, game.tick).as_deref())
            .into_iter()
            .map(|p| p.1)
            .collect();
    }
    if close {
        game.kh3_menu = None;
        let tick = game.tick;
        game.event(serde_json::json!({"event":"kh3_menu","tick":tick,"open":false}));
    }
}

/// Camp pad bits: START in the field's mode; Down, Right, Cancel, R2 in
/// the menu's (mode 7).
const PRESS_START: u64 = 0x2;
const PRESS_DOWN: u64 = 0x20;
const PRESS_RIGHT: u64 = 0x80;
const PRESS_CANCEL: u64 = 0x8;
const PRESS_R2: u64 = 0x1_0000;
/// [Test] When the unattended menu opens: past the KH3 script's acting
/// half (rival::KH3_SECOND), while Sora stands.
pub(crate) const SCRIPT_OPEN: u64 = rival::KH3_SECOND + 120;
/// [Test] When the unattended run locks on to the dummy.
const SCRIPT_LOCK: u64 = 90;
/// [Test] The unattended menu's presses (ticks after `SCRIPT_OPEN`; the
/// clock runs two a frame while the camp is open, so each press is taken
/// once its time has come): START, down to Dodge Roll, off and on again,
/// the Combo page, down to Combo Plus, x1, Cancel twice.
const SCRIPT: [(u64, u64); 10] = [
    (0, PRESS_START),
    (200, PRESS_DOWN),
    (260, PRESS_RIGHT),
    (360, PRESS_RIGHT),
    (420, PRESS_R2),
    (460, PRESS_R2),
    (500, PRESS_R2),
    (560, PRESS_DOWN),
    (620, PRESS_RIGHT),
    (760, PRESS_CANCEL),
];

/// The log's line for a change: the set is wider than a JSON number.
fn abilities_event(set: Set, tick: u64) -> serde_json::Value {
    serde_json::json!({"event":"kh3_abilities","tick":tick,
        "set":set.0.to_string(),"ap":set.ap()})
}

/// [Test] Unattended Test KH3 mode (`--kh3-test --autoplay`): the menu's
/// presses, on the camp's pad as the lobby's script's are; a second Cancel
/// after the first shuts the menu.
fn script(game: &mut Game, at: &mut usize) {
    if game.autoplay.is_none() || !game.kh3_dummy {
        return;
    }
    // The lock-on, for the KH3 lock-on camera, once the dummy has risen.
    if game.tick == SCRIPT_LOCK {
        game.lock_queued = true;
    }
    let Some(t) = game.tick.checked_sub(SCRIPT_OPEN) else {
        return;
    };
    if let Some(&(when, key)) = SCRIPT.get(*at)
        && t >= when
    {
        game.duel_camp_press = key;
        *at += 1;
    } else if *at == SCRIPT.len() && t >= 820 && game.kh3_menu.is_some() {
        game.duel_camp_press = PRESS_CANCEL;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use player::kh3::ability::Ability;

    #[test]
    fn every_ability_equips_and_logs() {
        let mut all = Set::NONE;
        for info in player::kh3::ability::ALL.iter() {
            let mut one = Set::NONE;
            one.put(info.ability, info.most);
            assert!(one.has(info.ability) && one.valid(), "{}", info.name);
            assert_eq!(abilities_event(one, 1)["set"], one.0.to_string());
            all.put(info.ability, info.most);
        }
        // The whole list at once, past the top of a JSON number.
        assert!(all.0 > u128::from(u64::MAX));
        assert_eq!(abilities_event(all, 1)["set"], all.0.to_string());
    }

    #[test]
    fn the_pages_list_every_ability_once_after_the_ap() {
        assert_eq!(PAGES.iter().map(|p| p.1).sum::<usize>(), ALL.len());
        let set = Set::default();
        let p = pages(set, None);
        assert_eq!(p.len(), PAGES.len());
        assert!(p.iter().all(|(_, r)| r[0].locked && r.len() <= 9));
        let combo = &p[3].1;
        assert_eq!(combo[1].choices.len(), 4, "Combo Plus: Off, x1-x3");
        assert_eq!(combo[1].selected, 0);
        assert_eq!(p[0].1[1].selected, 1, "Dodge Roll is on by default");
        // A refusal stands in every help line.
        let n = pages(set, Some("No."));
        assert!(
            n.iter()
                .all(|(_, r)| r.iter().all(|r| r.help == duel_camp::help_text("No.")))
        );
    }

    #[test]
    fn steps_change_the_set_and_refuse_past_the_ap() {
        let mut set = Set::NONE;
        assert_eq!(step(&mut set, 0, 0, true), None, "the AP row");
        assert_eq!(step(&mut set, 0, 9, true), None);
        assert_eq!(step(&mut set, 9, 1, true), None);
        assert_eq!(step(&mut set, 0, 1, true), Some(Ok(())));
        assert!(set.has(ALL[0].ability));
        assert_eq!(step(&mut set, 3, 1, true), Some(Ok(())));
        assert_eq!(set.count(Ability::ComboPlus), 1);
        let mut full = Set::NONE;
        for i in ALL {
            full.put(i.ability, i.most);
        }
        // Nothing can be added once over; taking off still works.
        full.put(Ability::ComboBoost, 0);
        assert!(matches!(step(&mut full, 3, 4, true), Some(Err(_))));
        assert_eq!(step(&mut full, 0, 1, true), Some(Ok(())));
    }
}
