use core::fmt;
use hkcp::ConnectionFactory;

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
