# refunnel

> A fast, lightweight DNS sinkhole written in Rust. Blocks ads, trackers, and malware at the DNS layer — before a single byte of unwanted content reaches your network.


## Features

- **DNS sinkhole** — Blocked domains receive a `0.0.0.0` / `::` response, silently dropping the connection
- **StevenBlack compatible** — Reads the standard `hosts`-format blocklist (120,000+ domains out of the box)
- **LRU response cache** — Caches up to 10,000 upstream responses in memory with full TTL respect
- **Cloudflare upstream** — Forwards allowed queries to `1.1.1.1:53` with a 2-second timeout
- **Structured logging** — Configurable verbosity via the `RUST_LOG` environment variable
- **Async core** — Built on Tokio; handles concurrent queries without blocking


## How It Works

```
Client DNS Query
      │
      ▼
 ┌──────────────┐    blocked?    ┌───────────────────────┐
 │  refunnel    │ ─────────────▶│  Sinkhole Response    │
 │  (port 53)   │                │  A: 0.0.0.0 / AAAA :: │
 └─────┬────────┘                └───────────────────────┘
       │ allowed
       ▼
 ┌──────────────┐   cache hit?   ┌───────────────────────┐
 │  LRU Cache   │ ─────────────▶│  Cached Response      │
 │ (10k entries)│                │  (TTL-aware)          │
 └─────┬────────┘                └───────────────────────┘
       │ cache miss
       ▼
 ┌──────────────┐                ┌───────────────────────┐
 │  Cloudflare  │ ─────────────▶│  Live Response        │
 │  1.1.1.1:53  │                │  (cached for next use)│
 └──────────────┘                └───────────────────────┘
```

The blocklist is loaded from `hosts.txt` once at startup and held entirely in memory using a hash set for O(1) lookups.


## Building from Source

