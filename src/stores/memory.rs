#![allow(dead_code)]
use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use futures::future::ok;
use futures::Future;

use crate::stores::Store;
use crate::Switch;

#[derive(Debug, Clone)]
pub struct MemoryStore {
    ordered: BTreeMap<i64, Vec<String>>,
    switches: HashMap<String, Switch>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            ordered: BTreeMap::new(),
            switches: HashMap::new(),
        }
    }
}

impl Store for MemoryStore {
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()>> {
        Box::new(ok(self
            .ordered
            .iter()
            .map(|(_, v)| v)
            .flatten()
            .filter_map(|name| self.switches.get(name))
            .map(|v| v.to_owned())
            .collect()))
    }

    fn expired(&mut self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()>> {
        let ordered = &mut self.ordered;
        let switches = &mut self.switches;

        let expired: Vec<Switch> = ordered
            .range_mut(0..when.timestamp())
            .map(|(_, v)| v)
            .flatten()
            .filter_map(|name| switches.remove(name))
            .collect();

        for s in &expired {
            self.switches.remove(&s.name);
        }

        Box::new(ok(expired))
    }

    fn insert(&mut self, s: Switch) -> Box<Future<Item = (), Error = ()>> {
        self.switches.insert(s.name.clone(), s.clone());

        self.ordered
            .entry(s.deadline.timestamp())
            .or_default()
            .push(s.name.clone());

        Box::new(futures::future::ok(()))
    }

    fn take(&mut self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()>> {
        Box::new(futures::future::ok(self.switches.remove(name)))
    }
}
