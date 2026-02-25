# Server Messages

PPM provides a clear, structured, and fully configurable server messaging system
designed to help you monitor and diagnose server health and behavior with ease.

Example log (using [data/test-logger-config.yml](https://github.com/fargies/ppm/blob/master/data/test-logger-config.yml)
configuration file):

```bash
INFO starting daemon
INFO find_config_file: return=Some("data/test-logger-config.yml")
INFO new: listening addr=127.0.0.1:5000
INFO restart{name="test" id=1}: existing log file found name="test" file="/var/log/ppm/test-2026-02-17T18:08:41+01:00.log"
INFO restart{name="test" id=1}: Created -> Running pid=1282130
INFO init:reschedule{last=Some(2026-02-25T17:36:00+01:00) id=1 name="test"}: next=2026-02-25T17:36:01+01:00
INFO monitor: service exited id=1 name="test" pid=1282130 code=0
INFO monitor: Running -> Finished
INFO monitor:process:restart{name="test" id=1}: Finished -> Running pid=1282132
INFO monitor:process:reschedule{last=Some(2026-02-25T17:36:01+01:00) id=1 name="test"}: next=2026-02-25T17:36:02+01:00
INFO monitor: service exited id=1 name="test" pid=1282132 code=0
INFO monitor: Running -> Finished
```


## Message Configuration

PPM’s log output can be customized using the following environment variables:

- `RUST_LOG=debug`  
  Sets the log level (`trace`, `debug`, `info`, `warn`, `error`).

- `LOG_THREAD_ID=1`  
  Displays the thread ID that generated the log message.

- `LOG_SRC_FILE=1`  
  Displays the source file where the log message originated.

- `LOG_TARGET=1`  
  Displays the logging target (useful for filtering with `RUST_LOG`).

- `LOG_COLOR=auto`  
  Enables or disables colored log output (`auto`, `yes`, `no`).
