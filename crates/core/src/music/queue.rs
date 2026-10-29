//! The play queue: what plays now and next, with shuffle and repeat.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Queue {
    /// Track ids in play order.
    pub list: Vec<i64>,
    /// Index into `list` of the current track.
    pub pos: Option<usize>,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// The unshuffled order, to go back to when shuffle is turned off.
    original: Option<Vec<i64>>,
    seed: u64,
}

impl Queue {
    fn rand(&mut self) -> u64 {
        if self.seed == 0 {
            self.seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1;
        }
        // xorshift64*
        let mut x = self.seed;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.seed = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn shuffled(&mut self, mut v: Vec<i64>) -> Vec<i64> {
        for i in (1..v.len()).rev() {
            let j = (self.rand() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
        v
    }

    /// Replace the queue with `items`, starting at `items[start]`.
    pub fn set(&mut self, items: Vec<i64>, start: usize) -> Option<i64> {
        if items.is_empty() {
            *self = Queue { shuffle: self.shuffle, repeat: self.repeat, seed: self.seed, ..Default::default() };
            return None;
        }
        let start = start.min(items.len() - 1);
        if self.shuffle {
            let first = items[start];
            let mut rest = items.clone();
            rest.remove(start);
            let mut list = vec![first];
            list.extend(self.shuffled(rest));
            self.original = Some(items);
            self.list = list;
            self.pos = Some(0);
        } else {
            self.original = None;
            self.list = items;
            self.pos = Some(start);
        }
        self.current()
    }

    pub fn current(&self) -> Option<i64> {
        self.pos.and_then(|p| self.list.get(p).copied())
    }

    /// What plays after the current track finishes on its own.
    pub fn peek_next(&self) -> Option<i64> {
        let p = self.pos?;
        match self.repeat {
            Repeat::One => self.current(),
            _ if p + 1 < self.list.len() => Some(self.list[p + 1]),
            Repeat::All => self.list.first().copied(),
            Repeat::Off => None,
        }
    }

    /// The current track finished: move on (repeat-one stays put).
    pub fn advance(&mut self) -> Option<i64> {
        let p = self.pos?;
        match self.repeat {
            Repeat::One => {}
            _ if p + 1 < self.list.len() => self.pos = Some(p + 1),
            Repeat::All => self.pos = Some(0),
            Repeat::Off => return None,
        }
        self.current()
    }

    /// The Next button: always moves, wrapping when repeating.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<i64> {
        let p = self.pos?;
        if p + 1 < self.list.len() {
            self.pos = Some(p + 1);
        } else if self.repeat != Repeat::Off {
            self.pos = Some(0);
        } else {
            return None;
        }
        self.current()
    }

    /// The Previous button.
    pub fn prev(&mut self) -> Option<i64> {
        let p = self.pos?;
        if p > 0 {
            self.pos = Some(p - 1);
        } else if self.repeat != Repeat::Off && !self.list.is_empty() {
            self.pos = Some(self.list.len() - 1);
        }
        self.current()
    }

    pub fn jump(&mut self, index: usize) -> Option<i64> {
        if index < self.list.len() {
            self.pos = Some(index);
        }
        self.current()
    }

    /// Turn shuffle on or off. The current track keeps playing either way.
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        let cur = self.current();
        if on {
            self.original = Some(self.list.clone());
            let Some(p) = self.pos else {
                let l = std::mem::take(&mut self.list);
                self.list = self.shuffled(l);
                return;
            };
            let mut rest = self.list.clone();
            let first = rest.remove(p);
            let mut list = vec![first];
            list.extend(self.shuffled(rest));
            self.list = list;
            self.pos = Some(0);
        } else if let Some(orig) = self.original.take() {
            self.list = orig;
            self.pos = cur.and_then(|c| self.list.iter().position(|&x| x == c)).or(if self.list.is_empty() { None } else { Some(0) });
        }
    }

    /// Add tracks after the current one (`next`) or at the end.
    pub fn add(&mut self, ids: &[i64], next: bool) {
        if ids.is_empty() {
            return;
        }
        let at = if next { self.pos.map(|p| p + 1).unwrap_or(0) } else { self.list.len() };
        let at = at.min(self.list.len());
        self.list.splice(at..at, ids.iter().copied());
        if let Some(o) = &mut self.original {
            o.extend_from_slice(ids);
        }
        if self.pos.is_none() {
            self.pos = Some(0);
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.list.len() {
            return;
        }
        let id = self.list.remove(index);
        if let Some(o) = &mut self.original {
            if let Some(i) = o.iter().position(|&x| x == id) {
                o.remove(i);
            }
        }
        self.pos = match self.pos {
            _ if self.list.is_empty() => None,
            Some(p) if index < p => Some(p - 1),
            Some(p) => Some(p.min(self.list.len() - 1)),
            None => None,
        };
    }

    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.list.len() || to >= self.list.len() || from == to {
            return;
        }
        let id = self.list.remove(from);
        self.list.insert(to, id);
        if let Some(p) = self.pos {
            self.pos = Some(if p == from {
                to
            } else if from < p && to >= p {
                p - 1
            } else if from > p && to <= p {
                p + 1
            } else {
                p
            });
        }
    }

    pub fn clear(&mut self) {
        let (shuffle, repeat, seed) = (self.shuffle, self.repeat, self.seed);
        *self = Queue { shuffle, repeat, seed, ..Default::default() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_in_order_and_repeats() {
        let mut q = Queue::default();
        assert_eq!(q.set(vec![1, 2, 3], 1), Some(2));
        assert_eq!(q.peek_next(), Some(3));
        assert_eq!(q.advance(), Some(3));
        assert_eq!(q.peek_next(), None);
        assert_eq!(q.advance(), None);
        q.repeat = Repeat::All;
        assert_eq!(q.peek_next(), Some(1));
        q.repeat = Repeat::One;
        assert_eq!(q.peek_next(), Some(3));
        assert_eq!(q.advance(), Some(3));
        // The Next button still moves on repeat-one.
        assert_eq!(q.next(), Some(1));
        assert_eq!(q.prev(), Some(3));
    }

    #[test]
    fn shuffle_keeps_the_current_track_and_everything_else() {
        let mut q = Queue::default();
        q.set((1..=50).collect(), 9);
        q.set_shuffle(true);
        assert_eq!(q.current(), Some(10));
        assert_eq!(q.pos, Some(0));
        let mut s = q.list.clone();
        s.sort();
        assert_eq!(s, (1..=50).collect::<Vec<_>>());
        assert_ne!(q.list, (1..=50).collect::<Vec<_>>());
        q.next();
        let now = q.current().unwrap();
        q.set_shuffle(false);
        assert_eq!(q.current(), Some(now));
        assert_eq!(q.list, (1..=50).collect::<Vec<_>>());
    }

    #[test]
    fn edits_keep_the_current_track() {
        let mut q = Queue::default();
        q.set(vec![1, 2, 3, 4], 2);
        q.add(&[9], true);
        assert_eq!(q.list, vec![1, 2, 3, 9, 4]);
        q.remove(0);
        assert_eq!(q.current(), Some(3));
        q.move_item(3, 0);
        assert_eq!(q.list, vec![4, 2, 3, 9]);
        assert_eq!(q.current(), Some(3));
        q.remove(2);
        assert_eq!(q.current(), Some(9));
        q.clear();
        assert_eq!(q.current(), None);
    }
}
