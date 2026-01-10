# PPM

**PPM** (**P**artner **P**rocess **M**onitor) is a process-monitoring application
designed to run and supervise processes inside [Docker](https://www.docker.com/)
containers.

It aims to be **low-overhead** and **low-footprint**, with ease of use as a core
principle, while still offering optional advanced features such as:

- Resource usage reporting
- Cron support
- Other monitoring utilities

## Why “PPM”?

**PPM** stands for **P**artner **P**rocess **M**onitor. _Partner_ refers to a
personal suite of tools and utilities used to scan and monitor networks, desktops,
files, and more, written in different programming languages.

After evaluating existing solutions—most notably [pm2](https://pm2.keymetrics.io),
which is written in [Node.js](https://nodejs.org), as well as alternatives like
[pmc](https://pmc.dev)—none fully matched my requirements.

As a result, I decided to build a **minimalist yet feature-rich process monitoring tool**
in [Rust](https://www.rust-lang.org).

## Getting Started

### Installation

<!-- FIXME -->

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
```

### Usage

Below are some example commands. Additional details are available using the `--help` option.

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

**PPM** spawns and monitors _services_, each of which can be in one of the following states:

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
