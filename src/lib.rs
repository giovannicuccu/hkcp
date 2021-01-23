use std::ops::Deref;
use std::sync::{Arc};
use std::{fmt, error, thread};
use std::time::Duration;
use crossbeam_channel::unbounded;
use parking_lot::{Condvar, Mutex};
use std::sync::atomic::{AtomicU16, Ordering};
use crate::concurrent_bag_list::ConcurrentBag;

mod concurrent_bag_list;

#[cfg(test)]
mod test;



pub struct ConnectionPoolConfig {
    pub initial_connections: u16,
    pub max_connections: u16,
    pub connect_timeout_millis: u32,
    pub get_timeout_millis: u32,
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

pub struct InternalPool<T>
    where
        T: ConnectionFactory,
{
    bag: ConcurrentBag<T::Connection>,
    status: Mutex<ConnectionPoolStatus>,
    available_entries: AtomicU16,
    connection_factory: Arc<T>,
    config: ConnectionPoolConfig,
    condvar: Condvar,
}

fn create_initial_connections<T: ConnectionFactory>(status : &Mutex<ConnectionPoolStatus>,
                                                    pool_config: &ConnectionPoolConfig,
                                                    conn_num: &AtomicU16,
                                                    bag : &ConcurrentBag<T::Connection>,
                                                    connection_factory: &Arc<T>)->Result<(),HkcpError> {
    let lock_result = status.lock();
/*    if lock_result.is_err() {
        return Err(HkcpError { message: String::from("Internal Error while locking status") });
    }*/
    let mut status_lock = lock_result;
    for _ in 0..pool_config.initial_connections {
        create_connection(pool_config, &mut status_lock, conn_num, bag,
                                              connection_factory)?;
    }
    Ok(())
}

fn create_connection<T: ConnectionFactory>(config: &ConnectionPoolConfig,
                                           status : &mut ConnectionPoolStatus,
                                           conn_num: &AtomicU16,
                                           bag : &ConcurrentBag<T::Connection>,
                                           connection_factory: &Arc<T>)->Result<(),HkcpError> {
    let connection_factory_in_thread = connection_factory.clone();
    let (tx, rx) = unbounded();
    thread::spawn(move || {
        let conn_res = connection_factory_in_thread.connect();
        tx.send(conn_res)
    });
    let thread_result = rx.recv_timeout(Duration::from_millis(config.connect_timeout_millis as u64));
    match thread_result {
        Ok(conn_res) => {
            match conn_res {
                Ok(conn) => {
                    status.current_connections_num += 1;
                    conn_num.fetch_add(1, Ordering::SeqCst);
                    bag.release_entry(conn);
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

impl<T: ConnectionFactory> InternalPool<T> {

    pub fn new_with_config(
        connection_factory: T,
        pool_config: ConnectionPoolConfig,
    ) -> Result<InternalPool<T>,HkcpError> {
        let initial_status =Mutex::new(ConnectionPoolStatus {
            current_connections_num: 0,
        });
        let initial_config=pool_config;
        let initial_bag = ConcurrentBag::new();
        let initial_connection_factory= Arc::new(connection_factory);
        let initial_available_entries= AtomicU16::new(0);
        let create_result=create_initial_connections(&initial_status,
                                                     &initial_config, &initial_available_entries,
                                                     &initial_bag,
                                                     &initial_connection_factory);

        match create_result {
            Ok(_) => {
                Ok(InternalPool {
                    bag: initial_bag,
                    status: initial_status,
                    available_entries: initial_available_entries,
                    connection_factory: initial_connection_factory,
                    config: initial_config,
                    condvar: Condvar::new(),
                })
            }
            Err(error) => {
                Err(error)
            }
        }
    }

    pub fn new(connection_factory: T) -> Result<InternalPool<T>,HkcpError> {
        InternalPool::new_with_config(connection_factory, ConnectionPoolConfig {
            initial_connections: 1,
            max_connections: 2,
            connect_timeout_millis: 500,
            get_timeout_millis: 5000,
        })
    }



    fn release_connection(&self, entry: T::Connection) {
        self.bag.release_entry(entry);
        self.condvar.notify_one();
    }

    pub fn get_connection(&self) -> Result<T::Connection, HkcpError> {
        let opt_entry = self.bag.borrow_entry();
        if let Some(conn) = opt_entry {
            return if self.connection_factory.is_valid(&conn) {
                Ok(conn)
            } else {
                println!("fail to get a connection try lock");
                let lock_result = self.status.lock();
/*                if lock_result.is_err() {
                    return Err(HkcpError { message: String::from("Internal Error while locking status") });
                }*/
                let mut status_lock = lock_result;
                status_lock.current_connections_num -= 1;
                self.get_connection()
            }
        }
        println!("fail to get a connection get internal");
        let create_result = self.get_connection_internal();
        match create_result {
            Ok(bag_entry) => {
                Ok(bag_entry)
            }
            Err(err) => {
                Err(err)
            }
        }


    }

    fn get_connection_internal(&self)->Result<T::Connection,HkcpError> {

        let lock_result = self.status.lock();
/*        if lock_result.is_err() {
            return Err(HkcpError { message: String::from("Internal Error while locking status") });
        }*/
        let mut status_lock = lock_result;
        if status_lock.current_connections_num < self.config.max_connections {
            let create_result = create_connection(&self.config, &mut status_lock,
                                                  &self.available_entries,
                                                  &self.bag,
                                                  &self.connection_factory);
            match create_result {
                Ok(()) => {
                    status_lock.current_connections_num += 1;
                    let opt_entry = self.bag.borrow_entry();
                    match opt_entry {
                        Some(opt_entry) => Ok(opt_entry),
                        None => Err(HkcpError { message: String::from("Internal Error while getting entry from bag") }),
                    }
                }
                Err(error) => Err(HkcpError { message: error.to_string() }),
            }
        } else {
            //loop {
                let result = self.condvar
                    .wait_for(
                        &mut status_lock,
                        Duration::from_millis(self.config.get_timeout_millis as u64),
                    );
                //status_lock = result.0;
                if result.timed_out() {
                    Err(HkcpError { message: String::from("No available connection in pool"),
                    })
                } else {
                    let opt_entry = self.bag.borrow_entry();
                    match opt_entry {
                        Some(conn) => {
                            if self.connection_factory.is_valid(&conn) {
                                Ok(conn)
                            } else {
                                let lock_result = self.status.lock();
    /*                            if lock_result.is_err() {
                                    return Err(HkcpError { message: String::from("Internal Error while locking status") });
                                }*/
                                let mut status_lock = lock_result;
                                status_lock.current_connections_num -= 1;
                                self.get_connection_internal()
                            }
                        },
                        None => Err(HkcpError { message: String::from("No available connection in pool"), }),
                    }
                }
            //}
        }
    }
}



pub struct ConnectionPool<T>(Arc<InternalPool<T>>)
    where
        T: ConnectionFactory;

impl<T> Clone for ConnectionPool<T>
    where
        T: ConnectionFactory,
{
    fn clone(&self) -> ConnectionPool<T> {
        ConnectionPool(self.0.clone())
    }
}
impl<T: ConnectionFactory> ConnectionPool<T> {
    pub fn new_with_config(
        connection_factory: T,
        pool_config: ConnectionPoolConfig,
    ) -> Result<ConnectionPool<T>,HkcpError> {
        let internal_result=InternalPool::new_with_config(connection_factory, pool_config);
        match internal_result {
            Ok(internal_pool) => {
                Ok(ConnectionPool(Arc::new(internal_pool)))
            }
            Err(error) => {
                Err(error)
            }
        }

    }

    pub fn new(
        connection_factory: T
    ) -> Result<ConnectionPool<T>,HkcpError> {
        let internal_result=InternalPool::new(connection_factory);
        match internal_result {
            Ok(internal_pool) => {
                Ok(ConnectionPool(Arc::new(internal_pool)))
            }
            Err(error) => {
                Err(error)
            }
        }

    }


    pub fn get_connection(&self) -> Result<PooledConnection<T>, HkcpError> {
        let get_result=self.0.get_connection();
        match get_result {
            Ok(connection) => {
                Ok(PooledConnection::new(self.clone(),connection))
            }
            Err(error) => {
                Err(error)
            }
        }
    }

    pub fn release_connection(&self, entry: T::Connection) {
        self.0.release_connection(entry);
    }
}

pub struct PooledConnection<T>
    where
        T: ConnectionFactory,
{
    pool: ConnectionPool<T>,
    conn: Option<T::Connection>,
}

impl<'a, T> Drop for PooledConnection<T>
    where
        T: ConnectionFactory,
{
    fn drop(&mut self) {
        self.pool.release_connection(self.conn.take().unwrap());
    }
}

impl<'a, T> Deref for PooledConnection<T>
    where
        T: ConnectionFactory,
{
    type Target = T::Connection;

    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().unwrap()
    }
}

impl<'a, T: ConnectionFactory> PooledConnection<T> {
    pub fn new(pool: ConnectionPool<T>, conn: T::Connection) -> PooledConnection<T> {
        PooledConnection { pool, conn:Some(conn) }
    }
}
