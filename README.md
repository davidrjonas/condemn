Condemn
=======

A curl-able [dead man's switch](https://en.wikipedia.org/wiki/Dead_man%27s_switch).

Call this service at the start of a process and at the end. If the second call never arrives, be notified of the failure.

Client Usage
------------

At the beginning of your script call condemn with a deadline. `deadline` will accept durations like `15days 2min 2s`. See Durations for more info.

```bash
curl http://condemn.example.net/myscriptname?deadline=1h
```

At the end of your script call condemn again with the same path. Include a deadline and optional window if you want to ensure the script runs again, for instance a cron job.

```bash
curl http://condemn.example.net/myscriptname?deadline=25h&window=2h
```

If condemn is called again for your scriptname less than 23h for now it will notify that it started early. If there has been no call within the next 25 hours it will notify that the script is dead.

Durations
---------

Will accept,

- nsec, ns -- microseconds
- usec, us -- microseconds
- msec, ms -- milliseconds
- seconds, second, sec, s
- minutes, minute, min, m
- hours, hour, hr, h
- days, day, d
- weeks, week, w
- months, month, M -- defined as 30.44 days
- years, year, y -- defined as 365.25 days

Set up
------

Docker image available at https://hub.docker.com/r/davidrjonas/condemn

Requires redis >= 2.4, necessary for ZREM with multiple keys.

### Configuration

```
condemn 0.1.0
David Jonas <djonas@noip.com>

USAGE:
    condemn [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --listen <listen>                    The IP and port to listen on. [env: LISTEN=]  [default: 0.0.0.0:80]
    -n, --notify <notify>...                 The notifiers to use. May require other options to be set, such as `sentry-
                                             dsn`. The Command notifier is configured separately, see `--notify-
                                             command`. [env: NOTIFY=]  [possible values: sentry]
    -c, --notify-command <notify-command>    Command to run on notify. CONDEMN_NAME env var will be set. CONDEMN_EARLY
                                             env var will be set to the number of seconds, 0 if deadlined. [env:
                                             NOTIFY_COMMAND=]
    -r, --redis-url <redis-url>              The URL for Redis with database; redis://host:port/db [env: REDIS_URL=]
                                             [default: redis://127.0.0.1:6379]
        --sentry-dsn <sentry-dsn>            Configures `sentry` notifier. If notify includes 'sentry', `sentry-dsn` is
                                             required. [env: SENTRY_DSN=]
```

Contributing
------------

Pull requests welcome!

Notifiers are easy to add. Just implement the `Notifier` trait and add an entry to main() for configuration. See `sentry` as an example.

Future improvements
-------------------

- [ ] Test(s)
- [X] Sentry notifier
- [ ] Basic auth with username as prefix to all keys
- [ ] Other notifiers like Slack, OpsGenie, webhook
- [ ] Use a transaction with redis / pipeline
- [ ] Refactor for readability
- [ ] Add client ip to notifies, other additional context the client may supply, like &ctx={"a":"b"} or &ctx[a]=b&ctx[c]=d
