# Handoff: `owl` — brand & TUI layout

## Overview
`owl` is a lean terminal system monitor written in Rust (using `ratatui`). This package
delivers its visual identity and the target dashboard layout:

- **Wordmark / launch art** — ASCII treatments of "owl" printed on launch (`src/splash.rs`)
- **README splash** — a monochrome owl illustration for the project README
- **Tagline** — the locked product line
- **Dashboard layout** — the target arrangement of the live monitor panels

## About the design files
The files in this bundle are **design references**. `owl-brand-lab.html` is an HTML
prototype that renders the ASCII and the dashboard exactly as intended (cyan accent on a
dark terminal). It is a *spec*, not code to ship.

Your job is to reproduce these in the real Rust/`ratatui` app:
- The `.txt` files under `ascii/` are the **actual artwork** — paste them straight into
  string constants in `splash.rs` / the README. They are not approximations.
- The dashboard `.txt` is a layout map; rebuild it with real `ratatui` widgets
  (`Block`, `Gauge`/`LineGauge`, `Sparkline`, `Layout` constraints), not as a printed string.

## Fidelity
**High-fidelity.** Colors, characters, and column widths are final and deliberate. The
ASCII is column-exact; preserve every space. Do not substitute box-drawing styles or
"clean up" the spacing.

---

## Brand tokens

### Color (ANSI / truecolor)
The app uses a single accent (cyan) plus standard status colors. Hex values are the
truecolor targets; the ANSI column is the 16-color fallback.

| Role            | Hex       | ANSI            | Used for |
|-----------------|-----------|-----------------|----------|
| Accent / RX     | `#3fdcdc` | Cyan            | wordmark, borders, primary bars, rx sparkline |
| Accent dim      | `#1f8a8a` | Cyan (dim)      | separators, tagline dots, faint rules |
| Network TX      | `#e06ce0` | Magenta         | tx sparkline & label |
| Healthy         | `#5fd38a` | Green           | ok gauges, health dot |
| Warn            | `#e6c46b` | Yellow          | 40–70% gauges |
| Critical        | `#e0685f` | Red             | >70% gauges |
| Load (low)      | `#6cb6e0` | Blue            | low CPU-core gauges (<40%) |
| Text            | `#c5d0d3` | Default fg      | primary values |
| Text dim        | `#6a777d` | Bright black    | labels |
| Text faint      | `#3f4a4f` | Bright black    | gauge troughs, help bar |
| Background      | `#0a0e0f` | Default bg      | terminal background |

**Gauge color thresholds** (apply per metric): `< 40%` → blue, `40–69%` → yellow,
`>= 70%` → red. MEM `ram` bar uses cyan regardless; `swp` uses green when near-empty.

### Typography
Terminal cell font. The reference renders in **JetBrains Mono**; any monospace works.
The only constraint that matters: **every glyph must be single cell-width.** See the
braille note below.

### Character set
- Wordmarks/dashboard borders: ASCII + Unicode block elements
  (`█ ░ ▁▂▃▄▅▆▇ ▟ ▙`) and box-drawing (`│ ─ ┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼`).
- Splash "Plume" uses **braille** (`U+2800–28FF`). In a real terminal braille is
  cell-width and aligns fine; the blank cells in that file are braille-blank
  (`U+2800`), **not** ASCII spaces — keep them as-is so the block stays aligned.

---

## 1 · Wordmark (launch art)

Six directions in `ascii/`. Pick one (or ship a couple behind a flag). All are
≤ 60 columns wide so they fit a default terminal.

| File | Style | Owl motif | Size (cols × rows) |
|------|-------|-----------|--------------------|
| `wordmark_standard.txt` | figlet-style slab | no | 28 × 5 |
| `wordmark_heavy.txt`    | bold block | no | 31 × 6 |
| `wordmark_solid.txt`    | filled blocks, uppercase | no | 18 × 5 |
| `wordmark_pixel.txt`    | filled blocks, lowercase | no | 15 × 5 |
| `wordmark_hoot.txt`     | owl-face glyph + Standard | **yes** | 36 × 5 |
| `wordmark_eye.txt`      | eye as the 'o' | **yes** | 12 × 3 |

**Recommendation:** `Solid` or `Pixel` for the launch screen (compact, on-brand),
`Hoot` when you want the owl character present. Print in the accent cyan.

Implementation: store as a `const &str` (raw string literal `r#"..."#`), print with the
cyan style, then the tagline line beneath it.

## 2 · README splash

Three monochrome owl illustrations in `ascii/`:

| File | Style | Size (cols × rows) |
|------|-------|--------------------|
| `splash_lineform.txt` | clean geometric line-art owl | 33 × 13 |
| `splash_facet.txt`    | minimal head & eyes | 21 × 8 |
| `splash_plume.txt`    | soft braille-shaded owl face | 18 × 8 |

