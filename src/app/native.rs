//! A native window (winit + wgpu): Metal on macOS, Vulkan elsewhere (DX12 is not built in). It forwards
//! the pointer, touch, wheel, keys, typed text, the input method and the system clipboard (arboard) to
//! the runtime, and draws a frame only when the runtime needs one; between frames the event loop sleeps
//! until input or the runtime's next timed wake.
use super::{App, Window};
use crate::el::{Cursor, Key, Mods};
use crate::error::{SurfaceError, WindowError};
use crate::render::Renderer;
use crate::text::Face;
use crate::ui::{PointerKind, Ui};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::WindowId;

/// Open a window and run the application until the window closes.
///
/// `setup` is called once with the renderer before the first frame (to register pictures and meshes);
/// `make` builds the `Ui` from the bundled faces, the window's size in device px and its scale. The
/// window then opens `window.route` through [`App::route`], if there is one.
pub fn run<S: App + 'static, M: Clone + 'static>(
    window: Window,
    setup: impl FnOnce(&mut Renderer) + 'static,
    make: impl FnOnce(Vec<Face>, u32, u32, f32) -> Ui<S, M> + 'static,
) -> Result<(), WindowError> {
    let el = EventLoop::new().map_err(|e| WindowError::NoEventLoop(e.to_string()))?;
    let mut shell = Shell {
        display: el.owned_display_handle(), opts: window, start: Instant::now(), run: None, made: Some((Box::new(setup), Box::new(make))),
        failed: None, mods: ModifiersState::empty(), composing: false, cursor: (0.0, 0.0), reported: 0.0,
    };
    el.run_app(&mut shell).map_err(|e| WindowError::EventLoop(e.to_string()))?;
    shell.failed.map_or(Ok(()), Err)
}

type Setup = Box<dyn FnOnce(&mut Renderer)>;
type Make<S, M> = Box<dyn FnOnce(Vec<Face>, u32, u32, f32) -> Ui<S, M>>;

struct Running<S, M> {
    window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    r: Renderer,
    ui: Ui<S, M>,
    frames: u64,
    /// Redraw requests that found nothing to draw.
    skipped: u64,
}

struct Shell<S, M> {
    display: OwnedDisplayHandle,
    opts: Window,
    start: Instant,
    run: Option<Running<S, M>>,
    made: Option<(Setup, Make<S, M>)>,
    failed: Option<WindowError>,
    mods: ModifiersState,
    /// An input method is composing (its keys are its own).
    composing: bool,
    /// Logical px.
    cursor: (f32, f32),
    reported: f32,
}

impl<S: App, M: Clone> Shell<S, M> {
    fn now(&self) -> f32 { self.start.elapsed().as_secs_f32() }

    /// Every two seconds, when reporting: how many frames were drawn and how many redraws found nothing.
    fn report(&mut self, now: f32) {
        if !self.opts.report || now - self.reported < 2.0 { return; }
        self.reported = now;
        if let Some(s) = &self.run { eprintln!("{:6.1} s: {} frames, {} skipped", now, s.frames, s.skipped); }
    }

    fn open(&mut self, el: &ActiveEventLoop) -> Result<Running<S, M>, WindowError> {
        let (setup, make) = self.made.take().ok_or_else(|| WindowError::Create("the window was already opened".into()))?;
        let attrs = winit::window::Window::default_attributes().with_title(self.opts.title.clone()).with_inner_size(LogicalSize::new(self.opts.size.0, self.opts.size.1));
        let window = Arc::new(el.create_window(attrs).map_err(|e| WindowError::Create(e.to_string()))?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(self.display.clone())));
        let surface = instance.create_surface(window.clone()).map_err(|e| WindowError::Surface(e.to_string()))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() }))
            .map_err(|_| SurfaceError::NoAdapter)?;
        let info = adapter.get_info();
        if self.opts.report { eprintln!("adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type); }
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).map_err(|e| SurfaceError::Device(e.to_string()))?;
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        // A linear format: the material writes the colours it means (it encodes for sRGB surfaces itself).
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or_else(|| caps.formats.first().copied())
            .ok_or_else(|| WindowError::Surface("the surface offers no format".into()))?;
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).ok_or_else(|| WindowError::Surface("not supported by the adapter".into()))?;
        config.format = format;
        surface.configure(&device, &config);
        let mut r = Renderer::new(device, queue, format, config.width, config.height);
        r.set_particle_tier(crate::render::particle_tier(&adapter));
        setup(&mut r);
        let faces = crate::text::load_bundled(crate::text::ASSETS)?;
        let mut ui = make(faces, config.width, config.height, window.scale_factor() as f32);
        ui.perf_mut().backend = format!("{:?}", info.backend);
        ui.perf_mut().adapter = info.name.clone();
        if let Some(p) = &self.opts.route && ui.state.route(p) { ui.relayout(); }
        ui.restart(self.now());
        Ok(Running { window, surface, config, r, ui, frames: 0, skipped: 0 })
    }
}

