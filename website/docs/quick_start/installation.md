# Installation

## In a Dockerfile

To ship PPM in a container, here's an example installation in a [Dockerfile](https://docs.docker.com/reference/dockerfile/)
using a prebuilt binary:

```docker
FROM debian:stable-slim

ARG VERSION=v1.5.2 FILE=partner-pm-linux-amd64.tar.gz

ADD https://github.com/fargies/ppm/releases/download/$${VERSION}/$${FILE} .
RUN tar xf $${FILE} -C /usr/bin && rm $${FILE}

RUN <<EOF
echo '
services:
  - name: test
    command:
      path: date
    schedule: "*/1 * * * * *"
logger: { path: /var/log }
' > /ppm_config
EOF

ENTRYPOINT [ "ppm", "daemon", "--config", "/ppm_config" ]
```

Some additional examples using [docker-compose.yaml](https://docs.docker.com/compose/)
are available in the [examples](https://github.com/fargies/ppm/blob/master/examples)
source directory.

## On Debian

To install PPM on a [Debian](https://www.debian.org/) Linux distribution:

```bash
export REPO_URL=https://www.gremory.org/gitea/api/packages/fargie_s/debian
source /etc/os-release

sudo curl ${REPO_URL}/repository.key -o /etc/apt/keyrings/gitea-fargie_s.asc
echo "deb [signed-by=/etc/apt/keyrings/gitea-fargie_s.asc] ${REPO_URL} ${VERSION_CODENAME} main" | sudo tee -a /etc/apt/sources.list.d/gitea.list

sudo apt update
sudo apt install partner-pm
```

## On ArchLinux

To install PPM on an [ArchLinux](https://archlinux.org/) Linux distribution:

```bash
echo "
[fargie_s.gitea:3000]
SigLevel = Optional TrustAll
Server = https://www.gremory.org/gitea/api/packages/fargie_s/arch/core/x86_64
" >> /etc/pacman.conf

pacman -Sy partner-pm
```
