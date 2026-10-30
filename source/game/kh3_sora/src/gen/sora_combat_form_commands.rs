// The row type of the reference's sheet sora-combat-form-commands; rows come from the player's files at run time (crate::sheets).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandRow {
    pub id: &'static str,
    pub kind: i32,
    pub cmd_type: i32,
    pub cate: i32,
    pub attr: i32,
    pub param0: f32,
    pub param1: f32,
    pub param3: f32,
}

pub static COMMAND_ROWS: std::sync::LazyLock<Vec<CommandRow>> = std::sync::LazyLock::new(|| {
    crate::sheets::Table::named("sora-combat-form-commands").rows().map(|r| CommandRow {
        id: r.text("id"),
        kind: r.int("kind"),
        cmd_type: r.int("Type"),
        cate: r.int("Cate"),
        attr: r.int("Attr"),
        param0: r.num("Param0"),
        param1: r.num("Param1"),
        param3: r.num("Param3"),
    }).collect()
});
