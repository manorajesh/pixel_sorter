use log::LevelFilter;
// use image::{ImageBuffer, Rgb};
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use rayon::prelude::*;
use rand::prelude::SliceRandom;
use rand::thread_rng;

fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let event_loop = EventLoop::new();
    let img = image::open("birds.png").unwrap().to_rgb8();
    let img_width = img.width();
    let img_height = img.height();
    let mut img_buf = img.into_raw();
    let mut threshold = 100u8;
    let mut rgba_img_buf = pixel_sort(&mut img_buf, img_width as usize, img_height as usize, threshold);

    // Rest of the code
    let size = LogicalSize::new(img_width as f64, img_height as f64);
    let window = WindowBuilder::new()
        .with_title("Image Processing")
        .with_inner_size(size)
        .build(&event_loop)
        .unwrap();

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(img_width, img_height, surface_texture).unwrap()
    };

    // Initialize ControlFlow as Wait
    let mut control_flow = ControlFlow::Wait;

    event_loop.run(move |event, _, control_flow_ptr| {
        *control_flow_ptr = control_flow;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow_ptr = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                if let Some(key_code) = input.virtual_keycode {
                    match key_code {
                        VirtualKeyCode::Left => {
                            log::info!("Reducing threshold");
                            threshold = threshold.saturating_sub(1);
                            rgba_img_buf = pixel_sort(&mut img_buf.clone(), img_width as usize, img_height as usize, threshold);
                            log::info!("Threshold: {}", threshold);
                            window.request_redraw();
                        }
                        VirtualKeyCode::Right => {
                            log::info!("Increasing threshold");
                            threshold = threshold.saturating_add(1);
                            rgba_img_buf = pixel_sort(&mut img_buf.clone(), img_width as usize, img_height as usize, threshold);
                            log::info!("Threshold: {}", threshold);
                            window.request_redraw();
                        }
                        _ => (),
                    }

                    // Request redraw
                    window.request_redraw();
                    
                    // Change control flow to Poll to force immediate redraw
                    control_flow = ControlFlow::Poll;
                }
            }
            Event::RedrawRequested(_) => {
                // Redraw the frame
                let frame = pixels.frame_mut();
                frame.copy_from_slice(&rgba_img_buf);
                if pixels.render().is_err() {
                    *control_flow_ptr = ControlFlow::Exit;
                    return;
                }

                // After drawing, revert control_flow to Wait
                control_flow = ControlFlow::Wait;
            }
            _ => (),
        }
    });

}

fn pixel_sort(img_buf: &mut Vec<u8>, img_width: usize, img_height: usize, threshold: u8) -> Vec<u8> {
    // Create a mask based on blue channel threshold
    let mask: Vec<bool> = img_buf
        .chunks_exact(3)
        .map(|pixel| pixel[2] > threshold)
        .collect();

    let mut rgba_img_buf = Vec::with_capacity(img_width * img_height * 4);
    let mut rng = thread_rng();  // Create a random number generator

    for row in 0..img_height {
        let mut segment = Vec::new();

        for i in (row * img_width)..((row + 1) * img_width) {
            if mask[i] {
                segment.push([img_buf[i * 3], img_buf[i * 3 + 1], img_buf[i * 3 + 2]]);
            } else {
                if !segment.is_empty() {
                    segment.sort_by(|a, b| a[2].cmp(&b[2]));

                    // Shuffle part of the sorted segment for randomness
                    let shuffle_start = (segment.len() as f64 * 0.3).round() as usize;
                    let shuffle_end = (segment.len() as f64 * 0.7).round() as usize;
                    segment[shuffle_start..shuffle_end].shuffle(&mut rng);

                    for pixel in segment.iter() {
                        rgba_img_buf.extend_from_slice(pixel);
                        rgba_img_buf.push(255); // Alpha channel
                    }
                    segment.clear();
                }
                rgba_img_buf.extend_from_slice(&img_buf[i * 3..(i * 3 + 3)]);
                rgba_img_buf.push(255); // Alpha channel
            }
        }

        if !segment.is_empty() {
            segment.sort_by(|a, b| a[2].cmp(&b[2]));
            for pixel in segment.iter() {
                rgba_img_buf.extend_from_slice(pixel);
                rgba_img_buf.push(255); // Alpha channel
            }
        }
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