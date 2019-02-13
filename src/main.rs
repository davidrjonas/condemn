use std::net::SocketAddr;
use std::process::Command;
use std::time::{Duration, SystemTime};

use clap::{crate_authors, crate_version, App, Arg};
use futures::future::{lazy, ok, Either};
use futures::Future;
use log::{debug, info, warn};
use serde_derive::Deserialize;
use serde_humantime::De;
use tokio::prelude::*;
use tokio::timer::Interval;
use tokio_process::CommandExt;
use warp::{filters, http::StatusCode, Filter};

#[derive(Deserialize)]
struct Options {
    deadline: De<Option<Duration>>,
    window: De<Option<Duration>>,
}

type ConnFut = redis::RedisFuture<redis::r#async::Connection>;

const Z_KEY: &'static str = "condemn_z";
const H_KEY: &'static str = "condemn_h";

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

fn check_notify(connect: ConnFut, notifier: Notifier) -> impl Future<Item = (), Error = ()> {
    debug!("check_notify!");

    let now_ts = now_ts();

    connect
        .and_then(move |conn| {
            redis::cmd("ZRANGEBYSCORE")
                .arg(Z_KEY)
                .arg("-inf")
                .arg(now_ts)
                .query_async::<_, Vec<String>>(conn)
        })
        .and_then(move |(conn, condemned)| match condemned.len() {
            0 => Either::A(ok((conn, 0))),
            c => {
                warn!("Removing {} items; [{}]", c, condemned.join(","));
                condemned
                    .iter()
                    .map(|name| notifier.notify(name.to_owned(), None))
                    .count();
                Either::B(
                    redis::cmd("ZREM")
                        .arg(Z_KEY)
                        .arg(condemned)
                        .query_async::<_, u8>(conn),
                )
            }
        })
        .map_err(|e| warn!("redis failure; {:?}", e))
        .map(|(_, _)| ())
}

#[derive(Clone)]
enum Notifier {
    Command(String),
    Noop,
}

impl Notifier {
    fn notify(&self, name: String, early: Option<u64>) {
        if let Some(secs) = early {
            info!("notify early: name={}, early={}s", name, secs);
        } else {
            info!("notify late: name={}", name);
        }

        match self {
            Notifier::Command(ref cmd) => self.notify_command(cmd, name, early),
            Notifier::Noop => (),
        }
    }

    fn notify_command(&self, cmd: &str, name: String, early: Option<u64>) {
        info!("running notify command: cmd={}", cmd);
        let cmd_array = cmd.split_whitespace().collect::<Vec<&str>>();

        let proc = Command::new(&cmd_array[0])
            .args(cmd_array[1..].into_iter())
            .env("CONDEMN_NAME", name)
            .env("CONDEMN_EARLY", format!("{}", early.unwrap_or(0)))
            .spawn_async();

        tokio::spawn(match proc {
            Ok(f) => Either::A(
                f.map(|status| info!("command exited with status {}", status))
                    .map_err(|e| warn!("failed to wait for exit: {}", e)),
            ),
            Err(e) => {
                warn!("failed to spawn command; {}", e);
                Either::B(ok(()))
            }
        });
    }
}

