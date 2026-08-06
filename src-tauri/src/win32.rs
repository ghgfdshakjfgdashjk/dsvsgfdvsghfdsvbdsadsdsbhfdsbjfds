#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

pub type HWND = isize;
pub type HRGN = isize;

pub const INPUT_MOUSE: u32 = 0;
pub const INPUT_KEYBOARD: u32 = 1;

pub const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
pub const KEYEVENTF_KEYUP: u32 = 0x0002;
pub const KEYEVENTF_UNICODE: u32 = 0x0004;

pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;

const MAPVK_VK_TO_VSC: u32 = 0;

pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_RBUTTONUP: u32 = 0x0205;
pub const WM_MBUTTONDOWN: u32 = 0x0207;
pub const WM_MBUTTONUP: u32 = 0x0208;
pub const WM_XBUTTONDOWN: u32 = 0x020B;
pub const WM_XBUTTONUP: u32 = 0x020C;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_MOUSEWHEEL: u32 = 0x020A;

pub const MK_LBUTTON: usize = 0x0001;
pub const MK_RBUTTON: usize = 0x0002;
pub const MK_MBUTTON: usize = 0x0010;
pub const MK_XBUTTON1: usize = 0x0020;
pub const MK_XBUTTON2: usize = 0x0040;

pub const XBUTTON1_W: usize = 0x0001;
pub const XBUTTON2_W: usize = 0x0002;

pub const MOUSEEVENTF_MOVE: u32 = 0x0001;
pub const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
pub const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
pub const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
pub const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
pub const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
pub const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
pub const MOUSEEVENTF_XDOWN: u32 = 0x0080;
pub const MOUSEEVENTF_XUP: u32 = 0x0100;
pub const MOUSEEVENTF_WHEEL: u32 = 0x0800;
pub const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
pub const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;

pub const WHEEL_DELTA: i32 = 120;

pub const XBUTTON1: u32 = 0x0001;
pub const XBUTTON2: u32 = 0x0002;

