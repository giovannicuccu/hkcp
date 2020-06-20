use std::cell::RefCell;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicU8};
use std::sync::{Arc, RwLock};
use std::fmt;
use thread_local::ThreadLocal;

#[cfg(test)]
mod test;

const USED: u8 = 2;
const UNUSED: u8 = 4;
//const DELETED: u8 = 16;

static COUNTER: AtomicUsize = AtomicUsize::new(1);

trait BagEntryType: Send + Sync {}

pub struct BagEntry<T: Send + Sync> {
    state: AtomicU8,
    value: T,
    id: usize,
}

impl<'a, T: Send + Sync> BagEntry<T> {
    pub fn new(value: T) -> BagEntry<T> {
        BagEntry {
            value,
            state: AtomicU8::new(UNUSED),
            id: COUNTER.fetch_add(10, Ordering::SeqCst),
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn as_mut_value(&mut self) -> &mut T {
        &mut self.value
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

impl<T: Send + Sync> PartialEq for BagEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Send + Sync> fmt::Display for BagEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BagEntry id {}", self.id)
    }
}

pub struct ConcurrentBag<T: Send + Sync> {
    entry_list: RwLock<Vec<Arc<BagEntry<T>>>>,
    local_entry_list: ThreadLocal<RefCell<Vec<Arc<BagEntry<T>>>>>,
}

impl<'a, T: Send + Sync> ConcurrentBag<T> {
    pub fn new() -> ConcurrentBag<T> {
        ConcurrentBag {
            entry_list: RwLock::new(vec![]),
            local_entry_list: ThreadLocal::new(),
        }
    }

    pub fn lease_entry(&self) -> Option<Arc<BagEntry<T>>> {
        let local_list_ref = self.local_entry_list.get_or(|| RefCell::new(vec![]));
        let mut local_list = (*local_list_ref).borrow_mut();
        let opt_bag_entry = self.find_in_list(&local_list);
        match opt_bag_entry {
            Some(entry_list) => Option::Some(entry_list),
            None => {
                let entry_list = self.entry_list.read().unwrap();
                let opt_bag_entry_notl = self.find_in_list(entry_list.deref());
                match opt_bag_entry_notl {
                    Some(bag_entry) => {
                        local_list.push(Arc::clone(&bag_entry));
                        Option::Some(bag_entry)
                    }
                    None => Option::None
                }
            }
        }
    }

    fn find_in_list(&self, list: &Vec<Arc<BagEntry<T>>>) -> Option<Arc<BagEntry<T>>> {
        for bag_entry in list.iter() {
            let actual_state = bag_entry.state.load(Ordering::Acquire);
            if actual_state == UNUSED {
                let state = bag_entry.state.compare_and_swap(UNUSED, USED, Ordering::Acquire);
                if state == UNUSED {
                    return Option::Some(Arc::clone(bag_entry));
                }
            }
        }
        Option::None
    }

    pub fn release_entry(&self, entry: &BagEntry<T>) {
        let entry_list = self.entry_list.read().unwrap();
        //println!("Release entry size {}", entry_list.len());
        let opt_pos = entry_list
            .iter()
            .position(|entry_it| (**entry_it) == *entry);
        if opt_pos.is_some() {
            let found_entry = entry_list.get(opt_pos.unwrap()).unwrap();
            found_entry.state.swap(UNUSED, Ordering::Acquire);
        }
    }

    pub fn add_entry(&self, value: T) -> usize {
        let mut entry_list = self.entry_list.write().unwrap();
        let bag_entry=BagEntry::new(value);
        let id =bag_entry.id;
        entry_list.push(Arc::new(bag_entry));
        id
    }

    pub fn remove_entry(&self, value: usize) {
        let mut entry_list = self.entry_list.write().unwrap();
        let index_opt = entry_list.iter().position(|x| x.id == value);
        if index_opt.is_some() {
            entry_list.remove(index_opt.unwrap());
        }
    }
}