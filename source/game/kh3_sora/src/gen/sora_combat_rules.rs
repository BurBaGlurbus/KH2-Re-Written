// The row type of the reference's sheet sora-combat-rules; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComboRuleRow {
    pub id: &'static str,
    pub order: i32,
    pub phase: &'static str,
    pub condition: &'static str,
    pub result: &'static str,
    pub kind: &'static str,
}

pub static COMBO_RULES: std::sync::LazyLock<Vec<ComboRuleRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-rules").rows().map(|r| ComboRuleRow {
        id: r.text("id"),
        order: r.int("order"),
        phase: r.text("phase"),
        condition: r.text("condition"),
        result: r.text("result"),
        kind: r.text("kind"),
    }).collect()
});