/// A winit key as the runtime's keys (a character key can produce several characters).
fn keys(k: &WKey) -> Vec<Key> {
    match k {
        WKey::Named(n) => match n {
            NamedKey::Enter => vec![Key::Enter], NamedKey::Backspace => vec![Key::Backspace], NamedKey::Delete => vec![Key::Delete],
            NamedKey::Escape => vec![Key::Escape], NamedKey::Tab => vec![Key::Tab], NamedKey::Space => vec![Key::Char(' ')],
            NamedKey::ArrowUp => vec![Key::Up], NamedKey::ArrowDown => vec![Key::Down], NamedKey::ArrowLeft => vec![Key::Left], NamedKey::ArrowRight => vec![Key::Right],
            NamedKey::Home => vec![Key::Home], NamedKey::End => vec![Key::End], NamedKey::PageUp => vec![Key::PageUp], NamedKey::PageDown => vec![Key::PageDown],
            NamedKey::F1 => vec![Key::F(1)], NamedKey::F2 => vec![Key::F(2)], NamedKey::F3 => vec![Key::F(3)], NamedKey::F4 => vec![Key::F(4)],
            NamedKey::F5 => vec![Key::F(5)], NamedKey::F6 => vec![Key::F(6)], NamedKey::F7 => vec![Key::F(7)], NamedKey::F8 => vec![Key::F(8)],
            NamedKey::F9 => vec![Key::F(9)], NamedKey::F10 => vec![Key::F(10)], NamedKey::F11 => vec![Key::F(11)], NamedKey::F12 => vec![Key::F(12)],
            _ => vec![],
        },
        WKey::Character(s) => s.chars().map(Key::Char).collect(),
        _ => vec![],
    }
}

fn cursor_icon(c: Cursor) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as I;
    match c { Cursor::Default => I::Default, Cursor::Pointer => I::Pointer, Cursor::Text => I::Text, Cursor::Grab => I::Grab, Cursor::Grabbing => I::Grabbing, Cursor::ColResize => I::ColResize, Cursor::RowResize => I::RowResize }
}

/// Use the system clipboard (opened for each use; `None` when there is none).
fn clipboard<R>(f: impl FnOnce(&mut arboard::Clipboard) -> R) -> Option<R> {
    match arboard::Clipboard::new() {
        Ok(mut b) => Some(f(&mut b)),
        Err(e) => { eprintln!("no clipboard: {e}"); None }
    }
}

