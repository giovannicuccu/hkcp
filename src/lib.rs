use std::cell::RefCell;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicU8};
use std::sync::{Arc, RwLock, Mutex, Condvar};
use std::{fmt, error, thread};
use thread_local::ThreadLocal;
use std::time::Duration;
use crossbeam_channel::unbounded;

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

pub struct ConnectionPoolConfig {
    initial_connections: u16,
    max_connections: u16,
    connect_timeout_millis: u32,
    get_timeout_millis: u32,
}

impl ConnectionPoolConfig {
    pub fn new(
        initial_connections: u16,
        max_connections: u16,
        connect_timeout_millis: u32,
        get_timeout_millis: u32,
    ) -> ConnectionPoolConfig {
        ConnectionPoolConfig {
            initial_connections,
            max_connections,
            connect_timeout_millis,
            get_timeout_millis,
        }
    }
}
/// A trait which allows for customization of connections.
/// This trait is a simplified form of the trait with the same name defined in R2D2 package
/// https://github.com/sfackler/r2d2 licensed under either of
///     Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
///     MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)
/// at your option
pub trait ConnectionFactory: Send + Sync + 'static {
    /// The connection type this manager deals with.
    type Connection: Send + Sync + 'static;

    /// The error type returned by `Connection`s.
    type Error: error::Error + Send + 'static;

    /// Attempts to create a new connection.
    fn connect(&self) -> Result<Self::Connection, Self::Error>;

    fn is_valid(&self, conn: &Self::Connection) -> bool;
}

/*struct ConnectionPoolStatus<T: ConnectionFactory> {
    current_connections_num: u16,
    connection_factory: T,
}*/

struct ConnectionPoolStatus {
    current_connections_num: u16,
}

#[derive(Debug, Clone)]
pub struct HkcpError {
    message: String,
}

impl fmt::Display for HkcpError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.message)
    }
}

impl std::error::Error for HkcpError {
    fn description(&self) -> &str {
        &self.message
    }
}

pub struct ConnectionPool<T>
    where
        T: ConnectionFactory,
{
    bag: Arc<ConcurrentBag<T::Connection>>,
    status: Arc<Mutex<ConnectionPoolStatus>>,
    connection_factory: Arc<T>,
    config: Arc<ConnectionPoolConfig>,
    condvar: Arc<Condvar>,
}

/// Returns a new `Pool` referencing the same state as `self`.
impl<T> Clone for ConnectionPool<T>
    where
        T: ConnectionFactory,
{
    fn clone(&self) -> ConnectionPool<T> {
        ConnectionPool {
            bag: self.bag.clone(),
            status: self.status.clone(),
            connection_factory: self.connection_factory.clone(),
            config: self.config.clone(),
            condvar: self.condvar.clone(),
        }
    }
}

fn create_initial_connections<T: ConnectionFactory>(status : &Arc<Mutex<ConnectionPoolStatus>>,
                                                    pool_config: &Arc<ConnectionPoolConfig>,
                                                    bag : &Arc<ConcurrentBag<T::Connection>>,
                                                    connection_factory: &Arc<T>)->Result<(),HkcpError> {
    let lock_result = status.lock();
    if lock_result.is_err() {
        return Err(HkcpError { message: String::from("Internal Error while locking status") });
    }
    let mut status_lock = lock_result.unwrap();
    for _ in 1..pool_config.initial_connections+1 {
        let create_result = create_connection(pool_config,&mut status_lock, bag,
                                              connection_factory);
        if create_result.is_err() {
            return Err(create_result.err().unwrap());
        }
    }
    return Ok(());
}

fn create_connection<T: ConnectionFactory>(config: &Arc<ConnectionPoolConfig>,
                                           status : &mut ConnectionPoolStatus,
                                           bag : &Arc<ConcurrentBag<T::Connection>>,
                                           connection_factory: &Arc<T>)->Result<(),HkcpError> {
    let connection_factory_in_thread = connection_factory.clone();
    let (tx, rx) = unbounded();
    thread::spawn(move || {
        let conn_res = connection_factory_in_thread.connect();
        tx.send(conn_res)
    });
    let thread_result = rx.recv_timeout(Duration::from_millis(config.connect_timeout_millis as u64));
    return match thread_result {
        Ok(conn_res) => {
            match conn_res {
                Ok(conn) => {
                    status.current_connections_num =
                        status.current_connections_num + 1;
                    bag.add_entry(conn);
                    Ok(())
                }
                Err(error) => {
                    Err(HkcpError { message: error.to_string() })
                }
            }
        }
        Err(error) => {
            Err(HkcpError { message: error.to_string() })
        }
    }
}

impl<T: ConnectionFactory> ConnectionPool<T> {

