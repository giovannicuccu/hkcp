use core::fmt;
use parking_lot::{Condvar, Mutex};
use rand::prelude::*;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

pub struct SimpleBag<T> {
    entry_list: Vec<Mutex<Option<T>>>,
    available_entries: AtomicU16,
    mutex_for_wait: Mutex<bool>,
    condition_for_wait: Condvar,
}

impl<'a, T> SimpleBag<T> {
    pub fn new(initial_size: u16) -> SimpleBag<T> {
        let mut local_entry_list = vec![];
        for i in 0..initial_size {
            local_entry_list.push(Mutex::new(None));
        }
        SimpleBag {
            entry_list: local_entry_list,
            available_entries: AtomicU16::new(0),
            mutex_for_wait: Mutex::new(false),
            condition_for_wait: Condvar::new(),
        }
    }

    pub fn borrow_entry(&self) -> Option<T> {
        let opt_value = self.borrow_entry_std();
        if opt_value.is_none() {
            let mut mutex_guard = self.mutex_for_wait.lock();
            let wait_result = self
                .condition_for_wait
                .wait_for(&mut mutex_guard, Duration::from_millis(500));
            return if wait_result.timed_out() {
                None
            } else {
                self.borrow_entry_std()
            };
        }
        opt_value
    }

    pub fn borrow_entry_std(&self) -> Option<T> {
        for mutex in &self.entry_list {
            let opt_lock = mutex.try_lock();
            if opt_lock.is_some() {
                let mut opt = opt_lock.unwrap();
                if opt.is_some() {
                    &self.available_entries.fetch_sub(1, Ordering::SeqCst);
                    return opt.take();
                }
            }
        }
        None
    }

    pub fn borrow_entry_rnd(&self) -> Option<T> {
        let mut rng = thread_rng();
        let randomvalue = rng.gen_range(0, self.entry_list.len());
        if randomvalue > 0 {
            for i in 0..randomvalue {
                let mutex = &self.entry_list[i];
                let opt_lock = mutex.try_lock();
                if opt_lock.is_some() {
                    let mut opt = opt_lock.unwrap();
                    if opt.is_some() {
                        &self.available_entries.fetch_sub(1, Ordering::SeqCst);
                        return opt.take();
                    }
                }
            }
        }
        if randomvalue < self.entry_list.len() {
            for i in randomvalue..self.entry_list.len() {
                let mutex = &self.entry_list[i];
                let opt_lock = mutex.try_lock();
                if opt_lock.is_some() {
                    let mut opt = opt_lock.unwrap();
                    if opt.is_some() {
                        &self.available_entries.fetch_sub(1, Ordering::SeqCst);
                        return opt.take();
                    }
                }
            }
        }
        None
    }

    pub fn release_entry(&self, entry: T) {
        self.release_entry_std(entry);
        self.mutex_for_wait.lock();
        self.condition_for_wait.notify_one();
    }

    pub fn release_entry_std(&self, entry: T) {
        for mutex in &self.entry_list {
            let opt_lock = mutex.try_lock();
            if opt_lock.is_some() {
                let mut opt = opt_lock.unwrap();
                if opt.is_none() {
                    &self.available_entries.fetch_add(1, Ordering::SeqCst);
                    opt.replace(entry);
                    return;
                }
            }
        }
    }

    pub fn release_entry_rnd(&self, entry: T) {
        let mut rng = thread_rng();
        let randomvalue = rng.gen_range(0, self.entry_list.len());
        if randomvalue > 0 {
            for i in 0..randomvalue {
                let mutex = &self.entry_list[i];
                let opt_lock = mutex.try_lock();
                if opt_lock.is_some() {
                    let mut opt = opt_lock.unwrap();
                    if opt.is_none() {
                        &self.available_entries.fetch_add(1, Ordering::SeqCst);
                        opt.replace(entry);
                        return;
                    }
                }
            }
        }
        if randomvalue < self.entry_list.len() {
            for i in randomvalue..self.entry_list.len() {
                let mutex = &self.entry_list[i];
                let opt_lock = mutex.try_lock();
                if opt_lock.is_some() {
                    let mut opt = opt_lock.unwrap();
                    if opt.is_none() {
                        &self.available_entries.fetch_add(1, Ordering::SeqCst);
                        opt.replace(entry);
                        return;
                    }
                }
            }
        }
    }

    pub fn size(&self) -> usize {
        self.entry_list.len()
    }

    pub fn available_size(&self) -> u16 {
        *&self.available_entries.load(Ordering::Relaxed)
    }

    pub fn add_entry(&mut self, value: T) {
        self.entry_list.push(Mutex::new(Some(value)));
        println!("addEntry vc size {}", self.entry_list.len())
    }

    pub fn remove_entry(&mut self) {
        self.entry_list.pop();
    }
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};
    use std::sync::atomic::{AtomicU16, Ordering};
    use std::sync::Arc;
    use std::time::Instant;
    use std::{thread, time};
    use crate::concurrent_bag::SimpleBag;

    #[test]
    fn add_entry() {
        let simple_bag: SimpleBag<String> = SimpleBag::new(10);
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 10);
        assert_eq!(simple_bag.available_size(), 1);
        simple_bag.release_entry(String::from("hello Rust 2"));
        assert_eq!(simple_bag.size(), 10);
        assert_eq!(simple_bag.available_size(), 2);
    }

    #[test]
    fn borrow_entry() {
        let simple_bag: SimpleBag<String> = SimpleBag::new(10);
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 10);
        assert_eq!(simple_bag.available_size(), 1);
        let str = simple_bag.borrow_entry().unwrap();
        assert_eq!(simple_bag.size(), 10);
        assert_eq!(simple_bag.available_size(), 0);
        assert_eq!(str, String::from("hello Rust"));
    }

    #[test]
    fn borrow_entry_mut() {
        let simple_bag: SimpleBag<String> = SimpleBag::new(10);
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 10);
        assert_eq!(simple_bag.available_size(), 1);
        let mut str = simple_bag.borrow_entry().unwrap();
        assert_eq!(simple_bag.size(), 10);
        assert_eq!(simple_bag.available_size(), 0);
        assert_eq!(str, String::from("hello Rust"));
        str.insert_str(0, "hello,");
        assert_eq!(str, String::from("hello,hello Rust"));
    }

    #[test]
    fn test_mt() {
        let simple_bag: SimpleBag<String> = SimpleBag::new(10);
        simple_bag.release_entry(String::from("hello Rust 1"));
        simple_bag.release_entry(String::from("hello Rust 2"));
        simple_bag.release_entry(String::from("hello Rust 3"));
        simple_bag.release_entry(String::from("hello Rust 4"));
        simple_bag.release_entry(String::from("hello Rust 5"));
        simple_bag.release_entry(String::from("hello Rust 6"));
        simple_bag.release_entry(String::from("hello Rust 7"));
        simple_bag.release_entry(String::from("hello Rust 8"));
        simple_bag.release_entry(String::from("hello Rust 9"));
        simple_bag.release_entry(String::from("hello Rust 10"));
        assert_eq!(simple_bag.size(), 10);
        assert_eq!(simple_bag.available_size(), 10);

        let shared_bag = Arc::new(simple_bag);
        let ok_shared = Arc::new(AtomicU16::new(0));
        let ko_shared = Arc::new(AtomicU16::new(0));
        let mut handle_vec = vec![];
        let start = Instant::now();

        for _ in 0..100 {
            let bag_for_thread = Arc::clone(&shared_bag);
            let ok_for_thread = Arc::clone(&ok_shared);
            let ko_for_thread = Arc::clone(&ko_shared);
            let handle = thread::spawn(move || {
                let mut rng = thread_rng();
                let randomvalue = rng.gen_range(0, 30);
                let ten_millis = time::Duration::from_millis(randomvalue);
                thread::sleep(ten_millis);
                let opt_str = bag_for_thread.borrow_entry();
                if opt_str.is_some() {
                    let mut str = opt_str.unwrap();
                    let mut rng = thread_rng();
                    let randomvalue = rng.gen_range(0, 5);
                    let ten_millis = time::Duration::from_millis(randomvalue);
                    thread::sleep(ten_millis);
                    bag_for_thread.release_entry(str);
                    ok_for_thread.fetch_add(1, Ordering::SeqCst);
                } else {
                    ko_for_thread.fetch_add(1, Ordering::SeqCst);
                }
                let randomvalue = rng.gen_range(0, 30);
                let ten_millis = time::Duration::from_millis(randomvalue);
                thread::sleep(ten_millis);
                let opt_str = bag_for_thread.borrow_entry();
                if opt_str.is_some() {
                    let mut str = opt_str.unwrap();
                    let mut rng = thread_rng();
                    let randomvalue = rng.gen_range(0, 5);
                    let ten_millis = time::Duration::from_millis(randomvalue);
                    thread::sleep(ten_millis);
                    bag_for_thread.release_entry(str);
                    ok_for_thread.fetch_add(1, Ordering::SeqCst);
                } else {
                    ko_for_thread.fetch_add(1, Ordering::SeqCst);
                }
            });
            handle_vec.push(handle);
        }

        for handle in handle_vec {
            handle.join().unwrap();
        }
        let duration = start.elapsed();

        println!(
            "Time elapsed in expensive_function() is: {:?} ok={:?} ko={:?}",
            duration,
            ok_shared.load(Ordering::Relaxed),
            ko_shared.load(Ordering::Relaxed)
        );
    }
}
