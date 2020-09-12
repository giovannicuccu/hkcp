use hkcp::{ConnectionPool, ConnectionPoolConfig};
use crate::common::FakeDbDriver;
use std::thread;
use std::time::Duration;

mod common;

#[test]
fn test_pool_max_size_ok() {
    let my_pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new(FakeDbDriver {});

    let mut conns = vec![];
    for _ in 0..2 {
        conns.push(my_pool.get_connection().ok().unwrap());
    }
}

#[test]
fn test_pool_acquire_release() {
    let my_pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new(FakeDbDriver {});

    let conn1 = my_pool.get_connection().ok().unwrap();
    let conn2 = my_pool.get_connection().ok().unwrap();
    drop(conn1);
    let conn3 = my_pool.get_connection().ok().unwrap();
    drop(conn2);
    drop(conn3);
}

#[test]
fn get_timeout() {
    let pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new_with_config(
            FakeDbDriver {},
            ConnectionPoolConfig::new(1, 1, 100, 100),
        );
    let mut children = vec![];
    let succeeds_immediately = pool.get_connection();
    assert!(succeeds_immediately.is_ok());
    children.push(thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        drop(succeeds_immediately);
    }));

    let succeeds_delayed = pool.get_connection();
    assert!(succeeds_delayed.is_ok());

    children.push(thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        drop(succeeds_delayed);
    }));

    let fails = pool.get_connection();
    assert!(fails.is_err());
}
