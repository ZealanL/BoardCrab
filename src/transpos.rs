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
    pub age_count: u64
}

impl Entry {
    pub fn new() -> Entry {
        Entry {
            hash: 0,
            eval: 0.0,
            best_move_idx: 0,
            depth_remaining: 0,
            entry_type: EntryType::Invalid,
            age_count: 0
        }
    }

    pub fn is_valid(&self) -> bool {
        self.entry_type != EntryType::Invalid
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
    age_count: u64
}

impl Table {
    pub fn new(size_mbs: usize) -> Table {
        let num_buckets = (size_mbs * 1_000_000) / size_of::<Bucket>();
        let mut buckets = Vec::with_capacity(num_buckets);
        buckets.resize(num_buckets, Bucket::new());
        Table {
            buckets,
            age_count: 0
        }
    }

    fn get_bucket_idx(&self, hash: Hash) -> usize {
        (hash as usize) % self.buckets.len()
    }

    pub fn get(&self, hash: Hash) ->  Entry {
        let bucket = &self.buckets[self.get_bucket_idx(hash)];
        for i in 0..ENTRIES_PER_BUCKET {
            if bucket.entries[i].hash == hash {
                return bucket.entries[i];
            }
        }

        Entry::new()
    }

    pub fn set(&mut self, mut entry: Entry) {
        let bucket_idx = self.get_bucket_idx(entry.hash);
        let bucket = &mut self.buckets[bucket_idx];

        // Find the oldest entry to replace
        let mut replace_entry_idx = 0;
        let mut oldest_entry_age = u64::max_value();
        for i in 0..ENTRIES_PER_BUCKET {
            let existing_entry = &bucket.entries[i];
            if existing_entry.hash == entry.hash {
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
        entry.age_count = self.age_count;

        bucket.entries[replace_entry_idx] = entry;
    }
}