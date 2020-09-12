use crate::ConcurrentBag;
use std::ops::Deref;

#[test]
fn test_concurrent_bag_lease() {
    let concurrent_bag=ConcurrentBag::new();
    concurrent_bag.add_entry(String::from("hello Rust"));
    concurrent_bag.add_entry(String::from("hello Rust again"));
    let opt_entry_1=concurrent_bag.lease_entry();
    assert!(opt_entry_1.is_some());
    let opt_entry_2=concurrent_bag.lease_entry();
    assert!(opt_entry_2.is_some());
    let opt_entry_3=concurrent_bag.lease_entry();
    assert!(opt_entry_3.is_none());
}

#[test]
fn test_concurrent_bag_lease_release() {
    let concurrent_bag=ConcurrentBag::new();
    concurrent_bag.add_entry(String::from("hello Rust"));
    concurrent_bag.add_entry(String::from("hello Rust again"));
    let opt_entry_1=concurrent_bag.lease_entry();
    assert!(opt_entry_1.is_some());
    let opt_entry_2=concurrent_bag.lease_entry();
    assert!(opt_entry_2.is_some());
    concurrent_bag.release_entry(opt_entry_1.unwrap().deref());
    let opt_entry_3=concurrent_bag.lease_entry();
    assert!(opt_entry_3.is_some());
}

#[test]
fn test_concurrent_bag_remove() {
    let concurrent_bag=ConcurrentBag::new();
    let str_hello=String::from("hello Rust");
    let str_hello_again=String::from("hello Rust again");
    let id1=concurrent_bag.add_entry(str_hello);
    let id2=concurrent_bag.add_entry(str_hello_again);
    concurrent_bag.remove_entry(id1);
    concurrent_bag.remove_entry(id2);
    let opt_entry_1=concurrent_bag.lease_entry();
    assert!(opt_entry_1.is_none());

}

