#![deny(unsafe_code)]
//! `Winit`-backed desktop host adapter for `Hawk2UI`.

mod gpu_frame;
mod runtime;
mod software_frame;

use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};

use hawk2ui_api::Diagnostic;
use hawk2ui_host::{
    ClipboardCapability, DesktopDialogFileFilter, DesktopDialogLevel, DesktopDialogRequest,
    DesktopDialogResponse, DesktopHostAdapter, DesktopHostEvent, DesktopWindowConfig,
    HostPlatformHandle, KeyboardInput, LinuxWindowSystem, PointerInput, RepaintRequest,
    SurfaceClipboardRequest, SurfaceMetrics, SurfaceOwnership, WindowMode,
};
use hawk2ui_platform::{
    AudioCueBinding, AudioPlaybackSink, ClipboardAccess, DialogKind, DialogRequest,
    GlobalShortcutSink, HostDialogResponse, NotificationBinding, NotificationSink,
    PlatformBackendError, PlatformDiagnostic, PlatformHostBackend, PlatformOperation,
    ShortcutBinding,
};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};

pub use runtime::{
    WinitDesktopReload, WinitDesktopReloadKind, WinitDesktopReloadReport, WinitDesktopRuntime,
    WinitDesktopRuntimeConfig, WinitDesktopRuntimeSummary, WinitDesktopRuntimeSurfaceState,
    WinitDesktopScriptEntry, WinitPresentationBackend, WinitPresentationBackendUsed,
};
pub use software_frame::{
    DesktopErrorOverlay, SoftwareFrame, SoftwareFrameRenderer, physical_frame_size,
};

/// The canonical Cargo package name for this crate.
pub const CRATE_NAME: &str = "hawk2ui-host-winit";

/// Returns the canonical Cargo package name for diagnostics and conformance checks.
#[must_use]
pub const fn crate_name() -> &'static str {
    CRATE_NAME
}

/// Winit platform fixture used by headless tests and adapter validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitPlatformFixture {
    handle: HostPlatformHandle,
}

impl WinitPlatformFixture {
    /// Creates a Linux fixture for the selected window system.
    #[must_use]
    pub const fn linux(window_system: LinuxWindowSystem) -> Self {
        let handle = match window_system {
            LinuxWindowSystem::Wayland => HostPlatformHandle::linux_wayland(1, 2),
            LinuxWindowSystem::X11 | LinuxWindowSystem::XWayland => {
                HostPlatformHandle::linux_x11(1, 3)
            }
            LinuxWindowSystem::Xcb => HostPlatformHandle::linux_xcb(4, 5),
        };
        Self { handle }
    }

    /// Creates a Windows HWND fixture.
    #[must_use]
    pub const fn windows() -> Self {
        Self {
            handle: HostPlatformHandle::windows_hwnd(6),
        }
    }

    /// Creates a macOS `NSWindow` fixture.
    #[must_use]
    pub const fn macos() -> Self {
        Self {
            handle: HostPlatformHandle::macos_ns_window(7),
        }
    }

    /// Returns the platform handle.
    #[must_use]
    pub const fn handle(&self) -> HostPlatformHandle {
        self.handle
    }
}

/// Winit host capability report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitHostCapabilities {
    flags: u16,
}

impl WinitHostCapabilities {
    /// Returns desktop Winit capabilities.
    #[must_use]
    pub const fn desktop() -> Self {
        Self {
            flags: WINIT_CAP_OWNS_WINDOW
                | WINIT_CAP_CLOSE
                | WINIT_CAP_MINIMIZE
                | WINIT_CAP_MAXIMIZE
                | WINIT_CAP_FULLSCREEN
                | WINIT_CAP_FOCUS
                | WINIT_CAP_KEYBOARD
                | WINIT_CAP_POINTER
                | WINIT_CAP_CLIPBOARD
                | WINIT_CAP_RESIZE
                | WINIT_CAP_REPAINT,
        }
    }

    /// Returns whether Winit owns the desktop window.
    #[must_use]
    pub const fn owns_window(&self) -> bool {
        self.supports(WinitCapability::OwnsWindow)
    }

    /// Returns whether a capability is supported.
    #[must_use]
    pub const fn supports(&self, capability: WinitCapability) -> bool {
        self.flags & capability.flag() != 0
    }
}

/// Single Winit desktop capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinitCapability {
    /// Owns the desktop top-level window.
    OwnsWindow,
    /// Handles close requests.
    Close,
    /// Handles minimize requests.
    Minimize,
    /// Handles maximize requests.
    Maximize,
    /// Handles fullscreen requests.
    Fullscreen,
    /// Handles focus events.
    Focus,
    /// Handles keyboard input.
    Keyboard,
    /// Handles pointer input.
    Pointer,
    /// Handles clipboard availability.
    Clipboard,
    /// Handles resize events.
    Resize,
    /// Handles repaint requests.
    Repaint,
}

impl WinitCapability {
    const fn flag(self) -> u16 {
        match self {
            Self::OwnsWindow => WINIT_CAP_OWNS_WINDOW,
            Self::Close => WINIT_CAP_CLOSE,
            Self::Minimize => WINIT_CAP_MINIMIZE,
            Self::Maximize => WINIT_CAP_MAXIMIZE,
            Self::Fullscreen => WINIT_CAP_FULLSCREEN,
            Self::Focus => WINIT_CAP_FOCUS,
            Self::Keyboard => WINIT_CAP_KEYBOARD,
            Self::Pointer => WINIT_CAP_POINTER,
            Self::Clipboard => WINIT_CAP_CLIPBOARD,
            Self::Resize => WINIT_CAP_RESIZE,
            Self::Repaint => WINIT_CAP_REPAINT,
        }
    }
}

/// Host events produced from a single native `winit` window event.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitTranslatedEvent {
    /// Desktop host events emitted by the translation.
    pub events: Vec<DesktopHostEvent>,
    /// Whether this event requires a redraw request.
    pub requires_redraw: bool,
    /// Whether this event requests native event-loop exit.
    pub requests_close: bool,
}

impl WinitTranslatedEvent {
    fn new(events: Vec<DesktopHostEvent>) -> Self {
        Self {
            events,
            requires_redraw: false,
            requests_close: false,
        }
    }

    fn redraw(mut self) -> Self {
        self.requires_redraw = true;
        self
    }

    fn close(mut self) -> Self {
        self.requests_close = true;
        self
    }
}

/// Stateful translator from native `winit` events into `Hawk2UI` desktop host events.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitEventTranslator {
    metrics: SurfaceMetrics,
    last_pointer_position: (f64, f64),
}

/// Response from a native clipboard request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WinitClipboardResponse {
    /// Text read from the native clipboard.
    Text(String),
    /// Text was written to the native clipboard.
    Written,
    /// Native clipboard text was cleared.
    Cleared,
}

/// Backend boundary for native desktop clipboard access.
pub trait WinitClipboardBackend {
    /// Reads clipboard text.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be read.
    fn read_text(&mut self) -> Result<String, WinitHostError>;

    /// Writes clipboard text.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be written.
    fn write_text(&mut self, text: String) -> Result<(), WinitHostError>;

    /// Clears clipboard text when supported.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be cleared.
    fn clear_text(&mut self) -> Result<(), WinitHostError>;
}

