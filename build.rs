fn main() {
    #[cfg(target_os = "windows")]
    {
        generate_ico_from_png();
        embed_resource::compile("installer/resources.rc", None::<&str>);
    }
}

#[cfg(target_os = "windows")]
fn generate_ico_from_png() {
    let png_path = std::path::Path::new("assets/mzc_app_icon.png");
    let ico_path = std::path::Path::new("assets/mzc_app_icon.ico");

    if ico_path.exists() {
        return;
    }

    if !std::path::Path::new("assets").exists() {
        let _ = std::fs::create_dir("assets");
    }

    if png_path.exists() {
        // Try using Python PIL first as it generates a highly compatible multi-size ICO
        let status = std::process::Command::new("python")
            .args(&[
                "-c",
                "from PIL import Image; img = Image.open('assets/mzc_app_icon.png'); img.save('assets/mzc_app_icon.ico', format='ICO')"
            ])
            .status();

        if status.is_ok() && status.unwrap().success() {
            return;
        }

        // Fallback: write a basic ICO wrapper if Python is not available
        if let Ok(png_data) = std::fs::read(png_path) {
            let mut ico_data = Vec::new();

            // ICO Header: 6 bytes
            ico_data.extend_from_slice(&[0, 0]); // Reserved (must be 0)
            ico_data.extend_from_slice(&[1, 0]); // Resource Type (1 = Icon)
            ico_data.extend_from_slice(&[1, 0]); // Number of images (1)

            // Icon Directory Entry: 16 bytes
            ico_data.push(0); // Width
            ico_data.push(0); // Height
            ico_data.push(0); // Color palette count
            ico_data.push(0); // Reserved
            ico_data.extend_from_slice(&[0, 0]); // Planes (0 is standard for PNG in ICO)
            ico_data.extend_from_slice(&[0, 0]); // BitCount (0 is standard for PNG in ICO)

            let png_len = png_data.len() as u32;
            ico_data.extend_from_slice(&png_len.to_le_bytes()); // Size of image data in bytes
            ico_data.extend_from_slice(&22u32.to_le_bytes()); // Offset of image data from beginning of file (22)

            // Image Data: raw PNG bytes
            ico_data.extend_from_slice(&png_data);

            let _ = std::fs::write(ico_path, ico_data);
        }
    }
}
