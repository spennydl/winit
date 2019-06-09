use window::{WindowAttributes};
use std::collections::VecDeque;
use std::rc::Rc;
use std::cell::Cell;
use dpi::{PhysicalPosition, LogicalPosition, PhysicalSize, LogicalSize};
use icon::Icon;
use super::event_loop::{EventLoopWindowTarget};

use ::error::{ExternalError, NotSupportedError};
use ::window::CursorIcon;

use ::wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u32);

impl DeviceId {
    pub fn dummy() -> Self {
        DeviceId(0)
    }
}

///
/// ElementSelection allows the window creator
/// to select an existing canvas in the DOM
/// or a container in which to create a canvas.
///
#[derive(Clone)]
pub enum ElementSelection {
    CanvasId(String),
    ContainerId(String)
}

impl Default for ElementSelection {
    fn default() -> Self { ElementSelection::CanvasId("".to_string()) }
}

///
/// Platform specific attributes for window creation.
/// 
#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub element: ElementSelection
}

#[derive(Copy, Clone, Debug)]
pub struct MonitorHandle;

impl MonitorHandle {
    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore.
    #[inline]
    pub fn name(&self) -> Option<String> {
        Some(String::from("Browser Window"))
    }

    /// Returns the monitor's resolution.
    #[inline]
    pub fn dimensions(&self) -> PhysicalSize {
        let win = ::web_sys::window().expect("there to be a window");
        let w = match win.inner_width() {
            Ok(val) => val.as_f64().unwrap(),
            Err(val) => 0.0
        };
        let h = match win.inner_height() {
            Ok(val) => val.as_f64().unwrap(),
            Err(val) => 0.0
        };

        (w, h).into()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    #[inline]
    pub fn position(&self) -> PhysicalPosition {
        (0, 0).into()
    }

    /// Returns the DPI factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](dpi/index.html) module for more information.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Can be overridden using the `WINIT_HIDPI_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        1.0
    }
}

pub struct Window {
    pub(crate) canvas: ::web_sys::HtmlCanvasElement,
    pub(crate) redraw_requested: Cell<bool>
}

pub(crate) struct WindowInternal<'a, T: 'static> {
    pub target: &'a EventLoopWindowTarget<T>,
    _marker: std::marker::PhantomData<T>,
}

impl Window {
    /// Creates a new Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to `WindowBuilder::new().build(event_loop)`.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    pub fn new<T: 'static>(target: &EventLoopWindowTarget<T>, 
                           attr: WindowAttributes,
                           ps_attr: PlatformSpecificWindowBuilderAttributes) 
                           -> Result<Window, ::error::OsError> {
        let window = ::web_sys::window()
            .expect("No global window object found!");
        let document = window.document()
            .expect("Global window does not have a document!");

        let element = match ps_attr.element {
            ElementSelection::CanvasId(id) => {
                document.get_element_by_id(&id)
                    .expect(&format!("No canvas with ID {} found", id))
                    .dyn_into::<::web_sys::HtmlCanvasElement>().unwrap()
            },
            ElementSelection::ContainerId(id) => {
                let parent = document.get_element_by_id(&id)
                    .expect(&format!("No container element with Id {} found", id));
                
                let canvas = document.create_element("canvas")
                    .expect("Could not create a canvas")
                    .dyn_into::<::web_sys::HtmlCanvasElement>().unwrap();
                
                parent.append_child(&canvas)?;

                canvas
            }
        };

        target.setup_window(&element);

        Ok(Window {
            canvas: element,
            redraw_requested: Cell::new(false)
        })
    }

    /// Returns an identifier unique to the window.
    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId::dummy()
    }

    /// Returns the DPI factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](dpi/index.html) module for more information.
    ///
    /// Note that this value can change depending on user action (for example if the window is
    /// moved to another screen); as such, tracking `WindowEvent::HiDpiFactorChanged` events is
    /// the most robust way to track the DPI you need to use to draw.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** This respects Xft.dpi, and can be overridden using the `WINIT_HIDPI_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    /// - **iOS:** Can only be called on the main thread. Returns the underlying `UIView`'s
    ///   [`contentScaleFactor`].
    ///
    /// [`contentScaleFactor`]: https://developer.apple.com/documentation/uikit/uiview/1622657-contentscalefactor?language=objc
    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        1.0
    }

    /// Emits a `WindowEvent::RedrawRequested` event in the associated event loop after all OS
    /// events have been processed by the event loop.
    ///
    /// This is the **strongly encouraged** method of redrawing windows, as it can integrates with
    /// OS-requested redraws (e.g. when a window gets resized).
    ///
    /// This function can cause `RedrawRequested` events to be emitted after `Event::EventsCleared`
    /// but before `Event::NewEvents` if called in the following circumstances:
    /// * While processing `EventsCleared`.
    /// * While processing a `RedrawRequested` event that was sent during `EventsCleared` or any
    ///   directly subsequent `RedrawRequested` event.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn request_redraw(&self) {
        self.redraw_requested.replace(true);
    }

    #[inline]
    fn canvas_as_element(&self) -> &HtmlElement {
        &self.canvas
    }
}

