use hkcp::{ConnectionPool, ConnectionPoolConfig};
use crate::common::{FakeDbDriver, FakeDbWithTimeoutDriver};
use std::{thread, time};
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

mod common;

#[test]
fn test_pool_max_size_ok() {
    let my_pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new(FakeDbDriver {}).unwrap();

    let mut conns = vec![];
    for _ in 0..2 {
        conns.push(my_pool.get_connection().ok().unwrap());
    }
}

#[test]
fn test_pool_acquire_release() {
    let my_pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new(FakeDbDriver {}).unwrap();

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
        ).unwrap();
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
        thread::sleep(Duration::from_millis(250));
        drop(succeeds_delayed);
    }));

    let fails = pool.get_connection();
    assert!(fails.is_err());
}

#[test]
fn initial_connect_timeout_err() {
    let new_result =
        ConnectionPool::new_with_config(
            FakeDbWithTimeoutDriver { timeout_mills: 150},
            ConnectionPoolConfig::new(1, 1, 100, 100),
        );
    assert!(new_result.is_err())
}

#[test]
fn connect_timeout_err() {
    let new_result =
        ConnectionPool::new_with_config(
            FakeDbWithTimeoutDriver { timeout_mills: 150},
            ConnectionPoolConfig::new(0, 1, 100, 100),
        );
    assert!(new_result.is_ok());
    let fail_first = new_result.unwrap().get_connection();
    assert!(fail_first.is_err());
}

#[test]
fn connect_timeout_ok() {
    let new_result =
        ConnectionPool::new_with_config(
            FakeDbWithTimeoutDriver { timeout_mills: 80},
            ConnectionPoolConfig::new(1, 1, 100, 100),
        );
    assert!(new_result.is_ok())
}

#[test]
fn test_multithread() {
    let my_pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new_with_config(FakeDbDriver {}, ConnectionPoolConfig{
            initial_connections: 10,
            max_connections: 10,
            connect_timeout_millis: 500,
            get_timeout_millis: 5000,
        }).unwrap();


    let shared_pool = Arc::new(my_pool);
    let ok_shared = Arc::new(AtomicU16::new(0));
    let ko_shared = Arc::new(AtomicU16::new(0));
    let mut handle_vec = vec![];
    let start = Instant::now();

    for _ in 0..20 {
        let pool_for_thread = Arc::clone(&shared_pool);
        let ok_for_thread = Arc::clone(&ok_shared);
        let ko_for_thread = Arc::clone(&ko_shared);
        let handle = thread::spawn(move || {
            let opt_str =pool_for_thread.get_connection().ok();
            if opt_str.is_some() {
                let str = opt_str.unwrap();
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
        "Time elapsed in test() is: {:?} ok={:?} ko={:?}",
        duration,
        ok_shared.load(Ordering::Relaxed),
        ko_shared.load(Ordering::Relaxed)
    );
}

#[test]
fn test_multithread_r2d2() {

    let my_pool = r2d2::Pool::builder()
        .max_size(10)
        .min_idle(Some(10))
        .build(FakeDbDriver {})
        .unwrap();

    let shared_pool = Arc::new(my_pool);
    let ok_shared = Arc::new(AtomicU16::new(0));
    let ko_shared = Arc::new(AtomicU16::new(0));
    let mut handle_vec = vec![];
    let start = Instant::now();

    for _ in 0..20 {
        let pool_for_thread = Arc::clone(&shared_pool);
        let ok_for_thread = Arc::clone(&ok_shared);
        let ko_for_thread = Arc::clone(&ko_shared);
        let handle = thread::spawn(move || {
            let opt_str =pool_for_thread.get().ok();
            if opt_str.is_some() {
                let str = opt_str.unwrap();
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
        "Time elapsed in test() is: {:?} ok={:?} ko={:?}",
        duration,
        ok_shared.load(Ordering::Relaxed),
        ko_shared.load(Ordering::Relaxed)
    );
}
