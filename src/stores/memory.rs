use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::ok;
use futures::Future;
use log::debug;
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

    fn expired(&self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        let switches = self.switches.clone();

        let expired: Vec<i64> = self
            .ordered
            .read()
            .range(0..when.timestamp())
            .map(|(k, _)| *k)
            .collect();

        let condemned: Vec<Switch> = expired
            .iter()
            .filter_map(|k| self.ordered.write().remove(k))
            .flatten()
            .filter_map(|name: String| switches.write().remove(&name))
            .collect();

        debug!("condemned: {}", serde_json::to_string(&condemned).unwrap());
        Box::new(ok(condemned))
    }

    fn insert(&self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send> {
        debug!("inserting: {:?}", s);

        self.switches.write().insert(s.name.clone(), s.clone());

        debug!("switches: {:?}", self.switches);

        self.ordered
            .write()
            .entry(s.deadline.timestamp())
            .or_default()
            .push(s.name.clone());

        debug!("ordered: {:?}", self.ordered);

        Box::new(futures::future::ok(()))
    }

    fn take(&self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send> {
        let s = self.switches.write().remove(name);
        Box::new(ok(s))
    }
}
