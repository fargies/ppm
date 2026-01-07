# PPM

## Getting started

### Installation

<!-- FIXME -->

### Configuration

A configuration file may be created in one of the following locations:
- `~/.config/partner/partner-pm.yml`
- `~/.partner-pm.yml`
- `.partner-pm.yml`

Or using a custom file:
```bash
# Using ppm command-line utility:
ppm daemon --config "config.yml"

# Running the daemon directly :
PPM_CONFIG="config.yml" ppm-daemon
```

### Usage

Here are some example commands, details may be retrieved using `--help` option:
```bash
# Run the daemon, likely to run this in the background
ppm daemon

# List running services
ppm info
ppm ls

# Add a new service
ppm add --name my_test_service --env 'RUST_LOG=trace' -- sh -c "while true; echo world; sleep 30; done"

# Add a one-shot service
ppm add --name my_oneshot_service -- ls -la

# Remove a service
ppm rm my_oneshot_service

# Get statistics about running services
ppm stats

# Create configuration file
ppm show-configuration > ~/.partner-pm.yml
```

## How does it work ?

### Status

A service may be in different states:
- **Created**  : this is a temporary state, the daemon hasn't yet handled the newly created service
- **Running**  : service is live and running
- **Finished** : service is terminated either on its own with a `0` exit code, or by receiving a `SIGTERM`
- **Stopped**  : service is live but stopped (it received a `SIGSTP`)
- **Crashed**  : service died with a `!= 0` exit code, or by receiving a signal `!= SIGTERM`

When a service goes into **Crashed** state it will be restarted by the daemon, using
and exponential backoff mechanism: `interval * (2^(nb_restart - 1))`.

## Why "PPM" ?

**PPM** stands for **P**artner **P**rocess **M**onitor, *Partner* being a
personnal (as in used only by Sylvain Fargier <fargier.sylvain@gmail.com>) suite
of tools and utilities to scan and monitor networks, desktops, files ... written
in different languages.

Considering that [pm2](https://pm2.keymetrics.io) written in [Node.js](https://nodejs.org)
and not finding any alternatives that would fit my needs, (having considered
[pmc](https://pmc.dev) and others) I decided to write a minimalist, yet feature-full
process-monitoring software, in [Rust](https://rust-lang.org).
