pub mod rocksdb;
pub mod errors;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub mod mock;

use errors::Error;

pub trait StateMachine {
    fn set(&self, key: &[u8], value: &[u8]) -> Result<(), Error>;
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error>;
    // fn snapshot();
}