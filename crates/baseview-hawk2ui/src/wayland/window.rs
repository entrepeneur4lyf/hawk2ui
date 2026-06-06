use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::error::Error;
use std::ffi::c_void;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use keyboard_types::{Code, Key, KeyState, KeyboardEvent, Location, Modifiers};
use nix::poll::{poll, PollFd, PollFlags};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};
use wayland_client::{
    backend::{
        protocol::{Argument, Message},
        smallvec::smallvec,
        ObjectId,
    },
    delegate_noop,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use xkbcommon::xkb;

use crate::{
    Event, MouseButton, MouseCursor, MouseEvent, Point, ScrollDelta, Size, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};

pub struct WindowHandle {
    raw_display_handle: Option<RawDisplayHandle>,
    raw_window_handle: Option<RawWindowHandle>,
    event_loop_handle: Option<JoinHandle<()>>,
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl WindowHandle {
    pub(crate) fn closed() -> Self {
        Self {
            raw_display_handle: None,
            raw_window_handle: None,
            event_loop_handle: None,
            close_requested: Arc::new(AtomicBool::new(false)),
            is_open: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn close(&mut self) {
        self.close_requested.store(true, Ordering::Relaxed);
        if let Some(event_loop) = self.event_loop_handle.take() {
            let _ = event_loop.join();
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed)
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        self.close();
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        if let Some(raw_window_handle) = self.raw_window_handle {
            if self.is_open.load(Ordering::Relaxed) {
                return raw_window_handle;
            }
        }

        RawWindowHandle::Wayland(WaylandWindowHandle::empty())
    }
}

unsafe impl HasRawDisplayHandle for WindowHandle {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        if let Some(raw_display_handle) = self.raw_display_handle {
            if self.is_open.load(Ordering::Relaxed) {
                return raw_display_handle;
            }
        }

        RawDisplayHandle::Wayland(WaylandDisplayHandle::empty())
    }
}

pub(crate) struct ParentHandle {
    close_requested: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
}

impl ParentHandle {
    pub fn new() -> (Self, WindowHandle) {
        let close_requested = Arc::new(AtomicBool::new(false));
        let is_open = Arc::new(AtomicBool::new(true));
        let handle = WindowHandle {
            raw_display_handle: None,
            raw_window_handle: None,
            event_loop_handle: None,
            close_requested: Arc::clone(&close_requested),
            is_open: Arc::clone(&is_open),
        };

        (Self { close_requested, is_open }, handle)
    }

    pub fn parent_did_drop(&self) -> bool {
        self.close_requested.load(Ordering::Relaxed)
    }
}

impl Drop for ParentHandle {
    fn drop(&mut self) {
        self.is_open.store(false, Ordering::Relaxed);
    }
}

pub(crate) struct WindowInner {
    software_buffers: RefCell<Vec<SoftwareBuffer>>,
    _subsurface: wl_subsurface::WlSubsurface,
    surface: wl_surface::WlSurface,
    shm: wl_shm::WlShm,
    window_info: Cell<WindowInfo>,
    close_requested: Cell<bool>,
    event_state: RefCell<WaylandState>,
    event_queue: RefCell<EventQueue<WaylandState>>,
    queue_handle: QueueHandle<WaylandState>,
    connection: Connection,
}

struct SoftwareBuffer {
    width: u32,
    height: u32,
    byte_len: usize,
    file: File,
    buffer: wl_buffer::WlBuffer,
    released: Arc<AtomicBool>,
}

impl WindowInner {
    fn present_software_frame(
        &self, width: u32, height: u32, xrgb_pixels: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let byte_len = software_frame_byte_len(width, height)?;
        if xrgb_pixels.len() != byte_len {
            return Err("Wayland software frame byte length does not match dimensions".into());
        }

        self.dispatch_pending_events()?;
        let buffer_index = self.reusable_buffer_index(width, height, byte_len);
        let buffer_index = if let Some(index) = buffer_index {
            index
        } else {
            self.roundtrip_events()?;
            if let Some(index) = self.reusable_buffer_index(width, height, byte_len) {
                index
            } else {
                let buffer = self.create_software_buffer(width, height, byte_len)?;
                let mut buffers = self.software_buffers.borrow_mut();
                buffers.push(buffer);
                buffers.len() - 1
            }
        };

        let mut buffers = self.software_buffers.borrow_mut();
        let buffer = &mut buffers[buffer_index];
        write_frame_file(&mut buffer.file, xrgb_pixels)?;
        buffer.released.store(false, Ordering::Relaxed);
        self.surface.attach(Some(&buffer.buffer), 0, 0);
        self.surface.damage(0, 0, i32::try_from(width)?, i32::try_from(height)?);
        self.surface.commit();
        self.connection.flush()?;
        Ok(())
    }

    fn reusable_buffer_index(&self, width: u32, height: u32, byte_len: usize) -> Option<usize> {
        self.software_buffers.borrow().iter().position(|buffer| {
            buffer.width == width
                && buffer.height == height
                && buffer.byte_len == byte_len
                && buffer.released.load(Ordering::Relaxed)
        })
    }

    fn create_software_buffer(
        &self, width: u32, height: u32, byte_len: usize,
    ) -> Result<SoftwareBuffer, Box<dyn Error>> {
        let byte_len_i32 = i32::try_from(byte_len)?;
        let width_i32 = i32::try_from(width)?;
        let height_i32 = i32::try_from(height)?;
        let stride_i32 = i32::try_from(width.checked_mul(4).ok_or("Wayland stride overflow")?)?;
        let file = tempfile::tempfile()?;
        file.set_len(u64::try_from(byte_len)?)?;
        let released = Arc::new(AtomicBool::new(true));
        let pool = self.shm.create_pool(file.as_fd(), byte_len_i32, &self.queue_handle, ());
        let buffer = pool.create_buffer(
            0,
            width_i32,
            height_i32,
            stride_i32,
            wl_shm::Format::Xrgb8888,
            &self.queue_handle,
            Arc::clone(&released),
        );
        pool.destroy();
        Ok(SoftwareBuffer { width, height, byte_len, file, buffer, released })
    }

    fn dispatch_pending_events(&self) -> Result<(), Box<dyn Error>> {
        let mut event_state = self.event_state.borrow_mut();
        let mut event_queue = self.event_queue.borrow_mut();
        event_queue.flush()?;
        while event_queue.dispatch_pending(&mut event_state)? > 0 {}

        let Some(read_guard) = event_queue.prepare_read() else {
            while event_queue.dispatch_pending(&mut event_state)? > 0 {}
            return Ok(());
        };

        let fd = read_guard.connection_fd().as_raw_fd();
        let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
        let ready = poll(&mut fds, 0)?;
        if ready > 0 && fds[0].revents().is_some_and(|events| events.contains(PollFlags::POLLIN)) {
            read_guard.read()?;
            while event_queue.dispatch_pending(&mut event_state)? > 0 {}
        }

        Ok(())
    }

    fn take_pending_events(&self) -> Vec<Event> {
        self.event_state.borrow_mut().take_pending_events()
    }

    fn roundtrip_events(&self) -> Result<(), Box<dyn Error>> {
        let mut event_state = self.event_state.borrow_mut();
        self.event_queue.borrow_mut().roundtrip(&mut event_state)?;
        Ok(())
    }
}

pub struct Window<'a> {
    pub(crate) inner: &'a WindowInner,
}

struct SendableWaylandParent {
    display: usize,
    surface: usize,
}

unsafe impl Send for SendableWaylandParent {}

struct SendableRwh {
    display: RawDisplayHandle,
    window: RawWindowHandle,
}

unsafe impl Send for SendableRwh {}

type WindowOpenResult = Result<SendableRwh, ()>;

impl<'a> Window<'a> {
    pub fn open_parented<P, H, B>(parent: &P, options: WindowOpenOptions, build: B) -> WindowHandle
    where
        P: HasRawDisplayHandle + HasRawWindowHandle,
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let parent = match (parent.raw_display_handle(), parent.raw_window_handle()) {
            (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window))
                if !display.display.is_null() && !window.surface.is_null() =>
            {
                SendableWaylandParent {
                    display: display.display as usize,
                    surface: window.surface as usize,
                }
            }
            _ => return WindowHandle::closed(),
        };

        let (tx, rx) = mpsc::sync_channel::<WindowOpenResult>(1);
        let (parent_handle, mut window_handle) = ParentHandle::new();
        let join_handle = thread::spawn(move || {
            if Self::window_thread(parent, options, build, tx.clone(), parent_handle).is_err() {
                let _ = tx.send(Err(()));
            }
        });

        match rx.recv() {
            Ok(Ok(raw_handles)) => {
                window_handle.raw_display_handle = Some(raw_handles.display);
                window_handle.raw_window_handle = Some(raw_handles.window);
                window_handle.event_loop_handle = Some(join_handle);
            }
            _ => {
                window_handle.close_requested.store(true, Ordering::Relaxed);
                window_handle.is_open.store(false, Ordering::Relaxed);
                let _ = join_handle.join();
            }
        }
        window_handle
    }

    fn window_thread<H, B>(
        parent: SendableWaylandParent, options: WindowOpenOptions, build: B,
        tx: mpsc::SyncSender<WindowOpenResult>, parent_handle: ParentHandle,
    ) -> Result<(), Box<dyn Error>>
    where
        H: WindowHandler + 'static,
        B: FnOnce(&mut crate::Window) -> H,
        B: Send + 'static,
    {
        let connection = foreign_connection(parent.display)?;
        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();
        let display = connection.display();
        display.get_registry(&qh, ());

        let mut event_state = WaylandState::default();
        event_queue.roundtrip(&mut event_state)?;
        let compositor =
            event_state.compositor.clone().ok_or("Wayland compositor global missing")?;
        let subcompositor =
            event_state.subcompositor.clone().ok_or("Wayland subcompositor global missing")?;
        let shm = event_state.shm.clone().ok_or("Wayland wl_shm global missing")?;
        let surface = compositor.create_surface(&qh, ());
        let subsurface = create_borrowed_parent_subsurface(
            &connection,
            &subcompositor,
            &surface,
            parent.surface,
            &qh,
        )?;
        subsurface.set_position(0, 0);
        subsurface.set_desync();
        surface.commit();
        connection.flush()?;

        let scaling = match options.scale {
            WindowScalePolicy::SystemScaleFactor => 1.0,
            WindowScalePolicy::ScaleFactor(scale) => scale,
        };
        let window_info = WindowInfo::from_logical_size(options.size, scaling);
        let inner = WindowInner {
            software_buffers: RefCell::new(Vec::new()),
            _subsurface: subsurface,
            surface,
            shm,
            window_info: Cell::new(window_info),
            close_requested: Cell::new(false),
            event_state: RefCell::new(event_state),
            event_queue: RefCell::new(event_queue),
            queue_handle: qh,
            connection,
        };
        let mut window =
            crate::Window::new(crate::linux::Window::Wayland(Window { inner: &inner }));
        let mut handler = build(&mut window);
        handler.on_event(&mut window, Event::Window(WindowEvent::Resized(window_info)));

        let raw_display = raw_wayland_display(parent.display);
        let raw_window = raw_wayland_window(inner.surface.id().as_ptr() as *mut c_void as usize);
        let _ = tx.send(Ok(SendableRwh { display: raw_display, window: raw_window }));

        let mut event_loop = EventLoop::new(inner, handler, parent_handle);
        event_loop.run()?;
        Ok(())
    }

    pub fn set_mouse_cursor(&self, _mouse_cursor: MouseCursor) {}

    pub fn close(&mut self) {
        self.inner.close_requested.set(true);
    }

    pub fn has_focus(&mut self) -> bool {
        false
    }

    pub fn focus(&mut self) {}

    pub fn resize(&mut self, size: Size) {
        let scaling = self.inner.window_info.get().scale();
        self.inner.window_info.set(WindowInfo::from_logical_size(size, scaling));
    }

    pub fn hawk2ui_present_software_frame(
        &self, width: u32, height: u32, xrgb_pixels: &[u8],
    ) -> Result<(), String> {
        self.inner
            .present_software_frame(width, height, xrgb_pixels)
            .map_err(|error| error.to_string())
    }

    #[cfg(feature = "opengl")]
    pub fn gl_context(&self) -> Option<&crate::gl::GlContext> {
        None
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        raw_wayland_window(self.inner.surface.id().as_ptr() as *mut c_void as usize)
    }
}