/// Production native clipboard backend backed by the operating-system clipboard.
pub struct ArboardClipboardBackend {
    clipboard: arboard::Clipboard,
}

impl ArboardClipboardBackend {
    /// Opens the native clipboard.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform clipboard cannot be opened.
    pub fn new() -> Result<Self, WinitHostError> {
        let clipboard = arboard::Clipboard::new().map_err(|error| {
            WinitHostError::new(
                "desktop.clipboard.open-failed",
                format!("failed to open native clipboard: {error}"),
            )
        })?;
        Ok(Self { clipboard })
    }
}

impl WinitClipboardBackend for ArboardClipboardBackend {
    fn read_text(&mut self) -> Result<String, WinitHostError> {
        self.clipboard.get_text().map_err(|error| {
            WinitHostError::new(
                "desktop.clipboard.read-failed",
                format!("failed to read native clipboard text: {error}"),
            )
        })
    }

    fn write_text(&mut self, text: String) -> Result<(), WinitHostError> {
        self.clipboard.set_text(text).map_err(|error| {
            WinitHostError::new(
                "desktop.clipboard.write-failed",
                format!("failed to write native clipboard text: {error}"),
            )
        })
    }

    fn clear_text(&mut self) -> Result<(), WinitHostError> {
        self.clipboard.clear().map_err(|error| {
            WinitHostError::new(
                "desktop.clipboard.clear-failed",
                format!("failed to clear native clipboard text: {error}"),
            )
        })
    }
}

/// Capability-checked native clipboard bridge for Winit desktop hosts.
#[derive(Clone, Debug)]
pub struct WinitClipboardBridge<B> {
    capability: ClipboardCapability,
    backend: B,
}

impl<B: WinitClipboardBackend> WinitClipboardBridge<B> {
    /// Creates a clipboard bridge from a host capability and backend.
    #[must_use]
    pub const fn new(capability: ClipboardCapability, backend: B) -> Self {
        Self {
            capability,
            backend,
        }
    }

    /// Executes a clipboard request against the native backend.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when capability policy denies the operation or the native
    /// clipboard backend fails.
    pub fn handle_request(
        &mut self,
        request: SurfaceClipboardRequest,
    ) -> Result<WinitClipboardResponse, WinitHostError> {
        match request {
            SurfaceClipboardRequest::Read => {
                self.ensure_can_read()?;
                self.backend.read_text().map(WinitClipboardResponse::Text)
            }
            SurfaceClipboardRequest::Write(text) => {
                self.ensure_can_write()?;
                self.backend.write_text(text)?;
                Ok(WinitClipboardResponse::Written)
            }
            SurfaceClipboardRequest::Clear => {
                self.ensure_can_write()?;
                self.backend.clear_text()?;
                Ok(WinitClipboardResponse::Cleared)
            }
        }
    }

    fn ensure_can_read(&self) -> Result<(), WinitHostError> {
        match self.capability {
            ClipboardCapability::Read | ClipboardCapability::ReadWrite => Ok(()),
            ClipboardCapability::None | ClipboardCapability::Write => Err(WinitHostError::new(
                "desktop.clipboard.read-denied",
                "desktop clipboard read requires read capability",
            )),
        }
    }

    fn ensure_can_write(&self) -> Result<(), WinitHostError> {
        match self.capability {
            ClipboardCapability::Write | ClipboardCapability::ReadWrite => Ok(()),
            ClipboardCapability::None | ClipboardCapability::Read => Err(WinitHostError::new(
                "desktop.clipboard.write-denied",
                "desktop clipboard write requires write capability",
            )),
        }
    }
}

/// Backend boundary for native desktop dialog access.
pub trait WinitDialogBackend {
    /// Shows a native message dialog.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform dialog backend fails.
    fn show_message(
        &mut self,
        title: String,
        message: String,
        level: DesktopDialogLevel,
    ) -> Result<(), WinitHostError>;

    /// Shows a native single-file open dialog.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform dialog backend fails.
    fn open_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError>;

    /// Shows a native save-file dialog.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform dialog backend fails.
    fn save_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        file_name: Option<String>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError>;
}

/// Production native dialog backend backed by platform dialogs.
#[derive(Clone, Copy, Debug, Default)]
pub struct RfdDialogBackend;

impl RfdDialogBackend {
    /// Creates the default native dialog backend.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl WinitDialogBackend for RfdDialogBackend {
    fn show_message(
        &mut self,
        title: String,
        message: String,
        level: DesktopDialogLevel,
    ) -> Result<(), WinitHostError> {
        let _result = rfd::MessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_level(rfd_message_level(level))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        Ok(())
    }

    fn open_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError> {
        let dialog = configure_file_dialog(title, directory.as_deref(), filters);
        Ok(dialog.pick_file())
    }

    fn save_file(
        &mut self,
        title: String,
        directory: Option<PathBuf>,
        file_name: Option<String>,
        filters: Vec<DesktopDialogFileFilter>,
    ) -> Result<Option<PathBuf>, WinitHostError> {
        let mut dialog = configure_file_dialog(title, directory.as_deref(), filters);
        if let Some(file_name) = file_name {
            dialog = dialog.set_file_name(file_name);
        }
        Ok(dialog.save_file())
    }
}

/// Native dialog bridge for Winit desktop hosts.
#[derive(Clone, Debug)]
pub struct WinitDialogBridge<B> {
    backend: B,
}

impl<B: WinitDialogBackend> WinitDialogBridge<B> {
    /// Creates a dialog bridge from a native backend.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Executes a native dialog request.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when request validation or the native dialog backend fails.
    pub fn handle_request(
        &mut self,
        request: DesktopDialogRequest,
    ) -> Result<DesktopDialogResponse, WinitHostError> {
        match request {
            DesktopDialogRequest::Message {
                title,
                message,
                level,
            } => {
                validate_dialog_title(&title)?;
                validate_dialog_message(&message)?;
                self.backend.show_message(title, message, level)?;
                Ok(DesktopDialogResponse::Acknowledged)
            }
            DesktopDialogRequest::OpenFile {
                title,
                directory,
                filters,
            } => {
                validate_dialog_title(&title)?;
                validate_dialog_filters(&filters)?;
                Ok(self
                    .backend
                    .open_file(title, directory, filters)?
                    .map_or(DesktopDialogResponse::Cancelled, |path| {
                        DesktopDialogResponse::SelectedFile(path)
                    }))
            }
            DesktopDialogRequest::SaveFile {
                title,
                directory,
                file_name,
                filters,
            } => {
                validate_dialog_title(&title)?;
                if let Some(file_name) = file_name.as_ref() {
                    validate_dialog_file_name(file_name)?;
                }
                validate_dialog_filters(&filters)?;
                Ok(self
                    .backend
                    .save_file(title, directory, file_name, filters)?
                    .map_or(DesktopDialogResponse::Cancelled, |path| {
                        DesktopDialogResponse::SavedFile(path)
                    }))
            }
        }
    }
}

/// Backend boundary for native desktop audio cue playback.
pub trait WinitAudioBackend {
    /// Plays a host-resolved audio source URI.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the native audio backend cannot play the source.
    fn play_source_uri(&mut self, source_uri: String) -> Result<(), WinitHostError>;
}

