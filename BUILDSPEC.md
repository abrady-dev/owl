# owl

A terminal system monitor for Linux, written in Rust. Live CPU, memory, disk,
network, thermal, and power stats in a clean TUI — inspired by `btop` and the
`mo status` dashboard from [Mole](https://github.com/tw93/Mole), but Linux-native
and built directly on `/proc` and `/sys`.

---

## Instructions for Claude Code

**Build this project end-to-end, milestone by milestone, through v0.4 only.**
v0.5 onward (the "Phase 2 — Cleaning" section at the bottom of this README) is
**out of scope for this build pass** and is included as a sketch only. Do not
begin v0.5+ unless the user explicitly authorizes a new build pass for it.

After completing each milestone:

1. Write the unit tests specified for that milestone.
2. Run `cargo build` and `cargo test`. Both must pass with zero warnings before
   you move on. If either fails, fix it before continuing.
3. Run `cargo fmt` and `cargo clippy -- -D warnings`. Both must be clean.
4. **Stop and report.** Print a short summary of what you built, what tests
   passed, and the manual verification checklist for this milestone (copied from
   below). Wait for the user to run the manual verification themselves and
   confirm before you proceed to the next milestone.

Do **not** chain milestones. Do **not** start the next milestone until the user
explicitly says to proceed. The whole point of this workflow is a clean stop
after each step so the user can run the binary, verify the UI, and catch issues
before they compound.

If a test or manual check is impossible to satisfy as specified, stop and ask
rather than weakening the test.

---

## Tech stack

- **Language:** Rust, edition 2021.
- **TUI:** [`ratatui`](https://docs.rs/ratatui) `0.29`. Use the modern
  `ratatui::init()` / `ratatui::restore()` / `DefaultTerminal` API. Do not
  hand-roll alternate-screen / raw-mode / panic-hook setup; `ratatui::init()`
  installs the panic hook automatically (since 0.28.1).
- **Terminal backend:** `crossterm` `0.28` (re-exported by ratatui). If a version
  mismatch appears, drop the explicit `crossterm` dependency and import its types
  from `ratatui::crossterm::*` so the version always matches ratatui's.
- **Platform:** Linux only. Reads `/proc` and `/sys`. No cross-platform layer.

`Cargo.toml`:

```toml
[package]
name = "owl"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
```

---

## Architecture

**Collection and rendering are strictly separated, with state in the middle.**

```
src/
├── main.rs          # entry point: splash, ratatui::init/restore, kicks off run()
├── app.rs           # App struct — owns all state; has refresh() and on_key()
├── collect/         # the DATA layer. Reads /proc and /sys. Returns plain structs.
│   ├── mod.rs
│   ├── cpu.rs       # /proc/stat
│   ├── memory.rs    # /proc/meminfo
│   ├── disk.rs      # /proc/mounts + statvfs; /proc/diskstats for I/O rates
│   ├── network.rs   # /proc/net/dev
│   ├── thermal.rs   # /sys/class/hwmon
│   └── power.rs     # /sys/class/power_supply
└── ui/              # the RENDER layer. Pure functions: state in, frame out.
    ├── mod.rs
    └── widgets.rs   # one render fn per widget (mem gauge, cpu bars, net sparkline)
```

Data flow is one-directional:

```
collect::* ──(plain structs)──▶ App state ──(read-only)──▶ ui::*
```

- `collect/` modules **never** touch ratatui or the terminal. They read kernel
  files and return data. They are independently testable.
- `ui/` modules **never** read `/proc` or hold state. They take a reference to
  state and draw it. No side effects.
- `app.rs` is the only thing that connects them.

Crucially for testing: every parser in `collect/` must be factored so that it
takes a **`&str` of file contents** as input, not a path. The parser is pure and
unit-testable; a thin wrapper reads the file and calls the parser. Example:

```rust
// in collect/memory.rs
pub fn read() -> io::Result<MemStats> {
    let raw = std::fs::read_to_string("/proc/meminfo")?;
    Ok(parse(&raw))
}

pub fn parse(raw: &str) -> MemStats { /* ... */ }
```

Tests then exercise `parse()` against canned fixtures, never the live filesystem.

---

## The run loop

Non-blocking, timer-driven. **Not** "sleep, redraw, repeat" — that makes
keypresses feel laggy.

1. Draw the current state.
2. Compute time remaining until the next scheduled refresh.
3. Block for input with a timeout equal to that remaining time.
   - A keypress wakes the loop instantly and is handled immediately.
   - Otherwise the timeout expires, we re-sample, and reset the tick clock.

Default tick rate is 1000ms; drop toward ~500ms once the network sparkline exists.
Guard key handling on `KeyEventKind::Press`. `q` or `Esc` quits.

---

## The splash constant

Use this verbatim in `main.rs`. Preserve all leading whitespace. Padded raw-string
delimiter (`r##"..."##`) is required.

```rust
const SPLASH: &str = r##"
 __________-------____                 ____-------__________
          \------____-------___--__---------__--___-------____------/
           \//////// / / / / / \   _-------_   / \ \ \ \ \ \\\\\\\\/
             \////-/-/------/_/_| /___   ___\ |_\_\------\-\-\\\\/
               --//// / /  /  //|| (O)\ /(O) ||\\  \  \ \ \\\\--
                    ---__/  // /| \_  /V\  _/ |\ \\  \__---
                         -//  / /\_ ------- _/\ \  \\-
                           \_/_/ /\---------/\ \_\_/
                               ----\   |   /----
                                    | -|- |
                                   /   |   \
                                   ---- \___|

                    owl · system monitor v0.1
"##;
```

`println!("{SPLASH}")` at the very top of `main()`, before `ratatui::init()`, so
it stays in scrollback.

---

## Milestones

Each milestone has the same shape: **scope · implementation notes · unit tests ·
manual verification · STOP**.

### v0.1 — Scaffold + memory

**Scope.** The project compiles, prints the splash, opens a TUI, shows a live
memory gauge, and quits cleanly on `q` / `Esc`.

**Implementation notes.**
- Create the module layout above. `cpu.rs`, `disk.rs`, `network.rs`, `thermal.rs`,
  `power.rs` are empty stubs in v0.1 — just `pub fn read()` returning a default
  struct, or omit them entirely until their milestone. Don't pre-implement.
- `memory.rs`: parse `/proc/meminfo`. Compute `used = MemTotal − MemAvailable`.
  Do **not** use `MemFree` for "used" — `MemFree` counts reclaimable cache as
  used and produces the "Linux ate my RAM" illusion.
- One widget: a `Gauge` bound to memory used-ratio, labeled `X.X / Y.Y GiB`.
- The render fn lays the screen out with `Layout::vertical` so future widgets
  slot in without rewriting it.

**Unit tests** (`collect/memory.rs` — `#[cfg(test)] mod tests`):
- `parse_real_meminfo`: feed a realistic multi-line `/proc/meminfo` fixture
  containing `MemTotal`, `MemFree`, `MemAvailable`, `Buffers`, `Cached`, and at
  least 5 other fields. Assert `total_kb` and `used_kb = total − available`.
- `parse_missing_available`: feed input that lacks `MemAvailable`. Parser must
  not panic; `used_kb` should be 0 (or `total - 0`, document which).
- `parse_empty`: empty string. No panic. `total_kb == 0`, `used_ratio() == 0.0`.
- `used_ratio_zero_total`: `MemStats { total_kb: 0, used_kb: 0 }.used_ratio()`
  returns `0.0`, not `NaN`.

**Manual verification** (user runs):
- [ ] `cargo run` prints the owl splash to scrollback.
- [ ] After the splash, the dashboard opens, taking over the screen.
- [ ] The memory gauge shows a sensible value (compare with `free -h`; should be
      within a few hundred MB).
- [ ] The gauge updates roughly once per second.
- [ ] `q` quits cleanly; terminal is restored (prompt comes back, splash still
      visible in scrollback).
- [ ] `Esc` also quits.
- [ ] Forcing a panic (temporarily add `panic!()` somewhere in the loop, then
      revert) does **not** leave the terminal in a broken state — `ratatui::init()`'s
      panic hook should restore it.

**STOP.** Report results and wait.

---

### v0.2 — CPU

**Scope.** Add a total-CPU usage gauge below the memory gauge, updating live.
Per-core bars optional as a stretch.

**Implementation notes.**
- `/proc/stat` line `cpu  ...` gives cumulative ticks: `user nice system idle
  iowait irq softirq steal guest guest_nice`. The values are **monotonic
  counters**, not percentages. A single read is meaningless.
- Keep the previous `CpuStats` reading in `App` state. Each refresh: read again,
  compute deltas, usage = `(total_delta − idle_delta) / total_delta`. On the
  first refresh there's no previous reading; report 0 or skip the frame.
- The parser must return raw counters; the **delta math lives in `app.rs`** or
  a small helper, so it can be unit-tested with two manufactured `CpuStats`
  values without touching the filesystem.

**Unit tests:**
- `parse_proc_stat`: fixture with the aggregate `cpu ` line plus several `cpuN `
  per-core lines. Assert the aggregate counters match exactly.
- `cpu_usage_from_deltas`: given two `CpuStats` values (before/after), assert
  the computed usage ratio matches a hand-calculated expected value.
- `cpu_usage_no_change`: identical before/after readings. Usage should be 0.0,
  not `NaN` (the delta total is zero — guard the division).
- `cpu_usage_all_busy`: before/after where all delta is non-idle. Usage = 1.0.
- `parse_proc_stat_per_core`: assert N per-core entries are extracted in order.

**Manual verification:**
- [ ] CPU gauge appears below memory.
- [ ] Idle system shows low usage (<10%).
- [ ] Running `yes > /dev/null &` (one core's worth of load) noticeably raises
      the gauge; killing it brings it back down within a couple of refreshes.
- [ ] Running `stress -c $(nproc)` (or equivalent) saturates the gauge near 100%.
- [ ] The gauge is **never** stuck at 0% or 100% — that means the delta logic
      is wrong.

**STOP.** Report results and wait.

---

### v0.3 — Disk + network

**Scope.** Add a disk-usage widget (one row per mounted filesystem, excluding
pseudo-filesystems) and a network throughput sparkline (rx and tx, bytes/sec).

**Implementation notes.**
- Disk usage: parse `/proc/mounts`, filter to real filesystems (exclude `tmpfs`,
  `proc`, `sysfs`, `cgroup*`, `devtmpfs`, `overlay`, `squashfs`, etc. — keep a
  small allowlist of fs types like `ext4`, `btrfs`, `xfs`, `zfs`, `f2fs`, `vfat`,
  `ntfs`). For each, call `statvfs` to get used/total bytes.
- Network: `/proc/net/dev` gives cumulative rx/tx bytes per interface. Like CPU,
  this is a **rate** — two reads and a delta. Skip the loopback interface.
- Sparkline history: a `VecDeque<u64>` in `App` per direction, capped at the
  sparkline's render width (e.g. 60 samples). Push on each refresh.
- Drop `tick_rate` to ~500ms now that there's a sparkline; otherwise it animates
  too slowly.

**Unit tests:**
- `parse_proc_mounts`: fixture with a mix of real and pseudo filesystems.
  Assert only the allowlisted types come through.
- `parse_proc_net_dev`: realistic fixture with `lo`, `eth0`, `wlan0`. Assert
  loopback is excluded and rx/tx byte counters are extracted.
- `net_rate_from_deltas`: given two readings 500ms apart with a known byte
  delta, assert the computed bytes/sec.
- `net_rate_counter_wrap`: simulate a counter going backwards (interface reset
  or 32-bit wrap). The rate function must return 0, not a huge negative.
- `sparkline_history_caps_at_width`: pushing more samples than the cap keeps
  the deque at exactly the cap length.

**Manual verification:**
- [ ] Disk widget shows your real filesystems (cross-check with `df -h`).
  No tmpfs or other pseudo entries.
- [ ] Network sparkline is mostly flat when idle.
- [ ] Running `curl -o /dev/null https://speed.cloudflare.com/__down?bytes=100000000`
      produces a visible rx spike; the sparkline animates smoothly.
- [ ] Stopping the transfer brings rx back to ~0 within a couple of seconds.
- [ ] Refresh feels smooth, not jumpy.

**STOP.** Report results and wait.

---

### v0.4 — Thermal + power + health score

**Scope.** Add temperature readouts and (if a battery is present) battery state.
Synthesize a single health-score indicator.

**Implementation notes.**
- Temps: walk `/sys/class/hwmon/hwmon*/`. Each has a `name` file identifying the
  chip and `tempN_input` files in **millidegrees Celsius** (divide by 1000).
  hwmon indices are **not stable across boots or hardware** — detect by the
  `name` file (e.g. prefer `coretemp`, `k10temp`, or `acpitz`) rather than
  hardcoding `hwmon0`.
- Power: `/sys/class/power_supply/`. Iterate for entries whose `type` is
  `Battery`. Use `capacity` (percent), `status` (`Charging`/`Discharging`/`Full`),
  and optionally `energy_now`/`energy_full` for a more accurate percentage.
  If no battery (desktop), simply omit the widget.
- Health score: a function of CPU usage, memory pressure, disk usage on root,
  and max temp. Define it in `app.rs`. Color it green/yellow/red.

**Unit tests:**
- `parse_hwmon_temp`: given a string `"45000"`, assert parsed temp is 45.0°C.
- `select_preferred_hwmon`: given a list of (name, temp) pairs from multiple
  hwmon entries, assert the preferred one (`coretemp` over `acpitz`, etc.) wins.
- `parse_battery_capacity`: given `"87"`, assert 0.87.
- `parse_battery_missing`: returns `None`, doesn't panic.
- `health_score_bounds`: across a sweep of inputs the score stays in [0, 100].
- `health_score_thresholds`: an all-quiet system scores in the green range; a
  hot, full, busy system scores red.

**Manual verification:**
- [ ] Temperature widget shows a plausible CPU temp (compare with
      `sensors` from `lm_sensors`).
- [ ] Stressing the CPU raises the temp visibly over ~10s.
- [ ] On a laptop: battery percentage matches the system tray indicator;
      `Charging`/`Discharging` status follows the cable.
- [ ] On a desktop: battery widget is absent, not showing 0% or an error.
- [ ] Health score visibly shifts when you stress the system.

**STOP.** Dashboard phase complete. The cleaning phase (v0.5+) is sketched below
but is **not** in scope for this build pass. Stop here and wait for the user to
explicitly authorize starting the cleaning phase as a separate effort.

---

## Phase 2 — Cleaning (sketch, deferred)

The dashboard phase above is read-only. Phase 2 adds destructive operations
(deleting caches, removing orphan packages, etc.). It is **deliberately sketched
rather than fully specified** because its design will be refined when the user is
ready to start it, and because the safety constraints are too important to lock
in prematurely.

**Do not begin Phase 2 from this README alone.** When the user is ready, this
section will be expanded into full milestones with the same shape as v0.1–v0.4
(scope, implementation notes, unit tests, manual verification, STOP).

### Hard rule before any destructive code

v0.5 is **safety primitives only — no user-visible features.** It must ship and
be tested before any deletion code exists anywhere in the repo. Required
primitives:

- A protected-path predicate (`/`, `/home`, `/etc`, `/usr`, `/var`, `/boot`,
  `/proc`, `/sys`, `/dev`, anything outside a small allowlist).
- Path canonicalization before every protection check, so symlinks and `..`
  segments can't bypass it.
- Dry-run as the default mode; `--execute` is opt-in per invocation.
- A two-phase flow: scan → present manifest → explicit confirm → execute.
  No "auto-clean" that fuses these.
- An append-only audit log at `~/.local/state/owl/audit.log`.
- The tool never invokes `sudo` itself. If a target needs root, it prints the
  command for the user to run manually.

Safety primitives are unit-tested against adversarial inputs (empty paths, `~`,
`.`, `/`, `..` traversal, symlinks pointing up, etc.) before any cleaner is
wired to them.

### Sketch of subsequent milestones

- **v0.6 — Read-only scanner.** Walks allowlisted targets, produces the
  manifest ("would reclaim X GiB"). Cannot delete anything. Run it for a week
  to validate the numbers before wiring the execute path.
- **v0.7 — Safest execute targets.** Thumbnail cache, browser caches via
  known-safe paths, journald vacuum (via `journalctl --vacuum-*`), pacman cache
  via `paccache -r`, trash. These either have narrow blast radius or delegate
  to well-tested upstream tools.
- **v0.8 — Orphan packages + orphan configs.** Pacman orphans via
  `pacman -Qtdq`, then the genuinely novel feature: `~/.config/<app>` and
  `~/.local/share/<app>` directories where `<app>` is no longer installed.
  Higher false-positive risk, so interactive per-item confirmation, not bulk.
- **v0.9 — Docker prune.** `docker system prune` integration with size
  preview. Gated behind detection that Docker is present and the user is in the
  `docker` group.
- **v0.10 — User-level deny-list.** `~/.config/owl/protect.toml` for paths owl
  must never touch even if otherwise allowlisted. Belt and suspenders.

### Explicit non-goals (do not propose adding these)

- `echo 3 > /proc/sys/vm/drop_caches`. Placebo; the kernel manages page cache.
- Swappiness twiddling, preload daemons, `prelink`, etc. "Optimization" tricks
  whose value can't be measured in concrete bytes reclaimed or apps cleanly
  removed.
- On-demand `fstrim` (the systemd timer already handles this; if anything, owl
  should *check that the timer is enabled* rather than running trim itself).
- Flatpak/Snap leftover-data cleaning. Deferred indefinitely — the heuristics
  for "truly orphaned" vs "user hasn't opened it lately" are unreliable and
  the blast radius (live app state) is too large.

---

## Project-wide conventions

- **Hand-roll `/proc` and `/sys` parsing.** Do not add `sysinfo` or similar.
- **Every parser is pure** (`parse(&str) -> Stats`) with a thin file-reading
  wrapper, so it's unit-testable against fixtures.
- **Rate metrics** (CPU, network, disk I/O) keep last reading in `App` state and
  compute deltas. Single-read parsers for snapshot metrics (memory, disk usage).
- **No panics in collectors.** Malformed `/proc` lines must be skipped, not
  crashed on. The parser tests above enforce this.
- **No blocking the run loop.** All reads here are fast, but if you ever add one
  that might stall, spawn it.
- **Format every number before it hits the screen.** No raw floats. Bytes use
  binary units (KiB/MiB/GiB).
- **`cargo fmt` and `cargo clippy -- -D warnings` clean** at every milestone.

---

## Build & run

```sh
cargo run              # launch the dashboard
cargo run --release    # smoother for the live view
cargo test             # run the parser/math unit tests
```

Quit with `q` or `Esc`.

---

## License

MIT (placeholder — set as preferred).