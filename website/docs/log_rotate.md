# Log File Rotation

PPM provides built-in log file rotation to help manage disk usage and retain
recent logs.

By default, this feature is disabled. When disabled, services inherit _stdout_
and _stderr_ from the PPM daemon, meaning logs are displayed directly in your
container console.

## Enabling the Logger

To enable log file management, add the following section to your configuration file:

```yaml
logger:
```

Once enabled, PPM will write logs to files instead of forwarding them to the console.

### Configuration Options

You can fine-tune the logger behavior using the following options (shown here
with their default values):

```yaml
logger:
  # Directory where log files are stored
  path: /var/log

  # Maximum number of log files to retain
  # (including the currently active log file)
  max_files: 3

  # Maximum file size before rotation occurs
  max_file_size: 20MiB
```

### Option Details

- path\
  Directory where log files will be written.\
  If the directory does not exist, it will be created automatically.\
  PPM must have write permissions for this directory.

- max_files\
  The maximum number of log files kept per service.\
  When the limit is reached, the oldest log file is removed.

- max_file_size\
  The size threshold that triggers log rotation.\
  When the active log file reaches this size, a new log file is created.

**Note:** A log file may slightly exceed max_file_size to ensure the last
buffered log line is fully written and not truncated.

## Log File Naming

Log files are named using the following format: `<service_name>-<date>.log`

Each service maintains its own set of rotated log files.

## Viewing Logs

The PPM client provides convenient commands for accessing service logs:

```bash
# Dump all logs for a service
ppm log my_service

# Dump the last 200 lines
ppm log my_service -n 200

# Dump last 200 lines and follow output
# (properly handles log rotation)
ppm log my_service -n 200 -f
```