    pub fn new_with_config(
        connection_factory: T,
        pool_config: ConnectionPoolConfig,
    ) -> Result<ConnectionPool<T>,HkcpError> {
        let initial_status =Arc::new(Mutex::new(ConnectionPoolStatus {
            current_connections_num: 0,
        }));
        let initial_config=Arc::new(pool_config);
        let initial_bag = Arc::new(ConcurrentBag::new());
        let initial_connection_factory= Arc::new(connection_factory);
        let create_result=create_initial_connections(&initial_status,
                                                     &initial_config, &initial_bag,
                                                     &initial_connection_factory);

        match create_result {
            Ok(_) => {
                Ok(ConnectionPool {
                    bag: initial_bag,
                    status: initial_status,
                    connection_factory: initial_connection_factory,
                    config: initial_config,
                    condvar: Arc::new(Condvar::new()),
                })
            }
            Err(error) => {
                Err(error)
            }
        }
    }

    pub fn new(connection_factory: T) -> Result<ConnectionPool<T>,HkcpError> {
        ConnectionPool::new_with_config(connection_factory, ConnectionPoolConfig {
            initial_connections: 1,
            max_connections: 2,
            connect_timeout_millis: 500,
            get_timeout_millis: 500,
        })
    }



    fn release_connection(&self, entry: &BagEntry<T::Connection>) {
        self.bag.release_entry(entry);
        self.condvar.notify_one();
    }

    pub fn get_connection(&self) -> Result<PooledConnection<T>, HkcpError> {
        let opt_entry = self.bag.lease_entry();
        if opt_entry.is_some() {
            let conn=opt_entry.unwrap();
            return if self.connection_factory.is_valid(&conn.value) {
                Ok(PooledConnection::new(self.clone(), conn))
            } else {
                let lock_result = self.status.lock();
                if lock_result.is_err() {
                    return Err(HkcpError { message: String::from("Internal Error while locking status") });
                }
                let mut status_lock = lock_result.unwrap();
                status_lock.current_connections_num=status_lock.current_connections_num-1;
                self.bag.remove_entry(conn.id);
                self.get_connection()
            }
        }

        let create_result = self.get_connection_internal();
        match create_result {
            Ok(bag_entry) => {
                Ok(PooledConnection::new(self.clone(), bag_entry))
            }
            Err(err) => {
                Err(err)
            }
        }


    }

    fn get_connection_internal(&self)->Result<Arc<BagEntry<T::Connection>>,HkcpError> {
        let lock_result = self.status.lock();
        if lock_result.is_err() {
            return Err(HkcpError { message: String::from("Internal Error while locking status") });
        }
        let mut status_lock = lock_result.unwrap();
        if status_lock.current_connections_num < self.config.max_connections {
            let create_result = create_connection(&self.config, &mut status_lock, &self.bag,
                                                  &self.connection_factory);
            match create_result {
                Ok(()) => {
                    status_lock.current_connections_num =
                        status_lock.current_connections_num + 1;
                    let opt_entry = self.bag.lease_entry();
                    if opt_entry.is_some() {
                        return Ok(opt_entry.unwrap());
                    }
                    return Err(HkcpError { message: String::from("Internal Error while getting entry from bag") });
                }
                Err(error) => {
                    return Err(HkcpError { message: error.to_string() });
                }
            }
        } else {
            loop {
                let result = self.condvar
                    .wait_timeout(
                        status_lock,
                        Duration::from_millis(self.config.get_timeout_millis as u64),
                    )
                    .unwrap();
                status_lock = result.0;
                if result.1.timed_out() {
                    return Err(HkcpError {
                        message: String::from("No available connection in pool"),
                    });
                } else {
                    let opt_entry = self.bag.lease_entry();
                    if opt_entry.is_some() {
                        let conn=opt_entry.unwrap();
                        return if self.connection_factory.is_valid(&conn.value) {
                            Ok(conn)
                        } else {
                            let lock_result = self.status.lock();
                            if lock_result.is_err() {
                                return Err(HkcpError { message: String::from("Internal Error while locking status") });
                            }
                            let mut status_lock = lock_result.unwrap();
                            status_lock.current_connections_num=status_lock.current_connections_num-1;
                            self.bag.remove_entry(conn.id);
                            self.get_connection_internal()
                        }
                    }
                }
            }
        }
    }
}

pub struct PooledConnection<T>
    where
        T: ConnectionFactory,
{
    pool: ConnectionPool<T>,
    conn: Arc<BagEntry<T::Connection>>,
}

impl<'a, T> Drop for PooledConnection<T>
    where
        T: ConnectionFactory,
{
    fn drop(&mut self) {
        self.pool.release_connection(&(self.conn.as_ref()));
    }
}

impl<'a, T> Deref for PooledConnection<T>
    where
        T: ConnectionFactory,
{
    type Target = T::Connection;

    fn deref(&self) -> &Self::Target {
        &self.conn.as_ref().value()
    }
}

impl<'a, T: ConnectionFactory> PooledConnection<T> {
    pub fn new(pool: ConnectionPool<T>, conn: Arc<BagEntry<T::Connection>>) -> PooledConnection<T> {
        PooledConnection { pool, conn }
    }
}
