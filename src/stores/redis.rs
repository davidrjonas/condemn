use chrono::{DateTime, Utc};
use futures::future::err;
use futures::Future;
use log::warn;

use crate::stores::Store;
use crate::Switch;

const ORDERED_KEY: &'static str = "condemn_z";
const SWITCH_KEY: &'static str = "condemn_h";

#[derive(Debug)]
pub struct RedisStore {
    client: redis::Client,
}

/// RedisStore keeps a sorted set of names for expiry and a hash map of the names to json
/// serialized objects. When items are removed from the sorted set the names are looked up in the
/// hash map. If the name doesn't exist there then it is ignored. In this way Switches are not
/// leaked as long as _something_ is calling expired() on a regular basis.
impl RedisStore {
    pub fn new(url: &str) -> Self {
        RedisStore {
            client: redis::Client::open(url).unwrap(),
        }
    }
}

fn deserialize_switch(json: &str) -> Option<Switch> {
    match serde_json::from_str(json) {
        Ok(switch) => Some(switch),
        Err(e) => {
            warn!("failed to deserialize switch; err={}, data={}", e, json);
            None
        }
    }
}

fn serialize_switch(s: &Switch) -> Option<String> {
    match serde_json::to_string(s) {
        Ok(json) => Some(json),
        Err(e) => {
            warn!("failed to serialize switch; err={}, switch={:?}", e, s);
            None
        }
    }
}

impl Store for RedisStore {
    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        let mut hgetall = redis::cmd("HGETALL");
        hgetall.arg(SWITCH_KEY);

        let res = self
            .client
            .get_async_connection()
            .and_then(move |conn| hgetall.query_async(conn))
            .map_err(|e| warn!("redis failure; {:?}", e))
            .map(|(_, jsons): (_, Vec<String>)| {
                jsons
                    .iter()
                    .filter_map(|s| deserialize_switch(&s))
                    .collect()
            });

        Box::new(res)
    }

    fn expired(&self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        let mut zrange = redis::cmd("ZRANGEBYSCORE");
        zrange.arg(ORDERED_KEY);
        zrange.arg("-inf");
        zrange.arg(when.timestamp());

        let mut zrem = redis::cmd("ZREMRANGEBYSCORE");
        zrem.arg(ORDERED_KEY);
        zrem.arg("-inf");
        zrem.arg(when.timestamp());

        let mut expired = redis::pipe();
        expired.atomic();
        expired.add_command(&zrange);
        expired.add_command(&zrem).ignore();

        let getfn = |expired: Vec<String>| {
            let mut hmget = redis::cmd("HMGET");
            hmget.arg(SWITCH_KEY);
            hmget.arg(expired.clone());

            let mut hdel = redis::cmd("HDEL");
            hdel.arg(SWITCH_KEY);
            hdel.arg(expired);

            let mut p = redis::pipe();
            p.atomic();
            p.add_command(&hmget);
            p.add_command(&hdel).ignore();

            p
        };

        let res = self
            .client
            .get_async_connection()
            .and_then(move |conn| expired.query_async(conn))
            .and_then(move |(conn, expired): (_, Vec<String>)| getfn(expired).query_async(conn))
            .map_err(|e| warn!("redis failure; {:?}", e))
            .map(|(_, jsons): (_, Vec<String>)| {
                jsons
                    .iter()
                    .filter_map(|s| deserialize_switch(&s))
                    .collect()
            });

        Box::new(res)
    }

    fn insert(&self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send> {
        let serialized = match serialize_switch(&s) {
            Some(json) => json,
            None => return Box::new(err(())),
        };

        let mut hset = redis::cmd("HSET");
        hset.arg(SWITCH_KEY);
        hset.arg(serialized);

        let mut zadd = redis::cmd("ZADD");
        zadd.arg(ORDERED_KEY);
        zadd.arg(s.deadline.timestamp());
        zadd.arg(s.name);

        let mut p = redis::pipe();
        p.atomic();
        p.add_command(&hset);
        p.add_command(&zadd);

        let res = self
            .client
            .get_async_connection()
            .and_then(move |conn| p.query_async::<_, (i64, i64)>(conn))
            .map_err(|e| warn!("redis failure; {:?}", e))
            .map(|_| ());

        Box::new(res)
    }

    fn take(&self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send> {
        let mut hget = redis::cmd("HGET");
        hget.arg(SWITCH_KEY);
        hget.arg(name);

        let mut hrem = redis::cmd("HREM");
        hrem.arg(SWITCH_KEY);
        hrem.arg(name);

        let mut zrem = redis::cmd("ZREM");
        zrem.arg(ORDERED_KEY);
        zrem.arg(name);

        let mut p = redis::pipe();
        p.atomic();
        p.add_command(&hget);
        p.add_command(&hrem).ignore();
        p.add_command(&zrem).ignore();

        let res = self
            .client
            .get_async_connection()
            .and_then(move |conn| p.query_async(conn))
            .map_err(|e| warn!("redis failure; {:?}", e))
            .map(|(_, maybe_json): (_, Option<String>)| {
                maybe_json.map(|s| deserialize_switch(&s)).and_then(|y| y)
            });

        Box::new(res)
    }
}
