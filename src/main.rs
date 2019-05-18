use std::cmp::Ordering;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use clap::{crate_authors, crate_version, App, Arg};
use futures::future::{lazy, ok, Either};
use futures::{Future, Stream};
use log::{info, warn};
use serde_derive::{Deserialize, Serialize};
use serde_humantime::De;
use tokio::timer::Interval;
use warp::{filters, http::StatusCode, Filter};

mod notifiers;
mod stores;

use notifiers::{AggregateNotifier, Notifier};
use stores::{MemoryStore, Store};

#[derive(Deserialize)]
struct Options {
    deadline: De<Option<Duration>>,
    window: De<Option<Duration>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Switch {
    name: String,
    deadline: DateTime<Utc>,
    window_start: Option<DateTime<Utc>>,
}

fn store_check_notify<S: Store>(
    store: Arc<S>,
    notifier: Arc<AggregateNotifier<'static>>,
) -> impl Future<Item = (), Error = ()> {
    store.expired(Utc::now()).and_then(move |switches| {
        switches.iter().for_each(|sw| {
            info!("sw: {:?}", sw);
            notifier.notify(sw.name.clone(), None)
        });
        ok(())
    })
}

fn notify_on_switch(s: &Switch, notifier: Arc<AggregateNotifier<'static>>, checkin_only: bool) {
    let now = Utc::now();

    match s.deadline.cmp(&now) {
        Ordering::Less => {
            // Late?! this shouldn't happen (the switch should have already notified and been
            // removed). So we should only notify if it looks like this switch is just checking in
            // and not setting a new switch.
            if checkin_only {
                warn!("Late check-in; name={}, deadline={}", s.name, s.deadline);
                notifier.notify(s.name.clone(), None);
            }
        }
        Ordering::Equal => {
            // Right on the money? What are the odds. We'll let this count as "within the window"
            // regardless of the window duration.
        }
        Ordering::Greater => {
            // Check-in before the deadline, that's good. No need to notify unless it is not within the window.
            s.window_start
                .filter(|ws| ws > &now)
                .and_then::<DateTime<Utc>, _>(|ws| {
                    let secs = ws.timestamp() - now.timestamp();
                    notifier.notify(s.name.clone(), Some(secs as u64));
                    None
                });
        }
    }
}

fn store_handle<S: Store>(
    store: Arc<S>,
    name: String,
    opts: Options,
    notifier: Arc<AggregateNotifier<'static>>,
) -> impl Future<Item = warp::reply::WithStatus<&'static str>, Error = warp::Rejection> {
    let deadline = opts.deadline.into_inner();
    let window = opts.window.into_inner();
    let checkin_only = deadline.is_none();
    let later = store.clone();

    store
        .take(&name)
        .and_then(move |maybe_switch| {
            let status = match maybe_switch {
                None => StatusCode::NOT_FOUND,
                Some(s) => {
                    notify_on_switch(&s, notifier, checkin_only);
                    StatusCode::OK
                }
            };

            match deadline {
                None => Either::A(ok(status)),
                Some(deadline) => {
                    let new_deadline = Utc::now()
                        .checked_add_signed(chrono::Duration::from_std(deadline).unwrap())
                        .unwrap();

                    let new_window = window
                        .map(|d| chrono::Duration::from_std(d).unwrap())
                        .map(|d| new_deadline.checked_sub_signed(d).unwrap());

                    let s = Switch {
                        name: name.clone(),
                        deadline: new_deadline,
                        window_start: new_window,
                    };

                    Either::B(later.insert(s).map(|_| StatusCode::CREATED))
                }
            }
        })
        .map_err(|_| warp::reject::custom("Internal Store Error"))
        .map(|code| warp::reply::with_status("", code))
}

fn list_handle<S: Store>(
    store: Arc<S>,
) -> impl Future<Item = impl warp::Reply, Error = warp::Rejection> {
    store
        .all()
        .map_err(|_| warp::reject::custom("Internal Store Error"))
        .map(|data| warp::reply::json(&data))
}

fn valid_listen(v: String) -> Result<(), String> {
    match v.parse::<SocketAddr>() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{}", e)),
    }
}

fn valid_redis_url(v: String) -> Result<(), String> {
    match redis::parse_redis_url(&v) {
        Ok(_) => Ok(()),
        Err(_) => Err(format!("unknown format; See help.")),
    }
}