pub const SIGNATURE: usize = 0xB0_07_5E_ED;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MOUSEINPUT {
    pub dx: i32,
    pub dy: i32,
    pub mouseData: u32,
    pub dwFlags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KEYBDINPUT {
    pub wVk: u16,
    pub wScan: u16,
    pub dwFlags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HARDWAREINPUT {
    pub uMsg: u32,
    pub wParamL: u16,
    pub wParamH: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union INPUT_PAYLOAD {
    pub mi: MOUSEINPUT,
    pub ki: KEYBDINPUT,
    pub hi: HARDWAREINPUT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct INPUT {
    pub kind: u32,
    pub payload: INPUT_PAYLOAD,
}

#[link(name = "user32")]
extern "system" {
    fn GetDC(hWnd: HWND) -> isize;
    fn ReleaseDC(hWnd: HWND, hDC: isize) -> i32;
    fn SendInput(cInputs: u32, pInputs: *const INPUT, cbSize: i32) -> u32;
    fn GetAsyncKeyState(vKey: i32) -> i16;
    fn GetForegroundWindow() -> HWND;
    fn GetWindowTextLengthW(hWnd: HWND) -> i32;
    fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn SetWindowRgn(hWnd: HWND, hRgn: HRGN, bRedraw: i32) -> i32;
    fn VkKeyScanW(ch: u16) -> i16;
    fn MapVirtualKeyW(uCode: u32, uMapType: u32) -> u32;
    fn PostMessageW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> i32;
    fn WindowFromPoint(Point: POINT) -> HWND;
    fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetClipCursor(lpRect: *mut RECT) -> i32;
    fn IsWindow(hWnd: HWND) -> i32;
    fn GetAncestor(hwnd: HWND, gaFlags: u32) -> HWND;
    fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    fn CreateIconIndirect(piconinfo: *const ICONINFO) -> isize;
    fn SetCursor(hCursor: isize) -> isize;
    fn DestroyCursor(hCursor: isize) -> i32;
    fn LoadCursorW(hInstance: isize, lpCursorName: usize) -> isize;
    fn SetClassLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
    fn EnumWindows(lpEnumFunc: EnumWindowsProc, lParam: isize) -> i32;
    fn IsWindowVisible(hWnd: HWND) -> i32;
    fn IsIconic(hWnd: HWND) -> i32;
    fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
    fn GetClassNameW(hWnd: HWND, lpClassName: *mut u16, nMaxCount: i32) -> i32;
    fn GetSystemMetrics(nIndex: i32) -> i32;
    fn SetWindowsHookExW(idHook: i32, lpfn: HookProc, hmod: isize, dwThreadId: u32) -> isize;
    fn CallNextHookEx(hhk: isize, nCode: i32, wParam: usize, lParam: isize) -> isize;
    fn UnhookWindowsHookEx(hhk: isize) -> i32;
    fn PostThreadMessageW(idThread: u32, Msg: u32, wParam: usize, lParam: isize) -> i32;
    fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn SendMessageTimeoutW(
        hWnd: HWND,
        Msg: u32,
        wParam: usize,
        lParam: isize,
        fuFlags: u32,
        uTimeout: u32,
        lpdwResult: *mut usize,
    ) -> isize;

    fn GetWindowLongW(hWnd: HWND, nIndex: i32) -> i32;
    fn SetWindowLongW(hWnd: HWND, nIndex: i32, dwNewLong: i32) -> i32;
    fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;

    fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
    fn CallWindowProcW(
        lpPrevWndFunc: WndProc,
        hWnd: HWND,
        Msg: u32,
        wParam: usize,
        lParam: isize,
    ) -> isize;
    fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> isize;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub type EnumWindowsProc = unsafe extern "system" fn(HWND, isize) -> i32;
pub type HookProc = unsafe extern "system" fn(i32, usize, isize) -> isize;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: usize,
    pub lParam: isize,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
pub struct KBDLLHOOKSTRUCT {
    pub vkCode: u32,
    pub scanCode: u32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MSLLHOOKSTRUCT {
    pub pt: POINT,
    pub mouseData: u32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct ICONINFO {
    pub fIcon: i32,
    pub xHotspot: u32,
    pub yHotspot: u32,
    pub hbmMask: isize,
    pub hbmColor: isize,
}

pub fn log(message: &str) {
    eprintln!("{message}");
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("frame-debug.log")
    {
        let _ = writeln!(file, "{message}");
    }
}

const GWL_EXSTYLE: i32 = -20;

pub fn describe(hwnd: HWND, tag: &str) {
    if hwnd == 0 {
        log(&format!("[frame] {tag}: null hwnd"));
        return;
    }
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);

        let mut window_rect = RECT::default();
        let mut client_rect = RECT::default();
        GetWindowRect(hwnd, &mut window_rect);
        GetClientRect(hwnd, &mut client_rect);

        let window_w = window_rect.right - window_rect.left;
        let window_h = window_rect.bottom - window_rect.top;
        let client_w = client_rect.right - client_rect.left;
        let client_h = client_rect.bottom - client_rect.top;

        log(&format!(
            "[frame] {tag}: hwnd={hwnd:#x} style={style:#010x} ex={ex_style:#010x} \
             caption={} thickframe={} window={window_w}x{window_h} client={client_w}x{client_h} \
             nonclient={}x{}",
            style & WS_CAPTION != 0,
            style & WS_THICKFRAME != 0,
            window_w - client_w,
            window_h - client_h
        ));
    }
}

#[link(name = "gdi32")]
extern "system" {
    fn GetPixel(hdc: isize, x: i32, y: i32) -> u32;
    fn CreateRoundRectRgn(x1: i32, y1: i32, x2: i32, y2: i32, w: i32, h: i32) -> HRGN;
    fn CreateBitmap(nWidth: i32, nHeight: i32, nPlanes: u32, nBitCount: u32, lpBits: *const c_void)
        -> isize;
    fn CreateDIBSection(
        hdc: isize,
        pbmi: *const BITMAPINFOHEADER,
        usage: u32,
        ppvBits: *mut *mut c_void,
        hSection: isize,
        offset: u32,
    ) -> isize;
    fn DeleteObject(ho: isize) -> i32;
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn SelectObject(hdc: isize, obj: isize) -> isize;
    fn BitBlt(
        hdcDest: isize,
        xDest: i32,
        yDest: i32,
        width: i32,
        height: i32,
        hdcSrc: isize,
        xSrc: i32,
        ySrc: i32,
        rop: u32,
    ) -> i32;
    fn DeleteDC(hdc: isize) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwAttribute: u32,
        pvAttribute: *const c_void,
        cbAttribute: u32,
    ) -> i32;
    fn DwmGetWindowAttribute(
        hwnd: HWND,
        dwAttribute: u32,
        pvAttribute: *mut c_void,
        cbAttribute: u32,
    ) -> i32;
}

#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(uPeriod: u32) -> u32;
    fn timeEndPeriod(uPeriod: u32) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryA(lpLibFileName: *const u8) -> isize;
    fn GetProcAddress(hModule: isize, lpProcName: *const u8) -> *const c_void;
    fn CreateMutexW(
        lpMutexAttributes: *const c_void,
        bInitialOwner: i32,
        lpName: *const u16,
    ) -> isize;
    fn GetLastError() -> u32;
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> isize;
    fn QueryFullProcessImageNameW(
        hProcess: isize,
        dwFlags: u32,
        lpExeName: *mut u16,
        lpdwSize: *mut u32,
    ) -> i32;
    fn CloseHandle(hObject: isize) -> i32;
    fn GetModuleHandleW(lpModuleName: *const u16) -> isize;
    fn GetCurrentProcess() -> isize;
    fn GetCurrentProcessId() -> u32;
    fn GetCurrentThreadId() -> u32;
    fn GetProcessTimes(
        hProcess: isize,
        lpCreationTime: *mut FILETIME,
        lpExitTime: *mut FILETIME,
        lpKernelTime: *mut FILETIME,
        lpUserTime: *mut FILETIME,
    ) -> i32;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}

fn filetime_seconds(time: FILETIME) -> f64 {

    let ticks = ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
    ticks as f64 * 1e-7
}

pub fn process_cpu_seconds() -> f64 {
    unsafe {
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();

        if GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        ) == 0
        {
            return 0.0;
        }
        filetime_seconds(kernel) + filetime_seconds(user)
    }
}

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

pub fn window_process(hwnd: HWND) -> String {
    if hwnd == 0 {
        return String::new();
    }
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return String::new();
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; 260];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if ok == 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..size as usize])
            .rsplit('\\')
            .next()
            .unwrap_or("")
            .to_lowercase()
    }
}

const ERROR_ALREADY_EXISTS: u32 = 183;

pub fn claim_single_instance(name: &str) -> bool {
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    wide.push(0);
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, wide.as_ptr());
        if handle == 0 {
            return true;
        }
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

#[derive(Clone, Copy)]
pub struct ButtonSpec {
    pub down: u32,
    pub up: u32,
    pub data: u32,

    pub vk: u32,
}

