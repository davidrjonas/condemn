use chrono::{DateTime, Utc};

use crate::Switch;
use futures::Future;
use log::info;

pub mod disk;
pub mod memory;

pub use disk::DiskStore;
pub use memory::MemoryStore;

pub trait Store {
    fn init(&self) -> Box<Future<Item = (), Error = ()> + Send> {
        info!("default init");
        Box::new(futures::future::ok(()))
    }

    fn insert(&self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send>;
    fn expired(&self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send>;
    fn take(&self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send>;
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send>;
}

#[derive(Debug)]
pub enum Stores {
    Memory(MemoryStore),
    Disk(DiskStore<MemoryStore>),
}

impl Stores {
    pub fn memory() -> Stores {
        Stores::Memory(MemoryStore::new())
    }

    pub fn disk(filename: &str) -> Stores {
        Stores::Disk(DiskStore::new(MemoryStore::new(), filename))
    }
}

impl Store for Stores {
    fn init(&self) -> Box<Future<Item = (), Error = ()> + Send> {
        match self {
            Stores::Memory(store) => store.init(),
            Stores::Disk(store) => store.init(),
        }
    }

    fn insert(&self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send> {
        match self {
            Stores::Memory(store) => store.insert(s),
            Stores::Disk(store) => store.insert(s),
        }
    }
    fn expired(&self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        match self {
            Stores::Memory(store) => store.expired(when),
            Stores::Disk(store) => store.expired(when),
        }
    }
    fn take(&self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send> {
        match self {
            Stores::Memory(store) => store.take(name),
            Stores::Disk(store) => store.take(name),
        }
    }
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        match self {
            Stores::Memory(store) => store.all(),
            Stores::Disk(store) => store.all(),
        }
    }
}