/// Position and size functions.
impl Window {
    /// Returns the position of the top-left hand corner of the window's client area relative to the
    /// top-left hand corner of the desktop.
    ///
    /// The same conditions that apply to `outer_position` apply to this method.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the top left coordinates of the
    ///   window's [safe area] in the screen space coordinate system.
    ///
    /// [safe area]: https://developer.apple.com/documentation/uikit/uiview/2891103-safeareainsets?language=objc
    #[inline]
    pub fn inner_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        let rect = self.canvas_as_element().get_bounding_client_rect();
        Ok((rect.x(), rect.y()).into())
    }

    /// Returns the position of the top-left hand corner of the window relative to the
    ///  top-left hand corner of the desktop.
    ///
    /// Note that the top-left hand corner of the desktop is not necessarily the same as
    ///  the screen. If the user uses a desktop with multiple monitors, the top-left hand corner
    ///  of the desktop is the top-left hand corner of the monitor at the top-left of the desktop.
    ///
    /// The coordinates can be negative if the top-left hand corner of the window is outside
    ///  of the visible screen region.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the top left coordinates of the
    ///   window in the screen space coordinate system.
    #[inline]
    pub fn outer_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        self.inner_position()
    }

    /// Modifies the position of the window.
    ///
    /// See `outer_position` for more information about the coordinates.
    ///
    /// This is a no-op if the window has already been closed.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Sets the top left coordinates of the
    ///   window in the screen space coordinate system.
    #[inline]
    pub fn set_outer_position(&self, position: LogicalPosition) {
        // TODO: support this?
        unimplemented!()
    }

    /// Returns the logical size of the window's client area.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    ///
    /// Converting the returned `LogicalSize` to `PhysicalSize` produces the size your framebuffer should be.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the `LogicalSize` of the window's
    ///   [safe area] in screen space coordinates.
    ///
    /// [safe area]: https://developer.apple.com/documentation/uikit/uiview/2891103-safeareainsets?language=objc
    #[inline]
    pub fn inner_size(&self) -> LogicalSize {
        let rect = self.canvas_as_element().get_bounding_client_rect();
        (rect.width(), rect.height()).into()
    }

    /// Modifies the inner size of the window.
    ///
    /// See `inner_size` for more information about the values.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Unimplemented. Currently this panics, as it's not clear what `set_inner_size`
    ///   would mean for iOS.
    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        unimplemented!()
    }

    /// Returns the logical size of the entire window.
    ///
    /// These dimensions include the title bar and borders. If you don't want that (and you usually don't),
    /// use `inner_size` instead.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Returns the `LogicalSize` of the window in
    ///   screen space coordinates.
    #[inline]
    pub fn outer_size(&self) -> LogicalSize {
        self.inner_size()
    }

    /// Sets a minimum dimension size for the window.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Has no effect.
    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<LogicalSize>) {
        unimplemented!()
    }

    /// Sets a maximum dimension size for the window.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Has no effect.
    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<LogicalSize>) {
        unimplemented!()
    }
}

/// Misc. attribute functions.
impl Window {
    /// Modifies the title of the window.
    ///
    /// ## Platform-specific
    ///
    /// - Has no effect on iOS.
    #[inline]
    pub fn set_title(&self, title: &str) {
        unimplemented!()
    }

    /// Modifies the window's visibility.
    ///
    /// If `false`, this will hide the window. If `true`, this will show the window.
    /// ## Platform-specific
    ///
    /// - **Android:** Has no effect.
    /// - **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn set_visible(&self, visible: bool) {
        unimplemented!()
    }

    /// Sets whether the window is resizable or not.
    ///
    /// Note that making the window unresizable doesn't exempt you from handling `Resized`, as that event can still be
    /// triggered by DPI scaling, entering fullscreen mode, etc.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on desktop platforms.
    ///
    /// Due to a bug in XFCE, this has no effect on Xfwm.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Has no effect.
    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        unimplemented!()
    }

    /// Sets the window to maximized or back.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Has no effect.
    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        // no-op
    }

    /// Sets the window to fullscreen or back.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<::monitor::MonitorHandle>) {
        // no-op
    }

    /// Gets the window's current fullscreen state.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn fullscreen(&self) -> Option<::monitor::MonitorHandle> {
        None
    }

    /// Turn window decorations on or off.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread. Controls whether the status bar is hidden
    ///   via [`setPrefersStatusBarHidden`].
    ///
    /// [`setPrefersStatusBarHidden`]: https://developer.apple.com/documentation/uikit/uiviewcontroller/1621440-prefersstatusbarhidden?language=objc
    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        // no-op
    }

    /// Change whether or not the window will always be on top of other windows.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Has no effect.
    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        // no-op
    }

    /// Sets the window icon. On Windows and X11, this is typically the small icon in the top-left
    /// corner of the titlebar.
    ///
    /// For more usage notes, see `WindowBuilder::with_window_icon`.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on Windows and X11.
    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        // TODO: set favicon?
        unimplemented!()
    }

    /// Sets location of IME candidate box in client area coordinates relative to the top left.
    ///
    /// ## Platform-specific
    ///
    /// **iOS:** Has no effect.
    #[inline]
    pub fn set_ime_position(&self, position: LogicalPosition) {
        // no-op
    }
}