pub fn button_spec(name: &str) -> ButtonSpec {
    match name {
        "right" => ButtonSpec {
            down: MOUSEEVENTF_RIGHTDOWN,
            up: MOUSEEVENTF_RIGHTUP,
            data: 0,
            vk: 0x02,
        },
        "middle" => ButtonSpec {
            down: MOUSEEVENTF_MIDDLEDOWN,
            up: MOUSEEVENTF_MIDDLEUP,
            data: 0,
            vk: 0x04,
        },
        "mouse4" => ButtonSpec {
            down: MOUSEEVENTF_XDOWN,
            up: MOUSEEVENTF_XUP,
            data: XBUTTON1,
            vk: 0x05,
        },
        "mouse5" => ButtonSpec {
            down: MOUSEEVENTF_XDOWN,
            up: MOUSEEVENTF_XUP,
            data: XBUTTON2,
            vk: 0x06,
        },
        _ => ButtonSpec {
            down: MOUSEEVENTF_LEFTDOWN,
            up: MOUSEEVENTF_LEFTUP,
            data: 0,
            vk: 0x01,
        },
    }
}

pub fn mouse_event(spec: &ButtonSpec, flags: u32) -> INPUT {
    INPUT {
        kind: INPUT_MOUSE,
        payload: INPUT_PAYLOAD {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: spec.data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

pub fn build_burst(spec: &ButtonSpec, pairs: usize) -> Vec<INPUT> {
    let mut out = Vec::with_capacity(pairs * 2);
    for _ in 0..pairs {
        out.push(mouse_event(spec, spec.down));
        out.push(mouse_event(spec, spec.up));
    }
    out
}

pub fn build_single(spec: &ButtonSpec, press: bool) -> Vec<INPUT> {
    vec![mouse_event(spec, if press { spec.down } else { spec.up })]
}

fn is_extended(vk: u16) -> bool {
    matches!(
        vk,
        0x21..=0x28
            | 0x2D
            | 0x2E
            | 0x5B
            | 0x5C
            | 0x5D
            | 0x6F
            | 0x90
            | 0xA3
            | 0xA5
    )
}

pub fn key_event(vk: u16, release: bool) -> INPUT {
    let mut flags = if release { KEYEVENTF_KEYUP } else { 0 };
    if is_extended(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        kind: INPUT_KEYBOARD,
        payload: INPUT_PAYLOAD {
            ki: KEYBDINPUT {
                wVk: vk,

                wScan: unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 },
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

pub fn unicode_event(unit: u16, release: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if release {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        kind: INPUT_KEYBOARD,
        payload: INPUT_PAYLOAD {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

pub fn char_to_vk(ch: char) -> Option<(u16, bool)> {
    let unit = u16::try_from(ch as u32).ok()?;
    let result = unsafe { VkKeyScanW(unit) };
    if result == -1 {
        return None;
    }
    let vk = (result & 0xFF) as u16;
    let shift = (result >> 8) & 0x01 != 0;
    Some((vk, shift))
}

#[inline]
pub fn send_inputs(items: &[INPUT]) -> u32 {
    if items.is_empty() {
        return 0;
    }
    unsafe { SendInput(items.len() as u32, items.as_ptr(), size_of::<INPUT>() as i32) }
}

const WH_MOUSE_LL: i32 = 14;
const WH_KEYBOARD_LL: i32 = 13;

static MOUSE_PHYSICAL: [AtomicBool; 5] = [
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
];

static MOUSE_HOOK_READY: AtomicBool = AtomicBool::new(false);

fn mouse_slot(vk: u32) -> Option<usize> {
    match vk {
        0x01 => Some(0),
        0x02 => Some(1),
        0x04 => Some(2),
        0x05 => Some(3),
        0x06 => Some(4),
        _ => None,
    }
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 && lparam != 0 {
        let info = &*(lparam as *const MSLLHOOKSTRUCT);

        if info.dwExtraInfo != SIGNATURE {
            let x_button = (info.mouseData >> 16) as u16;
            let entry = match wparam as u32 {
                WM_LBUTTONDOWN => Some((0usize, true)),
                WM_LBUTTONUP => Some((0, false)),
                WM_RBUTTONDOWN => Some((1, true)),
                WM_RBUTTONUP => Some((1, false)),
                WM_MBUTTONDOWN => Some((2, true)),
                WM_MBUTTONUP => Some((2, false)),
                WM_XBUTTONDOWN => Some((if x_button == 2 { 4 } else { 3 }, true)),
                WM_XBUTTONUP => Some((if x_button == 2 { 4 } else { 3 }, false)),
                _ => None,
            };
            if let Some((slot, down)) = entry {
                MOUSE_PHYSICAL[slot].store(down, Ordering::Relaxed);

                if !down && crate::recorder::is_recording() {
                    const NAMES: [&str; 5] =
                        ["left", "right", "middle", "mouse4", "mouse5"];
                    crate::recorder::on_mouse_up(NAMES[slot], info.pt.x, info.pt.y);
                }
            }

            if wparam as u32 == WM_MOUSEWHEEL && crate::recorder::is_recording() {

                let delta = (info.mouseData >> 16) as i16 as i32 / 120;
                if delta != 0 {
                    crate::recorder::on_scroll(delta, info.pt.x, info.pt.y);
                }
            }
        }
    }

    CallNextHookEx(0, code, wparam, lparam)
}

pub fn install_mouse_hook() {
    std::thread::Builder::new()
        .name("mouse-hook".into())
        .spawn(|| unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let hook = SetWindowsHookExW(WH_MOUSE_LL, mouse_hook_proc, module, 0);
            if hook == 0 {
                eprintln!("low-level mouse hook failed to install");
                return;
            }
            MOUSE_HOOK_READY.store(true, Ordering::Relaxed);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {

            }

            MOUSE_HOOK_READY.store(false, Ordering::Relaxed);
            UnhookWindowsHookEx(hook);
        })
        .ok();
}

static KEYBOARD_HOOK_THREAD: AtomicU32 = AtomicU32::new(0);
const WM_QUIT: u32 = 0x0012;

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 && lparam != 0 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);

        if info.dwExtraInfo != SIGNATURE
            && matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP)
        {
            crate::recorder::on_key_up(info.vkCode);
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

pub fn install_keyboard_hook() {
    if KEYBOARD_HOOK_THREAD.load(Ordering::Relaxed) != 0 {
        return;
    }

    std::thread::Builder::new()
        .name("record-hook".into())
        .spawn(|| unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, keyboard_hook_proc, module, 0);
            if hook == 0 {
                eprintln!("low-level keyboard hook failed to install");
                return;
            }
            KEYBOARD_HOOK_THREAD.store(GetCurrentThreadId(), Ordering::Release);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {}

            KEYBOARD_HOOK_THREAD.store(0, Ordering::Release);
            UnhookWindowsHookEx(hook);
        })
        .ok();
}

pub fn remove_keyboard_hook() {
    let thread = KEYBOARD_HOOK_THREAD.swap(0, Ordering::AcqRel);
    if thread != 0 {

        unsafe { PostThreadMessageW(thread, WM_QUIT, 0, 0) };
    }
}

pub fn point_over_own_app(x: i32, y: i32) -> bool {
    unsafe {
        let hwnd = root_window(WindowFromPoint(POINT { x, y }));
        hwnd != 0 && (hwnd == OWN_HWND.load(Ordering::Relaxed) || is_own_window(hwnd))
    }
}

pub fn own_app_focused() -> bool {
    unsafe {
        let hwnd = root_window(GetForegroundWindow());
        hwnd != 0 && (hwnd == OWN_HWND.load(Ordering::Relaxed) || is_own_window(hwnd))
    }
}

pub fn physical_mouse_down(vk: u32) -> Option<bool> {
    if !MOUSE_HOOK_READY.load(Ordering::Relaxed) {
        return None;
    }
    mouse_slot(vk).map(|slot| MOUSE_PHYSICAL[slot].load(Ordering::Relaxed))
}

#[inline]
pub fn key_down(vk: u32) -> bool {
    if vk == 0 {
        return false;
    }
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

#[inline]
pub fn bind_held(vk: u32) -> bool {
    if vk == 0 {
        return false;
    }
    match physical_mouse_down(vk) {
        Some(down) => down,
        None => key_down(vk),
    }
}

pub fn scan_first_pressed() -> Option<u32> {
    for vk in 1u32..=254 {
        if vk == 0x03 {
            continue;
        }
        if bind_held(vk) {
            return Some(vk);
        }
    }
    None
}

pub fn begin_timer_period() {
    unsafe {
        timeBeginPeriod(1);
    }
}

pub fn end_timer_period() {
    unsafe {
        timeEndPeriod(1);
    }
}

pub fn window_title(hwnd: HWND) -> String {
    if hwnd == 0 {
        return String::new();
    }
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let written = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
        if written <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..written as usize])
    }
}

pub fn foreground_title() -> String {
    window_title(unsafe { GetForegroundWindow() })
}

pub fn window_alive(hwnd: HWND) -> bool {
    hwnd != 0 && unsafe { IsWindow(hwnd) != 0 }
}

pub fn window_class(hwnd: HWND) -> String {
    if hwnd == 0 {
        return String::new();
    }
    unsafe {
        let mut buffer = vec![0u16; 256];
        let written = GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if written <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..written as usize])
    }
}

pub fn ignores_posted_input(hwnd: HWND) -> bool {
    let process = window_process(hwnd);
    let class = window_class(hwnd);

    const RAW_INPUT_PROCESSES: [&str; 6] = [
        "robloxplayerbeta.exe",
        "robloxstudiobeta.exe",
        "javaw.exe",
        "fortniteclient-win64-shipping.exe",
        "valorant-win64-shipping.exe",
        "csgo.exe",
    ];
    const RAW_INPUT_CLASSES: [&str; 4] = [
        "WINDOWSCLIENT",
        "UnityWndClass",
        "UnrealWindow",
        "SDL_app",
    ];

    RAW_INPUT_PROCESSES.contains(&process.as_str())
        || RAW_INPUT_CLASSES.iter().any(|c| class.eq_ignore_ascii_case(c))
}

const CURSOR_BGRA: &[u8] = include_bytes!("cursor_bgra.bin");
const CURSOR_SIZE: i32 = 32;
const DIB_RGB_COLORS: u32 = 0;

static CURSOR_HANDLE: AtomicIsize = AtomicIsize::new(0);
static OWN_HWND: AtomicIsize = AtomicIsize::new(0);

fn cursor_handle() -> isize {
    let existing = CURSOR_HANDLE.load(Ordering::Relaxed);
    if existing != 0 {
        return existing;
    }

    unsafe {
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: CURSOR_SIZE,
            biHeight: CURSOR_SIZE,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            ..Default::default()
        };

        let mut bits: *mut c_void = std::ptr::null_mut();
        let colour = CreateDIBSection(0, &header, DIB_RGB_COLORS, &mut bits, 0, 0);
        if colour == 0 || bits.is_null() {
            return 0;
        }
        std::ptr::copy_nonoverlapping(CURSOR_BGRA.as_ptr(), bits as *mut u8, CURSOR_BGRA.len());

        let mask_bytes = vec![0u8; (CURSOR_SIZE * CURSOR_SIZE / 8) as usize];
        let mask = CreateBitmap(
            CURSOR_SIZE,
            CURSOR_SIZE,
            1,
            1,
            mask_bytes.as_ptr() as *const c_void,
        );
        if mask == 0 {
            DeleteObject(colour);
            return 0;
        }

        let info = ICONINFO {
            fIcon: 0,
            xHotspot: (CURSOR_SIZE / 2) as u32,
            yHotspot: (CURSOR_SIZE / 2) as u32,
            hbmMask: mask,
            hbmColor: colour,
        };
        let cursor = CreateIconIndirect(&info);

        DeleteObject(colour);
        DeleteObject(mask);

        CURSOR_HANDLE.store(cursor, Ordering::Relaxed);
        cursor
    }
}

