use hkcp::ConnectionPool;
use crate::common::FakeDbDriver;

mod common;

#[test]
fn test_pool_max_size_ok() {
    let mut my_pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new(FakeDbDriver {});

    let mut conns = vec![];
    for _ in 0..2 {
        conns.push(my_pool.get_connection().ok().unwrap());
    }
}

#[test]
fn test_pool_acquire_release() {
    let mut my_pool: ConnectionPool<FakeDbDriver> =
        ConnectionPool::new(FakeDbDriver {});

    let conn1 = my_pool.get_connection().ok().unwrap();
    let conn2 = my_pool.get_connection().ok().unwrap();
    drop(conn1);
    let conn3 = my_pool.get_connection().ok().unwrap();
    drop(conn2);
    drop(conn3);
}
