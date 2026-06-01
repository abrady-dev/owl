pub const ART: &str = "\
█████ █     █ █
█   █ █     █ █
█   █ █  █  █ █
█   █ █ █ █ █ █
█████  █   █  █████";

/// Decode all frames from the 32-px idle sprite sheet (8 frames × 32×32 RGBA).
/// Returns an empty Vec if the PNG can't be decoded.
pub fn load_idle_frames() -> Vec<Vec<u8>> {
    const SHEET: &[u8] = include_bytes!(
        "../design_handoff_owl_brand/assets/owl-pixel-sheet-32.png"
    );
    let Ok(img) = image::load_from_memory(SHEET) else { return Vec::new() };
    let rgba = img.to_rgba8();
    let frame_w = 32u32;
    let num_frames = rgba.width() / frame_w;
    (0..num_frames)
        .map(|f| {
            let mut frame = Vec::with_capacity((frame_w * 32 * 4) as usize);
            for y in 0..32u32 {
                for x in 0..frame_w {
                    let p = rgba.get_pixel(f * frame_w + x, y);
                    frame.extend_from_slice(&p.0);
                }
            }
            frame
        })
        .collect()
}
