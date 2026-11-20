# Network Listening

Services can be autmatically started when an incoming network connection is detected.

## Enabling the Network Listener

To enable network connection monitoring, add a `listen` entry to the service definition:

```yaml
- name: sample_service
  command: ppm-web
  listen: :8080
```

PPM opens the socket and listens for incoming connections.
When a connection is received, PPM duplicates the socket and passes it to the
service as its `stdin`. The service is responsible for accepting the connection.

Once the service is started, PPM stops listening for incoming connections.
When the service exits, PPM resumes listening, and the next incoming connection
triggers a service restart.

Because PPM retains a handle to the listening socket, it remains open even while
the service is stopped. This ensures that incoming connections are not lost
during service restarts and allows the service to shut down gracefully.

## Network Address Syntax

The `listen` entry supports the following formats:

- `1234` or `:1234`
  A port number, optionally prefixed with `:`.\
  Both forms bind to `0.0.0.0:1234` (the _IPv4 unspecified address_).
- `127.0.0.1:1234`
  An IPv4 address and port.
- `[::]:1234`
  An IPv6 address and port.