Drop the chosen one in a fenced code block at the top of `README.md`. If you use
`splash_plume.txt`, keep it in a ```` ``` ```` block so GitHub renders the braille
monospaced.

## 3 · Tagline

**Locked:**

```
owl · lean eyes on Linux · system monitor
```

- `lean eyes on Linux` carries the voice (lean build, owl eyes, the platform).
- `system monitor` keeps it plain/discoverable.
- The `·` (U+00B7, middle dot) separators read as status dots; render them in cyan-dim.
- Use the full line in the README subtitle and `--help`; the short form
  `owl · lean eyes on Linux` goes in the dashboard title bar (see below).

---

## 4 · Dashboard layout

Target arrangement of the live monitor. See `ascii/dashboard_layout.txt` for the
column-exact reference (80 columns wide). Rebuild with `ratatui`, not as a printed string.

### Overall structure
A single outer `Block` (rounded/plain border in cyan) whose **title** is
`owl · lean eyes on Linux` (the `owl` part bold cyan, the rest cyan-dim). Inside,
vertical `Layout` of rows; the middle rows split left/right.

```
┌─ owl · lean eyes on Linux ───────────────────────────────────────────────────┐
│  header status line (full width)                                              │
├────────────────────────────────────────────┬─────────────────────────────────┤
│  CPU  (left ~54%)                            │  MEM  (right ~46%)              │
│   per-core gauges, 2 columns                 │   ram / swp bars                │
│                                              │                                 │
│   load 60s sparkline                         │  DISK  /  /home bars            │
├────────────────────────────────────────────┼─────────────────────────────────┤
│  NET  rx/tx + sparklines                     │  TEMP  cpu/ssd gauges           │
├────────────────────────────────────────────┴─────────────────────────────────┤
│  BAT gauge + HEALTH score (full width)                                        │
└────────────────────────────────────────────────────────────────────────────────┘
  q quit   ↑↓ select   tab cycle panel   p pause   ? help
```

Left/right split ≈ **42 / 35** terminal columns at 80-wide; use `ratatui`
`Constraint::Percentage(54)` / `Percentage(46)` (or `Min`/`Length`) so it reflows.

### Panel spec

**Header (full width):** `▟▙ owl` (cyan) · `v{version}` (dim) · `{hostname}` (text) ·
`up {uptime}` (dim) · `load {1m} {5m} {15m}` (dim label, text values) · `{HH:MM:SS}` (dim).
Right-align the clock.

**CPU (top-left):**
- Header: `CPU  {model}` and aggregate `{n}%` right-aligned.
- Per-core: two columns of cores (`c0`…`c7`), each a labelled `LineGauge`/bar 12 cells
  wide + right-aligned `{n}%`. Color by the `<40/40-70/>=70` threshold (blue/yellow/red).
- Below the cores: `load 60s` label + a `Sparkline` of the 1-min load history, cyan.

**MEM (top-right):**
- `ram` bar (cyan), 22 cells, value `{used}G` right-aligned.
- `swp` bar (green when low), value `{used}G`.

**DISK (right, under MEM):**
- One labelled bar per mount (`/`, `/home`, …), 15 cells, `{n}%`. Threshold colors.

**NET (mid-left):**
- `NET  {iface}` header (dim).
- `rx ↓ {rate}` in cyan + a cyan `Sparkline` of rx history.
- `tx ↑ {rate}` in magenta + a magenta `Sparkline` of tx history.

**TEMP (mid-right):**
- `cpu {n}°C` and `ssd {n}°C`, each a 16-cell gauge, threshold-colored.

**BAT / HEALTH (full width bottom):**
- `BAT` gauge (24 cells, green/yellow/red by charge) + `{n}%` + `{time remaining}`.
- `HEALTH ● {score} {label}` — colored dot + numeric score + word (`good`/`warn`/`bad`).

**Footer (outside the border):** dim keybind hints; highlight active keys in cyan:
`q quit   ↑↓ select   tab cycle panel   p pause   ? help`.

### Sparklines
The reference uses block characters `▁▂▃▄▅▆▇█` for sparklines (the on-screen HTML can't
render braille monospaced, but a real terminal can). In `ratatui`, prefer the built-in
`Sparkline` widget fed from a ring buffer of recent samples; set its style to the panel's
accent color (cyan for rx/load, magenta for tx).

---

## State / data sources
Read from `/proc` and `/sys` (no external deps — that's the product promise):
- CPU per-core %: delta of `/proc/stat` jiffies between ticks.
- Load: `/proc/loadavg`. Uptime: `/proc/uptime`.
- MEM/swap: `/proc/meminfo`. NET: `/proc/net/dev` (delta for rates).
- DISK: `statvfs` per mount from `/proc/mounts`.
- TEMP: `/sys/class/hwmon/*/temp*_input`. BAT: `/sys/class/power_supply/BAT*/`.
Keep short ring buffers (≈60 samples) per metric to feed the sparklines. Default refresh
~1 s; `p` pauses sampling.

## Files in this bundle
- `owl-brand-lab.html` — the visual reference (open in a browser to see final colors/layout).
- `ascii/wordmark_*.txt` — the six wordmarks, raw.
- `ascii/splash_*.txt` — the three README owls, raw.
- `ascii/dashboard_layout.txt` — 80-col dashboard reference (plain text).

## Notes
- Column widths in every `.txt` are exact — verify with a fixed-width check after pasting;
  don't let an editor trim trailing spaces.
- The dashboard layout is the **target**, derived from the brief. If the existing code
  already has panels, map them onto this arrangement rather than rewriting wholesale.