unsafe impl<'a> HasRawDisplayHandle for Window<'a> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        raw_wayland_display(self.inner.connection.backend().display_ptr() as usize)
    }
}

#[derive(Default)]
struct AxisFrame {
    x: f32,
    y: f32,
}

impl AxisFrame {
    fn add_horizontal(&mut self, value: f32) {
        self.x += value;
    }

    fn add_vertical(&mut self, value: f32) {
        self.y += value;
    }

    fn take_scroll_delta(&mut self) -> Option<ScrollDelta> {
        if self.x == 0.0 && self.y == 0.0 {
            return None;
        }

        let delta = ScrollDelta::Pixels { x: self.x, y: self.y };
        self.x = 0.0;
        self.y = 0.0;
        Some(delta)
    }
}

struct WaylandState {
    compositor: Option<wl_compositor::WlCompositor>,
    subcompositor: Option<wl_subcompositor::WlSubcompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_context: xkb::Context,
    keyboard_state: Option<XkbKeyboardState>,
    pointer_position: Point,
    modifiers: Modifiers,
    pending_axis_frame: AxisFrame,
    pending_events: VecDeque<Event>,
}

struct XkbKeyboardState {
    _keymap: xkb::Keymap,
    state: xkb::State,
}

impl Default for WaylandState {
    fn default() -> Self {
        Self {
            compositor: None,
            subcompositor: None,
            shm: None,
            seat: None,
            pointer: None,
            keyboard: None,
            keyboard_context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            keyboard_state: None,
            pointer_position: Point::new(0.0, 0.0),
            modifiers: Modifiers::default(),
            pending_axis_frame: AxisFrame::default(),
            pending_events: VecDeque::new(),
        }
    }
}

