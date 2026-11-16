use std::path::Path;

const AVATAR_SIZE: i32 = 192;
const MAX_INPUT_BYTES: u64 = 5 * 1024 * 1024;

pub fn normalize(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_INPUT_BYTES {
        return Err("Image is larger than 5 MiB".to_owned());
    }
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).map_err(|error| error.to_string())?;
    let width = pixbuf.width();
    let height = pixbuf.height();
    let side = width.min(height);
    let x = (width - side) / 2;
    let y = (height - side) / 2;
    let cropped = gdk_pixbuf::Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, true, 8, side, side)
        .ok_or_else(|| "Could not create avatar image".to_owned())?;
    pixbuf.copy_area(x, y, side, side, &cropped, 0, 0);
    let scaled = cropped
        .scale_simple(AVATAR_SIZE, AVATAR_SIZE, gdk_pixbuf::InterpType::Bilinear)
        .ok_or_else(|| "Could not resize avatar".to_owned())?;
    encode_png(&scaled)
}

fn encode_png(pixbuf: &gdk_pixbuf::Pixbuf) -> Result<Vec<u8>, String> {
    let width = u32::try_from(pixbuf.width()).map_err(|error| error.to_string())?;
    let height = u32::try_from(pixbuf.height()).map_err(|error| error.to_string())?;
    let channels = usize::try_from(pixbuf.n_channels()).map_err(|error| error.to_string())?;
    let rowstride = usize::try_from(pixbuf.rowstride()).map_err(|error| error.to_string())?;
    let bytes = pixbuf.read_pixel_bytes();
    let raw = bytes.as_ref();
    let row_bytes = usize::try_from(width).map_err(|error| error.to_string())? * channels;
    let mut pixels = Vec::with_capacity(row_bytes * usize::try_from(height).unwrap_or_default());
    for row in 0..usize::try_from(height).map_err(|error| error.to_string())? {
        let start = row * rowstride;
        pixels.extend_from_slice(&raw[start..start + row_bytes]);
    }
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(if channels == 4 {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(&pixels)
            .map_err(|error| error.to_string())?;
    }
    Ok(encoded)
}
