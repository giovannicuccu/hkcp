mod fake_driver;

use std::sync::atomic::{Ordering, AtomicU16};
use std::sync::Arc;
use std::{thread, time};
use rand::thread_rng;
use std::time::Instant;
use hkcp::{ConnectionPool, ConnectionPoolConfig};
use crate::fake_driver::FakeDbDriver;

fn main() {
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
            let mut rng = thread_rng();
            //let randomvalue = 25;//rng.gen_range(20, 30);
            //let ten_millis = time::Duration::from_millis(randomvalue);
            //thread::sleep(ten_millis);
            let opt_str =pool_for_thread.get_connection().ok();
            if opt_str.is_some() {
                let str = opt_str.unwrap();
                let mut rng = thread_rng();
                //let randomvalue = 5;//rng.gen_range(0, 5);
                //let ten_millis = time::Duration::from_millis(randomvalue);
                //thread::sleep(ten_millis);
                ok_for_thread.fetch_add(1, Ordering::SeqCst);
            } else {
                ko_for_thread.fetch_add(1, Ordering::SeqCst);
            }
            /*let randomvalue = 20;//rng.gen_range(0, 30);
            let ten_millis = time::Duration::from_millis(randomvalue);
            thread::sleep(ten_millis);
            let opt_str =pool_for_thread.get().ok();
            if opt_str.is_some() {
                let str = opt_str.unwrap();
                let mut rng = thread_rng();
                let randomvalue = 5;//rng.gen_range(0, 5);
                let ten_millis = time::Duration::from_millis(randomvalue);
                thread::sleep(ten_millis);
                ok_for_thread.fetch_add(1, Ordering::SeqCst);
            } else {
                ko_for_thread.fetch_add(1, Ordering::SeqCst);
            }*/
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