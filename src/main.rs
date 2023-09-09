use log::LevelFilter;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rayon::prelude::*;
use show_image::{create_window, event, ImageInfo, ImageView};

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(LevelFilter::Warn).init();

    let path = "images/break.jpg";
    let img = image::open(path).unwrap().to_rgb8();
    let img_width = img.width();
    let img_height = img.height();
    let mut img_buf = img.into_raw();
    let mut threshold = 100;
    let mut rgba_img_buf = pixel_sort(
        &mut img_buf,
        img_width as usize,
        img_height as usize,
        threshold,
    );

    let image = ImageView::new(ImageInfo::rgb8(img_width, img_height), &rgba_img_buf);

    // Create a window with default options and display the image.
    let window = create_window(path, Default::default())?;
    window.set_image("image-001", image)?;

    for event in window.event_channel()? {
        if let event::WindowEvent::KeyboardInput(event) = event {
            if event.input.state.is_pressed() {
                match event.input.key_code {
                    Some(event::VirtualKeyCode::Escape) => break,
                    Some(event::VirtualKeyCode::Left) => {
                        log::warn!("Reducing threshold");

                        threshold = threshold.saturating_sub(1);
                        rgba_img_buf = pixel_sort(
                            &mut img_buf.clone(),
                            img_width as usize,
                            img_height as usize,
                            threshold,
                        );

                        window.set_image(
                            "image-001",
                            ImageView::new(ImageInfo::rgb8(img_width, img_height), &rgba_img_buf),
                        )?;

                        log::warn!("Threshold: {}", threshold);
                    }
                    Some(event::VirtualKeyCode::Right) => {
                        log::warn!("Increasing threshold");

                        threshold = threshold.saturating_add(1);
                        rgba_img_buf = pixel_sort(
                            &mut img_buf.clone(),
                            img_width as usize,
                            img_height as usize,
                            threshold,
                        );

                        window.set_image(
                            "image-001",
                            ImageView::new(ImageInfo::rgb8(img_width, img_height), &rgba_img_buf),
                        )?;

                        log::warn!("Threshold: {}", threshold);
                    }
                    _ => (),
                }
            }
        }
    }

    Ok(())
}

fn pixel_sort(
    img_buf: &mut Vec<u8>,
    img_width: usize,
    img_height: usize,
    threshold: u8,
) -> Vec<u8> {
    let mask: Vec<bool> = img_buf
        .chunks_exact(3)
        .map(|pixel| pixel[0] > threshold)
        .collect();

    // Use rayon's par_iter to parallelize row processing.
    let rows: Vec<Vec<u8>> = (0..img_height)
        .into_par_iter()
        .map(|row| {
            let mut rng = thread_rng(); // Create a random number generator
            let mut rgba_row_buf = Vec::new();
            let mut segment = Vec::new();

            for i in (row * img_width)..((row + 1) * img_width) {
                if mask[i] {
                    segment.push([img_buf[i * 3], img_buf[i * 3 + 1], img_buf[i * 3 + 2]]);
                } else {
                    if !segment.is_empty() {
                        segment.sort_by(|b, a| a[2].cmp(&b[2]));

                        // Shuffle part of the sorted segment
                        let shuffle_start = (segment.len() as f64 * 0.3).round() as usize;
                        let shuffle_end = (segment.len() as f64 * 0.7).round() as usize;
                        segment[shuffle_start..shuffle_end].shuffle(&mut rng);

                        for pixel in segment.iter() {
                            rgba_row_buf.extend_from_slice(pixel);
                            // rgba_row_buf.push(255); // Alpha channel
                        }
                        segment.clear();
                    }
                    rgba_row_buf.extend_from_slice(&img_buf[i * 3..(i * 3 + 3)]);
                    // rgba_row_buf.push(255); // Alpha channel
                }
            }

            if !segment.is_empty() {
                segment.sort_by(|b, a| a[2].cmp(&b[2]));
                for pixel in segment.iter() {
                    rgba_row_buf.extend_from_slice(pixel);
                    // rgba_row_buf.push(255); // Alpha channel
                }
            }
            rgba_row_buf
        })
        .collect();

    // Concatenate all the rows to form the complete image.
    let mut rgba_img_buf = Vec::with_capacity(img_width * img_height * 4);
    for row in rows {
        rgba_img_buf.extend(row);
    }

    rgba_img_buf
}

trait GetLuma {
    fn get_luma(&self) -> u8;
}

impl GetLuma for &[u8] {
    fn get_luma(&self) -> u8 {
        let [r, g, b] = self else { panic!("Pixel is not RGB") };
        (0.2126 * *r as f32 + 0.7152 * *g as f32 + 0.0722 * *b as f32) as u8
    }
}
