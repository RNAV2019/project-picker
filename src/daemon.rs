use raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use std::ptr::NonNull;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_callback, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
            Connection, Proxy, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{
        KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
        LayerSurfaceConfigure,
    },
};
use std::os::unix::net::UnixListener;
use calloop::generic::Generic;
use calloop::Interest;
use std::io::Read;

const SOCKET_PATH: &str = "/tmp/project-picker.sock";

struct WaylandSurfaceHandle {
    surface: *mut std::ffi::c_void,  // wl_surface raw pointer
    display: *mut std::ffi::c_void,  // wl_display raw pointer
}

unsafe impl Send for WaylandSurfaceHandle {}
unsafe impl Sync for WaylandSurfaceHandle {}

impl HasWindowHandle for WaylandSurfaceHandle {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let handle = WaylandWindowHandle::new(NonNull::new(self.surface).unwrap());
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::Wayland(handle)) })
    }
}

impl HasDisplayHandle for WaylandSurfaceHandle {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let handle = WaylandDisplayHandle::new(NonNull::new(self.display).unwrap());
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(handle)) })
    }
}

pub struct State {
    // Wayland protocol state (SCK managed)
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,

    // Input devices (must be kept alive to receive events)
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,

    // Our layer surface
    layer_surface: LayerSurface,
    wl_surface: wl_surface::WlSurface,

    // wgpu rendering
    device: wgpu::Device,
    queue: wgpu::Queue,
    wgpu_surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    egui_renderer: egui_wgpu::Renderer,

    // egui
    egui_ctx: egui::Context,
    pending_input: egui::RawInput,
    current_modifiers: egui::Modifiers,
    pointer_pos: egui::Pos2,

    // App logic
    app: crate::app::App,

    // Control
    needs_redraw: bool,
    configured: bool,
    pending_toggle: bool,
    loop_handle: LoopHandle<'static, Self>,
    qh: QueueHandle<State>,
    conn: Connection,
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.needs_redraw = true;
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for State {
    fn closed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
    ) {
        std::process::exit(0);
    }
    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (width, height) = configure.new_size;
        let width = if width == 0 { 680 } else { width };
        let height = if height == 0 { 40 } else { height };
        self.resize_surface(width, height, qh);
        self.configured = true;
        self.needs_redraw = true;
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }
    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(kb) => self.keyboard = Some(kb),
                Err(e) => eprintln!("Failed to get keyboard: {e}"),
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat_state.get_pointer(qh, &seat) {
                Ok(ptr) => self.pointer = Some(ptr),
                Err(e) => eprintln!("Failed to get pointer: {e}"),
            }
        }
    }
    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        _capability: Capability,
    ) {
    }
    fn remove_seat(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }
}

impl KeyboardHandler for State {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
    }
    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }
    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.handle_key(event, true);
        self.needs_redraw = true;
    }
    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.handle_key(event, false);
    }
    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
    ) {
        self.current_modifiers = egui::Modifiers {
            alt: modifiers.alt,
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            mac_cmd: false,
            command: modifiers.ctrl,
        };
    }
}