**Prerequisites:** [Rust toolchain](https://rustup.rs/) (edition 2024 / Rust 1.85+)

```bash
git clone https://github.com/DavidMANZI-093/refunnel-rs.git
cd refunnel-rs
cargo build --release

# Install the binary
sudo cp target/release/refunnel-rs /usr/local/bin/refunnel
```


## Blocklist Setup

`refunnel` reads its blocklist from `/etc/refunnel/hosts.txt` at startup. The file must exist before the service is started.

### Create the config directory

```bash
sudo mkdir -p /etc/refunnel
```

### Download the StevenBlack unified hosts list

The [StevenBlack/hosts](https://github.com/StevenBlack/hosts) project maintains a curated, regularly-updated blocklist aggregated from multiple trusted sources.

**Base list** — adware + malware (~120,000 domains):
```bash
sudo curl -o /etc/refunnel/hosts.txt \
  https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts
```

**Extended variants** — choose the one that fits your network policy:

| Variant | Blocks | URL path |
|---|---|---|
| Base | Adware + Malware | `master/hosts` |
| + Social | + Social media | `alternates/social/hosts` |
| + Gambling | + Gambling sites | `alternates/gambling/hosts` |
| + Porn | + Adult content | `alternates/porn/hosts` |
| Everything | All of the above | `alternates/fakenews-gambling-porn-social/hosts` |

```bash
# Example — base + social media:
sudo curl -o /etc/refunnel/hosts.txt \
  https://raw.githubusercontent.com/StevenBlack/hosts/master/alternates/social/hosts
```

> The blocklist is loaded **once at startup**. A service restart is required after updating the file.


## Running as a Service

### systemd (Ubuntu, Debian, Fedora, Arch, and most modern Linux)

#### 1. Create a dedicated service user

Running as a dedicated unprivileged user limits the blast radius if the process is ever compromised. The user is granted only the capability to bind port 53 — nothing else.

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin refunnel
sudo chown root:refunnel /etc/refunnel/hosts.txt
sudo chmod 640 /etc/refunnel/hosts.txt
```

#### 2. Create the unit file

Create `/etc/systemd/system/refunnel.service`:

```ini
[Unit]
Description=refunnel DNS Sinkhole
Documentation=https://github.com/DavidMANZI-093/refunnel-rs
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=refunnel
Group=refunnel

# hosts.txt is read from the working directory
WorkingDirectory=/etc/refunnel
ExecStart=/usr/local/bin/refunnel

Restart=on-failure
RestartSec=5s

# Grant only the capability required to bind port 53
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE

# Sandboxing — limits what the process can touch even if compromised
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ReadOnlyPaths=/etc/refunnel

# Log level — see the Logging section for available levels
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

#### 3. Enable and start

```bash
sudo systemctl daemon-reload
sudo systemctl enable refunnel
sudo systemctl start refunnel
sudo systemctl status refunnel
```


### OpenRC (Alpine Linux, Gentoo, Artix)

#### 1. Create the service user

```bash
sudo adduser -S -H -s /sbin/nologin refunnel
sudo chown root:refunnel /etc/refunnel/hosts.txt
sudo chmod 640 /etc/refunnel/hosts.txt
```

#### 2. Create the init script

Create `/etc/init.d/refunnel`:

```sh
#!/sbin/openrc-run

name="refunnel"
description="refunnel DNS Sinkhole"
command="/usr/local/bin/refunnel"
command_user="refunnel:refunnel"
command_background=true
pidfile="/run/${RC_SVCNAME}.pid"
directory="/etc/refunnel"

export RUST_LOG="${RUST_LOG:-info}"

depend() {
    need net
    after firewall
}
```

#### 3. Enable and start

```bash
sudo chmod +x /etc/init.d/refunnel
sudo rc-update add refunnel default
sudo rc-service refunnel start
```

> **Other init systems (runit, s6, SysV):** The binary is a straightforward long-running process that reads from `/etc/refunnel` and writes to stdout/stderr. Adapt the above to your init system's service definition format accordingly.


## Logging

`refunnel` uses structured logging via the [`tracing`](https://docs.rs/tracing) crate. The log level is controlled by the `RUST_LOG` environment variable.

### Log levels

| `RUST_LOG` value | What is logged |
|---|---|
| `error` | Failures only (parse errors, socket errors) |
| `warn` | Upstream timeouts, network warnings |
| `info` | **Default** — blocked domains, startup info, blocklist load count |
| `debug` | Cache hits/misses, type mismatches, upstream queries |
| `trace` | Every packet received and sent (very verbose) |

The default is `info` when `RUST_LOG` is not set.

### Viewing logs (systemd / journald)

```bash
# Live tail
journalctl -u refunnel -f

# Last 100 lines
journalctl -u refunnel -n 100

# Since last boot
journalctl -u refunnel -b

# Filter to blocked domains only
journalctl -u refunnel -f -g "BLOCKED"
```

### Changing the log level without editing the unit file

Use a systemd drop-in override so your changes survive package updates:

```bash
sudo systemctl edit refunnel
```

This opens an editor. Add:

```ini
[Service]
Environment=RUST_LOG=debug
```

Then apply:

```bash
sudo systemctl restart refunnel
```

### Viewing logs (OpenRC)

OpenRC services log to stdout/stderr. To persist logs, redirect via `syslog`:

```sh
# In /etc/init.d/refunnel, update the command line:
command_args="2>&1 | logger -t refunnel"
```

Then view with:
```bash
grep refunnel /var/log/messages
```


## Updating the Blocklist

The blocklist is loaded once at startup. To apply an updated list:

```bash
# 1. Re-download
sudo curl -o /etc/refunnel/hosts.txt \
  https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts

# 2. Restart the service
sudo systemctl restart refunnel   # systemd
# sudo rc-service refunnel restart  # OpenRC
```

To keep the list fresh automatically, add a cron job or systemd timer:

```bash
# Example cron — update every Sunday at 03:00
sudo crontab -e
# Add:
0 3 * * 0 curl -so /etc/refunnel/hosts.txt https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts && systemctl restart refunnel
```


## Pointing Devices at refunnel

Once the service is running, direct DNS traffic to it by setting the DNS server to the IP address of the host running `refunnel`.

**Router (recommended):** Set the DNS server in your router's DHCP settings. All devices on the network will use `refunnel` automatically.

**Single host (Linux):**
```bash
# /etc/resolv.conf
nameserver 127.0.0.1
```

**Verify it's working:**
```bash
# Should resolve normally:
dig @127.0.0.1 example.com

# Should return 0.0.0.0 (blocked):
dig @127.0.0.1 doubleclick.net
```


## License

MIT