impl WaylandState {
    fn push_pointer_motion(&mut self, surface_x: f64, surface_y: f64) {
        self.pointer_position = Point::new(surface_x, surface_y);
        self.pending_events.push_back(Event::Mouse(MouseEvent::CursorMoved {
            position: self.pointer_position,
            modifiers: self.modifiers,
        }));
    }

    fn flush_axis_frame(&mut self) {
        if let Some(delta) = self.pending_axis_frame.take_scroll_delta() {
            self.pending_events.push_back(Event::Mouse(MouseEvent::WheelScrolled {
                delta,
                modifiers: self.modifiers,
            }));
        }
    }

    fn take_pending_events(&mut self) -> Vec<Event> {
        self.flush_axis_frame();
        self.pending_events.drain(..).collect()
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self, registry: &wl_registry::WlRegistry, event: wl_registry::Event, _: &(),
        _: &Connection, qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" if state.compositor.is_none() => {
                    state.compositor =
                        Some(registry.bind::<wl_compositor::WlCompositor, _, _>(name, 1, qh, ()));
                }
                "wl_subcompositor" if state.subcompositor.is_none() => {
                    state.subcompositor = Some(
                        registry.bind::<wl_subcompositor::WlSubcompositor, _, _>(name, 1, qh, ()),
                    );
                }
                "wl_shm" if state.shm.is_none() => {
                    state.shm = Some(registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ()));
                }
                "wl_seat" if state.seat.is_none() => {
                    state.seat =
                        Some(registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(7), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, Arc<AtomicBool>> for WaylandState {
    fn event(
        _state: &mut Self, _proxy: &wl_buffer::WlBuffer, event: wl_buffer::Event,
        released: &Arc<AtomicBool>, _connection: &Connection, _qh: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            released.store(true, Ordering::Relaxed);
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self, seat: &wl_seat::WlSeat, event: wl_seat::Event, _: &(), _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(capabilities) } = event {
            if capabilities.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
            if capabilities.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self, _pointer: &wl_pointer::WlPointer, event: wl_pointer::Event, _: &(),
        _: &Connection, _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { surface_x, surface_y, .. } => {
                state.pending_events.push_back(Event::Mouse(MouseEvent::CursorEntered));
                state.push_pointer_motion(surface_x, surface_y);
            }
            wl_pointer::Event::Leave { .. } => {
                state.pending_events.push_back(Event::Mouse(MouseEvent::CursorLeft));
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                state.push_pointer_motion(surface_x, surface_y);
            }
            wl_pointer::Event::Button { button, state: button_state, .. } => {
                let event = match button_state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => MouseEvent::ButtonPressed {
                        button: wayland_button_to_mouse_button(button),
                        modifiers: state.modifiers,
                    },
                    WEnum::Value(wl_pointer::ButtonState::Released) => MouseEvent::ButtonReleased {
                        button: wayland_button_to_mouse_button(button),
                        modifiers: state.modifiers,
                    },
                    WEnum::Value(_) | WEnum::Unknown(_) => return,
                };
                state.pending_events.push_back(Event::Mouse(event));
            }
            wl_pointer::Event::Axis { axis, value, .. } => match axis {
                WEnum::Value(wl_pointer::Axis::HorizontalScroll) => {
                    state.pending_axis_frame.add_horizontal(value as f32);
                }
                WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
                    state.pending_axis_frame.add_vertical(value as f32);
                }
                WEnum::Value(_) | WEnum::Unknown(_) => {}
            },
            wl_pointer::Event::Frame => {
                state.flush_axis_frame();
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self, _keyboard: &wl_keyboard::WlKeyboard, event: wl_keyboard::Event, _: &(),
        _: &Connection, _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                let WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) = format else {
                    state.keyboard_state = None;
                    return;
                };
                let Ok(size) = usize::try_from(size) else {
                    state.keyboard_state = None;
                    return;
                };
                let keymap = unsafe {
                    xkb::Keymap::new_from_fd(
                        &state.keyboard_context,
                        fd,
                        size,
                        xkb::KEYMAP_FORMAT_TEXT_V1,
                        xkb::COMPILE_NO_FLAGS,
                    )
                };
                let Ok(Some(keymap)) = keymap else {
                    state.keyboard_state = None;
                    return;
                };
                let xkb_state = xkb::State::new(&keymap);
                state.modifiers = xkb_modifiers_to_keyboard_modifiers(&xkb_state);
                state.keyboard_state = Some(XkbKeyboardState { _keymap: keymap, state: xkb_state });
            }
            wl_keyboard::Event::Enter { .. } => {
                state.pending_events.push_back(Event::Window(WindowEvent::Focused));
            }
            wl_keyboard::Event::Leave { .. } => {
                state.pending_events.push_back(Event::Window(WindowEvent::Unfocused));
            }
            wl_keyboard::Event::Key { key, state: key_state, .. } => {
                let Some(keyboard_state) = state.keyboard_state.as_mut() else {
                    return;
                };
                let keycode = xkb::Keycode::new(key + 8);
                let text = keyboard_state.state.key_get_utf8(keycode);
                let keysym = keyboard_state.state.key_get_one_sym(keycode);
                let (event_state, direction, repeat) = match key_state {
                    WEnum::Value(wl_keyboard::KeyState::Pressed) => {
                        (KeyState::Down, xkb::KeyDirection::Down, false)
                    }
                    WEnum::Value(wl_keyboard::KeyState::Released) => {
                        (KeyState::Up, xkb::KeyDirection::Up, false)
                    }
                    WEnum::Value(wl_keyboard::KeyState::Repeated) => {
                        (KeyState::Down, xkb::KeyDirection::Down, true)
                    }
                    WEnum::Value(_) | WEnum::Unknown(_) => return,
                };
                if !repeat {
                    keyboard_state.state.update_key(keycode, direction);
                }
                state.modifiers = xkb_modifiers_to_keyboard_modifiers(&keyboard_state.state);
                state.pending_events.push_back(Event::Keyboard(KeyboardEvent {
                    state: event_state,
                    key: xkb_keysym_to_key(u32::from(keysym), &text),
                    code: wayland_evdev_key_to_code(key),
                    location: wayland_evdev_key_location(key),
                    modifiers: state.modifiers,
                    repeat,
                    is_composing: false,
                }));
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(keyboard_state) = state.keyboard_state.as_mut() {
                    keyboard_state.state.update_mask(
                        mods_depressed,
                        mods_latched,
                        mods_locked,
                        0,
                        0,
                        group,
                    );
                    state.modifiers = xkb_modifiers_to_keyboard_modifiers(&keyboard_state.state);
                }
            }
            wl_keyboard::Event::RepeatInfo { .. } => {}
            _ => {}
        }
    }
}