/// Production audio backend backed by `rodio`.
pub struct RodioAudioBackend {
    sink: rodio::MixerDeviceSink,
}

impl RodioAudioBackend {
    /// Opens the default native audio output sink.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when no usable native output device can be opened.
    pub fn new() -> Result<Self, WinitHostError> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink().map_err(|error| {
            WinitHostError::new(
                "desktop.audio.open-failed",
                format!("failed to open native audio output sink: {error}"),
            )
        })?;
        Ok(Self { sink })
    }
}

impl WinitAudioBackend for RodioAudioBackend {
    fn play_source_uri(&mut self, source_uri: String) -> Result<(), WinitHostError> {
        let path = audio_source_uri_to_path(&source_uri)?;
        let file = File::open(&path).map_err(|error| {
            WinitHostError::new(
                "desktop.audio.open-source-failed",
                format!("failed to open audio source {}: {error}", path.display()),
            )
        })?;
        let source = rodio::Decoder::try_from(file).map_err(|error| {
            WinitHostError::new(
                "desktop.audio.decode-failed",
                format!("failed to decode audio source {}: {error}", path.display()),
            )
        })?;
        self.sink.mixer().add(source);
        Ok(())
    }
}

/// Platform audio sink for policy-approved `Hawk2UI` audio cue bindings.
#[derive(Clone, Debug)]
pub struct WinitAudioSink<B> {
    backend: B,
}

impl<B: WinitAudioBackend> WinitAudioSink<B> {
    /// Creates a Winit audio sink from a native audio backend.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Returns the wrapped audio backend.
    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: WinitAudioBackend> AudioPlaybackSink for WinitAudioSink<B> {
    fn play_audio_cue(&mut self, binding: &AudioCueBinding) -> Result<(), PlatformBackendError> {
        self.backend
            .play_source_uri(binding.source_uri.clone())
            .map_err(|error| winit_error_to_platform(PlatformOperation::AudioPlayback, &error))
    }
}

/// Backend boundary for native desktop notifications.
pub trait WinitNotificationBackend {
    /// Sends a native desktop notification.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform notification service rejects the request.
    fn send_notification(&mut self, title: String, body: String) -> Result<(), WinitHostError>;
}

/// Production notification backend backed by `notify-rust` where supported.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotifyRustNotificationBackend;

impl NotifyRustNotificationBackend {
    /// Creates the default native notification backend.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl WinitNotificationBackend for NotifyRustNotificationBackend {
    fn send_notification(&mut self, title: String, body: String) -> Result<(), WinitHostError> {
        send_native_notification(&title, &body)
    }
}

/// Platform notification sink for policy-approved `Hawk2UI` notification bindings.
#[derive(Clone, Debug)]
pub struct WinitNotificationSink<B> {
    backend: B,
}

impl<B: WinitNotificationBackend> WinitNotificationSink<B> {
    /// Creates a Winit notification sink from a native notification backend.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Returns the wrapped notification backend.
    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: WinitNotificationBackend> NotificationSink for WinitNotificationSink<B> {
    fn send_notification(
        &mut self,
        binding: &NotificationBinding,
    ) -> Result<(), PlatformBackendError> {
        self.backend
            .send_notification(binding.title.clone(), binding.body.clone())
            .map_err(|error| winit_error_to_platform(PlatformOperation::NotificationSend, &error))
    }
}

/// Backend boundary for native global shortcut registration.
pub trait WinitShortcutBackend {
    /// Registers a native global shortcut mapped to a `Hawk2UI` action.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the accelerator cannot be parsed or registered.
    fn register_shortcut(
        &mut self,
        accelerator: String,
        action_id: String,
    ) -> Result<(), WinitHostError>;
}

/// Backend boundary for the Wayland global-shortcut portal.
pub trait WinitWaylandShortcutPortal {
    /// Binds a shortcut through the desktop portal.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the portal cannot bind the shortcut.
    fn bind_shortcut(
        &mut self,
        accelerator: String,
        action_id: String,
        preferred_trigger: Option<String>,
    ) -> Result<(), WinitHostError>;
}

/// Wayland shortcut backend backed by a portal implementation.
#[derive(Clone, Debug)]
pub struct WaylandPortalShortcutBackend<P> {
    portal: P,
}

impl<P: WinitWaylandShortcutPortal> WaylandPortalShortcutBackend<P> {
    /// Creates a Wayland portal shortcut backend.
    #[must_use]
    pub const fn new(portal: P) -> Self {
        Self { portal }
    }

    /// Returns the wrapped portal backend.
    #[must_use]
    pub const fn portal(&self) -> &P {
        &self.portal
    }
}

impl<P: WinitWaylandShortcutPortal> WinitShortcutBackend for WaylandPortalShortcutBackend<P> {
    fn register_shortcut(
        &mut self,
        accelerator: String,
        action_id: String,
    ) -> Result<(), WinitHostError> {
        let preferred_trigger = portal_preferred_trigger(&accelerator);
        self.portal
            .bind_shortcut(accelerator, action_id, preferred_trigger)
    }
}

/// Production Wayland global-shortcut portal backed by `ashpd`.
pub struct AshpdWaylandShortcutPortal {
    runtime: tokio::runtime::Runtime,
    portal: ashpd::desktop::global_shortcuts::GlobalShortcuts,
    session: ashpd::desktop::Session<ashpd::desktop::global_shortcuts::GlobalShortcuts>,
}

impl AshpdWaylandShortcutPortal {
    /// Opens the XDG desktop portal global-shortcut session.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the portal service or session cannot be created.
    pub fn new() -> Result<Self, WinitHostError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.shortcut.portal-runtime-failed",
                    format!("failed to create Wayland shortcut portal runtime: {error}"),
                )
            })?;
        let (portal, session) = runtime
            .block_on(async {
                let portal = ashpd::desktop::global_shortcuts::GlobalShortcuts::new().await?;
                let session = portal
                    .create_session(ashpd::desktop::CreateSessionOptions::default())
                    .await?;
                Ok::<_, ashpd::Error>((portal, session))
            })
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.shortcut.portal-open-failed",
                    format!("failed to open Wayland global shortcut portal session: {error}"),
                )
            })?;
        Ok(Self {
            runtime,
            portal,
            session,
        })
    }
}

impl WinitWaylandShortcutPortal for AshpdWaylandShortcutPortal {
    fn bind_shortcut(
        &mut self,
        accelerator: String,
        action_id: String,
        preferred_trigger: Option<String>,
    ) -> Result<(), WinitHostError> {
        let mut shortcut =
            ashpd::desktop::global_shortcuts::NewShortcut::new(&action_id, &action_id);
        shortcut = shortcut.preferred_trigger(preferred_trigger.as_deref());
        let request = self
            .runtime
            .block_on(self.portal.bind_shortcuts(
                &self.session,
                &[shortcut],
                None,
                ashpd::desktop::global_shortcuts::BindShortcutsOptions::default(),
            ))
            .map_err(|error| {
                WinitHostError::new(
                    "desktop.shortcut.portal-bind-failed",
                    format!("failed to bind Wayland shortcut {accelerator}: {error}"),
                )
            })?;
        request.response().map_err(|error| {
            WinitHostError::new(
                "desktop.shortcut.portal-response-failed",
                format!("Wayland shortcut portal rejected {accelerator}: {error}"),
            )
        })?;
        Ok(())
    }
}

