use chrono::{DateTime, Utc};

use crate::Switch;
use futures::Future;

pub mod disk;
pub mod memory;

pub use disk::DiskStore;
pub use memory::MemoryStore;

pub trait Store {
    fn insert(&mut self, s: Switch) -> Box<Future<Item = (), Error = ()>>;
    fn expired(&mut self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()>>;
    fn take(&mut self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()>>;
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()>>;
}
