use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
            Connection, QueueHandle,
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

pub struct State {
    // Wayland protocol state (SCK managed)
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,

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
        if capability == Capability::Keyboard
            && self.seat_state.get_keyboard(qh, &seat, None).is_ok()
        {
        }
        if capability == Capability::Pointer
            && self.seat_state.get_pointer(qh, &seat).is_ok()
        {
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

    fn resize_surface(&mut self, _width: u32, _height: u32, _qh: &QueueHandle<Self>) {
        // Placeholder: will be implemented in the rendering task
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

smithay_client_toolkit::delegate_compositor!(State);
smithay_client_toolkit::delegate_output!(State);
smithay_client_toolkit::delegate_seat!(State);
smithay_client_toolkit::delegate_keyboard!(State);
smithay_client_toolkit::delegate_pointer!(State);
smithay_client_toolkit::delegate_layer!(State);
smithay_client_toolkit::delegate_registry!(State);
