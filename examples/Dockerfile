FROM debian:stable-slim

ARG VERSION=v1.5.2 FILE=partner-pm-linux-amd64.tar.gz

ADD https://github.com/fargies/ppm/releases/download/${VERSION}/${FILE} .
RUN tar xf ${FILE} -C /usr/bin && rm ${FILE}

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