/// Production global shortcut backend backed by `global-hotkey` on supported platforms.
pub struct GlobalHotkeyShortcutBackend {
    manager: global_hotkey::GlobalHotKeyManager,
    actions_by_hotkey_id: BTreeMap<u32, String>,
}

impl GlobalHotkeyShortcutBackend {
    /// Creates a native global shortcut backend.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the current platform/session cannot support global shortcuts.
    pub fn new(linux_window_system: Option<LinuxWindowSystem>) -> Result<Self, WinitHostError> {
        reject_unsupported_linux_shortcut_session(linux_window_system)?;
        let manager = global_hotkey::GlobalHotKeyManager::new().map_err(|error| {
            WinitHostError::new(
                "desktop.shortcut.open-failed",
                format!("failed to open native global shortcut manager: {error}"),
            )
        })?;
        Ok(Self {
            manager,
            actions_by_hotkey_id: BTreeMap::new(),
        })
    }

    /// Returns the action mapped to a native global-hotkey ID.
    #[must_use]
    pub fn action_for_hotkey_id(&self, hotkey_id: u32) -> Option<&str> {
        self.actions_by_hotkey_id
            .get(&hotkey_id)
            .map(String::as_str)
    }
}

impl WinitShortcutBackend for GlobalHotkeyShortcutBackend {
    fn register_shortcut(
        &mut self,
        accelerator: String,
        action_id: String,
    ) -> Result<(), WinitHostError> {
        let hotkey: global_hotkey::hotkey::HotKey = accelerator.parse().map_err(|error| {
            WinitHostError::new(
                "desktop.shortcut.parse-failed",
                format!("failed to parse global shortcut accelerator {accelerator}: {error}"),
            )
        })?;
        self.manager.register(hotkey).map_err(|error| {
            WinitHostError::new(
                "desktop.shortcut.register-failed",
                format!("failed to register global shortcut {accelerator}: {error}"),
            )
        })?;
        self.actions_by_hotkey_id.insert(hotkey.id(), action_id);
        Ok(())
    }
}

/// Native shortcut backend selector for Winit desktop hosts.
///
/// On Linux Wayland this uses the XDG desktop portal. On X11/XWayland, Windows, and macOS it uses
/// `global-hotkey`.
pub enum WinitNativeShortcutBackend {
    /// XDG desktop portal backend for native Wayland sessions.
    WaylandPortal(WaylandPortalShortcutBackend<AshpdWaylandShortcutPortal>),
    /// `global-hotkey` backend for X11/XWayland, Windows, and macOS.
    GlobalHotkey(GlobalHotkeyShortcutBackend),
}

impl WinitNativeShortcutBackend {
    /// Creates the native shortcut backend for the current Winit platform/session.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the selected native shortcut backend cannot be opened.
    pub fn new(linux_window_system: Option<LinuxWindowSystem>) -> Result<Self, WinitHostError> {
        #[cfg(target_os = "linux")]
        {
            if matches!(linux_window_system, Some(LinuxWindowSystem::Wayland)) {
                return AshpdWaylandShortcutPortal::new()
                    .map(WaylandPortalShortcutBackend::new)
                    .map(Self::WaylandPortal);
            }
        }
        GlobalHotkeyShortcutBackend::new(linux_window_system).map(Self::GlobalHotkey)
    }
}

impl WinitShortcutBackend for WinitNativeShortcutBackend {
    fn register_shortcut(
        &mut self,
        accelerator: String,
        action_id: String,
    ) -> Result<(), WinitHostError> {
        match self {
            Self::WaylandPortal(backend) => backend.register_shortcut(accelerator, action_id),
            Self::GlobalHotkey(backend) => backend.register_shortcut(accelerator, action_id),
        }
    }
}

/// Platform shortcut sink for policy-approved `Hawk2UI` shortcut bindings.
#[derive(Clone, Debug)]
pub struct WinitShortcutSink<B> {
    backend: B,
}

