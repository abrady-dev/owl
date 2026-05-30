# owl

```
          .-=========-.
       .-'             '-.
      /''               ''\
     |   .--.       .--.   |
     |  /    \     /    \  |
     | |  ()  |   |  ()  | |
     |  \    /  ^  \    /  |
      \  '--'  /v\  '--'  /
       '.     \___/     .'
         '-.    |||   .-'
            '---'|'---'
          ~~~~~~~~~~~~~~~
           '  '  '  '  '
```

**owl · lean eyes on Linux · system monitor**

A terminal system monitor for Linux, written in Rust. Real-time CPU, memory,
disk, network, thermal, and power stats in a clean TUI — built entirely on
`/proc` and `/sys` with no external system dependencies.

---

## Overview

owl is built around a single constraint: **read from the kernel, nothing else.**
No calls to external tools like `sensors`, `lsblk`, or `ip`. No system libraries
beyond libc. Every number on screen comes directly from a `/proc` or `/sys` file,
parsed by hand in Rust.

The result is a monitor that starts instantly, uses minimal resources, and has
no surprise dependencies to break across distro versions.

---

## Dashboard

```
┌owl · lean eyes on Linux ───────────────────────────────────────────────────┐
│▟▙ owl  v0.1   arch   up 3d 04:12   load 0.42 0.55 0.61           14:22:07 │
├──────────────────────────────────────────┬─────────────────────────────────┤
│ CPU  Ryzen 7 5800U                  74% │ MEM                              │
│  c0 ████████░░░░ 22%  c4 ████░░░░░░ 61% │  ram  ████████████████░░░░░  9.4G│
│  c1 ██░░░░░░░░░░ 14%  c5 █████░░░░░ 44% │  swp  ███░░░░░░░░░░░░░░░░░░  0.6G│
│  c2 ██████░░░░░░ 52%  c6 ███████████ 88%│                                  │
│  c3 █░░░░░░░░░░░ 11%  c7 ███████░░░░ 58%│ DISK                             │
│                                         │  /      █████████░░░░░░ 63%      │
│  load 60s  ▁▂▃▄▅▇▆▅▄▃▂▃▄▆▇█▇▆▄▃▂▁      │  /home  ████████████░░░ 81%      │
├──────────────────────────────────────────┼─────────────────────────────────┤
│ NET  wlan0                              │ TEMP                             │
│  rx ↓ 2.4 MB/s  ▂▃▄▆▇▆▅▆▇█▇▆▅▃          │  cpu  64°C ██████████░░░░░░      │
│  tx ↑ 0.8 MB/s  ▁▂▂▃▂▁▂▃▄▃▂▁▂▁          │  ssd  41°C ██████░░░░░░░░░░      │
├──────────────────────────────────────────┴─────────────────────────────────┤
│ BAT  ████████████████░░░░░░░░  73% ↓         HEALTH  ● 92  good            │
└────────────────────────────────────────────────────────────────────────────┘
  q quit   ↑↓ select   tab cycle panel   p pause   ? help
```

---

## Features

| Panel | Data source | What you see |
|-------|-------------|--------------|
| **CPU** | `/proc/stat` | Per-core % bars (2-column), aggregate %, 60s load sparkline |
| **MEM** | `/proc/meminfo` | RAM used (cyan), swap used (threshold-colored) |
| **DISK** | `/proc/mounts` + `statvfs` | Per-mount usage bars, threshold-colored |
| **NET** | `/proc/net/dev` | rx/tx bytes per second, 60-sample sparklines |
| **TEMP** | `/sys/class/hwmon` | CPU and SSD temperatures, threshold-colored gauges |
| **BAT** | `/sys/class/power_supply` | Charge %, charging/discharging status |
| **HEALTH** | Derived | Composite score from CPU, memory, disk, and thermal |

**Color thresholds** apply to all gauges: below 40% blue · 40–69% yellow · 70%+ red.

**Keybinds**

| Key | Action |
|-----|--------|
| `q` | Quit |
| `p` | Pause / resume sampling |
| `Esc` | Return to launch screen |

---

## Architecture

Collection and rendering are strictly separated, with state in the middle.

```
/proc · /sys
    │
    ▼
collect/*          ← pure parsers; never touch the terminal
    │  plain structs
    ▼
app.rs             ← owns all state; drives the tick loop
    │  read-only ref
    ▼
ui/*               ← pure render functions; never read /proc
    │
    ▼
ratatui frame
```

Every parser in `collect/` takes a `&str` of file content and returns a plain
struct — testable against canned fixtures with no live system required. The thin
`read()` wrapper that reads the actual file is separate.

**Source layout**

