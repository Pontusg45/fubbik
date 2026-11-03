//! A value kept for a short time, so what several views ask for in one
//! frame (or what a view reads from disk) is computed once, not per call.

use std::cell::RefCell;
use std::time::{Duration, Instant};

pub struct Ttl<T> {
    ttl: Duration,
    cell: RefCell<Option<(Instant, T)>>,
}

impl<T: Clone> Ttl<T> {
    pub fn new(ttl: Duration) -> Self {
        Ttl {
            ttl,
            cell: RefCell::new(None),
        }
    }

    /// The kept value, or `make()`'s when it is older than the TTL.
    pub fn get(&self, make: impl FnOnce() -> T) -> T {
        if let Some((at, value)) = &*self.cell.borrow()
            && at.elapsed() < self.ttl
        {
            return value.clone();
        }
        let value = make();
        *self.cell.borrow_mut() = Some((Instant::now(), value.clone()));
        value
    }

    /// Forgets the value (after something changed it).
    pub fn clear(&self) {
        self.cell.borrow_mut().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_value_until_cleared_or_old() {
        // Given
        let cache = Ttl::new(Duration::from_secs(60));
        let mut calls = 0;

        // When
        let first = cache.get(|| {
            calls += 1;
            1
        });
        let second = cache.get(|| {
            calls += 1;
            2
        });
        cache.clear();
        let third = cache.get(|| 3);
        let short = Ttl::new(Duration::ZERO);
        short.get(|| 1);

        // Then
        assert_eq!((first, second, third), (1, 1, 3));
        assert_eq!(calls, 1);
        assert_eq!(short.get(|| 2), 2, "an old value is made again");
    }
}
