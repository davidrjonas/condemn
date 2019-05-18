#![allow(dead_code)]
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::ok;
use futures::Future;
use parking_lot::RwLock;

use crate::stores::Store;
use crate::Switch;

#[derive(Debug, Clone)]
pub struct MemoryStore {
    ordered: Arc<RwLock<BTreeMap<i64, Vec<String>>>>,
    switches: Arc<RwLock<HashMap<String, Switch>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            ordered: Arc::new(RwLock::new(BTreeMap::new())),
            switches: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Store for MemoryStore {
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        let all = self
            .ordered
            .read()
            .iter()
            .map(|(_, v)| v)
            .flatten()
            .filter_map(|name| self.switches.read().get(name).map(|s| s.to_owned()))
            .collect();

        Box::new(ok(all))
    }

    fn expired(
        &mut self,
        when: DateTime<Utc>,
    ) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        let switches = self.switches.clone();

        let expired: Vec<Switch> = self
            .ordered
            .write()
            .range_mut(0..when.timestamp())
            .map(|(_, v)| v)
            .flatten()
            .filter_map(|name| switches.write().remove(name))
            .collect();

        Box::new(ok(expired))
    }

    fn insert(&mut self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send> {
        self.switches.write().insert(s.name.clone(), s.clone());

        self.ordered
            .write()
            .entry(s.deadline.timestamp())
            .or_default()
            .push(s.name.clone());

        Box::new(futures::future::ok(()))
    }

    fn take(&mut self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send> {
        let s = self.switches.write().remove(name);
        Box::new(ok(s))
    }
}
