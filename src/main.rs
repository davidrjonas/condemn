use std::time::SystemTime;

use futures::future::ok;
use futures::future::Either;
use futures::Future;
use log::warn;
use serde_derive::Deserialize;
use serde_humantime::De;
use std::time::Duration;
use warp::filters;
use warp::http::StatusCode;
use warp::Filter;

#[derive(Deserialize)]
struct Options {
    deadline: De<Option<Duration>>,
    window: De<Option<Duration>>,
}

type ConnFut = redis::RedisFuture<redis::r#async::Connection>;

const Z_KEY: &'static str = "condemn_z";
const H_KEY: &'static str = "condemn_h";

fn notify_early_maybe(name: String, window_ts: i64) -> () {
    warn!("notify early maybe: name={}, ts={}", name, window_ts)
}

fn handle(
    connect: ConnFut,
    name: String,
    opts: Options,
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
                    .query_async::<_, Option<i64>>(conn)
                    .join(ok((name, score))),
            ),
        })
        .and_then(|((conn, window), (name, score))| match score {
            0 => ok((conn, name, false)),
            _ => match window {
                Some(window_start_ts) => {
                    notify_early_maybe(name.clone(), window_start_ts);
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

fn main() {
    pretty_env_logger::init();

    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    let rds = warp::any().map(move || client.get_async_connection());

    let r1 = warp::path("switch")
        .and(rds)
        .and(warp::path::param())
        .and(filters::query::query())
        .and_then(handle);

    let routes = warp::get2().and(r1).with(filters::log::log("http"));

    warp::serve(routes).run(([127, 0, 0, 1], 3030));
}
