# Service Restart Policy

When a service enters the **Crashed** state, the daemon automatically restarts
it using an exponential backoff strategy. The restart delay is calculated as follows:

`restart_interval * 2^nb_restart`

To accommodate services that occasionally crash but remain stable for extended
periods, the restart delay is adjusted based on the service uptime. Uptime is
measured in multiples of `restart_interval` (`uptime_slices`), with each slice
reducing the restart delay by half its value.

The resulting restart delay is calculated using the following formula:

`new_restart_interval = (old_restart_interval - uptime_slices / 2) * 2`

The resulting value is clamped to a minimum of `1`.

The `restart_interval` can be configured at the PPM daemon level.
See [Global Configuration Details](./configuration#global-configuration-details)
for more information.