delegate_noop!(WaylandState: ignore wl_compositor::WlCompositor);
delegate_noop!(WaylandState: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(WaylandState: ignore wl_shm::WlShm);
delegate_noop!(WaylandState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(WaylandState: ignore wl_surface::WlSurface);
delegate_noop!(WaylandState: ignore wl_subsurface::WlSubsurface);

struct EventLoop<H: WindowHandler + 'static> {
    window: WindowInner,
    handler: H,
    parent_handle: ParentHandle,
    frame_interval: Duration,
    running: bool,
}

impl<H: WindowHandler + 'static> EventLoop<H> {
    fn new(window: WindowInner, handler: H, parent_handle: ParentHandle) -> Self {
        Self {
            window,
            handler,
            parent_handle,
            frame_interval: Duration::from_millis(15),
            running: false,
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let mut last_frame = Instant::now();
        self.running = true;
        while self.running {
            let next_frame = last_frame + self.frame_interval;
            self.window.dispatch_pending_events()?;
            self.dispatch_pending_window_events();

            if Instant::now() >= next_frame {
                self.handler.on_frame(&mut crate::Window::new(crate::linux::Window::Wayland(
                    Window { inner: &self.window },
                )));
                self.window.surface.commit();
                self.window.connection.flush()?;
                last_frame = Instant::max(next_frame, Instant::now() - self.frame_interval);
            }

            if self.parent_handle.parent_did_drop() || self.window.close_requested.get() {
                self.handle_must_close();
                self.window.close_requested.set(false);
            }

            let sleep_for = next_frame
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| Duration::from_millis(1));
            thread::sleep(sleep_for.min(Duration::from_millis(15)));
        }
        Ok(())
    }

    fn handle_must_close(&mut self) {
        self.handler.on_event(
            &mut crate::Window::new(crate::linux::Window::Wayland(Window { inner: &self.window })),
            Event::Window(WindowEvent::WillClose),
        );
        self.running = false;
    }

    fn dispatch_pending_window_events(&mut self) {
        for event in self.window.take_pending_events() {
            self.handler.on_event(
                &mut crate::Window::new(crate::linux::Window::Wayland(Window {
                    inner: &self.window,
                })),
                event,
            );
        }
    }
}

