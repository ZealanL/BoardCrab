use crate::zobrist::*;
use crate::eval::Value;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EntryType {
    Invalid,
    Exact,
    FailLow,
    FailHigh
}

#[derive(Debug, Copy, Clone)]
pub struct Entry {
    pub hash: Hash,
    pub eval: Value,
    pub best_move_idx: u8,
    pub depth_remaining: u8,
    pub entry_type: EntryType,
    pub age_count: u64,
    pub checksum: u64
}

impl Entry {
    pub fn new() -> Entry {
        Entry {
            hash: 0,
            eval: 0.0,
            best_move_idx: 0,
            depth_remaining: 0,
            entry_type: EntryType::Invalid,
            age_count: 0,
            checksum: 0
        }
    }

    pub fn update_checksum(&mut self) {
        self.checksum = self.calc_checksum();
    }

    fn calc_checksum(&self) -> u64 {
        let mut cur_checksum = 0;

        cur_checksum += self.hash;
        unsafe {
            cur_checksum += (std::mem::transmute::<Value, i32>(self.eval) as u64) ^ cur_checksum;
        }
        cur_checksum += self.best_move_idx as u64 ^ cur_checksum;
        cur_checksum += self.depth_remaining as u64 ^ cur_checksum;
        cur_checksum += self.entry_type as u64 ^ cur_checksum;

        // NOTE: We don't care about the age count, it's not that important

        cur_checksum
    }

    pub fn is_set(&self) -> bool {
        self.entry_type != EntryType::Invalid
    }

    pub fn is_valid(&self) -> bool {
        if self.entry_type == EntryType::Invalid {
            return false;
        }

        self.checksum == self.calc_checksum()
    }
}

///////////////////////////////////////////

const ENTRIES_PER_BUCKET: usize = 4;

#[derive(Debug, Copy, Clone)]
struct Bucket {
    entries: [Entry; ENTRIES_PER_BUCKET],
}

impl Bucket {
    pub fn new() -> Bucket {
        Bucket {
            entries: [Entry::new(); ENTRIES_PER_BUCKET],
        }
    }
}

///////////////////////////////////////////

pub struct Table {
    buckets: Vec<Bucket>,
    age_count: u64,
    size_mbs: usize
}

impl Table {
    pub fn new(size_mbs: usize) -> Table {
        let num_buckets = (size_mbs * 1_000_000) / size_of::<Bucket>();
        let mut buckets = Vec::with_capacity(num_buckets);
        buckets.resize(num_buckets, Bucket::new());
        Table {
            buckets,
            age_count: 0,
            size_mbs
        }
    }

    pub fn get_size_mbs(&self) -> usize {
        self.size_mbs
    }

    pub fn is_any_entry_locked(&self) -> bool{
        for bucket in &self.buckets {
            for entry in &bucket.entries {
                if entry.is_set() && !entry.is_valid() {
                    return true;
                }
            }
        }

        false
    }

    fn get_bucket_idx(&self, hash: Hash) -> usize {
        (hash as usize) % self.buckets.len()
    }

    // If the entry is locked, just returns an empty entry
    pub fn get_fast(&self, hash: Hash) -> Entry {
        let bucket = &self.buckets[self.get_bucket_idx(hash)];
        for i in 0..ENTRIES_PER_BUCKET {
            if bucket.entries[i].hash == hash {
                return bucket.entries[i];
            }
        }

        Entry::new()
    }

    // Waits for the entry to be unlocked
    pub fn get_wait(&self, hash: Hash) -> Entry {
        let bucket = &self.buckets[self.get_bucket_idx(hash)];
        loop {
            let mut was_locked = false;
            for i in 0..ENTRIES_PER_BUCKET {
                if bucket.entries[i].hash == hash {
                    let result = bucket.entries[i];
                    if result.is_set() && !result.is_valid() {
                        was_locked = true;
                        break;
                    }
                    return result;
                }
            }

            if was_locked {
                continue;
            } else {
                return Entry::new();
            }
        }
    }

    pub fn set(&mut self, hash: Hash, eval: Value, best_move_idx: u8, depth_remaining: u8, entry_type: EntryType) {
        let bucket_idx = self.get_bucket_idx(hash);
        let bucket = &mut self.buckets[bucket_idx];

        // Find the oldest entry to replace
        let mut replace_entry_idx = 0;
        let mut oldest_entry_age = u64::max_value();
        for i in 0..ENTRIES_PER_BUCKET {
            let existing_entry = bucket.entries[i];
            if existing_entry.hash == hash {
                // We found a matching hash, just use that
                replace_entry_idx = i;
                break;
            }

            if existing_entry.age_count < oldest_entry_age {
                oldest_entry_age = existing_entry.age_count;
                replace_entry_idx = i;
            }
        }

        self.age_count += 1;

        let mut entry = Entry {
            hash,
            eval,
            best_move_idx,
            depth_remaining,
            entry_type,
            age_count: self.age_count,
            checksum: 0
        };
        entry.update_checksum();

        bucket.entries[replace_entry_idx] = entry;
    }
}