impl<B: WinitShortcutBackend> WinitShortcutSink<B> {
    /// Creates a Winit shortcut sink from a native shortcut backend.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Returns the wrapped shortcut backend.
    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: WinitShortcutBackend> GlobalShortcutSink for WinitShortcutSink<B> {
    fn register_shortcut(&mut self, binding: &ShortcutBinding) -> Result<(), PlatformBackendError> {
        self.backend
            .register_shortcut(binding.accelerator.clone(), binding.action_id.clone())
            .map_err(|error| {
                winit_error_to_platform(PlatformOperation::GlobalShortcutRegister, &error)
            })
    }
}

/// Platform host adapter backed by Winit desktop clipboard and dialog bridges.
#[derive(Clone, Debug)]
pub struct WinitPlatformHostBackend<C, D> {
    clipboard: WinitClipboardBridge<C>,
    dialogs: WinitDialogBridge<D>,
}

impl<C, D> WinitPlatformHostBackend<C, D>
where
    C: WinitClipboardBackend,
    D: WinitDialogBackend,
{
    /// Creates a platform host adapter from native Winit bridge backends.
    #[must_use]
    pub const fn new(
        clipboard_capability: ClipboardCapability,
        clipboard_backend: C,
        dialog_backend: D,
    ) -> Self {
        Self {
            clipboard: WinitClipboardBridge::new(clipboard_capability, clipboard_backend),
            dialogs: WinitDialogBridge::new(dialog_backend),
        }
    }
}

impl<C, D> PlatformHostBackend for WinitPlatformHostBackend<C, D>
where
    C: WinitClipboardBackend,
    D: WinitDialogBackend,
{
    fn write_clipboard_text(
        &mut self,
        access: &ClipboardAccess,
        text: String,
    ) -> Result<(), PlatformBackendError> {
        if access.operation != PlatformOperation::ClipboardWrite {
            return Err(platform_bridge_error(
                PlatformOperation::ClipboardWrite,
                "desktop.platform-clipboard.invalid-operation",
                "clipboard write bridge received a non-write access record",
            ));
        }
        self.clipboard
            .handle_request(SurfaceClipboardRequest::Write(text))
            .map(|_| ())
            .map_err(|error| winit_error_to_platform(PlatformOperation::ClipboardWrite, &error))
    }

    fn read_clipboard_text(
        &mut self,
        access: &ClipboardAccess,
    ) -> Result<Option<String>, PlatformBackendError> {
        if access.operation != PlatformOperation::ClipboardRead {
            return Err(platform_bridge_error(
                PlatformOperation::ClipboardRead,
                "desktop.platform-clipboard.invalid-operation",
                "clipboard read bridge received a non-read access record",
            ));
        }
        match self
            .clipboard
            .handle_request(SurfaceClipboardRequest::Read)
            .map_err(|error| winit_error_to_platform(PlatformOperation::ClipboardRead, &error))?
        {
            WinitClipboardResponse::Text(text) => Ok(Some(text)),
            WinitClipboardResponse::Written | WinitClipboardResponse::Cleared => {
                Err(platform_bridge_error(
                    PlatformOperation::ClipboardRead,
                    "desktop.platform-clipboard.invalid-response",
                    "clipboard read bridge returned a non-text response",
                ))
            }
        }
    }

    fn open_dialog(
        &mut self,
        request: &DialogRequest,
    ) -> Result<HostDialogResponse, PlatformBackendError> {
        let operation = match request.kind {
            DialogKind::Message => PlatformOperation::DialogOpen,
            DialogKind::FilePicker => PlatformOperation::FilePickerOpen,
        };
        let native_request = match request.kind {
            DialogKind::Message => DesktopDialogRequest::Message {
                title: "Hawk2UI".into(),
                message: "The application requested a host dialog.".into(),
                level: DesktopDialogLevel::Info,
            },
            DialogKind::FilePicker => DesktopDialogRequest::OpenFile {
                title: "Open file".into(),
                directory: None,
                filters: Vec::new(),
            },
        };
        let response = self
            .dialogs
            .handle_request(native_request)
            .map_err(|error| winit_error_to_platform(operation, &error))?;
        Ok(match response {
            DesktopDialogResponse::Acknowledged => {
                HostDialogResponse::accepted(request.kind, std::iter::empty::<String>())
            }
            DesktopDialogResponse::SelectedFile(path) | DesktopDialogResponse::SavedFile(path) => {
                HostDialogResponse::accepted(request.kind, [path.to_string_lossy().into_owned()])
            }
            DesktopDialogResponse::Cancelled => HostDialogResponse::cancelled(request.kind),
        })
    }
}

fn winit_error_to_platform(
    operation: PlatformOperation,
    error: &WinitHostError,
) -> PlatformBackendError {
    platform_bridge_error(operation, error.rule(), error.message())
}

fn platform_bridge_error(
    operation: PlatformOperation,
    rule: impl Into<String>,
    message: impl Into<String>,
) -> PlatformBackendError {
    PlatformBackendError {
        operation,
        diagnostic: PlatformDiagnostic::error(rule, message),
    }
}

fn audio_source_uri_to_path(source_uri: &str) -> Result<PathBuf, WinitHostError> {
    if let Ok(url) = url::Url::parse(source_uri) {
        if url.scheme() != "file" {
            return Err(WinitHostError::new(
                "desktop.audio.unsupported-source-uri",
                format!("audio source URI must use file:// scheme: {source_uri}"),
            ));
        }
        return url.to_file_path().map_err(|()| {
            WinitHostError::new(
                "desktop.audio.invalid-source-uri",
                format!("file audio source URI cannot be converted to a local path: {source_uri}"),
            )
        });
    }

    let path = PathBuf::from(source_uri);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(WinitHostError::new(
            "desktop.audio.invalid-source-uri",
            format!("audio source path must be absolute or file:// URI: {source_uri}"),
        ))
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn send_native_notification(title: &str, body: &str) -> Result<(), WinitHostError> {
    notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show()
        .map(|_| ())
        .map_err(|error| {
            WinitHostError::new(
                "desktop.notification.send-failed",
                format!("failed to send native notification: {error}"),
            )
        })
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
fn send_native_notification(_title: &str, _body: &str) -> Result<(), WinitHostError> {
    Err(WinitHostError::new(
        "desktop.notification.unsupported-platform",
        "native desktop notifications are not implemented for this platform",
    ))
}

fn reject_unsupported_linux_shortcut_session(
    linux_window_system: Option<LinuxWindowSystem>,
) -> Result<(), WinitHostError> {
    #[cfg(target_os = "linux")]
    {
        if matches!(linux_window_system, Some(LinuxWindowSystem::Wayland)) {
            return Err(WinitHostError::new(
                "desktop.shortcut.wayland-unsupported",
                "global-hotkey 0.8 supports Linux global shortcuts through X11 only; native Wayland shortcut portals are not available through this backend",
            ));
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = linux_window_system;
    Ok(())
}

fn portal_preferred_trigger(accelerator: &str) -> Option<String> {
    let tokens = accelerator
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let (key, modifiers) = tokens.split_last()?;
    let mut trigger = String::new();
    for modifier in modifiers {
        match modifier.to_ascii_uppercase().as_str() {
            "CONTROL" | "CTRL" | "COMMANDORCONTROL" | "COMMANDORCTRL" | "CMDORCTRL"
            | "CMDORCONTROL" => trigger.push_str("<Control>"),
            "ALT" | "OPTION" => trigger.push_str("<Alt>"),
            "SHIFT" => trigger.push_str("<Shift>"),
            "SUPER" | "META" | "COMMAND" | "CMD" => trigger.push_str("<Super>"),
            _ => return None,
        }
    }
    trigger.push_str(&portal_trigger_key(key)?);
    Some(trigger)
}

fn portal_trigger_key(key: &str) -> Option<String> {
    let upper = key.to_ascii_uppercase();
    if upper.len() == 1 && upper.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Some(upper);
    }
    if let Some(rest) = upper.strip_prefix("KEY")
        && rest.len() == 1
        && rest.chars().all(|ch| ch.is_ascii_alphabetic())
    {
        return Some(rest.to_owned());
    }
    if let Some(rest) = upper.strip_prefix("DIGIT")
        && rest.len() == 1
        && rest.chars().all(|ch| ch.is_ascii_digit())
    {
        return Some(rest.to_owned());
    }
    if upper.starts_with('F') && upper[1..].chars().all(|ch| ch.is_ascii_digit()) {
        return Some(upper);
    }
    None
}

impl WinitEventTranslator {
    /// Creates a translator with the current surface metrics.
    #[must_use]
    pub const fn new(metrics: SurfaceMetrics) -> Self {
        Self {
            metrics,
            last_pointer_position: (0.0, 0.0),
        }
    }

    /// Returns the latest metrics observed from native resize/DPI events.
    #[must_use]
    pub const fn metrics(&self) -> SurfaceMetrics {
        self.metrics
    }

    /// Translates a native `winit` window event into host events.
    #[must_use]
    pub fn translate(&mut self, event: &WindowEvent) -> WinitTranslatedEvent {
        match event {
            WindowEvent::Resized(size) => self.translate_resize(size.width, size.height),
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::CloseRequested(
                    "native close requested".into(),
                )])
                .close()
            }
            WindowEvent::DroppedFile(path) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FileDragDrop(format!(
                    "dropped:{}",
                    path.to_string_lossy()
                ))])
            }
            WindowEvent::HoveredFile(path) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FileDragDrop(format!(
                    "hovered:{}",
                    path.to_string_lossy()
                ))])
            }
            WindowEvent::HoveredFileCancelled => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FileDragDrop(
                    "hover-cancelled".into(),
                )])
            }
            WindowEvent::Focused(focused) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::FocusChanged(*focused)])
            }
            WindowEvent::KeyboardInput { event, .. } => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::KeyboardInput(
                    KeyboardInput::new(
                        format!("{:?}", event.logical_key),
                        event.state == ElementState::Pressed,
                    ),
                )])
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::KeyboardInput(
                    KeyboardInput::new(modifiers_label(*modifiers), modifiers_pressed(*modifiers)),
                )])
            }
            WindowEvent::Ime(event) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::ImeInput(ime_event_label(event))])
            }
            WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::PinchGesture { .. }
            | WindowEvent::PanGesture { .. }
            | WindowEvent::RotationGesture { .. }
            | WindowEvent::TouchpadPressure { .. }
            | WindowEvent::AxisMotion { .. }
            | WindowEvent::Touch(_) => self.translate_pointer_event(event),
            WindowEvent::Occluded(occluded) => {
                WinitTranslatedEvent::new(vec![DesktopHostEvent::WindowOcclusionChanged(*occluded)])
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.metrics.scale_factor = *scale_factor;
                WinitTranslatedEvent::new(vec![
                    DesktopHostEvent::DpiChanged(*scale_factor),
                    DesktopHostEvent::RendererTargetRecreateRequested,
                ])
                .redraw()
            }
            WindowEvent::RedrawRequested => WinitTranslatedEvent::new(Vec::new()).redraw(),
            _ => WinitTranslatedEvent::new(Vec::new()),
        }
    }

    fn translate_resize(
        &mut self,
        physical_width: u32,
        physical_height: u32,
    ) -> WinitTranslatedEvent {
        let scale = self.metrics.scale_factor;
        self.metrics = SurfaceMetrics::new(
            f64::from(physical_width) / scale,
            f64::from(physical_height) / scale,
            scale,
        );
        WinitTranslatedEvent::new(vec![
            DesktopHostEvent::Resized(self.metrics),
            DesktopHostEvent::RendererTargetRecreateRequested,
        ])
        .redraw()
    }

    fn translate_pointer_event(&mut self, event: &WindowEvent) -> WinitTranslatedEvent {
        let pointer = match event {
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x / self.metrics.scale_factor;
                let y = position.y / self.metrics.scale_factor;
                self.last_pointer_position = (x, y);
                PointerInput::new(x, y, "move")
            }
            WindowEvent::CursorEntered { .. } => self.pointer_at_last_position("enter"),
            WindowEvent::CursorLeft { .. } => self.pointer_at_last_position("leave"),
            WindowEvent::MouseWheel { delta, phase, .. } => {
                self.pointer_at_last_position(mouse_wheel_label(*delta, *phase))
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let suffix = if *state == ElementState::Pressed {
                    "down"
                } else {
                    "up"
                };
                self.pointer_at_last_position(format!("{}-{suffix}", mouse_button_label(*button)))
            }
            WindowEvent::PinchGesture { delta, phase, .. } => {
                self.pointer_at_last_position(format!(
                    "pinch-{}:{}",
                    touch_phase_label(*phase),
                    compact_f64(*delta)
                ))
            }
            WindowEvent::PanGesture { delta, phase, .. } => self.pointer_at_last_position(format!(
                "pan-{}:{}:{}",
                touch_phase_label(*phase),
                compact_f32(delta.x),
                compact_f32(delta.y)
            )),
            WindowEvent::RotationGesture { delta, phase, .. } => {
                self.pointer_at_last_position(format!(
                    "rotation-{}:{}",
                    touch_phase_label(*phase),
                    compact_f32(*delta)
                ))
            }
            WindowEvent::TouchpadPressure {
                pressure, stage, ..
            } => self
                .pointer_at_last_position(format!("pressure:{}:{stage}", compact_f32(*pressure))),
            WindowEvent::AxisMotion { axis, value, .. } => {
                self.pointer_at_last_position(format!("axis:{axis}:{}", compact_f64(*value)))
            }
            WindowEvent::Touch(touch) => {
                let x = touch.location.x / self.metrics.scale_factor;
                let y = touch.location.y / self.metrics.scale_factor;
                self.last_pointer_position = (x, y);
                PointerInput::new(x, y, touch_label(touch))
            }
            _ => return WinitTranslatedEvent::new(Vec::new()),
        };
        WinitTranslatedEvent::new(vec![DesktopHostEvent::PointerInput(pointer)])
    }

    fn pointer_at_last_position(&self, button: impl Into<String>) -> PointerInput {
        let (x, y) = self.last_pointer_position;
        PointerInput::new(x, y, button)
    }
}

