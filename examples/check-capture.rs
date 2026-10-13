//! Verify the launcher fills its window, without an opaque black surround.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let screenshot = image::open(&args[0])?.to_rgb8();
    let coords: Vec<u32> = args[1..]
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    let [x, y, width, height] = coords[..] else {
        return Err("expected x y width height".into());
    };
    // Rounded corners must reveal the blue backdrop, not an opaque black rectangle.
    for (dx, dy) in [
        (2, 2),
        (width - 3, 2),
        (2, height - 3),
        (width - 3, height - 3),
    ] {
        let pixel = screenshot.get_pixel(x + dx, y + dy).0;
        if pixel != [60, 110, 170] {
            return Err(
                format!("corner transparency regression at ({dx}, {dy}): {pixel:?}").into(),
            );
        }
    }
    // The 92% panel blends with the blue backdrop, including right up to its edges.
    for (dx, dy) in [
        (width / 2, 0),
        (0, height / 2),
        (width - 1, height / 2),
        (width / 2, height - 1),
    ] {
        let pixel = screenshot.get_pixel(x + dx, y + dy).0;
        for (actual, expected) in pixel.into_iter().zip([34_u8, 38, 43]) {
            if actual.abs_diff(expected) > 2 {
                return Err(format!(
                    "panel opacity/surround regression at ({dx}, {dy}): {pixel:?}"
                )
                .into());
            }
        }
    }
    Ok(())
}
