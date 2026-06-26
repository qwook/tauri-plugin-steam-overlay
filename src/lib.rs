use std::sync::Arc;
use std::sync::Mutex;
use tauri::plugin::TauriPlugin;
use tauri::Window;
use tauri::{async_runtime, LogicalPosition, LogicalSize, Manager, RunEvent, WindowEvent, Wry};

const SHADER: &str = r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var out: VertexOut;

    // Degenerate triangle outside clip space
    out.position = vec4<f32>(2.0, 2.0, 0.0, 1.0);

    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
"#;

pub struct WgpuState<'win> {
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub surface: wgpu::Surface<'win>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub config: Mutex<wgpu::SurfaceConfiguration>,
}

impl<'win> WgpuState<'win> {
    pub async fn new(window: Window) -> Self {
        let size = window.inner_size().unwrap();
        let instance: wgpu::Instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);

        let format = caps.formats[0];

        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|&m| m == wgpu::CompositeAlphaMode::PostMultiplied)
            .or_else(|| {
                caps.alpha_modes
                    .iter()
                    .copied()
                    .find(|&m| m == wgpu::CompositeAlphaMode::PreMultiplied)
            })
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),

            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),

            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
        });

        Self {
            device,
            queue,
            surface,
            // vertex_buffer,
            render_pipeline,
            config: Mutex::new(config),
        }
    }
}

pub fn create_overlay_window(
    app: &tauri::AppHandle<Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let _window = handle.get_window("main").unwrap();

        let overlay = tauri::WindowBuilder::new(&handle, "overlay")
            .parent(&_window)
            .unwrap()
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .closable(false)
            .decorations(false)
            .shadow(false)
            .background_color(tauri::window::Color(255, 255, 255, 0))
            .inner_size(800.0, 600.0)
            .always_on_top(true)
            .build()
            .unwrap();
        overlay.set_ignore_cursor_events(true).unwrap();

        #[cfg(target_os = "macos")]
        {
            // Don't know why, but this helps offset the overlay.
            _window
                .set_title_bar_style(tauri::TitleBarStyle::Transparent)
                .unwrap();
        }
    });

    Ok(())
}

pub fn init() -> TauriPlugin<Wry> {
    tauri::plugin::Builder::<Wry>::new("steam-overlay")
        .on_window_ready(|window| {
            if window.label() == "main" {
                // TODO: Maybe just pass window to create_overlay_window?
                let _ = create_overlay_window(window.app_handle());
            } else if window.label() == "overlay" {
                let _window = window.clone();
                let wgpu_state = async_runtime::block_on(WgpuState::new(_window));
                let wgpu_state = Arc::new(wgpu_state); // Make wgpu_state Arc<T>
                window.app_handle().manage(wgpu_state.clone()); // Store a clone in app state
            }
        })
        .on_event(|app_handle, event| match event {
            RunEvent::WindowEvent {
                label: _,
                event: WindowEvent::Resized(size),
                ..
            } => {
                if size.width > 0 && size.height > 0 {
                    // Resize the GPU drawing surface.
                    if let Some(wgpu_state) = app_handle.try_state::<Arc<WgpuState>>() {
                        let mut config = wgpu_state.config.lock().unwrap();
                        config.width = size.width;
                        config.height = size.height;
                        wgpu_state.surface.configure(&wgpu_state.device, &config);
                    }
                }
            }
            // Close overlay on main window close.
            RunEvent::WindowEvent {
                label: _,
                event: WindowEvent::Destroyed,
                ..
            } => {
                if app_handle.get_webview_window("main").is_none() {
                    if let Some(overlay_window) = app_handle.get_window("overlay") {
                        overlay_window.close().unwrap();
                    }
                }
            }
            // Redraw
            // Need to check if drawing is necessary or if just having a surface is good enough.
            RunEvent::MainEventsCleared => {
                if let (Some(wgpu_state), Some(main_window), Some(overlay_window)) = (
                    app_handle.try_state::<Arc<WgpuState>>(),
                    app_handle.get_webview_window("main"),
                    app_handle.get_window("overlay"),
                ) {
                    if let Ok(Some(monitor)) = main_window.current_monitor() {
                        let scale_factor = monitor.scale_factor();

                        let window_position: LogicalPosition<f64> = main_window
                            .inner_position()
                            .unwrap()
                            .to_logical(scale_factor);
                        let window_size: LogicalSize<f64> =
                            main_window.inner_size().unwrap().to_logical(scale_factor);

                        overlay_window.set_position(window_position).unwrap();
                        overlay_window.set_size(window_size).unwrap();

                        let frame = wgpu_state.surface.get_current_texture().unwrap();

                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        let mut encoder = wgpu_state.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("encoder"),
                            },
                        );

                        {
                            let mut render_pass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("render pass"),

                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,

                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                                r: 0.0,
                                                g: 0.0,
                                                b: 0.0,
                                                a: 0.0, // 50% opacity
                                            }),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    })],

                                    depth_stencil_attachment: None,
                                    occlusion_query_set: None,
                                    timestamp_writes: None,
                                });

                            render_pass.set_pipeline(&wgpu_state.render_pipeline);
                            render_pass.draw(0..3, 0..1);
                        }

                        wgpu_state.queue.submit(Some(encoder.finish()));

                        frame.present();
                    }
                }
            }
            _ => {}
        })
        .build()
}
