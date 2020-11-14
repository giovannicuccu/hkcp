use parking_lot::{Mutex};
use std::sync::atomic::{AtomicU16, Ordering};

pub struct ConcurrentBag<T> {
    protected_list_1: Mutex<Vec<T>>,
    protected_list_2: Mutex<Vec<T>>,
    available_entries_1: AtomicU16,
    available_entries_2: AtomicU16,
    sequence_gen: AtomicU16,
    sequence_gen_release: AtomicU16,
}

impl<'a, T> ConcurrentBag<T> {
    pub fn new() -> ConcurrentBag<T> {
        ConcurrentBag {
            protected_list_1: Mutex::new(vec![]),
            protected_list_2: Mutex::new(vec![]),
            available_entries_1: AtomicU16::new(0),
            available_entries_2: AtomicU16::new(0),
            sequence_gen: AtomicU16::new(0),
            sequence_gen_release: AtomicU16::new(0),
        }
    }

    pub fn borrow_entry(&self) -> Option<T> {
        if (self.available_entries_1.load(Ordering::SeqCst)==0) && (self.available_entries_2.load(Ordering::SeqCst)==0) {
            return None;
        }
        let in_ms=self.sequence_gen.fetch_add(1,Ordering::Relaxed);
        let randomvalue=(in_ms%2)+1;
        //let randomvalue=1;
        return if randomvalue == 1 {
            let mut list_1 = self.protected_list_1.lock();
            let opt_from_list_1 = list_1.pop();
            if opt_from_list_1.is_some() {
                self.available_entries_1.fetch_sub(1,Ordering::SeqCst);
                opt_from_list_1
            } else {
                drop(list_1);
                let mut list_2 = self.protected_list_2.lock();
                let opt_from_list_2 = list_2.pop();
                if opt_from_list_2.is_some() {
                    self.available_entries_2.fetch_sub(1,Ordering::SeqCst);
                }
                opt_from_list_2
            }
        } else {
            let mut list_2 = self.protected_list_2.lock();
            let opt_from_list_2 = list_2.pop();
            if opt_from_list_2.is_some() {
                self.available_entries_2.fetch_sub(1,Ordering::SeqCst);
                opt_from_list_2
            } else {
                drop(list_2);
                let mut list_1 = self.protected_list_1.lock();
                let opt_from_list_1 = list_1.pop();
                if opt_from_list_1.is_some() {
                    self.available_entries_1.fetch_sub(1,Ordering::SeqCst);
                }
                opt_from_list_1
            }
        }
    }


    pub fn release_entry(&self, entry: T) {
        let in_ms=self.sequence_gen_release.fetch_add(1,Ordering::Relaxed);
        let randomvalue=(in_ms%2)+1;
        if randomvalue == 1 {
            let mut list_1=self.protected_list_1.lock();
            list_1.push(entry);
            self.available_entries_1.fetch_add(1,Ordering::SeqCst);
        } else {
            let mut list_2=self.protected_list_2.lock();
            list_2.push(entry);
            self.available_entries_2.fetch_add(1,Ordering::SeqCst);
        }

    }

    pub fn size(&self) -> usize {
        let list_1=self.protected_list_1.lock();
        let size=list_1.len();
        let list_2=self.protected_list_2.lock();
        let size=size+list_2.len();
        size
    }
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};
    use std::sync::atomic::{AtomicU16, Ordering};
    use std::sync::Arc;
    use std::time::Instant;
    use std::{thread, time};
    use crate::concurrent_bag_list::ConcurrentBag;

    #[test]
    fn add_entry() {
        let simple_bag: ConcurrentBag<String> = ConcurrentBag::new();
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 1);
        simple_bag.release_entry(String::from("hello Rust 2"));
        assert_eq!(simple_bag.size(), 2);
    }

    #[test]
    fn borrow_entry() {
        let simple_bag: ConcurrentBag<String> = ConcurrentBag::new();
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 1);
        let str = simple_bag.borrow_entry().unwrap();
        assert_eq!(simple_bag.size(), 0);
        assert_eq!(str, String::from("hello Rust"));
    }

    #[test]
    fn borrow_entry_mut() {
        let simple_bag: ConcurrentBag<String> = ConcurrentBag::new();
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 1);
        let mut str = simple_bag.borrow_entry().unwrap();
        assert_eq!(simple_bag.size(), 0);
        assert_eq!(str, String::from("hello Rust"));
        str.insert_str(0, "hello,");
        assert_eq!(str, String::from("hello,hello Rust"));
    }


}
