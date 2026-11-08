use std::collections::VecDeque;

pub const LRU_CAP: usize = 4;

#[derive(Debug, Default)]
pub struct SessionLru {
    order: VecDeque<String>,
}

impl SessionLru {
    #[must_use]
    pub fn new() -> Self {
        Self {
            order: VecDeque::new(),
        }
    }

    fn key(dest: &str, profile_id: &str) -> String {
        format!("{dest}\0{profile_id}")
    }

    /// Touch dest×profile. Returns the evicted key when over capacity.
    pub fn touch(&mut self, dest: &str, profile_id: &str) -> Option<String> {
        let key = Self::key(dest, profile_id);
        self.order.retain(|k| k != &key);
        self.order.push_back(key);
        if self.order.len() > LRU_CAP {
            self.order.pop_front()
        } else {
            None
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len()
    }
}
