//! What a player chose last time, kept between duels [duel choice]: the
//! host's rules and this player's build (Keyblade, abilities, items), in
//! `duel-prefs.json` beside the source manifest (the player's own settings,
//! no game data). Each is checked against the tables when it is read back:
//! a value they no longer allow is dropped. Unattended runs neither read
//! nor write it, so their runs stay the same.

use super::duel_arena::ARENAS;
use super::duel_lobby::{Limits, Loadout};
use super::duel_rules::{self, MODES, MatchRules};
use super::*;
use serde_json::{Value, json};

#[derive(Resource, Default)]
pub(crate) struct DuelPrefs {
    /// None: not kept (an unattended run).
    path: Option<PathBuf>,
    pub rules: Option<MatchRules>,
    pub build: Option<Loadout>,
}

impl DuelPrefs {
    /// The file beside `manifest`, read if it is there.
    pub(crate) fn load(manifest: &Path, unattended: bool) -> Self {
        if unattended {
            return Self::default();
        }
        let path = manifest.with_file_name("duel-prefs.json");
        let v: Value = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        DuelPrefs {
            rules: rules_from(&v["rules"]),
            build: build_from(&v["build"]),
            path: Some(path),
        }
    }

    pub(crate) fn save_rules(&mut self, rules: MatchRules) {
        if self.rules != Some(rules) {
            self.rules = Some(rules);
            self.write();
        }
    }

    pub(crate) fn save_build(&mut self, build: &Loadout) {
        if self.build.as_ref() != Some(build) {
            self.build = Some(build.clone());
            self.write();
        }
    }

    fn write(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let v = json!({
            "rules": self.rules.map(|r| rules_json(&r)),
            "build": self.build.as_ref().map(|b| json!({
                "keyblade": b.keyblade, "abilities": b.abilities, "items": b.items,
            })),
        });
        if let Ok(text) = serde_json::to_string_pretty(&v)
            && let Err(e) = std::fs::write(path, text)
        {
            eprintln!("duel prefs not saved: {e}");
        }
    }
}

fn rules_json(r: &MatchRules) -> Value {
    json!({
        "mode": r.info().name,
        "damage": r.damage,
        "magic": r.magic,
        "hit_cap": r.hit_cap,
        "time_limit": r.time_limit,
        "arena": ARENAS.get(usize::from(r.arena)).map(|a| a.key),
        "ruleset": r.ruleset_name(),
    })
}

/// Rules read back; each value the tables do not allow keeps its default.
fn rules_from(v: &Value) -> Option<MatchRules> {
    if !v.is_object() {
        return None;
    }
    let mut r = MatchRules::default();
    if let Some(m) = MODES.iter().find(|m| v["mode"].as_str() == Some(m.name)) {
        r.mode = m.mode;
    }
    let (lo, hi) = duel_rules::DAMAGE_RANGE;
    let multiplier = |x: &Value| {
        x.as_f64()
            .map(|d| d as f32)
            .filter(|d| (lo..=hi).contains(d))
    };
    if let Some(d) = multiplier(&v["damage"]) {
        r.damage = d;
    }
    if let Some(d) = multiplier(&v["magic"]) {
        r.magic = d;
    }
    if let Some(c) = v["hit_cap"].as_u64().and_then(|c| u8::try_from(c).ok())
        && duel_rules::HIT_CAPS.contains(&c)
    {
        r.hit_cap = c;
    }
    if let Some(t) = v["time_limit"].as_u64().and_then(|t| u16::try_from(t).ok())
        && duel_rules::TIME_LIMITS.contains(&t)
    {
        r.time_limit = t;
    }
    if let Some(i) = ARENAS
        .iter()
        .position(|a| v["arena"].as_str() == Some(a.key))
    {
        r.arena = i as u8;
    }
    if let Some(&(ruleset, ..)) = duel_rules::RULESETS
        .iter()
        .find(|x| v["ruleset"].as_str() == Some(x.1))
    {
        r.ruleset = ruleset;
    }
    Some(r)
}

fn build_from(v: &Value) -> Option<Loadout> {
    let ids = |x: &Value| -> Option<Vec<u16>> {
        x.as_array()?
            .iter()
            .map(|n| n.as_u64().and_then(|n| u16::try_from(n).ok()))
            .collect()
    };
    Some(Loadout {
        keyblade: u16::try_from(v["keyblade"].as_u64()?).ok()?,
        abilities: ids(&v["abilities"])?,
        items: ids(&v["items"])?,
    })
}

/// A kept build made to fit this duel: a Keyblade it has (else `current`),
/// its abilities in their saved order and state as far as the duel has them
/// (the rest after, unequipped), items it allows in its slots.
pub(crate) fn fit(saved: &Loadout, limits: &Limits, current: &Loadout) -> Loadout {
    let keyblade = if limits.keyblades.contains(&saved.keyblade) {
        saved.keyblade
    } else {
        current.keyblade
    };
    let mut pool: Vec<u16> = limits.abilities.iter().map(|a| a & 0x7fff).collect();
    let mut abilities = Vec::new();
    for &s in &saved.abilities {
        if let Some(i) = pool.iter().position(|&p| p == s & 0x7fff) {
            pool.remove(i);
            abilities.push(s);
        }
    }
    abilities.extend(pool);
    let mut items: Vec<u16> = saved
        .items
        .iter()
        .map(|i| if limits.items.contains(i) { *i } else { 0 })
        .collect();
    items.resize(limits.item_slots, 0);
    Loadout {
        keyblade,
        abilities,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_round_trip_and_bad_values_keep_defaults() {
        let r = MatchRules {
            mode: duel_rules::Mode::BestOf3,
            damage: 4.5,
            hit_cap: 75,
            time_limit: 90,
            arena: 1,
            ruleset: duel_rules::Ruleset::Kh3,
            ..MatchRules::default()
        };
        assert_eq!(rules_from(&rules_json(&r)), Some(r));
        let bad =
            json!({"mode": "?", "damage": 99.0, "hit_cap": 41, "time_limit": 7, "arena": "x"});
        assert_eq!(rules_from(&bad), Some(MatchRules::default()));
        assert_eq!(rules_from(&Value::Null), None);
    }

    #[test]
    fn a_kept_build_fits_the_duel() {
        let limits = Limits {
            abilities: vec![10, 11, 11, 12],
            keyblades: vec![0x29, 0x2a],
            items: vec![1, 2],
            item_slots: 3,
            ..Limits::default()
        };
        let current = Loadout {
            keyblade: 0x29,
            ..Loadout::default()
        };
        let saved = Loadout {
            keyblade: 0x99,
            abilities: vec![0x8000 | 11, 13, 0x8000 | 11, 0x8000 | 11],
            items: vec![2, 5, 1, 1],
        };
        let b = fit(&saved, &limits, &current);
        assert_eq!(b.keyblade, 0x29);
        assert_eq!(b.abilities, vec![0x8000 | 11, 0x8000 | 11, 10, 12]);
        assert_eq!(b.items, vec![2, 0, 1]);
        assert_eq!(
            build_from(&json!({"keyblade": 1, "abilities": [2], "items": [3]})).map(|b| b.items),
            Some(vec![3])
        );
    }
}
