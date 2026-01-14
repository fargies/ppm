# Development documentation

## Bundling

Bundling is achived using the following tools:

- [cargo-arch](https://crates.io/crates/cargo-arch) for [ArchLinux](https://archlinux.org/)
- [cargo-deb](https://crates.io/crates/cargo-deb) for [Debian](https://www.debian.org/)

## Releases

Releasing is achived by [Gitea-CI](https://about.gitea.com/) using
[act](https://github.com/nektos/act) packages are created and deployed
using version tag: `v<major>.<minor>.<patch>`.

The project will be packaged for ArchLinux and Debian and made available
on [Gremory's package repository](https://www.gremory.org/gitea/fargie_s/-/packages?q=partner-pm).