const GCLP_HCURSOR: i32 = -12;
const IDC_ARROW: usize = 32512;

pub fn set_cursor_from_rgba(width: i32, height: i32, rgba: &[u8], hx: u32, hy: u32) -> bool {
    if width <= 0 || height <= 0 || rgba.len() < (width * height * 4) as usize {
        return false;
    }

    unsafe {
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            ..Default::default()
        };

        let mut bits: *mut c_void = std::ptr::null_mut();
        let colour = CreateDIBSection(0, &header, DIB_RGB_COLORS, &mut bits, 0, 0);
        if colour == 0 || bits.is_null() {
            return false;
        }

        let dst = bits as *mut u8;
        let stride = (width * 4) as usize;
        for y in 0..height as usize {
            let src_row = (height as usize - 1 - y) * stride;
            let dst_row = y * stride;
            for x in 0..width as usize {
                let s = src_row + x * 4;
                let d = dst_row + x * 4;
                *dst.add(d) = rgba[s + 2];
                *dst.add(d + 1) = rgba[s + 1];
                *dst.add(d + 2) = rgba[s];
                *dst.add(d + 3) = rgba[s + 3];
            }
        }

        let mask_bytes = vec![0u8; ((width * height) / 8).max(1) as usize];
        let mask = CreateBitmap(width, height, 1, 1, mask_bytes.as_ptr() as *const c_void);
        if mask == 0 {
            DeleteObject(colour);
            return false;
        }

        let info = ICONINFO {
            fIcon: 0,
            xHotspot: hx,
            yHotspot: hy,
            hbmMask: mask,
            hbmColor: colour,
        };
        let cursor = CreateIconIndirect(&info);

        DeleteObject(colour);
        DeleteObject(mask);
        if cursor == 0 {
            return false;
        }

        let previous = CURSOR_HANDLE.swap(cursor, Ordering::Relaxed);
        if previous != 0 {
            DestroyCursor(previous);
        }

        let hwnd = OWN_HWND.load(Ordering::Relaxed);
        if hwnd != 0 {
            SetClassLongPtrW(hwnd, GCLP_HCURSOR, cursor);
        }
        SetCursor(cursor);
        true
    }
}