fn foreign_connection(display: usize) -> Result<Connection, Box<dyn Error>> {
    let display = display as *mut _;
    let backend = unsafe { wayland_client::backend::Backend::from_foreign_display(display) };
    Ok(Connection::from_backend(backend))
}

fn create_borrowed_parent_subsurface(
    connection: &Connection, subcompositor: &wl_subcompositor::WlSubcompositor,
    surface: &wl_surface::WlSurface, parent_surface: usize, qh: &QueueHandle<WaylandState>,
) -> Result<wl_subsurface::WlSubsurface, Box<dyn Error>> {
    let parent_id = unsafe {
        ObjectId::from_ptr(wl_surface::WlSurface::interface(), parent_surface as *mut _)?
    };
    let child_id = connection.backend().send_request(
        Message {
            sender_id: subcompositor.id(),
            opcode: 1,
            args: smallvec![
                Argument::NewId(ObjectId::null()),
                Argument::Object(surface.id()),
                Argument::Object(parent_id),
            ],
        },
        Some(qh.make_data::<wl_subsurface::WlSubsurface, ()>(())),
        Some((wl_subsurface::WlSubsurface::interface(), 1)),
    )?;
    Ok(wl_subsurface::WlSubsurface::from_id(connection, child_id)?)
}

fn raw_wayland_display(display: usize) -> RawDisplayHandle {
    let mut handle = WaylandDisplayHandle::empty();
    handle.display = display as *mut c_void;
    RawDisplayHandle::Wayland(handle)
}

