// Mock StateMachine implementation for testing
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::state_machine::{StateMachine, errors::Error};

#[derive(Clone)]
pub struct MockStateMachine {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MockStateMachine {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_data(&self) -> HashMap<Vec<u8>, Vec<u8>> {
        self.data.lock().unwrap().clone()
    }
}

impl StateMachine for MockStateMachine {
    fn set(&self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        self.data.lock().unwrap().insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }
}

