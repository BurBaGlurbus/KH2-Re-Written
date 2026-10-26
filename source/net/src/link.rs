//! The link simulator: outgoing datagrams wait for a latency plus jitter and
//! a share of them is dropped, so Wi-Fi conditions can be tried on one PC.
//! It works on what this instance sends; set it on both instances for a
//! round trip.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;

/// Added one-way latency, its random spread, and the share of datagrams lost.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Conditions {
    pub latency_ms: u32,
    pub jitter_ms: u32,
    /// 0..=100.
    pub loss_percent: f32,
}

impl Conditions {
    pub fn is_clear(&self) -> bool {
        self.latency_ms == 0 && self.jitter_ms == 0 && self.loss_percent <= 0.
    }
}

/// Datagrams held back by the simulated link, released in time order.
pub struct Link {
    pub conditions: Conditions,
    held: BTreeMap<(Duration, u64), (SocketAddr, Vec<u8>)>,
    serial: u64,
    rng: u64,
    /// Datagrams the simulator dropped (for the overlay).
    pub dropped: u64,
}

impl Link {
    pub fn new(seed: u64) -> Self {
        Self {
            conditions: Conditions::default(),
            held: BTreeMap::new(),
            serial: 0,
            rng: seed | 1,
            dropped: 0,
        }
    }

    /// xorshift64*: a uniform value in 0..1.
    fn unit(&mut self) -> f32 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let v = self.rng.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (v >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Hands `bytes` for `to` to the link at `now`: Some when it goes out
    /// at once, None when it was held back or lost.
    pub fn send(
        &mut self,
        now: Duration,
        to: SocketAddr,
        bytes: Vec<u8>,
    ) -> Option<(SocketAddr, Vec<u8>)> {
        let c = self.conditions;
        if c.is_clear() && self.held.is_empty() {
            return Some((to, bytes));
        }
        if c.loss_percent > 0. && self.unit() * 100. < c.loss_percent {
            self.dropped += 1;
            return None;
        }
        let spread = if c.jitter_ms > 0 {
            (self.unit() * 2. - 1.) * c.jitter_ms as f32
        } else {
            0.
        };
        let delay_ms = (c.latency_ms as f32 + spread).max(0.);
        let release = now + Duration::from_micros((delay_ms * 1000.) as u64);
        self.serial += 1;
        self.held.insert((release, self.serial), (to, bytes));
        None
    }

    /// The held datagrams whose time has come, oldest first.
    pub fn due(&mut self, now: Duration) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut out = Vec::new();
        while let Some(entry) = self.held.first_entry() {
            if entry.key().0 > now {
                break;
            }
            out.push(entry.remove());
        }
        out
    }

    pub fn held(&self) -> usize {
        self.held.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr() -> SocketAddr {
        "127.0.0.1:9".parse().unwrap()
    }

    #[test]
    fn clear_link_sends_at_once() {
        let mut link = Link::new(5);
        assert_eq!(
            link.send(Duration::ZERO, addr(), vec![1]),
            Some((addr(), vec![1]))
        );
        assert_eq!(link.held(), 0);
    }

    #[test]
    fn latency_holds_datagrams_until_due() {
        let mut link = Link::new(5);
        link.conditions.latency_ms = 50;
        assert_eq!(link.send(Duration::ZERO, addr(), vec![1]), None);
        assert_eq!(link.send(Duration::from_millis(10), addr(), vec![2]), None);
        assert!(link.due(Duration::from_millis(49)).is_empty());
        assert_eq!(link.due(Duration::from_millis(55)), vec![(addr(), vec![1])]);
        assert_eq!(link.due(Duration::from_millis(60)), vec![(addr(), vec![2])]);
        assert_eq!(link.held(), 0);
    }

    #[test]
    fn loss_drops_about_the_share_asked_for() {
        let mut link = Link::new(12345);
        link.conditions.loss_percent = 25.;
        for i in 0..4000u32 {
            let _ = link.send(Duration::ZERO, addr(), i.to_le_bytes().to_vec());
        }
        let lost = link.dropped as f32 / 4000.;
        assert!((0.22..0.28).contains(&lost), "lost {lost}");
        assert_eq!(link.due(Duration::ZERO).len() as u64 + link.dropped, 4000);
    }

    #[test]
    fn jitter_stays_inside_its_spread() {
        let mut link = Link::new(77);
        link.conditions.latency_ms = 40;
        link.conditions.jitter_ms = 10;
        for _ in 0..500 {
            let _ = link.send(Duration::ZERO, addr(), vec![0]);
        }
        assert!(link.due(Duration::from_millis(29)).is_empty());
        assert_eq!(link.due(Duration::from_millis(51)).len(), 500);
    }
}
