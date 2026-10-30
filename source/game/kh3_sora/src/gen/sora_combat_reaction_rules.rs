// The row type of the reference's sheet sora-combat-reaction-rules; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReactionRuleRow {
    pub id: &'static str,
    pub body_type: &'static str,
    pub condition: &'static str,
    pub victim_result: &'static str,
    pub attacker_result: &'static str,
    pub no_react: bool,
    pub recorded: bool,
    pub clear_bad_status: bool,
}

pub static REACTION_RULES: std::sync::LazyLock<Vec<ReactionRuleRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-reaction-rules").rows().map(|r| ReactionRuleRow {
        id: r.text("id"),
        body_type: r.text("body_type"),
        condition: r.text("condition"),
        victim_result: r.text("victim_result"),
        attacker_result: r.text("attacker_result"),
        no_react: r.yesno("no_react"),
        recorded: r.yesno("recorded"),
        clear_bad_status: r.yesno("clear_bad_status"),
    }).collect()
});
