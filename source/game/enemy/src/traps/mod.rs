//! Trap implementations, one module per group. A group module exports
//! `lookup(table, index) -> Option<TrapFn>`; [`lookup`] asks each in turn.
//! Add a trap by adding its arm to the owning group's `lookup` and writing the
//! function in that module, citing the executable address it translates.

use crate::world::TrapFn;

pub mod attack;
pub mod flare;
pub mod friend;
pub mod gimmick;
pub mod homing;
pub mod limit;
pub mod math;
pub mod object;
pub mod presentation;
pub mod query;
pub mod random_position;
pub mod route;
pub mod searcher;
pub mod service;
pub mod sheet;
pub mod state;
pub mod system;
pub mod target;
#[cfg(test)]
pub(crate) mod test_support;
pub mod trinity;
pub mod weapon;

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    math::lookup(table, index)
        .or_else(|| limit::lookup(table, index))
        .or_else(|| flare::lookup(table, index))
        .or_else(|| trinity::lookup(table, index))
        .or_else(|| friend::lookup(table, index))
        .or_else(|| weapon::lookup(table, index))
        .or_else(|| attack::lookup(table, index))
        .or_else(|| homing::lookup(table, index))
        .or_else(|| object::lookup(table, index))
        .or_else(|| target::lookup(table, index))
        .or_else(|| system::lookup(table, index))
        .or_else(|| sheet::lookup(table, index))
        .or_else(|| query::lookup(table, index))
        .or_else(|| searcher::lookup(table, index))
        .or_else(|| random_position::lookup(table, index))
        .or_else(|| route::lookup(table, index))
        .or_else(|| presentation::lookup(table, index))
        .or_else(|| service::lookup(table, index))
        .or_else(|| gimmick::lookup(table, index))
        .or_else(|| state::lookup(table, index))
}
