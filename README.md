# hkcp



A generic connection pool for Rust.

A crate for pooling connections. 

Basically this is a port of the ideas behind [HikariCP](https://github.com/brettwooldridge/HikariCP), the most performant java connection pool.
The Rust code structs are deeply inspired from another Rust connection pool [r2d2](https://docs.rs/r2d2)

The aim of hkcp is to become one of the (in not the) most performant connection pool in Rust