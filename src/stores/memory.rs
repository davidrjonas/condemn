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
    switches: Arc<RwLock<BTreeMap<i64, HashMap<String, Switch>>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            switches: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

impl Store for MemoryStore {
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        // TODO: don't copy switches
        let all: Vec<Switch> = self
            .switches
            .read()
            .iter()
            .map::<Vec<Switch>, _>(|(_, m)| m.iter().map(|(_, s)| s.to_owned()).collect())
            .flatten()
            .collect();

        Box::new(ok(all))
    }

    fn expired(&self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        let expired: Vec<i64> = self
            .switches
            .read()
            .range(0..when.timestamp())
            .map(|(&k, _)| k)
            .collect();

        let condemned = expired
            .iter()
            .filter_map(|k| self.switches.write().remove(k))
            .map::<Vec<Switch>, _>(|mut m| m.drain().map(|(_, v)| v).collect())
            .flatten()
            .collect();

        Box::new(ok(condemned))
    }

    fn insert(&self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send> {
        debug!("inserting: {:?}", s);

        self.switches
            .write()
            .entry(s.deadline.timestamp())
            .or_default()
            .insert(s.name.clone(), s.clone());

        debug!("switches: {:?}", self.switches);

        Box::new(futures::future::ok(()))
    }

    fn take(&self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send> {
        let s = self
            .switches
            .write()
            .iter_mut()
            .find_map(|(_, m)| m.remove(name));
        Box::new(ok(s))
    }
}
