//! Bounded renderer check. Source action candidates and motion ordinals are inspection samples;
//! this does not execute the enemy scripts or assert their action semantics.
use super::*;

/// The field stops when the debug menu opens at `END`, so the menu's pages
/// are captured at that same tick, one per capture after `FIRST_PAGE_CAPTURE`.
pub const CAPTURES: &[u64] = &[
    50, 90, 130, 170, 210, 250, 280, 280, 280, 280, 280, 280, 280, 280,
];
pub const END: u64 = 280;
pub const FIRST_PAGE_CAPTURE: usize = 6;
pub const MODELS: &[&str] = &["M_EX020", "M_EX990", "M_EX350_04", "M_EX760_RAW"];
const DUSK_ACTIONS: &[&[u8]] = &[b"atk_craw", b"atk_kick", b"atk_walk"];

fn action_sample(kind: &enemy_catalog::Kind, name: &[u8]) -> Option<usize> {
    if kind.variants {
        return None;
    }
    kind.actions
        .iter()
        .filter(|a| a.name == name && a.callback_error.is_none())
        .flat_map(|a| &a.motions)
        // Select an exposed change-motion candidate; other paths stay visible
        // in the browser. This does not execute or choose a native action path.
        .filter(|r| r.trap == 12)
        .find_map(|r| {
            kind.motions
                .iter()
                .position(|m| m.slot as u32 == r.logical_motion && m.status.is_none())
        })
}

pub fn tick(
    mut game: ResMut<Game>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
    mut debug: ResMut<debug_menu::DebugMenu>,
) {
    if !game.enemy_tour {
        return;
    }
    let tick = game.tick;
    if tick == 5 {
        for (preferred, x) in MODELS.iter().zip([-350., -150., 150., 350.]) {
            let found = gallery.catalog.entries.iter().position(|e| {
                e.model_name == *preferred && e.model_available && e.motions_available
            });
            if let Some(index) = found {
                let at = Vec3::from_array(game.player.position) + Vec3::new(x, 0., 350.);
                gallery.queue_summon(index, at, std::f32::consts::PI);
            } else {
                game.autoplay_failed = true;
            }
        }
    }
    if let Some(sample) = [30, 110, 190].iter().position(|&at| at == tick) {
        let choices: Vec<_> = gallery
            .actors
            .iter()
            .map(|a| {
                let dusk = gallery.catalog.entries[a.entry].model_name == "M_EX990";
                (
                    a.id,
                    if dusk {
                        action_sample(&a.kind, DUSK_ACTIONS[sample])
                    } else {
                        a.kind
                            .motions
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| m.status.is_none())
                            .nth(sample)
                            .map(|(i, _)| i)
                    },
                    dusk.then(|| String::from_utf8_lossy(DUSK_ACTIONS[sample]).into_owned()),
                )
            })
            .collect();
        for (id, choice, source_action) in choices {
            if let Some(choice) = choice {
                gallery.select(id);
                gallery.queue_motion(choice);
                game.event(serde_json::json!({"event":"enemy_tour_sample","tick":tick,
                    "actor":id,"choice":choice,"source_action_candidate":source_action,
                    "semantic_action":"NOT QUALIFIED"}));
            } else {
                game.autoplay_failed = true;
            }
        }
    }
    if tick == END {
        debug.show_enemy_actors();
    }
}

/// The menu page for the capture about to be taken: the summoned list, then
/// the preview's Playback, Motions, Actions and View tabs, then (menu closed)
/// the skeleton over the model and the skeleton alone.
pub fn show_page(
    capture: usize,
    debug: &mut debug_menu::DebugMenu,
    gallery: &mut enemy_gallery::EnemyGallery,
) {
    let Some(step) = capture.checked_sub(FIRST_PAGE_CAPTURE).filter(|&s| s > 0) else {
        return;
    };
    if let Some(first) = gallery.actors.first().map(|a| a.id) {
        gallery.select(first);
    }
    gallery.overlays.skeleton = step >= 4;
    gallery.overlays.hide_model = step >= 6;
    if step >= 5 {
        debug.open = false;
    } else {
        // The Actions tab (step 3) shows a typed search.
        debug.show_preview(step - 1, if step == 3 { "move" } else { "" });
    }
}
