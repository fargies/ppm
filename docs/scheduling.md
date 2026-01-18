# Cron-like Scheduling

A service can be automatically restarted at specific intervals and on specific dates.

To enable this feature, add a `schedule` entry to the service definition:

```yaml
- id: 5
  name: periodic
  command:
    path: echo
    args: [world]
  schedule: "*/15 * * * * *"
```

The schedule becomes effective as soon as the service is `active`.\
Services can be activated and de-activated from command-line:

```bash
# Stop and deactivate a service by name
ppm stop periodic

# Activate and start a service using the service ID
ppm start 5
```

## Cron Syntax

Scheduling is based on [cron-like](https://en.wikipedia.org/wiki/Cron) rules, as
implemented by [croner](https://docs.rs/croner/latest/croner/#pattern):

```
┌──────────────── (optional) second (0 - 59)
│ ┌────────────── minute (0 - 59)
│ │ ┌──────────── hour (0 - 23)
│ │ │ ┌────────── day of month (1 - 31)
│ │ │ │ ┌──────── month (1 - 12, JAN-DEC)
│ │ │ │ │ ┌────── day of week (0 - 6, SUN-Mon)
│ │ │ │ │ │       (0 to 6 are Sunday to Saturday; 7 is Sunday, the same as 0)
│ │ │ │ │ │
* * * * * *
```

## Debugging

The internal scheduler can be queried from the command line using the
following command:

```bash
ppm show-scheduler
```

This command displays the currently registered schedules and their next planned
execution times, which can be useful for troubleshooting scheduling issues or
verifying cron expressions.