fn ime_event_label(event: &Ime) -> String {
    match event {
        Ime::Enabled => "enabled".into(),
        Ime::Preedit(value, Some((start, end))) => format!("preedit:{value}:{start}..{end}"),
        Ime::Preedit(value, None) => format!("preedit:{value}:hidden"),
        Ime::Commit(value) => format!("commit:{value}"),
        Ime::Disabled => "disabled".into(),
    }
}

fn mouse_button_label(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        MouseButton::Back => "back",
        MouseButton::Forward => "forward",
        MouseButton::Other(_) => "other",
    }
}

fn mouse_wheel_label(delta: MouseScrollDelta, _phase: TouchPhase) -> String {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => {
            format!("wheel-lines:{}:{}", compact_f32(x), compact_f32(y))
        }
        MouseScrollDelta::PixelDelta(position) => {
            format!(
                "wheel-pixels:{}:{}",
                compact_f64(position.x),
                compact_f64(position.y)
            )
        }
    }
}

fn touch_label(touch: &winit::event::Touch) -> String {
    let force = touch
        .force
        .map(|force| format!(":{}", compact_f64(force.normalized())))
        .unwrap_or_default();
    format!(
        "touch-{}:{}{}",
        touch_phase_label(touch.phase),
        touch.id,
        force
    )
}

fn touch_phase_label(phase: TouchPhase) -> &'static str {
    match phase {
        TouchPhase::Started => "started",
        TouchPhase::Moved => "moved",
        TouchPhase::Ended => "ended",
        TouchPhase::Cancelled => "cancelled",
    }
}

fn modifiers_label(modifiers: winit::event::Modifiers) -> String {
    let state = modifiers.state();
    let mut labels = Vec::new();
    if state.shift_key() {
        labels.push("shift");
    }
    if state.control_key() {
        labels.push("control");
    }
    if state.alt_key() {
        labels.push("alt");
    }
    if state.super_key() {
        labels.push("super");
    }
    if labels.is_empty() {
        "modifiers:none".to_string()
    } else {
        format!("modifiers:{}", labels.join("+"))
    }
}

fn modifiers_pressed(modifiers: winit::event::Modifiers) -> bool {
    let state = modifiers.state();
    state.shift_key() || state.control_key() || state.alt_key() || state.super_key()
}

fn compact_f32(value: f32) -> String {
    if value.fract() == 0.0 {
        #[allow(clippy::cast_possible_truncation)]
        {
            (value as i32).to_string()
        }
    } else {
        value.to_string()
    }
}

fn compact_f64(value: f64) -> String {
    if value.fract() == 0.0 {
        #[allow(clippy::cast_possible_truncation)]
        {
            (value as i64).to_string()
        }
    } else {
        value.to_string()
    }
}

const WINIT_CAP_OWNS_WINDOW: u16 = 1 << 0;
const WINIT_CAP_CLOSE: u16 = 1 << 1;
const WINIT_CAP_MINIMIZE: u16 = 1 << 2;
const WINIT_CAP_MAXIMIZE: u16 = 1 << 3;
const WINIT_CAP_FULLSCREEN: u16 = 1 << 4;
const WINIT_CAP_FOCUS: u16 = 1 << 5;
const WINIT_CAP_KEYBOARD: u16 = 1 << 6;
const WINIT_CAP_POINTER: u16 = 1 << 7;
const WINIT_CAP_CLIPBOARD: u16 = 1 << 8;
const WINIT_CAP_RESIZE: u16 = 1 << 9;
const WINIT_CAP_REPAINT: u16 = 1 << 10;

