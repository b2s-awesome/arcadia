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

/// How shuffle picks the order (Shuffle+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ShuffleMode {
    /// Every order equally likely.
    #[default]
    Random,
    /// The same artist twice in a row as rarely as possible.
    SpreadArtists,
    /// Songs heard lately drift towards the end.
    FewerRepeats,
    /// Songs with fewer plays come up sooner.
    LessPlayed,
}

/// What a shuffle mode needs to know about a song.
#[derive(Debug, Clone, Default)]
pub struct ShuffleInfo {
    pub artist: String,
    pub plays: i64,
    /// Seconds since it was last played; `None` if never.
    pub since_played: Option<i64>,
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
    pub mode: ShuffleMode,
    /// Facts for the shuffle modes; filled in before shuffling.
    #[serde(skip)]
    pub info: std::collections::HashMap<i64, ShuffleInfo>,
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

    fn unit(&mut self) -> f64 {
        // 53 random bits in [0, 1), never exactly 0.
        ((self.rand() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn shuffled(&mut self, mut v: Vec<i64>) -> Vec<i64> {
        match self.mode {
            ShuffleMode::Random => {
                for i in (1..v.len()).rev() {
                    let j = (self.rand() % (i as u64 + 1)) as usize;
                    v.swap(i, j);
                }
                v
            }
            ShuffleMode::SpreadArtists => self.spread(v),
            ShuffleMode::FewerRepeats | ShuffleMode::LessPlayed => {
                // Weighted shuffle (Efraimidis–Spirakis): sort by u^(1/w),
                // so heavier songs tend to come first but nothing is fixed.
                let mode = self.mode;
                let mut keyed: Vec<(f64, i64)> = v
                    .into_iter()
                    .map(|id| {
                        let i = self.info.get(&id);
                        let w = match mode {
                            ShuffleMode::FewerRepeats => match i.and_then(|i| i.since_played) {
                                None => 12.0,
                                Some(s) => 1.0 + (s as f64 / 86_400.0).min(30.0) / 3.0,
                            },
                            _ => 1.0 / (1.0 + i.map(|i| i.plays).unwrap_or(0).max(0) as f64).sqrt(),
                        };
                        (self.unit().powf(1.0 / w), id)
                    })
                    .collect();
                keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
                keyed.into_iter().map(|(_, id)| id).collect()
            }
        }
    }

    /// Spread each artist's songs evenly through the list, from a random
    /// offset, with a little jitter (the way Spotify described theirs).
    fn spread(&mut self, v: Vec<i64>) -> Vec<i64> {
        let mut groups: Vec<(String, Vec<i64>)> = Vec::new();
        for id in v {
            let a = self.info.get(&id).map(|i| i.artist.to_lowercase()).unwrap_or_default();
            match groups.iter_mut().find(|g| g.0 == a) {
                Some(g) => g.1.push(id),
                None => groups.push((a, vec![id])),
            }
        }
        let mut placed: Vec<(f64, i64)> = Vec::new();
        for (_, mut ids) in groups {
            for i in (1..ids.len()).rev() {
                let j = (self.rand() % (i as u64 + 1)) as usize;
                ids.swap(i, j);
            }
            let n = ids.len() as f64;
            let offset = self.unit() / n;
            for (k, id) in ids.into_iter().enumerate() {
                let jitter = (self.unit() - 0.5) * 0.2 / n;
                placed.push((offset + k as f64 / n + jitter, id));
            }
        }
        placed.sort_by(|a, b| a.0.total_cmp(&b.0));
        placed.into_iter().map(|(_, id)| id).collect()
    }

    /// Change how shuffle orders things; with shuffle on, the songs after
    /// the current one are reshuffled.
    pub fn set_mode(&mut self, mode: ShuffleMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        if self.shuffle {
            if let Some(p) = self.pos {
                let rest = self.list.split_off(p + 1);
                let rest = self.shuffled(rest);
                self.list.extend(rest);
            }
        }
    }

    /// Like [`Queue::peek_next`], passing over songs `skip` rejects.
    pub fn peek_next_where(&self, skip: impl Fn(i64) -> bool) -> Option<i64> {
        let p = self.pos?;
        if self.repeat == Repeat::One {
            return self.current();
        }
        self.after(p, self.repeat == Repeat::All).find(|&i| !skip(self.list[i])).map(|i| self.list[i])
    }

    /// Like [`Queue::advance`], passing over songs `skip` rejects.
    pub fn advance_where(&mut self, skip: impl Fn(i64) -> bool) -> Option<i64> {
        let p = self.pos?;
        if self.repeat == Repeat::One {
            return self.current();
        }
        let i = self.after(p, self.repeat == Repeat::All).find(|&i| !skip(self.list[i]))?;
        self.pos = Some(i);
        self.current()
    }

    /// Like [`Queue::next`] (the Next button), passing over songs `skip`
    /// rejects.
    pub fn next_where(&mut self, skip: impl Fn(i64) -> bool) -> Option<i64> {
        let p = self.pos?;
        let i = self.after(p, self.repeat != Repeat::Off).find(|&i| !skip(self.list[i]))?;
        self.pos = Some(i);
        self.current()
    }

    /// Indexes after `p`, wrapping round once if `wrap`.
    fn after(&self, p: usize, wrap: bool) -> impl Iterator<Item = usize> {
        let n = self.list.len();
        let end = if wrap { p + n } else { n };
        (p + 1..end).map(move |i| i % n)
    }

    /// Replace the queue with `items`, starting at `items[start]`.
    pub fn set(&mut self, items: Vec<i64>, start: usize) -> Option<i64> {
        if items.is_empty() {
            *self = Queue { shuffle: self.shuffle, repeat: self.repeat, seed: self.seed, mode: self.mode, info: std::mem::take(&mut self.info), ..Default::default() };
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
        let (shuffle, repeat, seed, mode) = (self.shuffle, self.repeat, self.seed, self.mode);
        *self = Queue { shuffle, repeat, seed, mode, ..Default::default() };
    }

    /// Queue tools: drop repeats of the same song, keeping the first (and
    /// the current one).
    pub fn dedupe(&mut self) {
        let mut seen: std::collections::HashSet<i64> = self.current().into_iter().collect();
        let mut i = 0;
        while i < self.list.len() {
            if Some(i) == self.pos || seen.insert(self.list[i]) {
                i += 1;
            } else {
                self.remove(i);
            }
        }
    }

    /// Queue tools: drop everything before the current song.
    pub fn clear_played(&mut self) {
        while self.pos.is_some_and(|p| p > 0) {
            self.remove(0);
        }
    }

    /// Queue tools: shuffle just what's still to come.
    pub fn shuffle_rest(&mut self) {
        if let Some(p) = self.pos {
            let rest = self.list.split_off(p + 1);
            let rest = self.shuffled(rest);
            self.list.extend(rest);
        }
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

    #[test]
    fn skipping_passes_over_rejected_songs() {
        let mut q = Queue::default();
        q.set(vec![1, 2, 3, 4], 0);
        let bad = |id: i64| id == 2 || id == 3;
        assert_eq!(q.peek_next_where(bad), Some(4));
        assert_eq!(q.advance_where(bad), Some(4));
        assert_eq!(q.advance_where(bad), None, "nothing left, no repeat");
        q.repeat = Repeat::All;
        assert_eq!(q.next_where(bad), Some(1));
        // Everything rejected: nothing plays rather than spinning.
        assert_eq!(q.peek_next_where(|_| true), None);
    }

    #[test]
    fn spread_artists_keeps_artists_apart() {
        let mut q = Queue { mode: ShuffleMode::SpreadArtists, ..Default::default() };
        // Three artists, four songs each.
        for id in 1..=12 {
            q.info.insert(id, ShuffleInfo { artist: format!("a{}", id % 3), ..Default::default() });
        }
        for _ in 0..20 {
            let l = q.shuffled((1..=12).collect());
            let same = l.windows(2).filter(|w| w[0] % 3 == w[1] % 3).count();
            assert!(same <= 2, "{l:?} has {same} back-to-back repeats");
            let mut s = l.clone();
            s.sort();
            assert_eq!(s, (1..=12).collect::<Vec<_>>());
        }
    }

    #[test]
    fn weighted_modes_lean_the_right_way() {
        let mut q = Queue { mode: ShuffleMode::LessPlayed, ..Default::default() };
        q.info.insert(1, ShuffleInfo { plays: 400, ..Default::default() });
        q.info.insert(2, ShuffleInfo { plays: 0, ..Default::default() });
        let first_two = (0..400).filter(|_| q.shuffled(vec![1, 2])[0] == 2).count();
        assert!(first_two > 300, "the unplayed song led {first_two}/400 times");
        q.mode = ShuffleMode::FewerRepeats;
        q.info.insert(1, ShuffleInfo { since_played: Some(60), ..Default::default() });
        q.info.insert(2, ShuffleInfo { since_played: None, ..Default::default() });
        let first_two = (0..400).filter(|_| q.shuffled(vec![1, 2])[0] == 2).count();
        assert!(first_two > 300, "the never-played song led {first_two}/400 times");
    }

    #[test]
    fn queue_tools() {
        let mut q = Queue::default();
        q.set(vec![1, 2, 1, 3, 2, 4], 3);
        q.dedupe();
        assert_eq!(q.list, vec![1, 2, 3, 4]);
        assert_eq!(q.current(), Some(3));
        q.clear_played();
        assert_eq!(q.list, vec![3, 4]);
        assert_eq!(q.pos, Some(0));
    }
}
