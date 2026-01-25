# Installation

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

## In a Dockerfile

To ship PPM in a container, here's an example installation in a [Dockerfile](https://docs.docker.com/reference/dockerfile/):

```docker
FROM debian:stable

# Environment variable for ppm-daemon to find its configuration file
ENV PPM_CONFIG=/app/data/service.yml

RUN apt-get update && apt-get install -y curl && apt-get clean

RUN curl https://www.gremory.org/gitea/api/packages/fargie_s/debian/repository.key -o /etc/apt/keyrings/gitea-fargie_s.asc
RUN echo "deb [signed-by=/etc/apt/keyrings/gitea-fargie_s.asc] https://www.gremory.org/gitea/api/packages/fargie_s/debian trixie main" | tee -a /etc/apt/sources.list.d/gitea.list

RUN apt-get update && \
    apt-get install -y partner-pm && \
    apt-get autoremove && apt-get clean

COPY . /app
WORKDIR /app

ENTRYPOINT [ "/usr/bin/ppm-daemon" ]
```