pub fn clear_custom_cursor() {
    let previous = CURSOR_HANDLE.swap(0, Ordering::Relaxed);
    unsafe {
        if previous != 0 {
            DestroyCursor(previous);
        }
        let arrow = LoadCursorW(0, IDC_ARROW);
        let hwnd = OWN_HWND.load(Ordering::Relaxed);
        if hwnd != 0 {
            SetClassLongPtrW(hwnd, GCLP_HCURSOR, arrow);
        }
        SetCursor(arrow);
    }
}

pub fn install_cursor(hwnd: HWND) {
    let cursor = cursor_handle();
    if cursor == 0 || hwnd == 0 {
        return;
    }
    unsafe {
        SetClassLongPtrW(hwnd, GCLP_HCURSOR, cursor);
    }
}

pub fn remember_own_window(hwnd: HWND) {
    OWN_HWND.store(hwnd, Ordering::Relaxed);
}

struct EdgeProbe {
    x: i32,
    y: i32,
    margin: i32,
    skip: HWND,
    hit: bool,
}

const DWMWA_CLOAKED: u32 = 14;
const WS_EX_TOOLWINDOW: i32 = 0x0000_0080;

unsafe fn is_real_window(hwnd: HWND) -> bool {
    if IsWindowVisible(hwnd) == 0 {
        return false;
    }

    if GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW != 0 {
        return false;
    }

    let mut cloaked: u32 = 0;
    let ok = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut u32 as *mut c_void,
        std::mem::size_of::<u32>() as u32,
    );

    ok != 0 || cloaked == 0
}

unsafe fn is_own_window(hwnd: HWND) -> bool {
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    pid != 0 && pid == GetCurrentProcessId()
}

unsafe extern "system" fn edge_probe_proc(hwnd: HWND, lparam: isize) -> i32 {
    let probe = &mut *(lparam as *mut EdgeProbe);

    if hwnd == probe.skip
        || is_own_window(hwnd)
        || !is_real_window(hwnd)
        || IsIconic(hwnd) != 0
    {
        return 1;
    }

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return 1;
    }

    if rect.right - rect.left < 64 || rect.bottom - rect.top < 64 {
        return 1;
    }

    let m = probe.margin;
    let within_outer = probe.x >= rect.left - m
        && probe.x <= rect.right + m
        && probe.y >= rect.top - m
        && probe.y <= rect.bottom + m;
    let within_inner = probe.x >= rect.left + m
        && probe.x <= rect.right - m
        && probe.y >= rect.top + m
        && probe.y <= rect.bottom - m;

    if within_outer && !within_inner {
        probe.hit = true;
        return 0;
    }
    1
}

const CLR_INVALID: u32 = 0xFFFF_FFFF;

