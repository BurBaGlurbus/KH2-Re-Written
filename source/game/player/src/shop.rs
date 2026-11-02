//! The shop's rules: FUN_1402a21c0 and FUN_14029f9a0's quantity step, and the
//! trade (FUN_140302500, FUN_140302470). No refusal message exists: a good the
//! rules refuse only plays the error sound.

/// A stock count's cap, and munny's (FUN_14029f9a0, FUN_140302470).
pub const STOCK_CAP: u32 = 99;
pub const MUNNY_CAP: u32 = 999_999;

/// FUN_14029c860's test at the goods row: a good can be bought or sold.
pub fn can_trade(buying: bool, stock: u32, buy: u32, sell: u32, munny: u32) -> bool {
    if buying {
        stock < STOCK_CAP && buy <= munny
    } else {
        sell != 0
    }
}

/// FUN_14029f9a0: quantity `q` after a step of `d`, or `None` when it doesn't
/// change.
pub fn step_quantity(
    q: u32,
    d: i32,
    buying: bool,
    stock: u32,
    munny: u32,
    price: u32,
) -> Option<u32> {
    let (limit, have) = if buying {
        (STOCK_CAP, stock + q)
    } else {
        (stock, q)
    };
    if q < 2 && d < 1 {
        return None;
    }
    let lower = |q: u32, d: i32| ((q as i32 + d).max(1)) as u32;
    if have >= limit {
        return (d < 0).then(|| lower(q, d));
    }
    if d < 0 {
        return Some(lower(q, d));
    }
    let mut d = d as u32;
    if buying && munny < price.saturating_mul(q + d) {
        let room = munny.saturating_sub(price.saturating_mul(q));
        if room < price || price == 0 {
            return None;
        }
        d = room / price;
    }
    let mut next = q + d;
    if buying && next + stock >= limit {
        next = limit - stock;
    }
    if !buying && next >= limit {
        next = limit;
    }
    Some(next)
}

/// Munny after a trade (FUN_1402a21c0; a sale caps, a purchase doesn't floor).
pub fn munny_after(munny: u32, total: u32, buying: bool) -> u32 {
    if buying {
        munny.saturating_sub(total)
    } else {
        munny.saturating_add(total).min(MUNNY_CAP)
    }
}

/// Stock after a trade (FUN_1403c3fe0, FUN_1403c4a40 floors at 0).
pub fn stock_after(stock: u32, quantity: u32, buying: bool) -> u32 {
    if buying {
        (stock + quantity).min(STOCK_CAP)
    } else {
        stock.saturating_sub(quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_good_at_the_cap_or_too_dear_is_refused_and_a_sale_needs_a_price() {
        assert!(can_trade(true, 98, 40, 10, 40));
        assert!(!can_trade(true, 99, 40, 10, 400));
        assert!(!can_trade(true, 0, 40, 10, 39));
        assert!(can_trade(false, 1, 40, 10, 0));
        assert!(!can_trade(false, 1, 40, 0, 0));
    }

    #[test]
    fn the_step_holds_at_one_and_goes_down_by_ten_to_one() {
        assert_eq!(step_quantity(1, -1, true, 0, 1000, 10), None);
        assert_eq!(step_quantity(1, -10, true, 0, 1000, 10), None);
        assert_eq!(step_quantity(5, -10, true, 0, 1000, 10), Some(1));
        assert_eq!(step_quantity(12, -10, true, 0, 1000, 10), Some(2));
    }

    #[test]
    fn buying_stops_at_what_the_munny_pays_for() {
        assert_eq!(step_quantity(1, 1, true, 0, 1000, 10), Some(2));
        // 35 munny, price 10, three held: one more.
        assert_eq!(step_quantity(3, 10, true, 0, 35, 10), None);
        assert_eq!(step_quantity(2, 10, true, 0, 35, 10), Some(3));
        assert_eq!(step_quantity(1, 10, true, 0, 95, 10), Some(9));
    }

    #[test]
    fn buying_stops_at_the_stock_cap() {
        assert_eq!(step_quantity(1, 10, true, 95, 10_000, 10), Some(4));
        assert_eq!(step_quantity(4, 1, true, 95, 10_000, 10), None);
        assert_eq!(step_quantity(4, -1, true, 95, 10_000, 10), Some(3));
    }

    #[test]
    fn selling_stops_at_the_stock() {
        assert_eq!(step_quantity(1, 10, false, 5, 0, 10), Some(5));
        assert_eq!(step_quantity(5, 1, false, 5, 0, 10), None);
        assert_eq!(step_quantity(5, -1, false, 5, 0, 10), Some(4));
    }

    #[test]
    fn a_trade_moves_munny_and_stock() {
        assert_eq!(munny_after(100, 30, true), 70);
        assert_eq!(munny_after(999_990, 30, false), MUNNY_CAP);
        assert_eq!(stock_after(98, 5, true), 99);
        assert_eq!(stock_after(3, 5, false), 0);
    }
}
