use std::sync::Arc;
use std::sync::atomic;

// A flag that can be triggered from a thread to notify another thread
// The bool starts as false
#[derive(Debug, Clone)]
pub struct ThreadFlag {
    atomic_bool: Arc<atomic::AtomicBool>
}

impl ThreadFlag {
    pub fn new() -> ThreadFlag {
        ThreadFlag {
            atomic_bool: Arc::new(atomic::AtomicBool::new(false))
        }
    }

    pub fn get(&self) -> bool {
        self.atomic_bool.load(atomic::Ordering::Relaxed)
    }

    pub fn trigger(&mut self) {
        self.atomic_bool.store(true, atomic::Ordering::Relaxed);
    }

    pub fn reset(&mut self) {
        self.atomic_bool.store(false, atomic::Ordering::Relaxed);
    }
}