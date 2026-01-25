# PPM

**PPM** (**P**artner **P**rocess **M**onitor) is a process-monitoring application
designed to run and supervise processes inside [Docker](https://www.docker.com/)
containers.

![ppm logo](./data/ppm_128x128.png)

It aims to be **low-overhead** and **low-footprint**, with ease of use as a core
principle, while still offering optional advanced features such as:

- [Resource Usage Reporting](website/docs/resource_usage.md)
- [File Change Monitoring](website/docs/file_monitoring.md)
- [Cron-like Scheduling](website/docs/scheduling.md)

## Why “PPM”?

**PPM** stands for **P**artner **P**rocess **M**onitor. _Partner_ refers to a
personal suite of tools and utilities used to scan and monitor networks, desktops,
files, and more, written in different programming languages.

After evaluating existing solutions—most notably [pm2](https://pm2.keymetrics.io),
which is written in [Node.js](https://nodejs.org), as well as alternatives like
[pmc](https://pmc.dev)—none fully matched my requirements.

As a result, I decided to build a
**minimalist yet feature-rich process monitoring tool**
in [Rust](https://www.rust-lang.org).

## Documentation

Complete documentation is available here: [https://www.gremory.org/ppm](https://www.gremory.org/ppm)

## Development

For additional informations on development please see [DEV](./DEV.md).
