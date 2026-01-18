# PPM

**PPM** (**P**artner **P**rocess **M**onitor) is a process-monitoring application
designed to run and supervise processes inside [Docker](https://www.docker.com/)
containers.

![ppm logo](./data/ppm_128x128.png)

It aims to be **low-overhead** and **low-footprint**, with ease of use as a core
principle, while still offering optional advanced features such as:

- [Resource Usage Reporting](./docs/resource_usage.md)
- [File Change Monitoring](./docs/file_monitoring.md)
- [Cron-like Scheduling](./docs/scheduling.md)

## Why “PPM”?

**PPM** stands for **P**artner **P**rocess **M**onitor. _Partner_ refers to a
personal suite of tools and utilities used to scan and monitor networks, desktops,
files, and more, written in different programming languages.

After evaluating existing solutions—most notably [pm2](https://pm2.keymetrics.io),
which is written in [Node.js](https://nodejs.org), as well as alternatives like
[pmc](https://pmc.dev)—none fully matched my requirements.

As a result, I decided to build a
**minimalist yet feature-rich process monitoring tool**
in [Rust](https://www.rust-lang.org).

## Getting Started

### Installation

On [Debian](https://www.debian.org/) :

```bash
export REPO_URL=https://www.gremory.org/gitea/api/packages/fargie_s/debian
source /etc/os-release

sudo curl ${REPO_URL}/repository.key -o /etc/apt/keyrings/gitea-fargie_s.asc
echo "deb [signed-by=/etc/apt/keyrings/gitea-fargie_s.asc] ${REPO_URL} ${VERSION_CODENAME} main" | sudo tee -a /etc/apt/sources.list.d/gitea.list

sudo apt update
sudo apt install partner-pm
```

On [ArchLinux](https://archlinux.org/) :

```bash
echo "
[fargie_s.gitea:3000]
SigLevel = Optional TrustAll
Server = https://www.gremory.org/gitea/api/packages/fargie_s/arch/core/x86_64
" >> /etc/pacman.conf

pacman -Sy partner-pm
```

### Configuration

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

```yml
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

It may be built and generated using the cli (see _Usage_ section).

### Usage

Below are some example commands. Additional details are available using
the `--help` option.

```bash
# Run the daemon (typically in the background)
ppm daemon

# List running services
ppm info
ppm ls

# Add a new long-running service
ppm add --name my_test_service --env 'RUST_LOG=trace' -- \
  sh -c "while true; do echo world; sleep 30; done"

# Add a one-shot service
ppm add --name my_oneshot_service -- ls -la

# Remove a service
ppm rm my_oneshot_service

# Get statistics about running services
ppm stats

# Generate a configuration file
ppm show-configuration > ~/.partner-pm.yml
```

## How Does It Work?

### Service Status

**PPM** spawns and monitors _services_, each of which can be in one of
the following states:

- **Created**
  Temporary state; the daemon has not yet handled the newly created service.

- **Running**
  The service is live and executing.

- **Finished**
  The service has terminated normally with exit code `0`, or received a `SIGTERM`.

- **Stopped**
  The service is paused after receiving a `SIGSTP`.

- **Crashed**
  The service terminated with a non-zero exit code, or received a signal other
  than `SIGTERM`.

When a service enters the **Crashed** state, it is automatically restarted by
the daemon using an exponential backoff strategy: `interval * (2^(nb_restart - 1))`

## Development

For additional informations on development please see [DEV](./DEV.md).