/// Desktop Winit adapter error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinitHostError {
    rule: String,
    message: String,
}

impl WinitHostError {
    /// Creates a Winit host error.
    #[must_use]
    pub fn new(rule: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Returns the diagnostic rule.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<WinitHostError> for Diagnostic {
    fn from(error: WinitHostError) -> Self {
        Self::error(error.rule, error.message)
    }
}

/// Headless-safe Winit desktop adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct WinitDesktopAdapter {
    config: DesktopWindowConfig,
    platform: WinitPlatformFixture,
    capabilities: WinitHostCapabilities,
    logical_size: LogicalSize<f64>,
    mode: WindowMode,
    focused: bool,
    close_requested: bool,
    events: Vec<DesktopHostEvent>,
    repaint_requests: Vec<RepaintRequest>,
}

impl WinitDesktopAdapter {
    /// Creates a desktop window adapter from a platform fixture.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the platform handle cannot own a desktop window.
    pub fn create_window(
        config: DesktopWindowConfig,
        platform: WinitPlatformFixture,
    ) -> Result<Self, WinitHostError> {
        validate_desktop_metrics(config.metrics)?;
        platform
            .handle()
            .validate_for(SurfaceOwnership::DesktopWindow)
            .map_err(|diagnostic| WinitHostError::new(diagnostic.code, diagnostic.message))?;
        let logical_size =
            LogicalSize::new(config.metrics.logical_width, config.metrics.logical_height);
        Ok(Self {
            config: config.clone(),
            platform,
            capabilities: WinitHostCapabilities::desktop(),
            logical_size,
            mode: WindowMode::Normal,
            focused: false,
            close_requested: false,
            events: vec![DesktopHostEvent::WindowCreated(config)],
            repaint_requests: Vec::new(),
        })
    }

    /// Returns Winit-specific capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> WinitHostCapabilities {
        self.capabilities
    }

    /// Returns current window mode.
    #[must_use]
    pub const fn mode(&self) -> WindowMode {
        self.mode
    }

    /// Returns whether the window is focused.
    #[must_use]
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether close was requested.
    #[must_use]
    pub const fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// Returns the platform handle.
    #[must_use]
    pub const fn platform_handle(&self) -> HostPlatformHandle {
        self.platform.handle()
    }

    /// Returns repaint requests.
    #[must_use]
    pub fn repaint_requests(&self) -> &[RepaintRequest] {
        &self.repaint_requests
    }

    /// Handles Winit resize events.
    pub fn handle_resize(&mut self, metrics: SurfaceMetrics) {
        let _ = self.try_handle_resize(metrics);
    }

    /// Handles Winit resize events and reports invalid metrics.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the resize metrics are not finite and positive.
    pub fn try_handle_resize(&mut self, metrics: SurfaceMetrics) -> Result<(), WinitHostError> {
        self.ensure_accepts_host_event()?;
        validate_desktop_metrics(metrics)?;
        self.config.metrics = metrics;
        self.logical_size = LogicalSize::new(metrics.logical_width, metrics.logical_height);
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
        Ok(())
    }

    /// Handles Winit DPI changes and reports invalid scale factors.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the resulting surface metrics are not finite and positive.
    pub fn try_dpi_changed(&mut self, scale_factor: f64) -> Result<(), WinitHostError> {
        self.ensure_accepts_host_event()?;
        let metrics = SurfaceMetrics::new(
            self.config.metrics.logical_width,
            self.config.metrics.logical_height,
            scale_factor,
        );
        validate_desktop_metrics(metrics)?;
        self.config.metrics.scale_factor = scale_factor;
        self.events.push(DesktopHostEvent::DpiChanged(scale_factor));
        self.events
            .push(DesktopHostEvent::RendererTargetRecreateRequested);
        Ok(())
    }

    /// Requests a repaint.
    pub fn request_repaint(&mut self, reason: impl Into<String>) {
        if !self.accepts_host_event() {
            return;
        }
        self.repaint_requests
            .push(RepaintRequest::full_surface(reason));
    }

    /// Drains host events.
    pub fn drain_events(&mut self) -> Vec<DesktopHostEvent> {
        std::mem::take(&mut self.events)
    }

    /// Executes a clipboard request through a native clipboard bridge.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the window is closed, the bridge denies the request, or the
    /// native clipboard backend fails.
    pub fn try_request_clipboard<B: WinitClipboardBackend>(
        &mut self,
        request: SurfaceClipboardRequest,
        bridge: &mut WinitClipboardBridge<B>,
    ) -> Result<WinitClipboardResponse, WinitHostError> {
        self.ensure_accepts_host_event()?;
        let response = bridge.handle_request(request.clone())?;
        self.events
            .push(DesktopHostEvent::ClipboardRequested(request));
        Ok(response)
    }

    /// Executes a native dialog request through a dialog bridge.
    ///
    /// # Errors
    ///
    /// Returns [`WinitHostError`] when the window is closed, request validation fails, or the
    /// native dialog backend fails.
    pub fn try_request_dialog<B: WinitDialogBackend>(
        &mut self,
        request: DesktopDialogRequest,
        bridge: &mut WinitDialogBridge<B>,
    ) -> Result<DesktopDialogResponse, WinitHostError> {
        self.ensure_accepts_host_event()?;
        let response = bridge.handle_request(request.clone())?;
        self.events.push(DesktopHostEvent::DialogRequested(request));
        Ok(response)
    }

    fn set_mode(&mut self, mode: WindowMode) {
        if !self.accepts_host_event() || self.mode == mode {
            return;
        }
        self.mode = mode;
        self.events.push(DesktopHostEvent::ModeChanged(mode));
    }

    fn accepts_host_event(&self) -> bool {
        !self.close_requested
    }

    fn ensure_accepts_host_event(&self) -> Result<(), WinitHostError> {
        if self.accepts_host_event() {
            Ok(())
        } else {
            Err(WinitHostError::new(
                "desktop.window.closed",
                "desktop window has already received a close request",
            ))
        }
    }
}

impl DesktopHostAdapter for WinitDesktopAdapter {
    fn config(&self) -> &DesktopWindowConfig {
        &self.config
    }

    fn metrics(&self) -> SurfaceMetrics {
        self.config.metrics
    }

    fn request_minimize(&mut self, minimized: bool) {
        self.set_mode(if minimized {
            WindowMode::Minimized
        } else {
            WindowMode::Normal
        });
    }

    fn request_maximize(&mut self, maximized: bool) {
        self.set_mode(if maximized {
            WindowMode::Maximized
        } else {
            WindowMode::Normal
        });
    }

    fn request_fullscreen(&mut self, fullscreen: bool) {
        self.set_mode(if fullscreen {
            WindowMode::Fullscreen
        } else {
            WindowMode::Normal
        });
    }

    fn request_close(&mut self, reason: impl Into<String>) {
        if self.close_requested {
            return;
        }
        self.close_requested = true;
        self.events
            .push(DesktopHostEvent::CloseRequested(reason.into()));
    }