impl State {
    fn handle_key(&mut self, event: KeyEvent, pressed: bool) {
        // Push text event for printable chars on press only
        if pressed {
            if let Some(text) = &event.utf8 {
                if !text.chars().all(|c| c.is_control()) {
                    self.pending_input.events.push(egui::Event::Text(text.clone()));
                }
            }
        }
        // Push key event for navigational/special keys
        if let Some(key) = keysym_to_egui(event.keysym) {
            self.pending_input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: self.current_modifiers,
            });
        }
    }

    fn resize_surface(&mut self, width: u32, height: u32, _qh: &QueueHandle<Self>) {
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.wgpu_surface.configure(&self.device, &self.surface_config);
    }

    /// Called from run_daemon after creating the EventLoop.
    /// Returns the fully initialized State; does NOT create a new EventLoop.
    pub fn init(
        loop_handle: LoopHandle<'static, Self>,
        qh: QueueHandle<Self>,
        conn: Connection,
        globals: GlobalList,
    ) -> Self {
        let compositor_state = CompositorState::bind(&globals, &qh)
            .expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(&globals, &qh)
            .expect("zwlr_layer_shell_v1 not available — is your compositor Hyprland/wlroots?");
        let seat_state = SeatState::new(&globals, &qh);
        let output_state = OutputState::new(&globals, &qh);
        let registry_state = RegistryState::new(&globals);

        // Create wl_surface and layer surface
        let wl_surface = compositor_state.create_surface(&qh);
        let layer_surface = layer_shell.create_layer_surface(
            &qh, wl_surface.clone(), Layer::Overlay, Some("project-picker"), None,
        );
        layer_surface.set_anchor(smithay_client_toolkit::shell::wlr_layer::Anchor::TOP);
        layer_surface.set_size(680, 480);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        wl_surface.commit();

        // Build wgpu surface using the raw-window-handle bridge.
        // The handle is leaked so it satisfies the 'static bound on wgpu::Surface<'static>.
        let display_ptr = conn.backend().display_ptr() as *mut std::ffi::c_void;
        let surface_ptr = wl_surface.id().as_ptr() as *mut std::ffi::c_void;
        let handle: &'static WaylandSurfaceHandle = Box::leak(Box::new(
            WaylandSurfaceHandle { surface: surface_ptr, display: display_ptr },
        ));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let wgpu_surface = instance.create_surface(handle)
            .expect("Failed to create wgpu surface");

        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&wgpu_surface),
                    ..Default::default()
                })
                .await
                .expect("No compatible GPU adapter");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to get device");
            (adapter, device, queue)
        });

        let caps = wgpu_surface.get_capabilities(&adapter);
        let format = caps.formats[0];
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 680,
            height: 480,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        wgpu_surface.configure(&device, &surface_config);

        let egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);
        let egui_ctx = egui::Context::default();

        // Apply dark theme
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(0x1c, 0x1c, 0x1c);
        egui_ctx.set_visuals(visuals);

        let app = crate::app::App::new();

        State {
            registry_state,
            seat_state,
            output_state,
            compositor_state,
            layer_shell,
            keyboard: None,
            pointer: None,
            layer_surface,
            wl_surface,
            device,
            queue,
            wgpu_surface,
            surface_config,
            egui_renderer,
            egui_ctx,
            pending_input: egui::RawInput::default(),
            current_modifiers: egui::Modifiers::default(),
            pointer_pos: egui::Pos2::ZERO,
            app,
            needs_redraw: false,
            configured: false,
            pending_toggle: false,
            loop_handle,
            qh,
            conn,
        }
    }
}

fn keysym_to_egui(
    sym: smithay_client_toolkit::seat::keyboard::Keysym,
) -> Option<egui::Key> {
    use smithay_client_toolkit::seat::keyboard::Keysym;
    match sym {
        Keysym::Up => Some(egui::Key::ArrowUp),
        Keysym::Down => Some(egui::Key::ArrowDown),
        Keysym::Left => Some(egui::Key::ArrowLeft),
        Keysym::Right => Some(egui::Key::ArrowRight),
        Keysym::Return | Keysym::KP_Enter => Some(egui::Key::Enter),
        Keysym::Escape => Some(egui::Key::Escape),
        Keysym::BackSpace => Some(egui::Key::Backspace),
        Keysym::Tab => Some(egui::Key::Tab),
        Keysym::ISO_Left_Tab => Some(egui::Key::Tab),
        _ => None,
    }
}

impl PointerHandler for State {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let pos = egui::pos2(event.position.0 as f32, event.position.1 as f32);
            match event.kind {
                PointerEventKind::Motion { .. } => {
                    self.pointer_pos = pos;
                    self.pending_input.events.push(egui::Event::PointerMoved(pos));
                    self.needs_redraw = true;
                }
                PointerEventKind::Press { button, .. } => {
                    let btn = wayland_button_to_egui(button);
                    self.pending_input.events.push(egui::Event::PointerButton {
                        pos,
                        button: btn,
                        pressed: true,
                        modifiers: self.current_modifiers,
                    });
                    self.needs_redraw = true;
                }
                PointerEventKind::Release { button, .. } => {
                    let btn = wayland_button_to_egui(button);
                    self.pending_input.events.push(egui::Event::PointerButton {
                        pos,
                        button: btn,
                        pressed: false,
                        modifiers: self.current_modifiers,
                    });
                }
                PointerEventKind::Leave { .. } => {
                    self.pending_input.events.push(egui::Event::PointerGone);
                }
                _ => {}
            }
        }
    }
}

fn wayland_button_to_egui(button: u32) -> egui::PointerButton {
    match button {
        0x110 => egui::PointerButton::Primary,
        0x111 => egui::PointerButton::Secondary,
        0x112 => egui::PointerButton::Middle,
        _ => egui::PointerButton::Primary,
    }
}

