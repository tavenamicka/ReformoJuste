/// Returns (x, y) cursor position in physical pixels.
#[cfg(target_os = "windows")]
pub fn get_position() -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut pt = POINT { x: 0, y: 0 };
    unsafe { let _ = GetCursorPos(&mut pt); }
    (pt.x, pt.y)
}

/// Returns (left, top, right, bottom) of the monitor containing the given point.
/// Uses MonitorFromPoint so it works correctly on multi-monitor setups.
#[cfg(target_os = "windows")]
pub fn get_monitor_rect(cursor: (i32, i32)) -> (i32, i32, i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};

    let pt = POINT { x: cursor.0, y: cursor.1 };
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    unsafe { let _ = GetMonitorInfoW(monitor, &mut info); }
    let r = info.rcMonitor;
    (r.left, r.top, r.right, r.bottom)
}

#[cfg(not(target_os = "windows"))]
pub fn get_position() -> (i32, i32) { (100, 100) }

#[cfg(not(target_os = "windows"))]
pub fn get_monitor_rect(_cursor: (i32, i32)) -> (i32, i32, i32, i32) { (0, 0, 1920, 1080) }

/// Compute popup top-left so it stays within the monitor containing the cursor.
pub fn clamp_popup(
    cursor:  (i32, i32),
    monitor: (i32, i32, i32, i32),
    size:    (i32, i32),
    offset:  (i32, i32),
) -> (i32, i32) {
    let (cx, cy) = cursor;
    let (ml, mt, mr, mb) = monitor;
    let (pw, ph) = size;
    let (ox, oy) = offset;

    let mut x = cx + ox;
    let mut y = cy + oy;

    if x + pw > mr { x = cx - pw - ox; }
    if y + ph > mb { y = cy - ph - oy; }

    (x.max(ml), y.max(mt))
}
