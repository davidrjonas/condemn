use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use futures::future::{err, ok, Either};
use futures::stream::Stream;
use futures::Future;
use log::{info, warn};

use crate::stores::Store;
use crate::Switch;

#[derive(Debug)]
pub struct DiskStore<S: Store> {
    filename: PathBuf,
    store: S,
}

impl<S: 'static + Clone + Store + Send + Sync> DiskStore<S> {
    pub fn new<P: AsRef<Path>>(store: S, filename: P) -> Self {
        Self {
            filename: filename.as_ref().to_path_buf(),
            store: store,
        }
    }
}

impl<S: 'static + Clone + Store + Send + Sync> Store for DiskStore<S> {
    fn init(&self) -> Box<Future<Item = (), Error = ()> + Send> {
        info!("Loading data from '{:?}'", self.filename);

        let r = self.store.clone();
        let filename = self.filename.clone();

        let result: Result<Vec<Switch>, _> = OpenOptions::new()
            .read(true)
            .open(&self.filename)
            .and_then(|fh| {
                Ok(serde_json::from_reader(fh).unwrap_or_else(|e| {
                    warn!("failed to deserialize db file '{:?}'; {}", self.filename, e);
                    vec![]
                }))
            });

        let f = match result {
            Err(e) => Either::A({
                warn!("failed to open db file '{:?}'; {}", self.filename, e);
                err(())
            }),
            Ok(data) => Either::B(
                futures::stream::futures_unordered(
                    data.into_iter().map(|sw: Switch| self.store.insert(sw)),
                )
                .collect()
                .and_then(move |_| {
                    r.all().and_then(|data: Vec<Switch>| {
                        write_switches(filename, &data).unwrap();
                        ok(())
                    })
                }),
            ),
        };

        Box::new(f)
    }

    fn all(&self) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        self.store.all()
    }

    fn expired(&self, when: DateTime<Utc>) -> Box<Future<Item = Vec<Switch>, Error = ()> + Send> {
        let filename = self.filename.clone();
        let w = self.store.clone();

        // Sync before pop'ing off the expired ones. Better safe than sorry.
        Box::new(
            self.store
                .all()
                .and_then(move |data: Vec<Switch>| {
                    write_switches(filename, &data).unwrap();
                    ok(())
                })
                .and_then(move |_| w.expired(when)),
        )
    }

    fn insert(&self, s: Switch) -> Box<Future<Item = (), Error = ()> + Send> {
        let filename = self.filename.clone();
        let r = self.store.clone();

        let f = self.store.insert(s).and_then(move |_| {
            r.all().and_then(|data: Vec<Switch>| {
                write_switches(filename, &data).unwrap();
                ok(())
            })
        });

        Box::new(f)
    }

    fn take(&self, name: &str) -> Box<Future<Item = Option<Switch>, Error = ()> + Send> {
        let filename = self.filename.clone();
        let r = self.store.clone();

        // Sync _after_ the take() here. Why? Because we expect it to be gone.
        let f = self.store.take(name).and_then(move |s| {
            r.all().and_then(move |data: Vec<Switch>| {
                write_switches(filename, &data).unwrap();
                ok(s)
            })
        });

        Box::new(f)
    }
}

fn write_switches<P: AsRef<Path>>(filename: P, switches: &[Switch]) -> Result<(), std::io::Error> {
    // TODO: handle unwrap()
    let json = serde_json::to_vec(switches).unwrap();
    write_file(filename, &json)
}

fn write_file<P: AsRef<Path>>(filename: P, data: &[u8]) -> Result<(), std::io::Error> {
    // TODO: write to temp file first
    // TODO: explore ser::to_writer()
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(filename)?
        .write_all(data)
}
