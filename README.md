Condemn
=======

A curl-able dead man's switch.

Client Usage
------------

At the beginning of your script call condemn with a deadline.

```bash
curl http://condemn.example.net/switch/myscriptname?deadline=1h
```

At the end of your script call condemn again with the same path. Include a deadline and optional window if you want to ensure the script runs again, for instance a cron job.

```bash
curl http://condemn.example.net/switch/myscriptname?deadline=25h&window=2h
```

If condemn is called again for your scriptname less than 23h for now it will notify that it started early. If there has been no call within the next 25 hours it will notify that the script is dead.

Set up
------

Requires redis >= 2.4.

### Configuration

```
USAGE:
    condemn [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --listen <listen>          The IP and port to listen on. [env: LISTEN=]  [default: 0.0.0.0:80]
    -n, --notify <notify>          Command to run on notify. CONDEMN_NAME env var will be set. If early, CONDEMN_EARLY
                                   env var will be set to the number of seconds. [env: NOTIFY=]
    -r, --redis-url <redis-url>    The URL for Redis with database; redis://host:port/db [env: REDIS_URL=]  [default:
                                   redis://127.0.0.1:6379]
```

TODO
----

- [ ] Test(s)

Future improvements
-------------------

- [ ] Basic auth with username as prefix to all keys
- [ ] Other notifiers like Slack, OpsGenie, webhook
