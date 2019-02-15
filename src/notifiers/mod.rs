use log::info;

pub mod command;
pub mod sentry;

pub use self::sentry::SentryNotifier;
pub use command::Command as CommandNotifier;

pub trait Notifier {
    fn notify(&self, name: String, early: Option<u64>);
}

pub struct AggregateNotifier<'a> {
    notifiers: Vec<Box<'a + Notifier + Send + Sync>>,
}

impl<'a> AggregateNotifier<'a> {
    pub fn new() -> Self {
        Self { notifiers: vec![] }
    }

    pub fn push<T: 'a + Notifier + Send + Sync>(&mut self, n: T) {
        self.notifiers.push(Box::new(n));
    }
}

impl<'a> Notifier for AggregateNotifier<'a> {
    fn notify(&self, name: String, early: Option<u64>) {
        for n in &self.notifiers {
            n.notify(name.clone(), early);
        }
    }
}

pub struct LogNotifier {}

impl Notifier for LogNotifier {
    fn notify(&self, name: String, early: Option<u64>) {
        if let Some(secs) = early {
            info!("notify early: name={}, early={}s", name, secs);
        } else {
            info!("notify late: name={}", name);
        }
    }
}
