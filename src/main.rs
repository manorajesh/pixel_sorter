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
use std::thread;

use pollster::block_on;

// Include the GPU module
mod gpu;
use gpu::GPUDevice;

// Define Pixel and Params structs
use bytemuck::{ Pod, Zeroable };

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Params {
    img_width: u32,
    img_height: u32,
    threshold: u32,
}

/// Struct representing the application state
struct MyApp {
    gpu: GPUDevice,
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

        // Initialize GPU
        let gpu = GPUDevice::new();

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

        // Initialize processed_image and processing flag
        let processed_image = Arc::new(Mutex::new(Vec::new()));
        let is_processing = Arc::new(Mutex::new(false));

        Self {
            gpu,
            processed_image,
            texture: None,
            threshold,
            img_width,
            img_height,
            original_img_buf,
            image_path: if default_image_path.is_empty() {
                None
            } else {
                Some(PathBuf::from(default_image_path))
            },
            is_processing,
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
                        // Start GPU processing for the new image
                        self.process_image_async();
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
            if self.texture.is_none() && !*self.is_processing.lock().unwrap() {
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
            if slider.changed() && !*self.is_processing.lock().unwrap() {
                info!("Threshold changed to {}", self.threshold);
                self.process_image_async();
            }

            ui.add_space(10.0);

            // Load the texture if not already loaded or if it needs to be updated
            if self.texture.is_none() && !*self.is_processing.lock().unwrap() {
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
    /// Load an image from the given path and update the application state
    fn load_image_from_path(&mut self, path: &PathBuf) -> Result<(), image::ImageError> {
        // Attempt to open the image
        let img = image::open(path)?;

        // Update image dimensions
        self.img_width = img.width() as usize;
        self.img_height = img.height() as usize;

        // Convert the image to RGB8 format
        let img_rgb = img.to_rgb8();
        self.original_img_buf = img_rgb.into_raw();

        // Update the image path
        self.image_path = Some(path.clone());

        // Reset the texture to force reload
        self.texture = None;

        Ok(())
    }

    fn create_gpu_buffers(
        &self,
        device: &wgpu::Device,
        img_buf: &[u8],
        img_width: usize,
        img_height: usize,
        threshold: u8
    ) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer) {
        // Convert input buffer to Pixel array
        let input_pixels: Vec<Pixel> = img_buf
            .chunks_exact(3)
            .map(|chunk| Pixel {
                r: chunk[0],
                g: chunk[1],
                b: chunk[2],
                a: 255, // Initialize alpha channel
            })
            .collect();

        let input_buffer = device.create_buffer(
            &(wgpu::BufferDescriptor {
                label: Some("Input Image Buffer"),
                size: (img_width *
                    img_height *
                    std::mem::size_of::<Pixel>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        );

        let output_buffer = device.create_buffer(
            &(wgpu::BufferDescriptor {
                label: Some("Output Image Buffer"),
                size: (img_width *
                    img_height *
                    std::mem::size_of::<Pixel>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        );

        // Create uniform buffer
        let params = Params {
            img_width: img_width as u32,
            img_height: img_height as u32,
            threshold: threshold as u32,
        };

        let uniform_buffer = device.create_buffer(
            &(wgpu::BufferDescriptor {
                label: Some("Uniform Buffer"),
                size: std::mem::size_of::<Params>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        );

        (input_buffer, output_buffer, uniform_buffer)
    }

    /// Process the image using GPU-based pixel sorting asynchronously
    fn process_image_async(&mut self) {
        // Prevent multiple simultaneous processing tasks
        let already_processing = {
            let processing = self.is_processing.lock().unwrap();
            *processing
        };

        if already_processing {
            info!("Already processing an image. Please wait.");
            return;
        }

        let processed_image = Arc::clone(&self.processed_image);
        let is_processing = Arc::clone(&self.is_processing);
        let gpu = self.gpu.device;
        let queue = self.gpu.queue;
        let original_img_buf = self.original_img_buf.clone();
        let img_width = self.img_width;
        let img_height = self.img_height;
        let threshold = self.threshold;

        // Set the processing flag
        {
            let mut processing = is_processing.lock().unwrap();
            *processing = true;
        }

        // Spawn a new thread for GPU processing
        thread::spawn(move || {
            // Create buffers
            let (input_buffer, output_buffer, uniform_buffer) = self.create_gpu_buffers(
                &gpu,
                &original_img_buf,
                img_width,
                img_height,
                threshold
            );

            // Load shader
            let shader = {
                let shader_source = include_str!("shaders/pixel_sort.wgsl");
                gpu.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Pixel Sort Shader"),
                    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
                })
            };

            // Create compute pipeline
            let pipeline = gpu.create_compute_pipeline(
                &(wgpu::ComputePipelineDescriptor {
                    label: Some("Pixel Sort Compute Pipeline"),
                    layout: None, // Let wgpu auto-create the pipeline layout
                    module: &shader,
                    entry_point: "main",
                })
            );

            // Create bind group
            let bind_group = gpu.create_bind_group(
                &(wgpu::BindGroupDescriptor {
                    label: Some("Pixel Sort Bind Group"),
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: input_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: output_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform_buffer.as_entire_binding(),
                        },
                    ],
                })
            );

            // Dispatch the compute shader
            let mut encoder = gpu.create_command_encoder(
                &(wgpu::CommandEncoderDescriptor {
                    label: Some("Compute Command Encoder"),
                })
            );

            {
                let mut compute_pass = encoder.begin_compute_pass(
                    &(wgpu::ComputePassDescriptor {
                        label: Some("Pixel Sort Compute Pass"),
                    })
                );
                compute_pass.set_pipeline(&pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);

                // Dispatch one workgroup per row
                compute_pass.dispatch_workgroups(img_height as u32, 1, 1);
            }

            // Submit the command buffer
            queue.submit(Some(encoder.finish()));

            // Read back the data
            let buffer_size = img_width * img_height * std::mem::size_of::<Pixel>();
            let output_data = block_on(
                MyApp::read_output_buffer_static(&gpu, &queue, &output_buffer, buffer_size)
            );

            // Update the processed_image buffer
            {
                let mut img = processed_image.lock().unwrap();
                *img = output_data;
            }

            // Unset the processing flag
            {
                let mut processing = is_processing.lock().unwrap();
                *processing = false;
            }

            info!("GPU processing completed.");
        });
    }

    /// Static version to be called within a thread
    async fn read_output_buffer_static(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &wgpu::Buffer,
        size: usize
    ) -> Vec<u8> {
        // Create a buffer to copy the data to
        let read_buffer = device.create_buffer(
            &(wgpu::BufferDescriptor {
                label: Some("Read Buffer"),
                size: size as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        );

        // Create a command encoder
        let mut encoder = device.create_command_encoder(
            &(wgpu::CommandEncoderDescriptor {
                label: Some("Read Buffer Encoder"),
            })
        );

        // Copy the data from the output buffer to the read buffer
        encoder.copy_buffer_to_buffer(buffer, 0, &read_buffer, 0, size as wgpu::BufferAddress);

        // Submit the copy command
        queue.submit(Some(encoder.finish()));

        // Wait for the GPU to finish
        let buffer_slice = read_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |i| { i.unwrap() });
        device.poll(wgpu::Maintain::Wait);

        // Read the data
        let data = buffer_slice.get_mapped_range().to_vec();
        read_buffer.unmap();

        data
    }

    /// Load the texture from the processed image buffer
    fn load_texture(&self, ui: &mut egui::Ui) -> Option<TextureHandle> {
        // Lock the processed image buffer
        let img = self.processed_image.lock().unwrap();

        if img.is_empty() {
            return None;
        }

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

            if img.is_empty() {
                error!("No processed image data to save.");
                return;
            }

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
