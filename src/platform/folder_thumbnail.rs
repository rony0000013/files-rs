use image::{imageops, ImageBuffer, Rgba};
use image::imageops::FilterType;

/// The coordinates (normalized 0.0-1.0) of the "viewport" inside a standard
/// folder icon where content thumbnails are visible.
const FOLDER_VIEWPORT: (f32, f32, f32, f32) = (0.12, 0.18, 0.76, 0.45);

/// Compose a folder thumbnail by clipping a content file thumbnail into the
/// transparent viewport of an open folder icon.
pub fn compose_folder_thumbnail(
    folder_icon: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    file_content: &ImageBuffer<Rgba<u8>, Vec<u8>>,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (fw, fh) = folder_icon.dimensions();

    // 1. Calculate viewport dimensions in pixels
    let v_x = (fw as f32 * FOLDER_VIEWPORT.0) as i64;
    let v_y = (fh as f32 * FOLDER_VIEWPORT.1) as i64;
    let v_w = (fw as f32 * FOLDER_VIEWPORT.2) as u32;
    let v_h = (fh as f32 * FOLDER_VIEWPORT.3) as u32;

    // 2. Resize file content to fit the viewport using high-quality Lanczos3
    let resized_content = imageops::resize(file_content, v_w, v_h, FilterType::Lanczos3);

    // 3. Create a blank canvas, put file in background, folder on top.
    let mut final_image = ImageBuffer::new(fw, fh);
    
    // Draw content into viewport
    imageops::overlay(&mut final_image, &resized_content, v_x, v_y);
    
    // Draw folder on top (the alpha mask of the folder icon handles the cropping)
    imageops::overlay(&mut final_image, folder_icon, 0, 0);

    final_image
}
