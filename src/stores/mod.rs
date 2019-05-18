use chrono::{DateTime, Utc};

use crate::Switch;
use futures::Future;

pub mod disk;
pub mod memory;

pub use disk::DiskStore;
pub use memory::MemoryStore;

pub trait Store {
    fn insert(&self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send>;
    fn expired(&self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send>;
    fn take(&self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send>;
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send>;
}
