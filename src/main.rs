#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Hide console window on Windows in release

use eframe::egui::{ self, TextureHandle };
use eframe::{ egui::Slider };
use egui::{ Image, Style };
use image::GenericImageView;
use log::{ error, info, LevelFilter };
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rayon::prelude::*;
use std::sync::{ Arc, Mutex };
use std::path::PathBuf;
use rfd::FileDialog; // Import rfd for file dialogs

/// Your original pixel_sort function remains unchanged
fn pixel_sort(img_buf: &[u8], img_width: usize, img_height: usize, threshold: u8) -> Vec<u8> {
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

            for i in row * img_width..(row + 1) * img_width {
                if mask[i] {
                    segment.push([img_buf[i * 3], img_buf[i * 3 + 1], img_buf[i * 3 + 2]]);
                } else {
                    if !segment.is_empty() {
                        segment.sort_by(|a, b| a[2].cmp(&b[2]));

                        // Shuffle part of the sorted segment
                        let shuffle_start = ((segment.len() as f64) * 0.3).round() as usize;
                        let shuffle_end = ((segment.len() as f64) * 0.7).round() as usize;
                        if shuffle_start < shuffle_end && shuffle_end <= segment.len() {
                            segment[shuffle_start..shuffle_end].shuffle(&mut rng);
                        }

                        for pixel in segment.iter() {
                            rgba_row_buf.extend_from_slice(pixel);
                            rgba_row_buf.push(255); // Alpha channel
                        }
                        segment.clear();
                    }
                    rgba_row_buf.extend_from_slice(&img_buf[i * 3..i * 3 + 3]);
                    rgba_row_buf.push(255); // Alpha channel
                }
            }

            if !segment.is_empty() {
                segment.sort_by(|a, b| a[2].cmp(&b[2]));
                for pixel in segment.iter() {
                    rgba_row_buf.extend_from_slice(pixel);
                    rgba_row_buf.push(255); // Alpha channel
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

/// Struct representing the application state
struct MyApp {
    processed_image: Arc<Mutex<Vec<u8>>>,
    texture: Option<TextureHandle>,
    threshold: u8,
    img_width: usize,
    img_height: usize,
    original_img_buf: Vec<u8>,
    image_path: Option<PathBuf>, // Optional path to the current image
    is_processing: Arc<Mutex<bool>>,
}

impl Default for MyApp {
    fn default() -> Self {
        // Initialize logging
        env_logger::builder().filter_level(LevelFilter::Info).init();

        // Optionally, load a default image at startup
        let default_image_path = ""; // Change this to your default image path
        let (original_image, img_width, img_height, original_img_buf) = match
            image::open(default_image_path)
        {
            Ok(img) => {
                let width = img.width() as usize;
                let height = img.height() as usize;
                let buf = img.to_rgb8().into_raw();
                (img, width, height, buf)
            }
            Err(e) => {
                error!("Failed to open default image '{}': {}", default_image_path, e);
                // Create a blank image if the default image fails to load
                let width = 800;
                let height = 600;
                let buf = vec![0u8; width * height * 3]; // Black image
                (image::DynamicImage::new_rgb8(width as u32, height as u32), width, height, buf)
            }
        };

        // Initial threshold
        let threshold = 100;

        // Process the image
        let processed_image = pixel_sort(&original_img_buf, img_width, img_height, threshold);

        Self {
            processed_image: Arc::new(Mutex::new(processed_image)),
            texture: None,
            threshold,
            img_width,
            img_height,
            original_img_buf,
            image_path: Some(PathBuf::from(default_image_path)),
            is_processing: Arc::new(Mutex::new(false)),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle drag-and-drop events
        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            for file in &ctx.input(|i| i.raw.dropped_files.clone()) {
                if let Some(path) = &file.path {
                    if let Err(e) = self.load_image_from_path(path) {
                        error!("Failed to load image '{}': {}", path.display(), e);
                        // Optionally, show a popup or notification to the user
                    } else {
                        info!("Loaded image '{}'", path.display());
                    }
                }
            }
            // Clear the dropped files after processing
            ctx.input_mut(|i| i.raw.dropped_files.clone()).clear();
        }

        // Create the UI
        egui::CentralPanel::default().show(ctx, |ui| {
            // Set a custom style for the slider to make it wider
            let mut style = Style::default();
            style.spacing.slider_width = 400.0; // Adjust this value as needed
            ui.set_style(style);

            // Heading with the image name
            let image_name = if let Some(path) = &self.image_path {
                if path.to_string_lossy().is_empty() {
                    "No Image Loaded".to_owned()
                } else {
                    format!("{}", path.file_name().unwrap_or_default().to_string_lossy())
                }
            } else {
                "No Image Loaded".to_owned()
            };
            ui.heading(format!("Pixel Sorter - {}", image_name));

            ui.add_space(10.0);

            // Instructions for drag-and-drop
            if self.texture.is_none() {
                ui.horizontal_centered(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label("Drag and drop an image here");
                    });
                });
            }

            ui.add_space(10.0);

            // Slider for threshold
            let slider = ui.add(
                Slider::new(&mut self.threshold, 0..=255)
                    .text("Threshold")
                    .show_value(true)
            );

            if *self.is_processing.lock().unwrap() {
                ui.spinner();
            }

            ui.separator();

            // Check if the slider value has changed
            if slider.changed() {
                info!("Threshold changed to {}", self.threshold);
                self.process_image();
            }

            ui.add_space(10.0);

            // Load the texture if not already loaded or if it needs to be updated
            if self.texture.is_none() {
                if let Some(texture) = self.load_texture(ui) {
                    self.texture = Some(texture);
                }
            }

            // Display the image
            if let Some(texture) = &self.texture {
                ui.horizontal_centered(|ui| {
                    // Optionally, you can add scaling or other transformations here
                    ui.add(Image::new(texture).shrink_to_fit());
                });
            }

            ui.add_space(10.0);

            // Add the "Save Image" button
            if ui.button("Save Image").clicked() {
                self.save_image();
            }
        });

        // Request a repaint to ensure the UI updates smoothly
        ctx.request_repaint();
    }
}