```
src/
├── main.rs
├── app.rs           # App state, tick loop, key handling
├── splash.rs        # Launch wordmark
├── collect/
│   ├── cpu.rs       # /proc/stat — delta-based usage
│   ├── memory.rs    # /proc/meminfo
│   ├── disk.rs      # /proc/mounts + statvfs + /proc/diskstats I/O rates
│   ├── network.rs   # /proc/net/dev — delta-based rates
│   ├── thermal.rs   # /sys/class/hwmon — sensor priority ranking
│   ├── power.rs     # /sys/class/power_supply
│   └── system.rs    # hostname, uptime, load averages, clock
└── ui/
    ├── mod.rs        # layout engine
    └── widgets.rs    # one render fn per panel
```

---

## Roadmap

### Phase 1 — Monitor (current, v0.1)

The read-only dashboard described above. Every metric comes directly from the
kernel. No writes, no deletions, nothing destructive.

**Upcoming in Phase 1:**

- `?` help overlay with keybind reference
- Panel focus with `↑↓` / `tab` — highlight selected panel
- Process list (top N by CPU/memory, sortable)
- GPU metrics via `/sys/class/drm` and `hwmon` where available
- Per-core frequency from `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq`
- Battery time-remaining estimate from energy drain rate
- Configurable refresh rate via `--interval`
- Mouse support for panel selection

---

### Phase 2 — Cleaning (planned, v0.5+)

owl will grow a second mode: **interactive disk cleaning**. The same terminal,
the same zero-dependency philosophy — but now with the ability to reclaim space
from caches, orphaned packages, and leftover config directories.

The cleaning phase is designed with a strict safety contract:

1. **Dry-run by default.** `--execute` is an explicit opt-in per invocation.
   The tool never deletes anything without being told to.
2. **Scan → manifest → confirm → execute.** No fused "auto-clean" flow.
   You see exactly what will be removed and how much space it reclaims before
   anything happens.
3. **Protected-path predicate.** `/`, `/etc`, `/usr`, `/var`, `/boot`, `/proc`,
   `/sys`, and anything outside a narrow allowlist are hard-blocked. Symlinks
   and `..` traversal are canonicalized before every check.
4. **Append-only audit log** at `~/.local/state/owl/audit.log`. Every action
   is recorded.
5. **No sudo.** If a target needs root, owl prints the command for you to run.

**Planned cleaning targets (in order of safety):**

| Milestone | Target | Notes |
|-----------|--------|-------|
| v0.5 | Safety primitives | Protected-path predicate, dry-run mode, audit log — no user features yet |
| v0.6 | Read-only scanner | Walks targets, produces manifest with size preview; cannot delete |
| v0.7 | Caches | Thumbnail cache, browser caches, journald vacuum, pacman `paccache` |
| v0.8 | Orphan packages + configs | `pacman -Qtdq` orphans; `~/.config/<app>` / `~/.local/share/<app>` where app is gone |
| v0.9 | Docker prune | `docker system prune` with size preview; gated on Docker presence |
| v0.10 | User deny-list | `~/.config/owl/protect.toml` — paths owl must never touch |

**Explicit non-goals:** page-cache dropping (`echo 3 > /proc/sys/vm/drop_caches`
is a placebo), swappiness tuning, preload daemons, Flatpak/Snap leftover cleaning
(heuristics are unreliable and blast radius is too large).

---

## Install & Run

**From source (requires Rust stable):**

```sh
git clone <repo>
cd owl
cargo run --release        # launch directly
cargo install --path .     # install as `owl` on your PATH
```

**Run from anywhere after install:**

```sh
owl
```

**Other commands:**

```sh
cargo test                 # run the unit test suite (58 tests, no live system needed)
cargo build --release      # build release binary at target/release/owl
```

---

## Design notes

- **No external dependencies** beyond `ratatui` and `libc`. Every metric is
  parsed from kernel-provided files — no `sysinfo`, `procfs` crate, or shell
  command invocations.
- **Testable by design.** All parsers are pure `parse(&str) -> Struct` functions
  exercised against fixture strings. The test suite runs on any machine, even
  one without the monitored hardware.
- **Rate metrics** (CPU %, network throughput, disk I/O) store the previous
  sample in `App` state and compute deltas. Single-read metrics (memory, disk
  usage) are straightforward snapshots.
- **No panics in collectors.** Malformed `/proc` lines are silently skipped.
  Missing hardware (no battery, no hwmon sensors) gracefully hides the
  relevant panel.
- **Truecolor throughout.** Accent `#3fdcdc` · TX magenta `#e06ce0` · healthy
  green `#5fd38a` · warn yellow `#e6c46b` · critical red `#e0685f`.

---

## License

MIT
