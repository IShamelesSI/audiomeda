//! Tray icon, menu, global hotkeys and toast notifications, all on one thread that owns a
//! hidden window (Windows delivers tray clicks and hotkeys as window messages).

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use audiomeda_protocol::{Request, Response, State, parse_hotkey};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::{HSTRING, PCWSTR, w};

use crate::STOP;
use crate::win::Res;

pub type Handler = Arc<dyn Fn(Request) -> Response + Send + Sync>;

struct Ctx {
    handler: Handler,
    state: Arc<Mutex<State>>,
}

static CTX: OnceLock<Ctx> = OnceLock::new();
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
const WM_TRAY: u32 = WM_APP + 1;
const TIMER: usize = 1;
const CMD_MICS: usize = 1;
const CMD_AUTOSTART: usize = 2;
const CMD_QUIT: usize = 3;
const CMD_OPEN: usize = 4;
const CMD_PRESET: usize = 100; // + preset index

thread_local! {
    /// Hotkeys currently registered with Windows; the index is the hotkey id.
    static REGISTERED: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn spawn(handler: Handler, state: Arc<Mutex<State>>) -> std::thread::JoinHandle<()> {
    let _ = CTX.set(Ctx { handler, state });
    std::thread::spawn(|| {
        if let Err(e) = run() {
            println!("tray stopped: {e}");
        }
    })
}

fn ctx() -> &'static Ctx {
    CTX.get().expect("tray context is set before the thread starts")
}

fn run() -> Res<()> {
    unsafe {
        let hinst = GetModuleHandleW(None)?.into();
        let class = w!("AudiomedaTray");
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: hinst, lpszClassName: class, ..Default::default() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0), class, w!("Audiomeda tray"), WINDOW_STYLE(0), 0, 0, 0, 0, None, None, Some(hinst), None,
        )?;
        // explorer restarts broadcast this; the icon must be added again
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
        add_icon(hwnd);
        SetTimer(Some(hwnd), TIMER, 250, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            match lp.0 as u32 {
                WM_LBUTTONUP => open_ui(hwnd),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
        }
        WM_HOTKEY => {
            let action = REGISTERED.with_borrow(|r| r.get(wp.0).map(|(_, a)| a.clone()));
            if let Some(a) = action {
                run_action(hwnd, &a, true);
            }
        }
        WM_TIMER => tick(hwnd),
        m if m == TASKBAR_CREATED.load(Ordering::Relaxed) => add_icon(hwnd),
        _ => return unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
    LRESULT(0)
}

fn copy_wide(dst: &mut [u16], s: &str) {
    let v: Vec<u16> = s.encode_utf16().take(dst.len() - 1).collect();
    dst[..v.len()].copy_from_slice(&v);
    dst[v.len()] = 0;
}

fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW { cbSize: size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: 1, ..Default::default() }
}

fn add_icon(hwnd: HWND) {
    let mut n = icon_data(hwnd);
    n.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = tray_icon();
    copy_wide(&mut n.szTip, "Audiomeda");
    let _ = unsafe { Shell_NotifyIconW(NIM_ADD, &n) };
}

/// The pink icon embedded by build.rs (resource id 2), at the tray's current size.
fn tray_icon() -> HICON {
    unsafe {
        let Ok(hinst) = GetModuleHandleW(None) else { return HICON::default() };
        let (cx, cy) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
        LoadImageW(Some(hinst.into()), PCWSTR(std::ptr::without_provenance(2)) /* MAKEINTRESOURCE(2) */, IMAGE_ICON, cx, cy, LR_DEFAULTCOLOR)
            .map(|h| HICON(h.0))
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))
            .unwrap_or_default()
    }
}

/// Windows shows this as a toast.
fn notify(hwnd: HWND, text: &str) {
    let mut n = icon_data(hwnd);
    n.uFlags = NIF_INFO;
    n.dwInfoFlags = NIIF_INFO;
    copy_wide(&mut n.szInfoTitle, "Audiomeda");
    copy_wide(&mut n.szInfo, text);
    let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &n) };
}

fn tick(hwnd: HWND) {
    if STOP.load(Ordering::Relaxed) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd));
            let _ = DestroyWindow(hwnd);
            PostQuitMessage(0);
        }
        return;
    }
    sync_hotkeys(hwnd);
}

/// Re-registers hotkeys whenever the state's hotkey list changed.
fn sync_hotkeys(hwnd: HWND) {
    let wanted: Vec<(String, String)> = ctx().state.lock().unwrap().hotkeys.clone().into_iter().collect();
    if REGISTERED.with_borrow(|r| *r == wanted) {
        return;
    }
    REGISTERED.with_borrow_mut(|r| {
        for id in 0..r.len() {
            let _ = unsafe { UnregisterHotKey(Some(hwnd), id as i32) };
        }
        r.clear();
    });
    let mut taken = Vec::new();
    for (id, (combo, _)) in wanted.iter().enumerate() {
        let Ok((mods, vk, _)) = parse_hotkey(combo) else { continue };
        let ok = unsafe { RegisterHotKey(Some(hwnd), id as i32, HOT_KEY_MODIFIERS(mods) | MOD_NOREPEAT, vk) };
        if ok.is_err() {
            taken.push(combo.as_str());
        }
    }
    if !taken.is_empty() {
        notify(hwnd, &format!("Hotkey already used by another app: {}", taken.join(", ")));
    }
    REGISTERED.with_borrow_mut(|r| *r = wanted.clone());
}

