use std::cell::RefCell;
use std::collections::hash_map::{Entry, HashMap};
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::cursor::Handle as CursorHandle;
use x11rb::protocol::xproto::{Cursor, Screen};
use x11rb::resource_manager;

use super::cursor;
use crate::wrappers::xlib::XlibXcbConnection;
use crate::MouseCursor;

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
    }
}

/// A very light abstraction around the XCB connection.
///
/// Keeps track of the xcb connection itself and the xlib display ID that was used to connect.
pub struct XcbConnection {
    pub(crate) conn: XlibXcbConnection,
    pub(crate) atoms: Atoms,
    pub(crate) resources: resource_manager::Database,
    pub(crate) cursor_handle: CursorHandle,
    pub(super) cursor_cache: RefCell<HashMap<MouseCursor, u32>>,
}

impl XcbConnection {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let conn = XlibXcbConnection::open()?;
        let screen = conn.default_screen();
        let xcb_conn = conn.xcb_connection();

        let atoms = Atoms::new(xcb_conn)?.reply()?;
        let resources = resource_manager::new_from_default(xcb_conn)?;
        let cursor_handle = CursorHandle::new(xcb_conn, screen as usize, &resources)?.reply()?;

        Ok(Self {
            conn,
            atoms,
            resources,
            cursor_handle,
            cursor_cache: RefCell::new(HashMap::new()),
        })
    }

    // Try to get the scaling with this function first.
    // If this gives you `None`, fall back to `get_scaling_screen_dimensions`.
    // If neither work, I guess just assume 96.0 and don't do any scaling.
    fn get_scaling_xft(&self) -> Result<Option<f64>, Box<dyn Error>> {
        if let Some(dpi) = self.resources.get_value::<u32>("Xft.dpi", "")? {
            Ok(Some(dpi as f64 / 96.0))
        } else {
            Ok(None)
        }
    }

    // Try to get the scaling with `get_scaling_xft` first.
    // Only use this function as a fallback.
    // If neither work, I guess just assume 96.0 and don't do any scaling.
    fn get_scaling_screen_dimensions(&self) -> f64 {
        // Figure out screen information
        let screen = self.screen();

        // Get the DPI from the screen struct
        //
        // there are 2.54 centimeters to an inch; so there are 25.4 millimeters.
        // dpi = N pixels / (M millimeters / (25.4 millimeters / 1 inch))
        //     = N pixels / (M inch / 25.4)
        //     = N * 25.4 pixels / M inch
        let xres = dpi_from_physical_size(screen.width_in_pixels, screen.width_in_millimeters);
        let yres = dpi_from_physical_size(screen.height_in_pixels, screen.height_in_millimeters);

        match (xres, yres) {
            (Some(xres), Some(yres)) => ((xres + yres) * 0.5) / 96.0,
            (Some(dpi), None) | (None, Some(dpi)) => dpi / 96.0,
            (None, None) => 1.0,
        }
    }

    #[inline]
    pub fn get_scaling(&self) -> Result<f64, Box<dyn Error>> {
        Ok(self.get_scaling_xft()?.unwrap_or(self.get_scaling_screen_dimensions()))
    }

    #[inline]
    pub fn get_cursor(&self, cursor: MouseCursor) -> Result<Cursor, Box<dyn Error>> {
        // PANIC: this function is the only point where we access the cache, and we never call
        // external functions that may make a reentrant call to this function
        let mut cursor_cache = self.cursor_cache.borrow_mut();

        match cursor_cache.entry(cursor) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let cursor = cursor::get_xcursor(
                    &self.conn,
                    self.conn.default_screen() as usize,
                    &self.cursor_handle,
                    cursor,
                )?;
                entry.insert(cursor);
                Ok(cursor)
            }
        }
    }

    pub fn screen(&self) -> &Screen {
        &self.conn.setup().roots[self.conn.default_screen() as usize]
    }
}

fn dpi_from_physical_size(pixels: u16, millimeters: u16) -> Option<f64> {
    if pixels == 0 || millimeters == 0 {
        return None;
    }
    let dpi = f64::from(pixels) * 25.4 / f64::from(millimeters);
    dpi.is_finite().then_some(dpi)
}
