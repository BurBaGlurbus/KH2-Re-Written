// The row type of the reference's sheet sora-equipment; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquipmentRow {
    pub id: &'static str,
    pub kind: &'static str,
    pub name: &'static str,
    pub ap: i32,
    pub attack: i32,
    pub magic: i32,
    pub defense: i32,
    pub resist: &'static [i32],
    pub abilities: &'static [&'static str],
}

pub static EQUIPMENT: std::sync::LazyLock<Vec<EquipmentRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-equipment").rows().map(|r| EquipmentRow {
        id: r.text("id"),
        kind: r.text("kind"),
        name: r.text("name"),
        ap: r.int("ap"),
        attack: r.int("attack"),
        magic: r.int("magic"),
        defense: r.int("defense"),
        resist: r.ints("resist"),
        abilities: r.texts("abilities"),
    }).collect()
});