/// Cursor functions.
impl Window {
    /// Modifies the cursor icon of the window.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Has no effect.
    /// - **Android:** Has no effect.
    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        let cursor_style = match cursor {
            CursorIcon::Crosshair => "crosshair",
            CursorIcon::Hand => "pointer",
            CursorIcon::Move => "move",
            CursorIcon::Text => "text",
            CursorIcon::Wait => "wait",
            CursorIcon::Help => "help",
            CursorIcon::Progress => "progress",

            /// Cursor showing that something cannot be done.
            CursorIcon::NotAllowed => "not-allowed",
            CursorIcon::ContextMenu => "context-menu",
            CursorIcon::Cell => "cell",
            CursorIcon::VerticalText => "vertical-text",
            CursorIcon::Alias => "alias",
            CursorIcon::Copy => "copy",
            CursorIcon::NoDrop => "no-drop",

            CursorIcon::EResize => "e-resize",
            CursorIcon::NResize => "n-resize",
            CursorIcon::NeResize => "ne-resize",
            CursorIcon::NwResize => "nw-resize",
            CursorIcon::SResize => "s-resize",
            CursorIcon::SeResize => "se-resize",
            CursorIcon::SwResize => "sw-resize",
            CursorIcon::WResize => "w-resize",
            CursorIcon::EwResize => "ew-resize",
            CursorIcon::NsResize => "ns-resize",
            CursorIcon::NeswResize => "nesw-resize",
            CursorIcon::NwseResize => "nwse-resize",
            CursorIcon::ColResize => "col-resize",
            CursorIcon::RowResize => "row-resize",
            _ => "auto",
        };
        self.canvas_as_element().style().set_property("cursor", cursor_style).unwrap();
    }

    /// Changes the position of the cursor in window coordinates.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Always returns an `Err`.
    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), ExternalError> {
        // unsupported
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    /// Grabs the cursor, preventing it from leaving the window.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** This presently merely locks the cursor in a fixed location, which looks visually
    ///   awkward.
    /// - **Android:** Has no effect.
    /// - **iOS:** Always returns an Err.
    #[inline]
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        // unsupported
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    /// Modifies the cursor's visibility.
    ///
    /// If `false`, this will hide the cursor. If `true`, this will show the cursor.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows:** The cursor is only hidden within the confines of the window.
    /// - **X11:** The cursor is only hidden within the confines of the window.
    /// - **macOS:** The cursor is hidden as long as the window has input focus, even if the cursor is
    ///   outside of the window.
    /// - **iOS:** Has no effect.
    /// - **Android:** Has no effect.
    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        let style = self.canvas_as_element().style();
        if visible {
            style.set_property("cursor", "none").unwrap();
        } else {
            style.set_property("cursor", "auto").unwrap();
        }
    }
}

/// Monitor info functions.
impl Window {
    /// Returns the monitor on which the window currently resides
    ///
    /// ## Platform-specific
    ///
    /// **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn current_monitor(&self) -> ::monitor::MonitorHandle {
        ::monitor::MonitorHandle{ inner: MonitorHandle{} }
    }

    /// Returns the list of all the monitors available on the system.
    ///
    /// This is the same as `EventLoop::available_monitors`, and is provided for convenience.
    ///
    /// ## Platform-specific
    ///
    /// **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        vec![MonitorHandle{}].into()
    }

    /// Returns the primary monitor of the system.
    ///
    /// This is the same as `EventLoop::primary_monitor`, and is provided for convenience.
    ///
    /// ## Platform-specific
    ///
    /// **iOS:** Can only be called on the main thread.
    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle { }
    }
}


#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId {
}


impl WindowId {
    /// Returns a dummy `WindowId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `WindowId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub fn dummy() -> Self {
        WindowId{}
    }
}