pub fn screen_pixel(x: i32, y: i32) -> Option<(u8, u8, u8)> {
    unsafe {
        let dc = GetDC(0);
        if dc == 0 {
            return None;
        }
        let value = GetPixel(dc, x, y);
        ReleaseDC(0, dc);

        if value == CLR_INVALID {
            return None;
        }

        Some((
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
        ))
    }
}

const SRCCOPY: u32 = 0x00CC_0020;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

pub fn screen_size() -> (i32, i32) {
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

pub struct Grabber {
    screen: isize,
    mem: isize,
    bitmap: isize,
    previous: isize,
    bits: *mut c_void,
    width: i32,
    height: i32,
}

impl Grabber {
    pub fn new(width: i32, height: i32) -> Option<Grabber> {
        if width <= 0 || height <= 0 {
            return None;
        }

        unsafe {
            let screen = GetDC(0);
            if screen == 0 {
                return None;
            }

            let mem = CreateCompatibleDC(screen);
            if mem == 0 {
                ReleaseDC(0, screen);
                return None;
            }

            let header = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };

            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(mem, &header, DIB_RGB_COLORS, &mut bits, 0, 0);
            if bitmap == 0 || bits.is_null() {
                DeleteDC(mem);
                ReleaseDC(0, screen);
                return None;
            }

            let previous = SelectObject(mem, bitmap);

            Some(Grabber {
                screen,
                mem,
                bitmap,
                previous,
                bits,
                width,
                height,
            })
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn grab(&mut self, x: i32, y: i32) -> Option<&[u32]> {
        unsafe {
            let ok = BitBlt(
                self.mem,
                0,
                0,
                self.width,
                self.height,
                self.screen,
                x,
                y,
                SRCCOPY,
            );
            if ok == 0 {
                return None;
            }

            let count = (self.width * self.height) as usize;
            Some(std::slice::from_raw_parts(self.bits as *const u32, count))
        }
    }
}

impl Drop for Grabber {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.mem);
            ReleaseDC(0, self.screen);
        }
    }
}

pub fn cursor_over_own_app() -> bool {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 {
            return false;
        }
        let hwnd = root_window(WindowFromPoint(point));
        hwnd != 0 && (hwnd == OWN_HWND.load(Ordering::Relaxed) || is_own_window(hwnd))
    }
}

pub fn cursor_near_window_edge(margin: i32) -> bool {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 {
            return false;
        }
        let mut probe = EdgeProbe {
            x: point.x,
            y: point.y,
            margin,
            skip: OWN_HWND.load(Ordering::Relaxed),
            hit: false,
        };
        EnumWindows(edge_probe_proc, &mut probe as *mut EdgeProbe as isize);
        probe.hit
    }
}

const SMTO_ABORTIFHUNG: u32 = 0x0002;

const HT_DANGER: [usize; 15] = [
    2,
    3,
    4,
    8,
    9,
    10,
    11,
    12,
    13,
    14,
    15,
    16,
    17,
    18,
    20,
];

pub fn cursor_over_window_chrome() -> bool {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 {
            return false;
        }

        let hwnd = root_window(WindowFromPoint(point));
        if hwnd == 0 || hwnd == OWN_HWND.load(Ordering::Relaxed) {
            return false;
        }

        let mut result: usize = 0;
        let ok = SendMessageTimeoutW(
            hwnd,
            WM_NCHITTEST,
            0,
            pack_point(point.x, point.y),
            SMTO_ABORTIFHUNG,
            30,
            &mut result,
        );
        if ok == 0 {
            return false;
        }

        HT_DANGER.contains(&result)
    }
}

const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;

pub fn cursor_near_screen_edge(margin: i32) -> bool {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 {
            return false;
        }
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let right = left + GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let bottom = top + GetSystemMetrics(SM_CYVIRTUALSCREEN);

        point.x - left < margin
            || right - point.x < margin
            || point.y - top < margin
            || bottom - point.y < margin
    }
}

pub fn client_center(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect) == 0 {
            return (0, 0);
        }
        ((rect.right - rect.left) / 2, (rect.bottom - rect.top) / 2)
    }
}

pub fn screen_to_client(hwnd: HWND, x: i32, y: i32) -> (i32, i32) {
    unsafe {
        let mut point = POINT { x, y };
        ScreenToClient(hwnd, &mut point);
        (point.x, point.y)
    }
}

pub fn foreground_target() -> Option<Target> {
    let hwnd = root_window(unsafe { GetForegroundWindow() });
    if hwnd == 0 || hwnd == OWN_HWND.load(Ordering::Relaxed) {
        return None;
    }
    let (x, y) = client_center(hwnd);
    Some(Target { hwnd, x, y })
}

struct WindowCollector {
    found: Vec<(HWND, String, String)>,
    skip: HWND,
}

unsafe extern "system" fn collect_windows_proc(hwnd: HWND, lparam: isize) -> i32 {
    let collector = &mut *(lparam as *mut WindowCollector);

    if hwnd == collector.skip || is_own_window(hwnd) || !is_real_window(hwnd) {
        return 1;
    }

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return 1;
    }

    if rect.right - rect.left < 100 || rect.bottom - rect.top < 100 {
        return 1;
    }

    let title = window_title(hwnd);
    if title.trim().is_empty() {
        return 1;
    }

    collector
        .found
        .push((hwnd, title, window_process(hwnd)));
    1
}

pub fn list_windows() -> Vec<(HWND, String, String)> {
    let mut collector = WindowCollector {
        found: Vec::new(),
        skip: OWN_HWND.load(Ordering::Relaxed),
    };
    unsafe {
        EnumWindows(
            collect_windows_proc,
            &mut collector as *mut WindowCollector as isize,
        );
    }
    collector.found
}

