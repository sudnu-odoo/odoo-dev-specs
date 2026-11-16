use anyhow::{ensure, Result};

/// Width and height from a PNG's IHDR header.
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    ensure!(
        bytes.len() >= 24 && bytes[..8] == SIGNATURE && &bytes[12..16] == b"IHDR",
        "The image is not a PNG file."
    );
    let read =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let (width, height) = (read(16), read(20));
    ensure!(width > 0 && height > 0, "The PNG image is empty.");
    Ok((width, height))
}
