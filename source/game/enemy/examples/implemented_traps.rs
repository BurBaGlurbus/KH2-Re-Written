//! Research tool: prints every trap (table:index) the engine implements, one
//! per line, to compare with the traps the object scripts call.
//!
//! cargo run -p enemy --example implemented_traps --locked
fn main() {
    for table in 0..32u16 {
        for index in 0..2048u16 {
            if enemy::traps::lookup(table, index).is_some() {
                println!("{table}:{index}");
            }
        }
    }
}