pub fn find_window(title: &str, process: &str) -> Option<HWND> {
    let needle = title.trim().to_lowercase();
    let exe = process.trim().to_lowercase();

    list_windows()
        .into_iter()
        .find(|(_, window_title, window_process)| {
            let title_ok = needle.is_empty() || window_title.to_lowercase().contains(&needle);
            let process_ok = exe.is_empty() || *window_process == exe;
            title_ok && process_ok
        })
        .map(|(hwnd, _, _)| hwnd)
}

const GA_ROOT: u32 = 2;

pub fn root_window(hwnd: HWND) -> HWND {
    if hwnd == 0 {
        return 0;
    }
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root == 0 {
        hwnd
    } else {
        root
    }
}

#[derive(Clone, Copy)]
pub struct Target {
    pub hwnd: HWND,
    pub x: i32,
    pub y: i32,
}

pub fn target_under_cursor() -> Option<Target> {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 {
            return None;
        }
        let hwnd = WindowFromPoint(point);
        if hwnd == 0 {
            return None;
        }
        let mut client = point;
        ScreenToClient(hwnd, &mut client);
        Some(Target {
            hwnd,
            x: client.x,
            y: client.y,
        })
    }
}

pub fn cursor_position() -> (i32, i32) {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 {
            return (0, 0);
        }
        (point.x, point.y)
    }
}

pub fn move_cursor(x: i32, y: i32) {
    unsafe {
        SetCursorPos(x, y);
    }
}

/// Is the pointer sitting on the middle of the window in front?
///
/// This is the giveaway for first person. A game that locks the mouse puts it
/// back in the centre of its own window every frame, and reads how far it had
/// moved as how far you looked. So a pointer that keeps turning up in the
/// middle of the front window is a pointer the game is holding.
///
/// `slack` is how far off centre still counts. Ask for a loose one before
/// moving anything, since the pointer will not be sitting exactly on centre
/// between frames, and a tight one straight after a move you made yourself,
/// where landing back on centre means something put it there.
pub fn cursor_centred(slack: i32) -> bool {
    unsafe {
        let window = GetForegroundWindow();
        if window == 0 {
            return false;
        }

        let mut client = RECT::default();
        if GetClientRect(window, &mut client) == 0 {
            return false;
        }

        // a window too small to play in cannot tell us anything
        if client.right < 200 || client.bottom < 200 {
            return false;
        }

        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 || ScreenToClient(window, &mut point) == 0 {
            return false;
        }

        (point.x - client.right / 2).abs() <= slack && (point.y - client.bottom / 2).abs() <= slack
    }
}

/// Has something taken the pointer away from you?
///
/// Two signs, either of which is enough, because games do not all go about it
/// the same way:
///
/// * The pointer keeps landing on the centre of the front window. This is the
///   one that catches Roblox, which locks the mouse by re-centring it rather
///   than by fencing it in.
/// * The pointer is fenced into a rectangle smaller than the desktop. That
///   fence is `ClipCursor`, which some games use instead.
///
/// A hidden pointer sounds like it belongs on that list and does not. Windows
/// counts cursor visibility per input queue, so a game hiding its own pointer
/// need not read as hidden from out here -- and worse, plenty of ordinary
/// things hide it, including Windows itself while you type. It says "a game
/// has the mouse" far too often to be worth asking.
pub fn cursor_locked(slack: i32) -> bool {
    if cursor_centred(slack) {
        return true;
    }

    unsafe {
        let mut clip = RECT::default();
        if GetClipCursor(&mut clip) != 0 {
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            // a fence at all, rather than the whole desktop
            if width > 0
                && height > 0
                && (clip.right - clip.left < width || clip.bottom - clip.top < height)
            {
                return true;
            }
        }
    }

    false
}

/// An absolute move carrying the destination, so a click sent alongside it
/// lands where we mean rather than wherever the cursor happens to be.
pub fn move_event(x: i32, y: i32) -> INPUT {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(2);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(2);

        let nx = ((x - left) as i64 * 65535 / (width - 1) as i64) as i32;
        let ny = ((y - top) as i64 * 65535 / (height - 1) as i64) as i32;

        INPUT {
            kind: INPUT_MOUSE,
            payload: INPUT_PAYLOAD {
                mi: MOUSEINPUT {
                    dx: nx,
                    dy: ny,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    time: 0,
                    dwExtraInfo: SIGNATURE,
                },
            },
        }
    }
}

