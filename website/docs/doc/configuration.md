# Server Configuration

The configuration file can be located in any of the following paths:

- `~/.config/partner/partner-pm.yml`
- `~/.partner-pm.yml`
- `.partner-pm.yml`

Alternatively, you can specify a custom configuration file:

```bash
# Using the ppm command-line utility
ppm daemon --config "config.yml"

# Running the daemon directly
PPM_CONFIG="config.yml" ppm-daemon
````

## Configuration Example

The following example shows a complete configuration:

```yaml
# Statistics refreshing period (optional, defaults to 10s)
stats_interval: 10s
# Service restart base time (optional, defaults to 1s)
restart_interval: 1s
# Filesystem event delay period (optional, defaults to 3s)
watch_restart_delay: 3s
# System clock checking interval (optional, defaults to 1h)
clock_check_interval: 1h

services:
  # Minimum service definition
  - name: my_test_service
    # Direct command, no options
    command: sh -c "while true; do echo world; sleep 30; done"

  # Complete service definition
  - name: complete_service
    # Preserve the service-id across restarts (optional, auto-assigned by default)
    id: 1
    command:
      # Executable path and arguments
      path: "/bin/sh"
      args:
        - "-c"
        - "while true; do echo ${MY_VAR}; sleep 30; done"
      # Service working directory
      workdir: "/app"
      # Environment variables
      env:
        MY_VAR: "value"
    # Schedule the service to run every 30 seconds
    schedule: "*/30 * * * * *"
    # Start the service on incoming connections
    listen: :8080
    # Files to watch for changes
    watch:
      # Files and directory patterns to force-include
      include: [ "*.{c,h}" ]
      # Files and directory patterns to exclude
      exclude: [ "*.o" ]
      # Paths where to look for files (required)
      paths: [ ".", "/app/config.yml" ]
      # Maximum depth for files to monitor
      max_depth: 1

# Logger configuration (optional)
logger:
  # Log files directory (defaults to /var/log)
  path: /var/log
  # Maximum number of files to retain (defaults to 3)
  max_files: 3
  # Maximum file size (defaults to 20MiB)
  max_file_size: 20MiB
  # Automatically prepend timestamps to log lines (defaults to true)
  auto_date: true
```

A [JSON Schema](https://json-schema.org/) is available at [data/config.schema.json](https://github.com/fargies/ppm/blob/master/data/config.schema.json).

## Global Configuration Details

- **stats_interval**\
  Statistics refreshing period (see [Service Resource Usage](./resource_usage)).\
  The PPM daemon periodically refreshes process statistics (RSS, CPU time, etc.).\
  _Default: `10s`_
- **restart_interval**\
  Service restart base time slice, used to compute the restart delay for crashed services (see [Service Restart Policy](./restart_policy)).\
  _Default: `1s`_
- **watch_restart_delay**\
  Filesystem event delay period (see [File Monitoring](./file_monitoring)).\
  _Default: `3s`_
- **clock_check_interval**\
  System clock checking interval, used to detect clock jumps and drifts.\
  _Default: `1h`_

## Services Configuration Details

The `services` entry is a list of objects with the following attributes:

- **name**\
  Service name, used to identify the service.\
  _Required_
- **id**\
  Service ID, used internally to identify the service.\
  _Default: auto-assigned_
- **command**\
  Command to spawn the service. It can be specified as a string, an array of strings, or an object with the following attributes:
  - **path** (required)\
    Executable path.
  - **args**\
    Command-line arguments.
  - **workdir**\
    Service working directory.
  - **env**\
    Environment variables to set.
- **schedule**\
  Cron-like service scheduling (see [Cron-like Scheduling](./scheduling)).
- **listen**\
  Network listening mode (see [Network Listening](./network_listening)).
- **watch**\
  File monitoring mode (see [File Monitoring](./file_monitoring)).

  ## Logger Configuration

  See [Log File Rotation](./log_rotate) for details on configuring the logger.