impl MyApp {
    /// Load an image from the given path and process it
    fn load_image_from_path(&mut self, path: &PathBuf) -> Result<(), image::ImageError> {
        // Attempt to open the image
        let img = image::open(path)?;

        // Update image dimensions
        self.img_width = img.width() as usize;
        self.img_height = img.height() as usize;

        // Convert the image to RGB8 format
        let img_rgb = img.to_rgb8();
        self.original_img_buf = img_rgb.into_raw();

        // Process the image with the current threshold
        self.process_image();

        // Update the image path
        self.image_path = Some(path.clone());

        Ok(())
    }

    /// Process the image using the pixel_sort function
    fn process_image(&mut self) {
        // Set the processing flag
        {
            let mut processing = self.is_processing.lock().unwrap();
            *processing = true;
        }

        // Perform the pixel sort
        let new_image = pixel_sort(
            &self.original_img_buf,
            self.img_width,
            self.img_height,
            self.threshold
        );

        // Update the processed image buffer
        {
            let mut img = self.processed_image.lock().unwrap();
            *img = new_image;
        }

        // Reset the texture to force reload
        self.texture = None;

        // Unset the processing flag
        {
            let mut processing = self.is_processing.lock().unwrap();
            *processing = false;
        }
    }

    /// Load the texture from the processed image buffer
    fn load_texture(&self, ui: &mut egui::Ui) -> Option<TextureHandle> {
        // Lock the processed image buffer
        let img = self.processed_image.lock().unwrap();

        // Convert the processed image to a color image
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [self.img_width, self.img_height],
            &img
        );

        // Allocate a texture
        Some(ui.ctx().load_texture("processed_image", color_image, egui::TextureOptions::LINEAR))
    }

    /// Save the processed image to a file chosen by the user
    fn save_image(&self) {
        // Open a save file dialog
        if
            let Some(path) = FileDialog::new()
                .add_filter("PNG Image", &["png"])
                .add_filter("JPEG Image", &["jpg", "jpeg"])
                .set_file_name("processed_image.png")
                .save_file()
        {
            // Lock the processed image buffer
            let img = self.processed_image.lock().unwrap();

            // Create an ImageBuffer from the raw RGBA data
            let buffer: image::ImageBuffer<image::Rgba<u8>, _> = match
                image::ImageBuffer::from_raw(
                    self.img_width as u32,
                    self.img_height as u32,
                    img.clone()
                )
            {
                Some(b) => b,
                None => {
                    error!("Failed to create ImageBuffer from processed image data");
                    return;
                }
            };

            // Determine the image format based on the file extension
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase();
            let result = match extension.as_str() {
                "png" => buffer.save_with_format(&path, image::ImageFormat::Png),
                "jpg" | "jpeg" => buffer.save_with_format(&path, image::ImageFormat::Jpeg), // Quality set to 80
                _ => {
                    error!("Unsupported file extension: {}", extension);
                    return;
                }
            };

            // Handle the result of saving
            match result {
                Ok(_) => info!("Image saved successfully to {}", path.display()),
                Err(e) => error!("Failed to save image to '{}': {}", path.display(), e),
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    // Configure eframe
    let options = eframe::NativeOptions {
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "Pixel Sorter",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default()))
    )
}
