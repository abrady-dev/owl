# owl

```
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

                    owl · system monitor
```

A terminal system monitor for Linux, written in Rust. Live CPU, memory, disk,
network, and thermal stats in a clean TUI — inspired by tools like `btop` and
the `mo status` dashboard from [Mole](https://github.com/tw93/Mole), but Linux-native
and built from `/proc` and `/sys` directly.

This is a learning project. The point is to understand how Linux exposes system
state, not just to wrap a crate that hides it. See "Conventions" below — some of
the implementation is intentionally hand-rolled rather than delegated to a library.

---

## Status

Early. The memory widget and the core run loop work (v0.1). Everything else on the
milestone ladder is unbuilt.

---

## Tech stack

- **Language:** Rust (edition 2021)
- **TUI:** [`ratatui`](https://docs.rs/ratatui) `0.29` — uses the modern
  `ratatui::init()` / `ratatui::restore()` / `DefaultTerminal` API. Do **not**
  hand-roll alternate-screen / raw-mode / panic-hook setup; `ratatui::init()`
  installs the panic hook automatically (since 0.28.1).
- **Terminal backend:** `crossterm` `0.28` (re-exported by ratatui).
- **Platform:** Linux only. Reads `/proc` and `/sys`. No cross-platform
  abstraction layer — this is deliberate.

If a `crossterm` version mismatch appears at build time, drop the explicit
`crossterm` dependency and import its types from `ratatui::crossterm::*` instead,
so the version always matches what ratatui expects.

---

## Architecture

The single most important rule: **collection and rendering are strictly
separated, with state in the middle.**

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
  files and return data (`MemStats`, `CpuStats`, …). They are independently
  testable and could be reused by a future non-TUI consumer.
- `ui/` modules **never** read `/proc` or hold state. They take a reference to
  state and draw it. No side effects.
- `app.rs` is the only thing that connects them: `App::refresh()` calls the
  collectors and stashes results; the render layer reads `App`.

Fusing "read the data" and "draw the data" into one function is the most common
TUI-project mistake. Don't.

---

## The run loop

Non-blocking, timer-driven. **Not** "sleep, redraw, repeat" — that makes
keypresses feel laggy. Instead:

1. Draw the current state.
2. Compute time remaining until the next scheduled refresh.
3. Block for input with a timeout equal to that remaining time.
   - A keypress wakes the loop instantly and is handled immediately.
   - Otherwise the timeout expires, we re-sample, and reset the tick clock.

This is what makes the dashboard feel live without pinning a CPU core. Default
tick rate is 1000ms; drop toward ~500ms once the network sparkline exists so the
graph animates smoothly. Changing the one `tick_rate` constant is all that takes.

Guard key handling on `KeyEventKind::Press` (Windows emits both Press and Release;
harmless on Linux, good habit).

`q` or `Esc` quits.

---

## Conventions

These are intentional. Please follow them rather than "improving" them away.

- **Hand-roll the `/proc` and `/sys` parsing.** Do not add `sysinfo` or similar
  to get metrics for free in the core milestones — parsing the kernel interfaces
  by hand is the entire point of the project. `sysinfo` may be introduced *later*,
  only for genuinely tedious metrics (e.g. per-process stats), and only when
  explicitly requested.
- **Splash on launch:** print the owl banner to normal stdout *before*
  `ratatui::init()`, so it stays in scrollback like a startup log rather than
  vanishing into the alternate screen. Store it as a `const SPLASH: &str` using a
  Rust raw string with a **padded delimiter** (`r##"..."##`) — the art is full of
  `\`, `/`, `)`, and `"` characters that would otherwise need escaping, and the
  padded `##` delimiter guarantees nothing inside accidentally terminates the
  string even if the art is edited later.
- **The splash is whitespace-sensitive fixed-width art**, 69 columns wide and 12
  lines tall (plus tagline). Preserve leading whitespace exactly — the left-side
  spaces are what center the owl. Do not let an editor strip or reflow it, and do
  not convert spaces to tabs. It is pure ASCII (no Unicode box-drawing), so it
  renders identically across Konsole, Alacritty, kitty, Ghostty, etc. It may wrap
  on terminals narrower than ~70 columns — acceptable, since it prints to
  scrollback. Do **not** render art this wide as a Paragraph inside the live TUI;
  that wrapping would break the layout.
- Keep widget render functions small and one-per-widget in `ui/widgets.rs`.
- Round/format every number that reaches the screen; don't print raw floats.

---

## Milestone ladder

Build in this order. Each step is mostly "add one collector + one widget" once
the scaffold exists — except v0.1, which is the scaffolding itself.

- **v0.1 — scaffold + memory.** Run loop, quit handling, clean teardown, splash,
  and one real widget reading `/proc/meminfo`. (Largely done.)
- **v0.2 — CPU.** `/proc/stat`. This is the conceptual centerpiece: CPU usage is a
  **rate, not a snapshot**. `/proc/stat` gives cumulative tick counters since boot;
  a single read tells you nothing. Read it, wait the interval, read again, and
  compute usage from the **delta** (busy ticks ÷ total ticks across the window).
  A "stuck at 0% or 100%" bug means the delta logic is wrong. Per-core bars are a
  good stretch once total works.
- **v0.3 — disk + network.** `/proc/net/dev` (also a rate, like CPU). Network
  sparklines are the biggest visual payoff; ratatui has a built-in `Sparkline`
  widget. Disk usage via `statvfs` on mount points from `/proc/mounts`; I/O rates
  via `/proc/diskstats` (rate again).
- **v0.4 — thermal + power + health score.** `/sys/class/hwmon/hwmon*/tempN_input`
  (millidegrees — divide by 1000). Battery at `/sys/class/power_supply/BAT0|BAT1/`.
  hwmon paths are not stable across hardware, so detect the right sensor at runtime
  rather than hardcoding an index. Finish with a synthesized health score
  (CPU + memory + disk + temp + I/O), color-coded.

---

## Implementation notes / gotchas

Capturing these so they aren't re-litigated mid-build:

- **Memory "used" = `MemTotal − MemAvailable`**, not `MemTotal − MemFree`. The
  `MemFree` version counts reclaimable disk cache as "used" and produces the
  infamous "Linux ate my RAM" illusion. `MemAvailable` is the correct modern
  figure. `/proc/meminfo` is a snapshot — no delta needed (contrast with CPU).
- **CPU and network are rates** — two reads and a delta. Memory and disk-usage
  are snapshots — one read. Understanding *why these differ* is most of the lesson.
- **Sparklines** need a rolling history buffer in `App` state (e.g. a `VecDeque`
  of recent samples), since each frame shows a window of past values.
- **Don't block the loop on slow reads.** All `/proc` reads here are fast, but if
  a `/sys` read ever stalls, it shouldn't freeze input handling.

---

## The splash constant

Use this verbatim in `main.rs`. Preserve all leading whitespace; padded raw-string
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

Printed once to stdout at the very top of `main()`, before `ratatui::init()`.

---

## Build & run

```sh
cargo run            # launch the dashboard
cargo run --release  # smoother for the live view
```

Quit with `q` or `Esc`.

---

## License

MIT (placeholder — set as preferred).