fn handle(
    connect: ConnFut,
    name: String,
    opts: Options,
    notifier: Notifier,
) -> impl Future<Item = warp::reply::WithStatus<&'static str>, Error = warp::Rejection> {
    // First we get the score so that we can make sure this check-in isn't early.
    // If we get a score then also get the window. Possibly notify early if we have a window.
    // If they set a new deadline, set it, otherwise clear deadline so it doesn't notify later.
    // If they set a window in addition to the deadline, set that. Clear the window if not to avoid
    // it being incorrectly set on a future non-window deadline.

    // Pre-calculate our timestamps so they can be based on each other and the same "now" easily.
    let now = SystemTime::now();

    let deadline = opts.deadline.into_inner().map(|dur| {
        (now + dur)
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
    });

    // Use `and_then()` because we may want to replace with None if there is no deadline.
    let window = opts.window.into_inner().and_then(|dur| match deadline {
        Some(deadline) => Some(deadline - dur),
        None => None,
    });

    connect
        .and_then(move |conn| {
            redis::cmd("ZSCORE")
                .arg(Z_KEY)
                .arg(name.clone())
                .query_async::<_, Option<u32>>(conn)
                .join(ok(name))
        })
        .and_then(|((conn, score), name)| match score {
            None => Either::A(ok(((conn, None), (name, 0)))),
            Some(score) => Either::B(
                redis::cmd("HGET")
                    .arg(H_KEY)
                    .arg(name.clone())
                    .query_async::<_, Option<u64>>(conn)
                    .join(ok((name, score))),
            ),
        })
        .and_then(move |((conn, window), (name, score))| match score {
            0 => ok((conn, name, false)),
            _ => match window {
                Some(window_start_ts) => {
                    let now = now_ts();
                    if window_start_ts > now {
                        notifier.notify(name.clone(), Some(window_start_ts - now))
                    }
                    ok((conn, name, true))
                }
                None => ok((conn, name, true)),
            },
        })
        .and_then(move |(conn, name, has_score)| match (deadline, has_score) {
            (Some(deadline_ts), _) => Either::A(
                redis::cmd("ZADD")
                    .arg(Z_KEY)
                    .arg(deadline_ts.as_secs())
                    .arg(name.clone())
                    .query_async::<_, u8>(conn)
                    .join(ok((name, has_score, true))),
            ),

            (None, true) => Either::A(
                redis::cmd("ZREM")
                    .arg(Z_KEY)
                    .arg(name.clone())
                    .query_async::<_, u8>(conn)
                    .join(ok((name, true, false))),
            ),
            (None, false) => {
                warn!("No deadline and no score, should return 404; name={}", name);
                Either::B(ok(((conn, 0), (name, false, false))))
            }
        })
        // Window will only be Some if deadline was Some. See the processing at the start of this
        // function.
        .and_then(
            move |((conn, _), (name, has_score, has_deadline))| match window {
                Some(window_ts) => redis::cmd("HSET")
                    .arg(H_KEY)
                    .arg(name.clone())
                    .arg(window_ts.as_secs())
                    .query_async::<_, u8>(conn)
                    .join(ok((has_score, has_deadline))),
                None => redis::cmd("HDEL")
                    .arg(H_KEY)
                    .arg(name)
                    .query_async(conn)
                    .join(ok((has_score, has_deadline))),
            },
        )
        .map_err(|e| {
            warn!("redis failure; {}", e);
            warp::reject::custom("")
        })
        .map(
            |((_, _), (has_score, has_deadline))| match (has_score, has_deadline) {
                (_, true) => warp::reply::with_status("", StatusCode::CREATED),
                (true, false) => warp::reply::with_status("", StatusCode::OK),
                (false, false) => warp::reply::with_status("", StatusCode::NOT_FOUND),
            },
        )
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

fn main() {
    pretty_env_logger::init();

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
                .env("NOTIFY")
                .help("Command to run on notify. CONDEMN_NAME env var will be set. If early, CONDEMN_EARLY env var will be set to the number of seconds."),
        )
        .get_matches();

    let listen: SocketAddr = app
        .value_of("listen")
        .expect("--listen should have a default")
        .parse()
        .expect("validator missed value of listen");

    let redis_url = app
        .value_of("redis-url")
        .expect("redis-url should have default");

    let notifier = app
        .value_of("notify")
        .map_or_else(|| Notifier::Noop, |cmd| Notifier::Command(cmd.to_owned()));
    let watcher_notifier = notifier.clone();

    let client = redis::Client::open(redis_url).unwrap();
    let rds = warp::any().map(move || client.get_async_connection());
    let notify_filter = warp::any().map(move || notifier.clone());

    let r1 = warp::path("switch")
        .and(rds)
        .and(warp::path::param())
        .and(filters::query::query())
        .and(notify_filter)
        .and_then(handle);

    let routes = warp::get2().and(r1).with(filters::log::log("http"));
    let (_, serve) = warp::serve(routes).bind_ephemeral(listen);

    let watcher_client = redis::Client::open(redis_url).unwrap();

    let watcher = Interval::new_interval(Duration::from_secs(1))
        .map_err(|_| ())
        .for_each(move |_| {
            check_notify(
                watcher_client.get_async_connection(),
                watcher_notifier.clone(),
            )
        });

    tokio::run(lazy(|| {
        tokio::spawn(watcher);
        serve
    }));
}
