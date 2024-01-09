use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ ControlFlow, EventLoop },
    window::{ WindowBuilder, Window, CursorGrabMode },
    dpi::PhysicalSize,
};
use bytemuck;
use log::info;

const XMIN: f32 = -2.5;
const XMAX: f32 = 1.0;
const YMIN: f32 = -1.0;
const YMAX: f32 = 1.0;

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    _num_vertices: u32,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    uniforms: Uniforms,
    uniforms_buffer: wgpu::Buffer,
    uniforms_bind_group: wgpu::BindGroup,
    window_cursor_config: WindowCursorConfig,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(window: Window) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = (unsafe { instance.create_surface(&window) }).unwrap();

        let adapter = instance
            .request_adapter(
                &(wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
            ).await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &(wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: wgpu::Limits::default(),
                    label: None,
                }),
                None // Trace path
            ).await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps.formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // Creating vertex buffer
        let vertex_buffer = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            })
        );
        let num_vertices = VERTICES.len() as u32;

        // Create index buffer for better memory use
        let index_buffer = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            })
        );
        let num_indices = INDICES.len() as u32;

        // Create Uniforms buffer
        let uniforms = Uniforms::new(window.inner_size());
        let uniforms_buffer = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("Uniforms Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        );

        let uniforms_bind_group_layout = device.create_bind_group_layout(
            &(wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("uniforms_bind_group_layout"),
            })
        );

        let uniforms_bind_group = device.create_bind_group(
            &(wgpu::BindGroupDescriptor {
                layout: &uniforms_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniforms_buffer.as_entire_binding(),
                    },
                ],
                label: Some("uniforms_bind_group"),
            })
        );

        // Configuring render pipeline with shader code
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let render_pipeline_layout = device.create_pipeline_layout(
            &(wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniforms_bind_group_layout],
                push_constant_ranges: &[],
            })
        );
        let render_pipeline = device.create_render_pipeline(
            &(wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: config.format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires Features::DEPTH_CLIP_CONTROL
                    unclipped_depth: false,
                    // Requires Features::CONSERVATIVE_RASTERIZATION
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            })
        );

        let window_cursor_config = WindowCursorConfig {
            cursor_visible: false,
        };

        // window
        //     .set_cursor_grab(CursorGrabMode::Confined)
        //     .or_else(|_e| window.set_cursor_grab(CursorGrabMode::Locked))
        //     .unwrap();
        // window.set_cursor_visible(false);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            _num_vertices: num_vertices,
            index_buffer,
            num_indices,
            uniforms,
            uniforms_buffer,
            uniforms_bind_group,
            window_cursor_config,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.uniforms.resize(new_size);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Space),
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => {
                self.window.set_cursor_visible(self.window_cursor_config.cursor_visible.toggle());
                match self.window_cursor_config.cursor_visible {
                    true => self.window.set_cursor_grab(CursorGrabMode::None).unwrap(),
                    false => self.window.set_cursor_grab(CursorGrabMode::Confined).unwrap(),
                }
                info!(
                    "Space pressed: cursor_visible: {}",
                    self.window_cursor_config.cursor_visible
                );

                true
            }

            _ => self.uniforms.process_events(event),
        }
    }

    fn update(&mut self) {
        self.queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::cast_slice(&[self.uniforms]));
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(
            &(wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            })
        );

        {
            let mut render_pass = encoder.begin_render_pass(
                &(wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 0.0,
                                }),
                                store: true,
                            },
                        }),
                    ],
                    depth_stencil_attachment: None,
                })
            );

            render_pass.set_pipeline(&self.render_pipeline);

            render_pass.set_bind_group(0, &self.uniforms_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex { position: [-1.0, -1.0, 0.0], color: [0.5, 0.0, 0.5] }, // Bottom-left
    Vertex { position: [1.0, -1.0, 0.0], color: [0.5, 0.0, 0.5] }, // Bottom-right
    Vertex { position: [1.0, 1.0, 0.0], color: [0.5, 0.0, 0.5] }, // Top-right
    Vertex { position: [-1.0, 1.0, 0.0], color: [0.5, 0.0, 0.5] }, // Top-left
];

const INDICES: &[u16] = &[
    0,
    1,
    2, // First triangle (bottom-left, bottom-right, top-right)
    2,
    3,
    0, // Second triangle (top-right, top-left, bottom-left)
];

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    image: [f32; 2],
}

impl Uniforms {
    fn new(inner_size: PhysicalSize<u32>) -> Self {
        Self {
            width: inner_size.width as f32,
            height: inner_size.height as f32,
            zoom: 1.0,
            center_x: 0.0,
            center_y: 0.0,
            max_iterations: 200,
        }
    }

    fn process_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        self.zoom *= 1.0 + y * 0.1;

                        true
                    }
                    _ => false,
                }
            }

            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    virtual_keycode: Some(key),
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } =>
                match key {
                    VirtualKeyCode::Up => {
                        self.max_iterations *= 2;
                        info!("max_iterations: {}", self.max_iterations);

                        true
                    }

                    VirtualKeyCode::Down => {
                        if self.max_iterations > 2 {
                            self.max_iterations /= 2;
                        } else {
                            self.max_iterations = 1;
                        }
                        info!("max_iterations: {}", self.max_iterations);

                        true
                    }

                    _ => false,
                }

            _ => false,
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.width = new_size.width as f32;
        self.height = new_size.height as f32;
    }
}

struct WindowCursorConfig {
    cursor_visible: bool,
}

trait Toggle {
    fn toggle(&mut self) -> Self;
}

impl Toggle for bool {
    fn toggle(&mut self) -> Self {
        *self = !*self;
        *self
    }
}

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = State::new(window).await;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(window_id) if window_id == state.window().id() => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        *control_flow = ControlFlow::Exit;
                    }
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                state.window().request_redraw();
            }

            Event::WindowEvent { ref event, window_id } if window_id == state.window().id() => if
                !state.input(event)
            {
                // UPDATED!
                match event {
                    | WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                          input: KeyboardInput {
                              state: ElementState::Pressed,
                              virtual_keycode: Some(VirtualKeyCode::Escape),
                              ..
                          },
                          ..
                      } => {
                        *control_flow = ControlFlow::Exit;
                    }

                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }

                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size);
                    }

                    _ => {}
                }
            }

            // Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
            //     if !state.uniforms.center_x.is_normal() || !state.uniforms.center_y.is_normal() {
            //         state.uniforms.center_x = (XMAX + XMIN) / 2.0;
            //         state.uniforms.center_y = (YMAX + YMIN) / 2.0;
            //     }

            //     if state.window_cursor_config.cursor_visible {
            //         return;
            //     }

            //     state.uniforms.center_x +=
            //         (((delta.0 as f32) / state.uniforms.width) * (XMAX - XMIN)) /
            //         state.uniforms.zoom;
            //     state.uniforms.center_y +=
            //         (((delta.1 as f32) / state.uniforms.height) * (YMAX - YMIN)) /
            //         state.uniforms.zoom;
            // }

            _ => {}
        }
    });
}
