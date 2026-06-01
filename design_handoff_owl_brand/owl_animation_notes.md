# Home-screen owl animation

A small owl that hops back and forth along a branch on the selection/home screen.
Designed to be cheap to render in `ratatui` (plain styled text on a cell grid).

See `owl_home.html` for the live reference (open in a browser).

## Sprite (6 rows × 10 cols)
A great-horned owl: big round eyes (the key owl read), ear tufts, hooked beak.
Keep the frames as `[&str; 6]` arrays and draw each line as a `Line`/`Span` styled
cyan (`#3fdcdc` truecolor, or `Color::Cyan`).





Glyphs are all single-cell: `◉ U+25C9` (eye), `▾ U+25BE` (beak), `╨ U+2568` (feet),
plus ASCII `_ | / \ ( ) ─`. The branch is a row of `─` (U+2500) drawn directly under
the feet. NB the eyes are the strongest signal — keep them big and round; don't shrink
to a single character.

## Motion
- The owl advances **one column per hop**, and a hop is a 2-tick cycle:
  - **airborne tick** → `hop` frame, lift sprite 1 row, move 1 column in travel dir
  - **landing tick**  → `perch` frame, back down on the branch
- Tick ≈ **170 ms** (~6 fps) — reads as a deliberate hop, not a glide.
- At each end of the travel range, **reverse direction and pause ~3 ticks** (a beat
  before hopping back).
- Blink on a perched tick every ~9 ticks (one tick).
- Owls face forward, so no horizontal flip is needed when direction changes.

## Pseudocode (ratatui tick handler)
```rust
// state: col: u16, dir: i16 (+1/-1), tick: u64, pause: u8
fn on_tick(s: &mut Owl, min: u16, max: u16) {
    if s.pause > 0 { s.pause -= 1; s.tick += 1; return; }
    if s.tick % 2 == 0 {                 // airborne: cover ground
        let n = s.col as i16 + s.dir;
        if n >= max as i16 { s.col = max; s.dir = -1; s.pause = 3; }
        else if n <= min as i16 { s.col = min; s.dir = 1; s.pause = 3; }
        else { s.col = n as u16; }
    }
    s.tick += 1;
}

fn frame(s: &Owl) -> (&'static [&'static str; 6], bool /*lifted*/) {
    let airborne = s.tick % 2 == 0;
    if !airborne && s.tick % 9 == 0 { return (&BLINK, false); }
    if airborne { (&HOP, true) } else { (&PERCH, false) }
}
```
Render the frame at `x = col`, `y = perch_row - (if lifted {1} else {0})`.
Drive `on_tick` from your event loop (e.g. a 170 ms tick alongside the metric-refresh
tick) so it keeps hopping while the menu waits for input.

## Notes
- Cheap: 5 short rows, one diff per tick. No extra deps.
- Calmer variant: raise the tick to 220–260 ms, or move 1 column every *other* hop.
- Pause the animation when the app is backgrounded / not focused to save cycles.
