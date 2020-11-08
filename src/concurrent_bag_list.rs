use parking_lot::{Mutex};
use rand::prelude::*;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ConcurrentBag<T> {
    protected_list_1: Mutex<Vec<T>>,
    protected_list_2: Mutex<Vec<T>>,
    available_entries: AtomicU16,
    //mutex_for_wait: Mutex<bool>,
    //condition_for_wait: Condvar,
}

impl<'a, T> ConcurrentBag<T> {
    pub fn new(initial_size: u16) -> ConcurrentBag<T> {
        ConcurrentBag {
            protected_list_1: Mutex::new(vec![]),
            protected_list_2: Mutex::new(vec![]),
            available_entries: AtomicU16::new(0),
            //mutex_for_wait: Mutex::new(false),
            //condition_for_wait: Condvar::new(),
        }
    }

    pub fn borrow_entry(&self) -> Option<T> {
        /*let opt_value = self.borrow_entry_std();
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
        opt_value*/
        self.borrow_entry_rnd()
    }

/*    pub fn borrow_entry_std(&self) -> Option<T> {
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
    }*/

    fn my_random(&self) ->u16 {
        //let mut rng = thread_rng();
        //rng.gen_range(1, 3) as u16
        let in_ms=self.available_entries.fetch_add(1,Ordering::Relaxed);
        (in_ms as u16%2)+1
        /*let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH);
        let in_ms = since_the_epoch.ok().unwrap().as_millis();
        (in_ms as u16%2)+1*/
    }

    pub fn borrow_entry_rnd(&self) -> Option<T> {
        //let in_ms=self.available_entries.fetch_add(1,Ordering::SeqCst);
        //let randomvalue=(in_ms as u16%2)+1;
        let randomvalue=self.my_random();
        //let randomvalue=1;
        if randomvalue == 1 {
            let mut list_1=self.protected_list_1.lock();
            if !list_1.is_empty() {
                return list_1.pop();
            } else {
                let mut list_2=self.protected_list_2.lock();
                if !list_2.is_empty() {
                    return list_2.pop();
                }
            }

        } else {
            let mut list_2=self.protected_list_2.lock();
            if !list_2.is_empty() {
                return list_2.pop();
            } else {
                let mut list_1=self.protected_list_1.lock();
                if !list_1.is_empty() {
                    return list_1.pop();
                }
            }
        }
        None
    }

    pub fn release_entry(&self, entry: T) {
        self.release_entry_rnd(entry);
        //self.mutex_for_wait.lock();
        //self.condition_for_wait.notify_one();
    }

    /*pub fn release_entry_std(&self, entry: T) {
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
    }*/

    pub fn release_entry_rnd(&self, entry: T) {
        //let in_ms=self.available_entries.fetch_add(1,Ordering::SeqCst);
        //let randomvalue=(in_ms as u16%2)+1;
        let randomvalue=self.my_random();
        //let randomvalue=1;
        if randomvalue == 1 {
            let mut list_1=self.protected_list_1.lock();
            list_1.push(entry);
        } else {
            let mut list_2=self.protected_list_2.lock();
            list_2.push(entry);
        }

    }

    pub fn size(&self) -> usize {
        let list_1=self.protected_list_1.lock();
        let size=list_1.len();
        let list_2=self.protected_list_2.lock();
        let size=size+list_2.len();
        size
    }

    pub fn available_size(&self) -> u16 {
        self.size() as u16
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
        let simple_bag: ConcurrentBag<String> = ConcurrentBag::new(10);
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 1);
        assert_eq!(simple_bag.available_size(), 1);
        simple_bag.release_entry(String::from("hello Rust 2"));
        assert_eq!(simple_bag.size(), 2);
        assert_eq!(simple_bag.available_size(), 2);
    }

    #[test]
    fn borrow_entry() {
        let simple_bag: ConcurrentBag<String> = ConcurrentBag::new(10);
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 1);
        assert_eq!(simple_bag.available_size(), 1);
        let str = simple_bag.borrow_entry().unwrap();
        assert_eq!(simple_bag.size(), 0);
        assert_eq!(simple_bag.available_size(), 0);
        assert_eq!(str, String::from("hello Rust"));
    }

    #[test]
    fn borrow_entry_mut() {
        let simple_bag: ConcurrentBag<String> = ConcurrentBag::new(10);
        simple_bag.release_entry(String::from("hello Rust"));
        assert_eq!(simple_bag.size(), 1);
        assert_eq!(simple_bag.available_size(), 1);
        let mut str = simple_bag.borrow_entry().unwrap();
        assert_eq!(simple_bag.size(), 0);
        assert_eq!(simple_bag.available_size(), 0);
        assert_eq!(str, String::from("hello Rust"));
        str.insert_str(0, "hello,");
        assert_eq!(str, String::from("hello,hello Rust"));
    }


}
