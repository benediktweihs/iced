mod controls;
mod scene;

use controls::Controls;
use scene::Scene;

use iced_runtime::Action;
use iced_wgpu::graphics::Viewport;
use iced_wgpu::{wgpu, Engine, Renderer};
use iced_winit::{conversion, Proxy};
use iced_winit::core::mouse;
use iced_winit::core::renderer;
use iced_winit::core::{Color, Font, Pixels, Size, Theme};
use iced_winit::futures;
use iced_winit::runtime::program;
use iced_winit::runtime::Debug;
use iced_winit::winit;
use iced_winit::Clipboard;

use winit::{
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    keyboard::ModifiersState,
};

use std::sync::Arc;
use iced_runtime::task::into_stream;
use iced_winit::winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use crate::controls::Message;

pub fn main() -> Result<(), winit::error::EventLoopError> {
    tracing_subscriber::fmt::init();

    // Initialize winit
    let event_loop = EventLoop::<Action<Message>>::with_user_event()
        .build()
        .unwrap();
    let proxy: EventLoopProxy<Action<Message>> = event_loop.create_proxy();

    #[allow(clippy::large_enum_variant)]
    enum Runner {
        Loading(EventLoopProxy<Action<Message>>),
        Ready {
            window: Arc<winit::window::Window>,
            device: wgpu::Device,
            queue: wgpu::Queue,
            surface: wgpu::Surface<'static>,
            format: wgpu::TextureFormat,
            engine: Engine,
            renderer: Renderer,
            scene: Scene,
            state: program::State<Controls>,
            cursor_position: Option<winit::dpi::PhysicalPosition<f64>>,
            clipboard: Clipboard,
            runtime: iced_futures::Runtime<
                iced_futures::backend::native::tokio::Executor,
                Proxy<Message>,
                Action<Message>,
            >,
            viewport: Viewport,
            modifiers: ModifiersState,
            resized: bool,
            debug: Debug,
        },
    }

    impl winit::application::ApplicationHandler<Action<Message>> for Runner {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if let Self::Loading(proxy) = self {
                let window = Arc::new(
                    event_loop
                        .create_window(
                            winit::window::WindowAttributes::default(),
                        )
                        .expect("Create window"),
                );

                let physical_size = window.inner_size();
                let viewport = Viewport::with_physical_size(
                    Size::new(physical_size.width, physical_size.height),
                    window.scale_factor(),
                );
                let clipboard = Clipboard::connect(window.clone());

                let backend =
                    wgpu::util::backend_bits_from_env().unwrap_or_default();

                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: backend,
                    ..Default::default()
                });
                let surface = instance
                    .create_surface(window.clone())
                    .expect("Create window surface");


                let (format, adapter, device, queue) =
                    futures::futures::executor::block_on(async {
                        let adapter =
                            wgpu::util::initialize_adapter_from_env_or_default(
                                &instance,
                                Some(&surface),
                            )
                            .await
                            .expect("Create adapter");

                        let adapter_features = adapter.features();

                        let capabilities = surface.get_capabilities(&adapter);

                        let (device, queue) = adapter
                            .request_device(
                                &wgpu::DeviceDescriptor {
                                    label: None,
                                    required_features: adapter_features
                                        & wgpu::Features::default(),
                                    required_limits: wgpu::Limits::default(),
                                    memory_hints:
                                        wgpu::MemoryHints::MemoryUsage,
                                },
                                None,
                            )
                            .await
                            .expect("Request device");

                        (
                            capabilities
                                .formats
                                .iter()
                                .copied()
                                .find(wgpu::TextureFormat::is_srgb)
                                .or_else(|| {
                                    capabilities.formats.first().copied()
                                })
                                .expect("Get preferred format"),
                            adapter,
                            device,
                            queue,
                        )
                    });

                surface.configure(
                    &device,
                    &wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format,
                        width: physical_size.width,
                        height: physical_size.height,
                        present_mode: wgpu::PresentMode::AutoVsync,
                        alpha_mode: wgpu::CompositeAlphaMode::Auto,
                        view_formats: vec![],
                        desired_maximum_frame_latency: 2,
                    },
                );

                // Initialize scene and GUI controls
                let scene = Scene::new(&device, format);
                let controls = Controls::new();

                // Initialize iced
                let mut debug = Debug::new();
                let engine =
                    Engine::new(&adapter, &device, &queue, format, None);
                let mut renderer = Renderer::new(
                    &device,
                    &engine,
                    Font::default(),
                    Pixels::from(16),
                );

                let state = program::State::new(
                    controls,
                    viewport.logical_size(),
                    &mut renderer,
                    &mut debug,
                );

                // You should change this if you want to render continuously
                event_loop.set_control_flow(ControlFlow::Wait);

                let (p, worker) = iced_winit::Proxy::new(proxy.clone());
                let Ok(executor) = iced_futures::backend::native::tokio::Executor::new() else {
                    panic!("could not create runtime")
                };

                executor.spawn(worker);
                let mut runtime = iced_futures::Runtime::new(executor, p);

                *self = Self::Ready {
                    window,
                    device,
                    queue,
                    surface,
                    format,
                    engine,
                    renderer,
                    scene,
                    state,
                    cursor_position: None,
                    modifiers: ModifiersState::default(),
                    clipboard,
                    runtime,
                    viewport,
                    resized: false,
                    debug,
                };
            }
        }

        fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            let Self::Ready {
                window,
                device,
                queue,
                surface,
                format,
                engine,
                renderer,
                scene,
                state,
                runtime,
                viewport,
                cursor_position,
                modifiers,
                clipboard,
                resized,
                debug,
            } = self
            else {
                return;
            };

            match event {
                WindowEvent::RedrawRequested => {
                    if *resized {
                        let size = window.inner_size();

                        *viewport = Viewport::with_physical_size(
                            Size::new(size.width, size.height),
                            window.scale_factor(),
                        );

                        surface.configure(
                            device,
                            &wgpu::SurfaceConfiguration {
                                format: *format,
                                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                                width: size.width,
                                height: size.height,
                                present_mode: wgpu::PresentMode::AutoVsync,
                                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                                view_formats: vec![],
                                desired_maximum_frame_latency: 2,
                            },
                        );

                        *resized = false;
                    }

                    match surface.get_current_texture() {
                        Ok(frame) => {
                            let mut encoder = device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor { label: None },
                            );

                            let program = state.program();

                            let view = frame.texture.create_view(
                                &wgpu::TextureViewDescriptor::default(),
                            );

                            {
                                // We clear the frame
                                let mut render_pass = Scene::clear(
                                    &view,
                                    &mut encoder,
                                    program.background_color(),
                                );

                                // Draw the scene
                                scene.draw(&mut render_pass);
                            }

                            // And then iced on top
                            renderer.present(
                                engine,
                                device,
                                queue,
                                &mut encoder,
                                None,
                                frame.texture.format(),
                                &view,
                                viewport,
                                &debug.overlay(),
                            );

                            // Then we submit the work
                            engine.submit(queue, encoder);
                            frame.present();

                            // Update the mouse cursor
                            window.set_cursor(
                                iced_winit::conversion::mouse_interaction(
                                    state.mouse_interaction(),
                                ),
                            );
                        }
                        Err(error) => match error {
                            wgpu::SurfaceError::OutOfMemory => {
                                panic!(
                                    "Swapchain error: {error}. \
                                Rendering cannot continue."
                                )
                            }
                            _ => {
                                // Try rendering again next frame.
                                window.request_redraw();
                            }
                        },
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    *cursor_position = Some(position);
                }
                WindowEvent::ModifiersChanged(new_modifiers) => {
                    *modifiers = new_modifiers.state();
                }
                WindowEvent::Resized(_) => {
                    *resized = true;
                }
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                _ => {}
            }

            // Map window event to iced event
            if let Some(event) = iced_winit::conversion::window_event(
                event,
                window.scale_factor(),
                *modifiers,
            ) {
                state.queue_event(event);
            }

            // If there are events pending
            if !state.is_queue_empty() {
                // We update iced
                let (_, task) = state.update(
                    viewport.logical_size(),
                    cursor_position
                        .map(|p| {
                            conversion::cursor_position(
                                p,
                                viewport.scale_factor(),
                            )
                        })
                        .map(mouse::Cursor::Available)
                        .unwrap_or(mouse::Cursor::Unavailable),
                    renderer,
                    &Theme::Dark,
                    &renderer::Style {
                        text_color: Color::WHITE,
                    },
                    clipboard,
                    debug,
                );

                let _ = 'runtime_call: {
                    let Some(t) = task else {
                        break 'runtime_call 1;
                    };
                    let Some(stream) = into_stream(t) else {
                        break 'runtime_call 1;
                    };

                    runtime.run(stream);
                    0
                };

                // and request a redraw
                window.request_redraw();
            }
        }

        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Action<Message>) {
            let Self::Ready {
                ref mut renderer,
                state,
                viewport,
                ref mut debug, ..
            } = self
            else {
                return;
            };

            match event {
                Action::Widget(w) => {
                    state.operate(
                        renderer,
                        std::iter::once(w),
                        Size::new(viewport.physical_size().width as f32, viewport.physical_size().height as f32),
                        debug,
                    );
                }
                _ => {}
            }
        }
    }

    let mut runner = Runner::Loading(proxy);
    event_loop.run_app(&mut runner)
}
