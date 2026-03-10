# TODO List

- [] split client/server to separate dependencies
- [x] add support for "workdir"
- [x] add support for "watchs"
- [x] add throttle for "watchs"
- [x] add support for logs and log rotation
- [x] fix watchs to filter filenames on MacOS, add a test to ensure this
- [x] add schedule and workdir options on cli
- [x] allow to monitor single files
- [x] documentation and schema for logger
- [] when rotating files do look for `\n` boundaries
- [] doc show minimalistic service config (id not mandatory)
- [] installation -> publish on docker hub
- [] external API (grpc ?)
- [] binaries shipped with releases on GitHub
- [] add mode to spawn service as container (having ppm in entry-point) CMD
