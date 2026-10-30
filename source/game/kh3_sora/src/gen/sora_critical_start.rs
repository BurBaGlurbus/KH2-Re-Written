// The row type of the reference's sheet sora-critical-start; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CriticalStartRow {
    pub id: &'static str,
    pub group: &'static str,
    pub ability: &'static str,
    pub mode: &'static str,
    pub ap: i32,
    pub order: i32,
    pub port: &'static str,
}

pub static CRITICAL_START: std::sync::LazyLock<Vec<CriticalStartRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-critical-start").rows().map(|r| CriticalStartRow {
        id: r.text("id"),
        group: r.text("group"),
        ability: r.text("ability"),
        mode: r.text("mode"),
        ap: r.int("ap"),
        order: r.int("order"),
        port: r.text("port"),
    }).collect()
});