impl State {
    pub fn render_frame(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured || !self.app.visible {
            return;
        }

        let surface_texture = match self.wgpu_surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                self.wgpu_surface.configure(&self.device, &self.surface_config);
                return;
            }
            Err(_) => return,
        };

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let width = self.surface_config.width;
        let height = self.surface_config.height;

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: 1.0,
        };

        // Collect pending input, clear for next frame
        let mut raw_input = std::mem::take(&mut self.pending_input);
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width as f32, height as f32),
        ));

        // Build and process egui frame
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            self.app.ui(ctx);
        });

        // Encode and submit
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // Upload texture deltas
        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        let primitives = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &primitives, &screen_descriptor);

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0x1c as f64 / 255.0,
                            g: 0x1c as f64 / 255.0,
                            b: 0x1c as f64 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            // egui-wgpu 0.29 requires RenderPass<'static>; forget_lifetime() erases the
            // encoder borrow (safe here because we do not use encoder after this block).
            let mut render_pass = render_pass.forget_lifetime();
            self.egui_renderer.render(&mut render_pass, &primitives, &screen_descriptor);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        self.wl_surface.commit();

        // Handle app actions after presenting so Hide doesn't skip the final frame
        self.handle_app_output();

        // Request next frame callback (drives vsync)
        self.wl_surface.frame(qh, ());
        self.needs_redraw = false;
    }

    pub fn show(&mut self, qh: &QueueHandle<Self>) {
        self.layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.wl_surface.commit();
        self.app.on_show();
        self.wl_surface.frame(qh, ()); // kick off render loop
        self.needs_redraw = true;
    }

    pub fn hide(&mut self) {
        self.layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        self.wl_surface.commit();
        self.app.on_hide();
    }

    fn handle_app_output(&mut self) {
        let actions = self.app.drain_actions();
        for action in actions {
            match action {
                crate::app::AppAction::Hide => self.hide(),
                crate::app::AppAction::OpenTerminal(path) => crate::terminal::open_terminal(&path),
                crate::app::AppAction::RemoveProject(_) => {}
            }
        }
    }
}

// Dispatch wl_callback (frame timing)
impl smithay_client_toolkit::reexports::client::Dispatch<wl_callback::WlCallback, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.needs_redraw = true;
        }
    }
}

smithay_client_toolkit::delegate_compositor!(State);
smithay_client_toolkit::delegate_output!(State);
smithay_client_toolkit::delegate_seat!(State);
smithay_client_toolkit::delegate_keyboard!(State);
smithay_client_toolkit::delegate_pointer!(State);
smithay_client_toolkit::delegate_layer!(State);
smithay_client_toolkit::delegate_registry!(State);

pub fn run_daemon() {
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new().expect("event loop");
    let loop_handle = event_loop.handle();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn).expect("Failed to get Wayland globals");
    let qh = event_queue.handle();

    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle.clone())
        .expect("Failed to insert Wayland source");

    let mut state = State::init(loop_handle.clone(), qh, conn, globals);

    State::setup_socket(&loop_handle);

    event_loop.run(None, &mut state, |state| {
        if state.pending_toggle {
            state.pending_toggle = false;
            if state.app.visible {
                state.hide();
            } else {
                let qh = state.qh.clone();
                state.show(&qh);
            }
        }

        if state.needs_redraw && state.app.visible {
            let qh = state.qh.clone();
            state.render_frame(&qh);
        }
    }).expect("Event loop error");
}

impl State {
    pub fn setup_socket(loop_handle: &LoopHandle<'static, Self>) {
        let _ = std::fs::remove_file(SOCKET_PATH);
        let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind Unix socket");
        listener.set_nonblocking(true).unwrap();

        let source = Generic::new(listener, Interest::READ, calloop::Mode::Level);
        loop_handle.insert_source(source, |_, listener, state| {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = String::new();
                    let _ = stream.read_to_string(&mut buf);
                    for line in buf.lines() {
                        state.handle_ipc_command(line.trim());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => eprintln!("Socket accept error: {}", e),
            }
            Ok(calloop::PostAction::Continue)
        }).expect("Failed to insert socket source");
    }

    fn handle_ipc_command(&mut self, cmd: &str) {
        match cmd {
            "toggle" => { self.pending_toggle = true; }
            _ => {}
        }
    }
}
