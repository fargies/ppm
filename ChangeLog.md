## [1.4.3] - 2026-03-02

### 🚀 Features

- Rework main README a bit

### 🐛 Bug Fixes

- Fix sysinfo process status on macos

### 📚 Documentation

- Update metadata and add ChangeLog
- Add changelog in website

### ⚙️ Miscellaneous Tasks

- Configure git-cliff
- Add cliff in CI
- Minor fix in release script
- Release partner-pm version 1.4.3
- Release partner-pm version 1.4.3
## [1.4.2] - 2026-02-28

### 🚀 Features

- Enable back log-capture in GitHub CI
- Minor cleanup
- Dix stats issue on MacOs
- Fix gitea website build

### 🐛 Bug Fixes

- Fix license name

### ⚙️ Miscellaneous Tasks

- Release partner-pm version 1.4.2
## [1.4.1] - 2026-02-27

### ⚙️ Miscellaneous Tasks

- Bump Cargo.toml
## [1.4.0] - 2026-02-27

### 🚀 Features

- Adding logger support
- Enhance logger serialization/deserialization
- Install check-jsonschema in ./target/venv
- Rename gitea actions
- Get rid of yq in schema validation
- Test ghpages
- Adding github ci basic tests
- Add some deps for CI
- Wip log client
- Add client-side tail support
- Update docs
- Add macos build
- Add some work-arrounds for FSEvents
- Add some logging in CI
- Adding some traces
- Add `wait_for` test helper
- Use `wait_for!` instead of sleeping
- Better tests cleanup
- Adding tracing utils
- Minor optimization
- Update tracing init
- Fix tracing after fork
- Adding some docs
- Minor changes on doc
- Add status as an alias for `ppm ls`
- Rework fcntl implementation
- Add launcher
- Add some tests
- Polish logs, add doc
- Fetch everything

### 🐛 Bug Fixes

- Minor fixes on error-recovery
- Fix website base-path
- Fix github workflow
- Fix gitea ci
- Fix logger test issue
- Fix fsevents build
- Fix std::io lock race
- Fix FSEvents build
- Fix server brutal shutdown
- Server may not send shutdown if client is already gone
- Fix gh-pages build

### 📚 Documentation

- Updating doc
- Update cast CSS

### 🧪 Testing

- Enhance tests a bit

### ⚙️ Miscellaneous Tasks

- Add some traces in CI
- Disabling log colors
- Update todo list
## [1.3.0] - 2026-01-30

### 🚀 Features

- Adding documentation website
- Adding config schema
- Update schema
- Add tests on config schema
- Rework [Service::stop]
- Update watch filters

### 🐛 Bug Fixes

- Mktemp tests false-positive
- Forgot to fetch sources in CI
- Fix CI
- Rework a bit the restart mechanism
- Fix sample config

### ⚙️ Miscellaneous Tasks

- Version bump
## [1.2.0] - 2026-01-24

### 🚀 Features

- Add workdir support
- Working on watchs
- Working on watchs
- Watchs implementation
- Adding some doc
- Prepare for macos support
- Use FSEvents
- Fix Linux builds

### 🐛 Bug Fixes

- Scheduler fixes
- Minor change
- Object rename

### 📚 Documentation

- Add logo in docs

### 🎨 Styling

- Apply clippy changes
- Lint tests
- Lint a bit
- Lint a bit

### ⚙️ Miscellaneous Tasks

- Bump version v1.2.0
## [1.1.1] - 2026-01-14

### 🚀 Features

- Add icons, rework bundling on debian

### ⚙️ Miscellaneous Tasks

- Add jq dependency
- Bump
## [1.1.0] - 2026-01-12

### 🚀 Features

- Add cron support
- Fix clocking issues
- Small update
- Fix MacOS support
- Linux fix
- Update CI

### 🐛 Bug Fixes

- Minor changes

### 📚 Documentation

- Update doc

### 🧪 Testing

- Fix tests

### ⚙️ Miscellaneous Tasks

- Bump version
## [1.0.0] - 2026-01-08

### 🚀 Features

- Linux support
- MacOS support
- Linux support
- Working no tabled
- Work on cmdline
- Adding some commands
- Working on stats and commands
- Add daemon stats, tune malloc options
- Adding CI
- Fix clippy errors
- Add package metadata
- Ignore .vscode
- Rename package

### 🚜 Refactor

- Wip

### 📚 Documentation

- Update readme

### 🧪 Testing

- Fix tests

### ⚙️ Miscellaneous Tasks

- First import
- Remove useless files
- Add packaging in CI