fn valid_notify_command(v: String) -> Result<(), String> {
    match shell_words::split(&v) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{}", e)),
    }
}

fn main() -> Result<(), i16> {
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "condemn=info");
    }

    pretty_env_logger::init_timed();

    let app = App::new("condemn")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("listen")
                .short("l")
                .long("listen")
                .takes_value(true)
                .env("LISTEN")
                .validator(valid_listen)
                .help("The IP and port to listen on.")
                .default_value("0.0.0.0:80"),
        )
        .arg(
            Arg::with_name("redis-url")
                .short("r")
                .long("redis-url")
                .takes_value(true)
                .env("REDIS_URL")
                .validator(valid_redis_url)
                .help("The URL for Redis with database; redis://host:port/db")
                .default_value("redis://127.0.0.1:6379"),
        )
        .arg(
            Arg::with_name("notify")
                .short("n")
                .long("notify")
                .takes_value(true)
                .multiple(true)
                .possible_values(&["sentry"])
                .env("NOTIFY")
                .help("The notifiers to use. May require other options to be set, such as `sentry-dsn`. The Command notifier is configured separately, see `--notify-command`."),
        )
        .arg(
            Arg::with_name("notify-command")
                .short("c")
                .long("notify-command")
                .takes_value(true)
                .env("NOTIFY_COMMAND")
                .validator(valid_notify_command)
                .help("Command to run on notify. CONDEMN_NAME env var will be set. CONDEMN_EARLY env var will be set to the number of seconds, 0 if deadlined."),
        )
        .arg(
            Arg::with_name("sentry-dsn")
                .long("sentry-dsn")
                .takes_value(true)
                .env("SENTRY_DSN")
                .required_if("notify", "sentry")
                .help("Configures `sentry` notifier. If notify includes 'sentry', `sentry-dsn` is required."),
        )
        .get_matches();

    let listen: SocketAddr = app
        .value_of("listen")
        .expect("--listen should have a default")
        .parse()
        .expect("validator missed value of listen");

    let _redis_url = app
        .value_of("redis-url")
        .expect("redis-url should have default");

    // ### Notifier

    let mut notifier = AggregateNotifier::new();
    notifier.push(notifiers::LogNotifier {});

    if let Some(s) = app.value_of("notify-command") {
        let cmd = shell_words::split(s)
            .expect("notify command should have been validated. This is a bug.");
        notifier.push(notifiers::CommandNotifier { cmd });
    }

    for notify in app.values_of("notify").unwrap_or_default() {
        match notify {
            "sentry" => notifier.push(notifiers::SentryNotifier::from_dsn(
                app.value_of("sentry-dsn")
                    .expect("required if sentry is set"),
            )),
            // *** Add other notifiers here ***
            _ => panic!("unhandled `--notify` type. This is a bug."),
        }
    }

    let notifier = Arc::new(notifier);
    let handle_notifier = Arc::clone(&notifier);
    let watcher_notifier = Arc::clone(&notifier);

    // ### Warp

    let store = Arc::new(MemoryStore::new());
    let list_store = Arc::clone(&store);
    let watcher_store = Arc::clone(&store);

    // `GET /`
    let list = warp::get2()
        .and(warp::any().map(move || Arc::clone(&list_store)))
        .and_then(list_handle);

    // `GET /:switch`
    let create = warp::get2()
        .and(warp::any().map(move || Arc::clone(&store)))
        .and(warp::path::param())
        .and(filters::query::query())
        .and(warp::any().map(move || Arc::clone(&handle_notifier)))
        .and_then(store_handle);

    // `create` must come first or `list` will capture everything.
    let routes = create.or(list).with(warp::log("condemn"));
    let (_, serve) = warp::serve(routes).bind_ephemeral(listen);

    // ### Watcher

    let watcher = Interval::new_interval(Duration::from_secs(1))
        .map_err(|_| ())
        .for_each(move |_| {
            store_check_notify(Arc::clone(&watcher_store), Arc::clone(&watcher_notifier))
        });

    // ### All reved up and ready to go
    info!("Listening on {}", listen);

    tokio::run(lazy(|| {
        tokio::spawn(watcher);
        serve
    }));

    Ok(())
}