fn run_action(hwnd: HWND, action: &str, toast: bool) {
    let c = ctx();
    let (req, done) = match action.strip_prefix("preset:") {
        Some(name) => (Request::LoadPreset { name: name.into() }, format!("Preset: {name}")),
        None => (Request::ToggleMics, String::new()),
    };
    match (c.handler)(req) {
        Response::Error { message } => notify(hwnd, &message),
        _ if toast => {
            let text = if done.is_empty() {
                if c.state.lock().unwrap().mics_muted() { "Mics muted".into() } else { "Mics live".into() }
            } else {
                done
            };
            notify(hwnd, &text);
        }
        _ => {}
    }
}

fn show_menu(hwnd: HWND) {
    let (presets, mics_muted, has_mics) = {
        let s = ctx().state.lock().unwrap();
        let has_mics = s.inputs.keys().any(|k| k.starts_with("mic:"));
        (s.presets.keys().cloned().collect::<Vec<_>>(), s.mics_muted(), has_mics)
    };
    let flag = |on: bool, f: MENU_ITEM_FLAGS| if on { f } else { MENU_ITEM_FLAGS(0) };
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MF_STRING, CMD_OPEN, w!("Open mixer"));
        let _ = SetMenuDefaultItem(menu, CMD_OPEN as u32, 0); // bold, like left-click
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        if presets.is_empty() {
            let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, w!("No presets yet (audiomeda preset save <name>)"));
        } else if let Ok(sub) = CreatePopupMenu() {
            for (i, p) in presets.iter().enumerate() {
                let _ = AppendMenuW(sub, MF_STRING, CMD_PRESET + i, &HSTRING::from(p));
            }
            let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, w!("Load preset"));
        }
        let mics = MF_STRING | flag(mics_muted, MF_CHECKED) | flag(!has_mics, MF_GRAYED);
        let _ = AppendMenuW(menu, mics, CMD_MICS, w!("Mute mics"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING | flag(autostart_enabled(), MF_CHECKED), CMD_AUTOSTART, w!("Start with Windows"));
        let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("Quit"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // without this the menu doesn't close when you click elsewhere
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, None, hwnd, None).0 as usize;
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);

        match cmd {
            CMD_MICS => run_action(hwnd, "toggle_mics", false),
            CMD_AUTOSTART => {
                if let Err(e) = set_autostart(!autostart_enabled()) {
                    notify(hwnd, &format!("Couldn't change Start with Windows: {e}"));
                }
            }
            CMD_QUIT => STOP.store(true, Ordering::Relaxed),
            CMD_OPEN => open_ui(hwnd),
            n if n >= CMD_PRESET => {
                if let Some(p) = presets.get(n - CMD_PRESET) {
                    run_action(hwnd, &format!("preset:{p}"), false);
                }
            }
            _ => {}
        }
    }
}

/// Brings the mixer window to the front, or starts `audiomeda-ui.exe` from next to this exe.
fn open_ui(hwnd: HWND) {
    unsafe {
        if let Ok(win) = FindWindowW(PCWSTR::null(), w!("Audiomeda")) {
            let _ = ShowWindow(win, SW_RESTORE);
            let _ = SetForegroundWindow(win);
            return;
        }
    }
    let ui = std::env::current_exe().ok().and_then(|p| Some(p.parent()?.join("audiomeda-ui.exe")));
    match ui.map(|p| std::process::Command::new(p).spawn()) {
        Some(Ok(_)) => {}
        _ => notify(hwnd, "Couldn't start the mixer: audiomeda-ui.exe isn't next to the engine"),
    }
}

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const RUN_VALUE: PCWSTR = w!("Audiomeda");

fn autostart_enabled() -> bool {
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, RRF_RT_REG_SZ, None, None, None) == ERROR_SUCCESS }
}

/// Per-user "Run" registry value pointing at this exe.
fn set_autostart(on: bool) -> Res<()> {
    unsafe {
        if on {
            let cmd = format!("\"{}\"", std::env::current_exe()?.display());
            let wide: Vec<u16> = cmd.encode_utf16().chain([0]).collect();
            RegSetKeyValueW(
                HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, REG_SZ.0, Some(wide.as_ptr().cast()), (wide.len() * 2) as u32,
            )
            .ok()?;
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE).ok()?;
        }
    }
    Ok(())
}
