# Usage

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