fn raw_wayland_window(surface: usize) -> RawWindowHandle {
    let mut handle = WaylandWindowHandle::empty();
    handle.surface = surface as *mut c_void;
    RawWindowHandle::Wayland(handle)
}

fn wayland_button_to_mouse_button(button: u32) -> MouseButton {
    match button {
        0x110 => MouseButton::Left,
        0x111 => MouseButton::Right,
        0x112 => MouseButton::Middle,
        0x113 => MouseButton::Back,
        0x114 => MouseButton::Forward,
        value => MouseButton::Other(u8::try_from(value & 0xff).unwrap_or(u8::MAX)),
    }
}

fn wayland_evdev_key_to_code(key: u32) -> Code {
    match key {
        1 => Code::Escape,
        2 => Code::Digit1,
        3 => Code::Digit2,
        4 => Code::Digit3,
        5 => Code::Digit4,
        6 => Code::Digit5,
        7 => Code::Digit6,
        8 => Code::Digit7,
        9 => Code::Digit8,
        10 => Code::Digit9,
        11 => Code::Digit0,
        12 => Code::Minus,
        13 => Code::Equal,
        14 => Code::Backspace,
        15 => Code::Tab,
        16 => Code::KeyQ,
        17 => Code::KeyW,
        18 => Code::KeyE,
        19 => Code::KeyR,
        20 => Code::KeyT,
        21 => Code::KeyY,
        22 => Code::KeyU,
        23 => Code::KeyI,
        24 => Code::KeyO,
        25 => Code::KeyP,
        26 => Code::BracketLeft,
        27 => Code::BracketRight,
        28 => Code::Enter,
        29 => Code::ControlLeft,
        30 => Code::KeyA,
        31 => Code::KeyS,
        32 => Code::KeyD,
        33 => Code::KeyF,
        34 => Code::KeyG,
        35 => Code::KeyH,
        36 => Code::KeyJ,
        37 => Code::KeyK,
        38 => Code::KeyL,
        39 => Code::Semicolon,
        40 => Code::Quote,
        41 => Code::Backquote,
        42 => Code::ShiftLeft,
        43 => Code::Backslash,
        44 => Code::KeyZ,
        45 => Code::KeyX,
        46 => Code::KeyC,
        47 => Code::KeyV,
        48 => Code::KeyB,
        49 => Code::KeyN,
        50 => Code::KeyM,
        51 => Code::Comma,
        52 => Code::Period,
        53 => Code::Slash,
        54 => Code::ShiftRight,
        55 => Code::NumpadMultiply,
        56 => Code::AltLeft,
        57 => Code::Space,
        58 => Code::CapsLock,
        59 => Code::F1,
        60 => Code::F2,
        61 => Code::F3,
        62 => Code::F4,
        63 => Code::F5,
        64 => Code::F6,
        65 => Code::F7,
        66 => Code::F8,
        67 => Code::F9,
        68 => Code::F10,
        69 => Code::NumLock,
        70 => Code::ScrollLock,
        71 => Code::Numpad7,
        72 => Code::Numpad8,
        73 => Code::Numpad9,
        74 => Code::NumpadSubtract,
        75 => Code::Numpad4,
        76 => Code::Numpad5,
        77 => Code::Numpad6,
        78 => Code::NumpadAdd,
        79 => Code::Numpad1,
        80 => Code::Numpad2,
        81 => Code::Numpad3,
        82 => Code::Numpad0,
        83 => Code::NumpadDecimal,
        87 => Code::F11,
        88 => Code::F12,
        96 => Code::NumpadEnter,
        97 => Code::ControlRight,
        98 => Code::NumpadDivide,
        99 => Code::PrintScreen,
        100 => Code::AltRight,
        102 => Code::Home,
        103 => Code::ArrowUp,
        104 => Code::PageUp,
        105 => Code::ArrowLeft,
        106 => Code::ArrowRight,
        107 => Code::End,
        108 => Code::ArrowDown,
        109 => Code::PageDown,
        110 => Code::Insert,
        111 => Code::Delete,
        125 => Code::MetaLeft,
        126 => Code::MetaRight,
        127 => Code::ContextMenu,
        _ => Code::Unidentified,
    }
}

