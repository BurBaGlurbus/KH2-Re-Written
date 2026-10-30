// The row type of the reference's sheet sora-combat-form-abilities; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormAbilityRow {
    pub id: &'static str,
    pub wear_form: &'static str,
    pub ability: &'static str,
    pub ability_id: i32,
    pub copies: i32,
}

pub static FORM_ABILITIES: std::sync::LazyLock<Vec<FormAbilityRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-form-abilities").rows().map(|r| FormAbilityRow {
        id: r.text("id"),
        wear_form: r.text("wear_form"),
        ability: r.text("ability"),
        ability_id: r.int("ability_id"),
        copies: r.int("copies"),
    }).collect()
});
