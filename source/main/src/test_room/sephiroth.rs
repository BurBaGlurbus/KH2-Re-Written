//! The Sephiroth fight: objentry 2230 (`N_HB630`), placed by spawn set `b_60`
//! of Hollow Bastion area 1 (`ard/us/hb01.ard`). The boss runs from his own
//! object script (`enemy_script`); this module only puts him in the room the
//! way the area script does and watches the fight end.
use super::*;
use enemy_gallery::EnemyGallery;

/// The summon waits for the room's first ticks to settle.
const START_TICK: u64 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Stage {
    #[default]
    Waiting,
    Summoned,
    Fighting,
    Won,
}

/// Ticks between the boss's last blow and the room closing for the outro.
const OUTRO_DELAY: u64 = 240;

#[derive(Resource, Default)]
pub struct Fight {
    stage: Stage,
    actor: Option<u64>,
    won_at: Option<u64>,
}

pub fn tick(
    mut game: ResMut<Game>,
    mut gallery: ResMut<EnemyGallery>,
    area: Option<Res<AreaRes>>,
    mut fight: ResMut<Fight>,
    mut exit: MessageWriter<AppExit>,
) {
    if !game.sephiroth {
        return;
    }
    let tick = game.tick;
    match fight.stage {
        Stage::Waiting if tick >= START_TICK => {
            let Some(entry) = gallery
                .catalog
                .entries
                .iter()
                .position(|e| e.object_id == enemy_battle::SEPHIROTH)
            else {
                game.autoplay_failed = true;
                game.event(serde_json::json!({"event":"sephiroth_missing","tick":tick}));
                fight.stage = Stage::Won;
                return;
            };
            // The area's own placement (scene coordinates, scene yaw), else
            // the flat development arena: in front of Sora.
            let placed = area.as_ref().and_then(|a| {
                a.current
                    .spawns
                    .iter()
                    .find(|s| s.entity.object == enemy_battle::SEPHIROTH)
                    .map(|s| {
                        let [x, y, z] = s.entity.position;
                        (Vec3::new(x, -y, -z), s.yaw)
                    })
            });
            let (at, yaw) = placed.unwrap_or_else(|| {
                (
                    Vec3::from_array(game.player.position) + Vec3::new(0., 0., 750.),
                    std::f32::consts::PI,
                )
            });
            gallery.queue_summon(entry, at, yaw);
            game.event(serde_json::json!({"event":"sephiroth_summon","tick":tick,
                "position":at.to_array(),"yaw":yaw,"from_area":placed.is_some()}));
            fight.stage = Stage::Summoned;
        }
        Stage::Summoned => {
            if let Some(id) = gallery.actors.iter().map(|a| a.id).next() {
                gallery.select(id);
                gallery.queue_battle();
                fight.actor = Some(id);
                fight.stage = Stage::Fighting;
                game.event(serde_json::json!({"event":"sephiroth_enable","tick":tick,"actor":id}));
            }
        }
        Stage::Fighting => {
            let alive = gallery
                .actors
                .iter()
                .find(|a| Some(a.id) == fight.actor)
                .and_then(|a| a.battle.as_ref())
                .map(|b| b.brain.alive());
            if alive == Some(false) {
                fight.stage = Stage::Won;
                fight.won_at = Some(tick);
                game.event(serde_json::json!({"event":"sephiroth_defeated","tick":tick}));
            }
        }
        Stage::Won => {
            if let (Some(at), Some(file)) = (fight.won_at, game.sephiroth_result.clone())
                && tick >= at + OUTRO_DELAY
            {
                fight.won_at = None;
                match std::fs::write(&file, b"won") {
                    Ok(()) => {
                        exit.write(AppExit::Success);
                    }
                    Err(e) => {
                        game.autoplay_failed = true;
                        game.event(serde_json::json!({"event":"sephiroth_result_failed","reason":e.to_string()}));
                    }
                }
            }
        }
        _ => {}
    }
}
