use core::fmt;
use hkcp::ConnectionFactory;
use std::thread;
use std::time::Duration;

pub struct FakeConnection {}

#[derive(Debug, Clone)]
pub struct FakeError {}

impl std::error::Error for FakeError {
    fn description(&self) -> &str {
        "Error"
    }
}

impl fmt::Display for FakeError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Error")
    }
}

pub struct FakeDbDriver {}

impl ConnectionFactory for FakeDbDriver {
    type Connection = FakeConnection;
    type Error = FakeError;

    fn connect(&self) -> Result<FakeConnection, FakeError> {
        Ok(FakeConnection {})
    }
}

pub struct FakeDbWithTimeoutDriver {
    pub(crate) timeout_mills:u32,
}

impl ConnectionFactory for FakeDbWithTimeoutDriver {
    type Connection = FakeConnection;
    type Error = FakeError;

    fn connect(&self) -> Result<FakeConnection, FakeError> {
        thread::sleep(Duration::from_millis(self.timeout_mills as u64));
        Ok(FakeConnection {})
    }
}
