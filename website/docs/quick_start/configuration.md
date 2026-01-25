# Configuration

A configuration file can be created in any of the following locations:

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

The configuration file describes services and daemon configuration:

```yaml
# Statistics refreshing period (optional, defaults to 10s)
stats_interval: 10s
# Service restart base time (optional, defaults to 1s)
restart_interval: 1s
# System clock checking interval (optional, defaults to 1h)
clock_check_interval: 1h

services:
  # Basic service definition
  - name: my_test_service
    command: sh -c "while true; do echo world; sleep 30; done"
  - name: complete_service
    # preserve the service-id amongst restarts
    id: 1
    command:
      # executable path and arguments
      path: "/bin/sh"
      args:
        - "-c"
        - "while true; do echo ${MY_VAR}; sleep 30; done"
      # service working directory
      workdir: "/app"
      # environment variables
      env:
        MY_VAR: "value"
    # schedule the service to run every 30 seconds
    schedule: "*/30 * * * * *"
```

It may be built and generated using the cli (see [Usage](/quick_start/usage) section).
