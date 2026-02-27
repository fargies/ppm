# Service File Change Monitoring

A service can be automatically restarted when files are modified in watched directories.

To enable this feature, add a `watch` entry to the service definition:

```yaml
- id: 0
  name: sample_service
  command: echo world
  watch: /app/config.yml
```

Files or directories can also be watched using filters to _include_ or _exclude_
specific patterns (see [Files and Paths filtering]):

```yaml
- id: 1
  name: another_service
  command: echo another world
  watch:
    include: [ "*.{c,h}" ]
    exclude: [ "*.o" ]
    paths: [ ".", "/app/config.yml" ]
    max_depth: 1
```

The following restrictions apply:

- Watching rules are created when the service starts. If a file or directory listed
  in `paths` does not exist at that time, it will **not** be monitored.
- Subdirectories created after service has started will not be monitored.

## Files and Paths Filtering

Files and paths filtering is based on [globbing](https://www.man7.org/linux/man-pages/man7/glob.7.html)
rules, as implemented by [globset](https://docs.rs/globset/latest/globset/#syntax).

**Important:** on Linux globbing is applied in two stages:

1. On the path when directories are initially registered for watching.
2. On the file-name when a change is detected.

The `include` and `exclude` rules are applied at both stages.

Some built-in exclusions are always enabled:

```yaml
[ ".?*", "**/{build,target}*", "*.{o,pyo,pyc}" ]
```

These patterns exclude hidden files, directories prefixed with build or target,
and common compiler object files (C and Python).

### Supported Globbin

- `?` -- matches any single character
- `*` -- matches zero or more characters (excluding directory separators `/`)
- `**` -- recursively matches directories
- `{a.b}` -- match any of the listed patterns
- `[ab]` -- matches any of the given characters
  (use `[!ab]` to match any character _except_ `a` and `b`)