    fn set_focus(&mut self, focused: bool) {
        if !self.accepts_host_event() || self.focused == focused {
            return;
        }
        self.focused = focused;
        self.events.push(DesktopHostEvent::FocusChanged(focused));
    }

    fn keyboard_input(&mut self, input: KeyboardInput) {
        if !self.accepts_host_event() {
            return;
        }
        self.events.push(DesktopHostEvent::KeyboardInput(input));
    }

    fn pointer_input(&mut self, input: PointerInput) {
        if !self.accepts_host_event() {
            return;
        }
        self.events.push(DesktopHostEvent::PointerInput(input));
    }

    fn clipboard_available(&mut self, capability: ClipboardCapability) {
        if !self.accepts_host_event() || self.config.clipboard == capability {
            return;
        }
        self.config.clipboard = capability;
        self.events
            .push(DesktopHostEvent::ClipboardCapabilityChanged(capability));
    }

    fn dpi_changed(&mut self, scale_factor: f64) {
        let _ = self.try_dpi_changed(scale_factor);
    }
}

fn configure_file_dialog(
    title: String,
    directory: Option<&Path>,
    filters: Vec<DesktopDialogFileFilter>,
) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    for filter in filters {
        dialog = dialog.add_filter(filter.name, &filter.extensions);
    }
    dialog
}

fn rfd_message_level(level: DesktopDialogLevel) -> rfd::MessageLevel {
    match level {
        DesktopDialogLevel::Info => rfd::MessageLevel::Info,
        DesktopDialogLevel::Warning => rfd::MessageLevel::Warning,
        DesktopDialogLevel::Error => rfd::MessageLevel::Error,
    }
}

fn validate_dialog_title(title: &str) -> Result<(), WinitHostError> {
    if title.trim().is_empty() {
        Err(WinitHostError::new(
            "desktop.dialog.invalid-title",
            "desktop dialog title must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_dialog_message(message: &str) -> Result<(), WinitHostError> {
    if message.trim().is_empty() {
        Err(WinitHostError::new(
            "desktop.dialog.invalid-message",
            "desktop dialog message must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_dialog_file_name(file_name: &str) -> Result<(), WinitHostError> {
    if file_name.trim().is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains('\0')
    {
        Err(WinitHostError::new(
            "desktop.dialog.invalid-file-name",
            "desktop dialog file name must be a non-empty file name, not a path",
        ))
    } else {
        Ok(())
    }
}

fn validate_dialog_filters(filters: &[DesktopDialogFileFilter]) -> Result<(), WinitHostError> {
    for filter in filters {
        if filter.name.trim().is_empty() || filter.extensions.is_empty() {
            return Err(WinitHostError::new(
                "desktop.dialog.invalid-filter",
                "desktop dialog filters require a non-empty name and at least one extension",
            ));
        }
        for extension in &filter.extensions {
            if extension.trim().is_empty()
                || extension.starts_with('.')
                || extension.contains('/')
                || extension.contains('\\')
                || extension.contains('\0')
            {
                return Err(WinitHostError::new(
                    "desktop.dialog.invalid-filter",
                    "desktop dialog filter extensions must not be empty and must omit path separators and leading dots",
                ));
            }
        }
    }
    Ok(())
}

fn validate_desktop_metrics(metrics: SurfaceMetrics) -> Result<(), WinitHostError> {
    if metrics.logical_width.is_finite()
        && metrics.logical_height.is_finite()
        && metrics.scale_factor.is_finite()
        && metrics.logical_width > 0.0
        && metrics.logical_height > 0.0
        && metrics.scale_factor > 0.0
    {
        Ok(())
    } else {
        Err(WinitHostError::new(
            "desktop.window.invalid-size",
            "desktop window dimensions and scale factor must be finite and greater than zero",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(crate_name(), "hawk2ui-host-winit");
    }

    #[test]
    fn winit_translator_projects_touch_events_to_logical_pointer_inputs() {
        let device_id = winit::event::DeviceId::dummy();
        let mut translator = WinitEventTranslator::new(SurfaceMetrics::new(320.0, 200.0, 2.0));

        let translated = translator.translate(&WindowEvent::Touch(winit::event::Touch {
            device_id,
            phase: TouchPhase::Started,
            location: winit::dpi::PhysicalPosition::new(24.0, 48.0),
            force: Some(winit::event::Force::Normalized(0.5)),
            id: 7,
        }));

        assert_eq!(
            translated.events,
            vec![DesktopHostEvent::PointerInput(PointerInput::new(
                12.0,
                24.0,
                "touch-started:7:0.5"
            ))]
        );
    }

    #[test]
    fn winit_translator_projects_gestures_and_axis_motion_to_pointer_inputs() {
        let device_id = winit::event::DeviceId::dummy();
        let mut translator = WinitEventTranslator::new(SurfaceMetrics::new(320.0, 200.0, 2.0));
        let _ = translator.translate(&WindowEvent::CursorMoved {
            device_id,
            position: winit::dpi::PhysicalPosition::new(20.0, 40.0),
        });

        let pinch = translator.translate(&WindowEvent::PinchGesture {
            device_id,
            delta: 1.25,
            phase: TouchPhase::Moved,
        });
        let pan = translator.translate(&WindowEvent::PanGesture {
            device_id,
            delta: winit::dpi::PhysicalPosition::new(6.0, -4.0),
            phase: TouchPhase::Moved,
        });
        let rotation = translator.translate(&WindowEvent::RotationGesture {
            device_id,
            delta: 45.0,
            phase: TouchPhase::Ended,
        });
        let pressure = translator.translate(&WindowEvent::TouchpadPressure {
            device_id,
            pressure: 0.75,
            stage: 2,
        });
        let axis = translator.translate(&WindowEvent::AxisMotion {
            device_id,
            axis: 3,
            value: -0.25,
        });

        assert_eq!(
            pinch.events,
            vec![DesktopHostEvent::PointerInput(PointerInput::new(
                10.0,
                20.0,
                "pinch-moved:1.25"
            ))]
        );
        assert_eq!(
            pan.events,
            vec![DesktopHostEvent::PointerInput(PointerInput::new(
                10.0,
                20.0,
                "pan-moved:6:-4"
            ))]
        );
        assert_eq!(
            rotation.events,
            vec![DesktopHostEvent::PointerInput(PointerInput::new(
                10.0,
                20.0,
                "rotation-ended:45"
            ))]
        );
        assert_eq!(
            pressure.events,
            vec![DesktopHostEvent::PointerInput(PointerInput::new(
                10.0,
                20.0,
                "pressure:0.75:2"
            ))]
        );
        assert_eq!(
            axis.events,
            vec![DesktopHostEvent::PointerInput(PointerInput::new(
                10.0,
                20.0,
                "axis:3:-0.25"
            ))]
        );
    }

    #[test]
    fn winit_translator_projects_modifier_changes_to_keyboard_inputs() {
        let mut translator = WinitEventTranslator::new(SurfaceMetrics::new(320.0, 200.0, 1.0));

        let translated = translator.translate(&WindowEvent::ModifiersChanged(
            winit::event::Modifiers::default(),
        ));

        assert_eq!(
            translated.events,
            vec![DesktopHostEvent::KeyboardInput(KeyboardInput::new(
                "modifiers:none",
                false
            ))]
        );
    }
}