pub fn move_relative(dx: i32, dy: i32) {
    if dx == 0 && dy == 0 {
        return;
    }
    let input = INPUT {
        kind: INPUT_MOUSE,
        payload: INPUT_PAYLOAD {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    };
    send_inputs(&[input]);
}

pub fn wheel_event(notches: i32) -> INPUT {
    INPUT {
        kind: INPUT_MOUSE,
        payload: INPUT_PAYLOAD {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: (notches * WHEEL_DELTA) as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

#[inline]
pub fn pack_point(x: i32, y: i32) -> isize {
    let lo = (x as u32) & 0xFFFF;
    let hi = (y as u32) & 0xFFFF;
    (((hi << 16) | lo) as u32) as i32 as isize
}

#[inline]
pub fn key_lparam(vk: u16, release: bool) -> isize {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } & 0xFF;
    let mut value: u32 = 1 | (scan << 16);
    if release {
        value |= 0xC000_0000;
    }
    value as i32 as isize
}

#[inline]
pub fn post(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> bool {
    unsafe { PostMessageW(hwnd, msg, wparam, lparam) != 0 }
}

pub fn scan_code(vk: u16) -> u16 {
    unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 }
}

const WCA_ACCENT_POLICY: i32 = 19;

const ACCENT_DISABLED: i32 = 0;
const ACCENT_ENABLE_BLURBEHIND: i32 = 3;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: i32 = 4;

#[repr(C)]
struct AccentPolicy {
    accent_state: i32,
    accent_flags: i32,
    gradient_color: u32,
    animation_id: i32,
}

#[repr(C)]
struct WindowCompositionAttribData {
    attribute: i32,
    data: *mut c_void,
    size_of_data: usize,
}

type SetWindowCompositionAttributeFn =
    unsafe extern "system" fn(HWND, *mut WindowCompositionAttribData) -> i32;

fn resolve_composition_fn() -> Option<SetWindowCompositionAttributeFn> {
    unsafe {
        let module = LoadLibraryA(b"user32.dll\0".as_ptr());
        if module == 0 {
            return None;
        }
        let proc = GetProcAddress(module, b"SetWindowCompositionAttribute\0".as_ptr());
        if proc.is_null() {
            return None;
        }
        Some(std::mem::transmute::<
            *const c_void,
            SetWindowCompositionAttributeFn,
        >(proc))
    }
}

fn abgr(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

pub fn apply_blur(hwnd: HWND, enabled: bool, acrylic: bool, tint: (u8, u8, u8), alpha: u8) -> bool {
    let Some(set_attr) = resolve_composition_fn() else {
        return false;
    };

    let state = if !enabled {
        ACCENT_DISABLED
    } else if acrylic {
        ACCENT_ENABLE_ACRYLICBLURBEHIND
    } else {
        ACCENT_ENABLE_BLURBEHIND
    };

    let mut policy = AccentPolicy {
        accent_state: state,
        accent_flags: 2,
        gradient_color: abgr(tint.0, tint.1, tint.2, alpha),
        animation_id: 0,
    };

    let mut data = WindowCompositionAttribData {
        attribute: WCA_ACCENT_POLICY,
        data: &mut policy as *mut AccentPolicy as *mut c_void,
        size_of_data: size_of::<AccentPolicy>(),
    };

    unsafe { set_attr(hwnd, &mut data as *mut WindowCompositionAttribData) != 0 }
}

const DWMWA_NCRENDERING_POLICY: u32 = 2;
const DWMNCRP_DISABLED: i32 = 1;

pub fn disable_nc_rendering(hwnd: HWND) {
    if hwnd == 0 {
        return;
    }
    let policy = DWMNCRP_DISABLED;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const i32 as *const c_void,
            size_of::<i32>() as u32,
        );
    }
}

const DWMWA_BORDER_COLOR: u32 = 34;
const DWMWA_CAPTION_COLOR: u32 = 35;

const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

fn colorref(r: u8, g: u8, b: u8) -> u32 {
    ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

pub fn blend_frame_colors(hwnd: HWND, tint: (u8, u8, u8)) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let caption = colorref(tint.0, tint.1, tint.2);
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &caption as *const u32 as *const c_void,
            size_of::<u32>() as u32,
        );

        let border = DWMWA_COLOR_NONE;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const u32 as *const c_void,
            size_of::<u32>() as u32,
        );
    }
}

const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWCP_ROUND: i32 = 2;

pub fn prefer_rounded_corners(hwnd: HWND) {
    if hwnd == 0 {
        return;
    }
    let preference = DWMWCP_ROUND;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const i32 as *const c_void,
            size_of::<i32>() as u32,
        );
    }
}

#[allow(dead_code)]
pub fn round_region(hwnd: HWND, width: i32, height: i32, radius: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    unsafe {
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2);
        if region != 0 {
            SetWindowRgn(hwnd, region, 1);
        }
    }
}

const GWL_STYLE: i32 = -16;

const WS_CAPTION: i32 = 0x00C0_0000;
const WS_THICKFRAME: i32 = 0x0004_0000;
const WS_SYSMENU: i32 = 0x0008_0000;
const WS_MINIMIZEBOX: i32 = 0x0002_0000;
const WS_MAXIMIZEBOX: i32 = 0x0001_0000;

const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;

const GWLP_WNDPROC: i32 = -4;
const WM_NCCALCSIZE: u32 = 0x0083;
const WM_SETCURSOR: u32 = 0x0020;
const WM_NCHITTEST: u32 = 0x0084;

pub type WndProc = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

static PREVIOUS_PROC: AtomicIsize = AtomicIsize::new(0);

unsafe extern "system" fn frame_proc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    if msg == WM_NCCALCSIZE && wparam != 0 {
        return 0;
    }

    if msg == WM_SETCURSOR {

        let cursor = cursor_handle();
        if cursor != 0 {
            SetCursor(cursor);
            return 1;
        }
    }

    let previous = PREVIOUS_PROC.load(Ordering::Relaxed);
    let result = if previous == 0 {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    } else {
        let chain: WndProc = std::mem::transmute(previous);
        CallWindowProcW(chain, hwnd, msg, wparam, lparam)
    };

    result
}

static SUBCLASSED_HWND: AtomicIsize = AtomicIsize::new(0);

pub fn suppress_non_client_area(hwnd: HWND) {
    if hwnd == 0 || SUBCLASSED_HWND.load(Ordering::Relaxed) == hwnd {
        return;
    }
    unsafe {
        let replacement = frame_proc as WndProc as usize as isize;
        let previous = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, replacement);
        SUBCLASSED_HWND.store(hwnd, Ordering::Relaxed);
        PREVIOUS_PROC.store(previous, Ordering::Relaxed);

        SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

pub fn strip_caption(hwnd: HWND) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE);

        let stripped = style & !(WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
        if stripped != style {
            SetWindowLongW(hwnd, GWL_STYLE, stripped);
        }
        SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}
