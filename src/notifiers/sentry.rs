use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::notifiers::Notifier;
use sentry::protocol::Event;

pub struct SentryNotifier {
    client: sentry::Client,
}

impl SentryNotifier {
    pub fn from_dsn(dsn: &str) -> Self {
        SentryNotifier { client: dsn.into() }
    }
}

impl Notifier for SentryNotifier {
    fn notify(&self, name: String, early: Option<u64>) {
        let mut tags = BTreeMap::new();
        tags.insert("switch".to_owned(), name.clone());

        let fp = format!("{}={}", name, early.map_or_else(|| "FAIL", |_| "EARLY"));

        self.client.capture_event(
            Event {
                tags,
                logger: Some("condemn".to_owned()),
                fingerprint: Cow::Owned(vec![Cow::Owned(fp)]),
                message: Some(match early {
                    Some(secs) => format!("Switch `{}` checked in early by {} seconds", name, secs),
                    None => format!("Switch `{}` failed to make its deadline.", name),
                }),
                ..Default::default()
            },
            None,
        );
    }
}
