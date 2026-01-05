# PPM

## Getting started

### Installation

<!-- FIXME -->

### Configuration

A configuration file may be created in one of the following locations:
- `~/.config/partner/partner-pm.yml`
- `~/.partner-pm.yml`
- `.partner-pm.yml`

Or using a custom file:
```bash
# Using ppm command-line utility:
ppm daemon --config "config.yml"

# Running the daemon directly :
PPM_CONFIG="config.yml" ppm-daemon
```

## Why "PPM" ?

**PPM** stands for **P**artner **P**rocess **M**onitor, *Partner* being a
personnal (as in used only by Sylvain Fargier <fargier.sylvain@gmail.com>) suite
of tools and utilities to scan and monitor networks, desktops, files ... written
in different languages.

Considering that [pm2](https://pm2.keymetrics.io) written in [Node.js](https://nodejs.org)
and not finding any alternatives that would fit my needs, (having considered
[pmc](https://pmc.dev) and others) I decided to write a minimalist, yet feature-full
process-monitoring software, in [Rust](https://rust-lang.org).
