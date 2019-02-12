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

Requires redis.

### Configuration

```
USAGE:
    condemn [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --listen <listen>          The IP and port to listen on. [env: LISTEN=]  [default: 0.0.0.0:80]
    -r, --redis-url <redis-url>    The URL for Redis with database; redis://host:port/db [env: REDIS_URL=]  [default:
                                   redis://127.0.0.1:6379]
```

TODO
----

- [ ] Implement notifies.
- [ ] Test

Future improvements
-------------------

- [ ] Basic auth with username as prefix to all keys