impl<S: App, M: Clone> ApplicationHandler for Shell<S, M> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.run.is_some() || self.failed.is_some() { return; }
        match self.open(el) {
            Ok(r) => self.run = Some(r),
            Err(e) => { self.failed = Some(e); el.exit(); }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let now = self.now();
        let Some(s) = self.run.as_mut() else { return };
        let scale = s.window.scale_factor() as f32;
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                s.config.width = size.width.max(1);
                s.config.height = size.height.max(1);
                s.surface.configure(s.r.device(), &s.config);
                s.ui.resize(s.config.width, s.config.height, scale);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => s.ui.resize(s.config.width, s.config.height, scale_factor as f32),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32 / scale, position.y as f32 / scale);
                s.ui.tick(now);
                s.ui.pointer_move(self.cursor.0, self.cursor.1);
            }
            WindowEvent::CursorLeft { .. } => { s.ui.tick(now); s.ui.pointer_leave() }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (x, y) = self.cursor;
                s.ui.tick(now);
                if state == ElementState::Pressed { s.ui.press_with(x, y, self.mods.shift_key()) } else { s.ui.release(x, y) }
            }
            // Touch screens: fingers scroll and tap (DESIGN.md §1.8.2); pinch and other gestures are not handled.
            WindowEvent::Touch(t) => {
                let (x, y) = (t.location.x as f32 / scale, t.location.y as f32 / scale);
                s.ui.tick(now);
                match t.phase {
                    TouchPhase::Started => s.ui.pointer_down(t.id, PointerKind::Touch, x, y, false),
                    TouchPhase::Moved => s.ui.pointer_moved(t.id, PointerKind::Touch, x, y),
                    TouchPhase::Ended => s.ui.pointer_up(t.id, PointerKind::Touch, x, y),
                    TouchPhase::Cancelled => s.ui.pointer_cancel(t.id),
                }
            }
            // The input method: its composition is shown in the field, its commit inserted.
            WindowEvent::Ime(ime) => {
                s.ui.tick(now);
                match ime {
                    Ime::Preedit(text, cursor) => { self.composing = !text.is_empty(); s.ui.ime_preedit(&text, cursor); }
                    Ime::Commit(text) => { self.composing = false; s.ui.ime_commit(&text); }
                    Ime::Disabled => if self.composing { self.composing = false; s.ui.ime_preedit("", None); },
                    Ime::Enabled => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // winit: positive is away from the user (up / left); the runtime scrolls down for positive.
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (-x * 40.0, -y * 40.0),
                    MouseScrollDelta::PixelDelta(p) => (-p.x as f32 / scale, -p.y as f32 / scale),
                };
                s.ui.tick(now);
                s.ui.wheel_xy(self.cursor.0, self.cursor.1, dx, dy);
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                s.ui.tick(now);
                let cmd = if cfg!(target_os = "macos") { self.mods.super_key() } else { self.mods.control_key() };
                let mods = Mods { shift: self.mods.shift_key(), cmd, alt: self.mods.alt_key() };
                if cmd {
                    // The clipboard, through the system's; Cmd+A / Z / Y go to the text field.
                    match &event.logical_key {
                        WKey::Character(c) if c.eq_ignore_ascii_case("c") => { if let Some(t) = s.ui.copy() { clipboard(|b| b.set_text(t).is_ok()); } }
                        WKey::Character(c) if c.eq_ignore_ascii_case("x") => { if let Some(t) = s.ui.cut() { clipboard(|b| b.set_text(t).is_ok()); } }
                        WKey::Character(c) if c.eq_ignore_ascii_case("v") => { if let Some(t) = clipboard(|b| b.get_text().ok()).flatten() { s.ui.paste(&t); } }
                        k => for k in keys(k) { s.ui.key_mods(k, mods); },
                    }
                    return;
                }
                // While the input method composes, the keys are its own.
                if self.composing { return; }
                // In a text field, characters are text (what the key produced, with the layout's shifts).
                if s.ui.text_focused() && !mods.alt && let Some(t) = event.text.as_ref().filter(|t| t.chars().all(|c| !c.is_control())) {
                    s.ui.text_input(t);
                    return;
                }
                if mods.alt { return; }
                for k in keys(&event.logical_key) { s.ui.key_mods(k, mods); }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                s.ui.tick(now);
                for k in keys(&event.logical_key) { s.ui.key_up(k); }
            }
            WindowEvent::RedrawRequested => {
                s.ui.tick(now);
                self.report(now);
                let Some(s) = self.run.as_mut() else { return };
                if self.opts.exit_after.is_some_and(|t| now * 1000.0 >= t) {
                    if self.opts.report { eprintln!("closing after {} frames", s.frames); }
                    el.exit();
                    return;
                }
                // A frame to save: drawn into a texture of the surface's format and read back.
                if self.opts.shots.first().is_some_and(|(_, t)| now * 1000.0 >= *t) {
                    let (path, _) = self.opts.shots.remove(0);
                    let look = s.ui.look();
                    match crate::offscreen::read_back(&mut s.r, &mut s.ui, &look, s.config.format, s.config.width, s.config.height) {
                        Ok(px) => { crate::offscreen::save_png(&path, s.config.width, s.config.height, &px); eprintln!("saved {path} at {now:.1} s"); }
                        Err(e) => eprintln!("could not read the frame back: {e}"),
                    }
                    // The saved frame was not presented: the window still has to show it.
                    s.ui.invalidate();
                    s.window.request_redraw();
                    return;
                }
                // Nothing changed and nothing moves: no frame (the event loop waits for input or a wake).
                if !s.ui.needs_frame(now) { s.skipped += 1; s.ui.perf_mut().skipped += 1; return; }
                // The input method follows the focused field's caret; the pointer's shape follows what is under it.
                match s.ui.ime_rect() {
                    Some(r) => {
                        s.window.set_ime_allowed(true);
                        s.window.set_ime_cursor_area(LogicalPosition::new(r.x, r.y), LogicalSize::new(r.w.max(1.0), r.h));
                    }
                    None => s.window.set_ime_allowed(false),
                }
                s.window.set_cursor(cursor_icon(s.ui.cursor()));
                let frame = match s.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    _ => { s.surface.configure(s.r.device(), &s.config); s.window.request_redraw(); return; }
                };
                let view = frame.texture.create_view(&Default::default());
                let look = s.ui.look();
                s.r.draw(&view, &mut s.ui, &look);
                drop(view);
                s.r.queue().present(frame);
                s.frames += 1;
                // A copy button asked for the clipboard.
                if let Some(t) = s.ui.state.take_clipboard() { let _ = clipboard(|c| c.set_text(t)); }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // Frames on demand: draw when the runtime needs one, else sleep until its next timed wake (the
        // caret's blink, a clock), the next report, a frame to save or the time to close.
        let now = self.now();
        self.report(now);
        let Some(s) = &self.run else { return };
        let mut wake = s.ui.next_wake(now);
        if wake.is_some_and(|w| w <= now) { s.window.request_redraw(); wake = None; }
        let report = self.opts.report.then_some(self.reported + 2.0);
        let others = [report, self.opts.shots.first().map(|(_, t)| t / 1000.0), self.opts.exit_after.map(|t| t / 1000.0)];
        let next = others.into_iter().flatten().chain(wake).fold(f32::INFINITY, f32::min);
        let due = self.opts.shots.first().is_some_and(|(_, t)| now * 1000.0 >= *t) || self.opts.exit_after.is_some_and(|t| now * 1000.0 >= t);
        if due { s.window.request_redraw(); }
        el.set_control_flow(if next.is_finite() { ControlFlow::WaitUntil(self.start + Duration::from_secs_f32(next.max(now))) } else { ControlFlow::Wait });
    }
}