fn wayland_evdev_key_location(key: u32) -> Location {
    match key {
        42 | 29 | 56 | 125 => Location::Left,
        54 | 97 | 100 | 126 => Location::Right,
        55 | 71..=83 | 96 | 98 => Location::Numpad,
        _ => Location::Standard,
    }
}

fn xkb_keysym_to_key(keysym: u32, text: &str) -> Key {
    use xkb::keysyms;

    match keysym {
        keysyms::KEY_Escape => Key::Escape,
        keysyms::KEY_Return | keysyms::KEY_KP_Enter => Key::Enter,
        keysyms::KEY_Tab | keysyms::KEY_KP_Tab => Key::Tab,
        keysyms::KEY_BackSpace => Key::Backspace,
        keysyms::KEY_Delete | keysyms::KEY_KP_Delete => Key::Delete,
        keysyms::KEY_Insert | keysyms::KEY_KP_Insert => Key::Insert,
        keysyms::KEY_Home | keysyms::KEY_KP_Home => Key::Home,
        keysyms::KEY_End | keysyms::KEY_KP_End => Key::End,
        keysyms::KEY_Page_Up | keysyms::KEY_KP_Page_Up => Key::PageUp,
        keysyms::KEY_Page_Down | keysyms::KEY_KP_Page_Down => Key::PageDown,
        keysyms::KEY_Left | keysyms::KEY_KP_Left => Key::ArrowLeft,
        keysyms::KEY_Right | keysyms::KEY_KP_Right => Key::ArrowRight,
        keysyms::KEY_Up | keysyms::KEY_KP_Up => Key::ArrowUp,
        keysyms::KEY_Down | keysyms::KEY_KP_Down => Key::ArrowDown,
        keysyms::KEY_Shift_L | keysyms::KEY_Shift_R => Key::Shift,
        keysyms::KEY_Control_L | keysyms::KEY_Control_R => Key::Control,
        keysyms::KEY_Alt_L | keysyms::KEY_Alt_R => Key::Alt,
        keysyms::KEY_Super_L | keysyms::KEY_Super_R => Key::Super,
        keysyms::KEY_Caps_Lock => Key::CapsLock,
        keysyms::KEY_Num_Lock => Key::NumLock,
        keysyms::KEY_ISO_Level3_Shift => Key::AltGraph,
        keysyms::KEY_F1 => Key::F1,
        keysyms::KEY_F2 => Key::F2,
        keysyms::KEY_F3 => Key::F3,
        keysyms::KEY_F4 => Key::F4,
        keysyms::KEY_F5 => Key::F5,
        keysyms::KEY_F6 => Key::F6,
        keysyms::KEY_F7 => Key::F7,
        keysyms::KEY_F8 => Key::F8,
        keysyms::KEY_F9 => Key::F9,
        keysyms::KEY_F10 => Key::F10,
        keysyms::KEY_F11 => Key::F11,
        keysyms::KEY_F12 => Key::F12,
        _ if !text.is_empty() => Key::Character(text.to_owned()),
        _ => Key::Unidentified,
    }
}

