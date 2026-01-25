# Service Resource Usage

The following statistics are monitored for each service:

- `CPU` -- CPU usage as a percentage (`%`).\
  May exceed 100% on multi-core systems.
- `CPU time` -- Total CPU execution time.
- `I/O read` / `I/O write` -- Bytes read in written by the service, reported
  either as a rate (`B/sec`) or as total bytes (`B`).
- `Mem RSS` -- Resident Set Size (physical memory currently in use).
- `Mem VSZ` -- Virtual memory size reserved by the process.
- `uptime` -- Time the service has been running since its last restart.

These statistics apply only to the service process and its threads; any child
processes are not taken into account.

Each service is identified by its `id` and `name`. Services that are no longer
running do not report statistics, but they are still displayed in the list.

An additional entry named `<PPM daemon>` is also shown. It reports the resource
usage of the _PPM_ daemon process itself.

To retrieve these statistics, use the following command:

```bash
ppm stats
```
