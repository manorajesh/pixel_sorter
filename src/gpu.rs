// gpu.rs
use wgpu::util::DeviceExt;
use pollster::block_on;

pub struct GPUDevice {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GPUDevice {
    pub fn new() -> Self {
        block_on(Self::initialize())
    }

    async fn initialize() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(
                &(wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None, // No window surface needed for compute
                    force_fallback_adapter: false,
                })
            ).await
            .expect("Failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(
                &(wgpu::DeviceDescriptor {
                    label: Some("Compute Device"),
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                }),
                None
            ).await
            .expect("Failed to create device");

        Self { device, queue }
    }
}