fn xkb_modifiers_to_keyboard_modifiers(state: &xkb::State) -> Modifiers {
    let mut modifiers = Modifiers::default();
    if state.mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE) {
        modifiers |= Modifiers::ALT;
    }
    if state.mod_name_is_active(xkb::MOD_NAME_ISO_LEVEL3_SHIFT, xkb::STATE_MODS_EFFECTIVE) {
        modifiers |= Modifiers::ALT_GRAPH;
    }
    if state.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_EFFECTIVE) {
        modifiers |= Modifiers::CAPS_LOCK;
    }
    if state.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE) {
        modifiers |= Modifiers::CONTROL;
    }
    if state.mod_name_is_active(xkb::MOD_NAME_NUM, xkb::STATE_MODS_EFFECTIVE) {
        modifiers |= Modifiers::NUM_LOCK;
    }
    if state.mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE) {
        modifiers |= Modifiers::SHIFT;
    }
    if state.mod_name_is_active(xkb::MOD_NAME_LOGO, xkb::STATE_MODS_EFFECTIVE) {
        modifiers |= Modifiers::SUPER;
    }
    modifiers
}

fn software_frame_byte_len(width: u32, height: u32) -> Result<usize, Box<dyn Error>> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or("Wayland software frame pixel count overflow")?;
    usize::try_from(pixels.checked_mul(4).ok_or("Wayland software frame byte count overflow")?)
        .map_err(Into::into)
}

fn write_frame_file(file: &mut File, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    file.seek(SeekFrom::Start(0))?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyboard_types::{Code, Key};
    use xkbcommon::xkb;

    #[test]
    fn maps_linux_pointer_buttons_to_baseview_buttons() {
        assert_eq!(wayland_button_to_mouse_button(0x110), MouseButton::Left);
        assert_eq!(wayland_button_to_mouse_button(0x111), MouseButton::Right);
        assert_eq!(wayland_button_to_mouse_button(0x112), MouseButton::Middle);
        assert_eq!(wayland_button_to_mouse_button(0x113), MouseButton::Back);
        assert_eq!(wayland_button_to_mouse_button(0x114), MouseButton::Forward);
        assert_eq!(wayland_button_to_mouse_button(0x120), MouseButton::Other(0x20));
        assert_eq!(wayland_button_to_mouse_button(0x1ff), MouseButton::Other(u8::MAX));
    }

    #[test]
    fn axis_frame_emits_single_combined_pixel_scroll_event() {
        let mut frame = AxisFrame::default();
        frame.add_horizontal(12.5);
        frame.add_vertical(-3.0);

        assert_eq!(frame.take_scroll_delta(), Some(ScrollDelta::Pixels { x: 12.5, y: -3.0 }));
        assert_eq!(frame.take_scroll_delta(), None);
    }

    #[test]
    fn maps_common_wayland_evdev_codes_to_dom_codes() {
        assert_eq!(wayland_evdev_key_to_code(1), Code::Escape);
        assert_eq!(wayland_evdev_key_to_code(30), Code::KeyA);
        assert_eq!(wayland_evdev_key_to_code(42), Code::ShiftLeft);
        assert_eq!(wayland_evdev_key_to_code(105), Code::ArrowLeft);
        assert_eq!(wayland_evdev_key_to_code(999), Code::Unidentified);
    }

    #[test]
    fn maps_common_xkb_keysyms_to_dom_keys() {
        assert_eq!(xkb_keysym_to_key(xkb::keysyms::KEY_Escape, ""), Key::Escape);
        assert_eq!(xkb_keysym_to_key(xkb::keysyms::KEY_Return, ""), Key::Enter);
        assert_eq!(xkb_keysym_to_key(xkb::keysyms::KEY_Left, ""), Key::ArrowLeft);
        assert_eq!(
            xkb_keysym_to_key(xkb::keysyms::KEY_NoSymbol, "é"),
            Key::Character("é".to_owned())
        );
        assert_eq!(xkb_keysym_to_key(xkb::keysyms::KEY_NoSymbol, ""), Key::Unidentified);
    }
}
