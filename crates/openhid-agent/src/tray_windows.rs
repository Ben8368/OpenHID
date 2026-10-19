//! The agent's Windows notification-area (tray) icon.
//!
//! Mirrors the macOS menu-bar item ([`crate::tray`]): the always-on agent
//! hosts the tray, the GUI is on-demand. Without it the app has no visible
//! presence at all once the GUI window is closed — the agent keeps working
//! but the user has no way to tell, or to get the window back (#347).
//!
//! The menu is smaller than macOS's: Settings / About / Check-for-Updates go
//! through `openhid://` deeplinks there, and Windows has no scheme
//! registration yet — so just "Show Main Window" (also the left-click action)
//! and "Quit OpenHID". Show focuses the running GUI if there is one (a
//! second launch would exit on the `openhid.lock` singleton) or spawns the
//! sibling `OpenHID.exe` / `openhid-desktop.exe`. Quit terminates the GUI
//! first — a surviving GUI's IPC retry loop would immediately respawn the
//! agent we are quitting — then exits.
//!
//! Everything runs on one dedicated thread: the hidden window, its message
//! pump, and the menu. The icon is re-added when Explorer restarts (the
//! `TaskbarCreated` broadcast), and the glyph tracks the taskbar theme
//! (black on a light taskbar, white on a dark one) as settings change.

#![expect(
    unsafe_code,
    reason = "raw win32: Shell_NotifyIconW + a hidden window's message pump — localized here"
)]
use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

mod icon;

use crate::tray_status::{self, Presentation};

use tracing::{info, warn};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::HBRUSH;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, EnumWindows, GetCursorPos, GetMessageW, GetSystemMetrics,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, MF_SEPARATOR, MF_STRING, MSG,
    PostMessageW, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SW_RESTORE,
    SetForegroundWindow, SetTimer, ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, WM_APP, WM_CONTEXTMENU, WM_DPICHANGED, WM_LBUTTONUP, WM_NULL,
    WM_RBUTTONUP, WM_SETTINGCHANGE, WM_THEMECHANGED, WM_TIMER, WNDCLASSW, WS_OVERLAPPED,
};

use crate::shutdown::{self, ShutdownRequestSender};

/// Menu device commands are disjoint from the fixed application commands.
const ID_DEVICE_BASE: usize = 100;

/// Tray callback message the icon posts to the hidden window.
const WM_TRAY: u32 = WM_APP + 1;
const WM_REFRESH: u32 = WM_APP + 2;
static TRAY_WINDOW: AtomicIsize = AtomicIsize::new(0);
/// Menu command ids returned by `TrackPopupMenu`.
const ID_SHOW: usize = 1;
const ID_QUIT: usize = 2;

/// The `TaskbarCreated` broadcast id, resolved once the window exists. Zero
/// until then; real ids are never zero (`RegisterWindowMessageW` starts at
/// 0xC000).
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

struct NativeTray {
    _icon: icon::OwnedIcon,
    presentation: Presentation,
    light: bool,
    size: i32,
}

thread_local! {
    static NATIVE_TRAY: RefCell<Option<NativeTray>> = const { RefCell::new(None) };
    /// Where the win32 tray callback hands process termination to the async
    /// lifecycle. The callback and message pump share this one tray thread.
    static SHUTDOWN_TX: RefCell<Option<ShutdownRequestSender>> = const { RefCell::new(None) };
}

/// Host a persistent message loop so tray visibility and content can change live.
///
/// Failures are logged, never fatal — the agent's real work (hook, HID++,
/// IPC) must not die because a shell icon couldn't be installed.
pub fn spawn(shutdown_tx: ShutdownRequestSender) {
    if let Err(e) = std::thread::Builder::new()
        .name("openhid-tray".into())
        .spawn(move || run_tray_loop(shutdown_tx))
    {
        warn!(error = %e, "could not spawn the tray thread");
    }
}

/// Create the hidden window, install the icon, and pump messages for the
/// agent's lifetime.
fn run_tray_loop(shutdown_tx: ShutdownRequestSender) {
    SHUTDOWN_TX.with_borrow_mut(|slot| *slot = Some(shutdown_tx));
    let class_name = wide("OpenHIDAgentTray");
    // SAFETY: plain win32 registration/creation calls with pointers that
    // outlive the calls; the class name buffer lives until thread exit.
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut::<core::ffi::c_void>() as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        if RegisterClassW(&raw const wc) == 0 {
            warn!("tray window class registration failed — no tray icon");
            return;
        }
        // A normal (never-shown) top-level window, not message-only: only
        // top-level windows receive the TaskbarCreated broadcast that tells
        // us to re-add the icon after an Explorer restart.
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide(openhid_core::brand::Helper::Agent.display_name()).as_ptr(),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            warn!("tray window creation failed — no tray icon");
            return;
        }
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            Ordering::Relaxed,
        );
        TRAY_WINDOW.store(hwnd as isize, Ordering::Relaxed);
        update_tray_icon(hwnd);
        // Bounded shell reconciliation only: no HID or inventory polling.
        SetTimer(hwnd, 1, 5_000, None);
        info!("tray message pump started");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&raw mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
        TRAY_WINDOW.store(0, Ordering::Relaxed);
        remove_tray_icon(hwnd);
    }
}

#[expect(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "WM_TRAY packs the mouse message id into the low bits of LPARAM"
)]
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_REFRESH | WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DPICHANGED | WM_TIMER => {
            // SAFETY: the owning message pump invokes this with its live window.
            unsafe { update_tray_icon(hwnd) };
            0
        }
        WM_TRAY => {
            match lparam as u32 {
                WM_LBUTTONUP => open_or_focus_gui(),
                // SAFETY: win32 dispatches this callback on the tray thread —
                // the one that created `hwnd` and pumps its messages — so the
                // handle is live for the whole call, and `TrackPopupMenu` gets
                // the owning thread it requires.
                WM_RBUTTONUP | WM_CONTEXTMENU => unsafe { show_menu(hwnd) },
                _ => {}
            }
            0
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            // Explorer restarted; every tray icon was dropped. Re-add ours.
            // SAFETY: `hwnd` is our own window, still live while its window
            // procedure runs, and this is the thread that created it — the
            // same conditions under which `run_tray_loop` first added the icon.
            unsafe {
                NATIVE_TRAY.with_borrow_mut(|tray| *tray = None);
                update_tray_icon(hwnd);
            };
            0
        }
        // SAFETY: handing the system back the message it just delivered,
        // unchanged: `hwnd` is live for the duration of the callback and
        // `wparam`/`lparam` are the payload win32 paired with `msg`, which is
        // exactly what the default handler expects.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Wake the owning tray thread. It reads the latest full state, not a queued payload.
pub(crate) fn refresh() {
    let address = TRAY_WINDOW.load(Ordering::Relaxed);
    if address != 0 {
        // SAFETY: PostMessage copies the scalar message, does not dereference the
        // window, and fails harmlessly if shutdown has destroyed it meanwhile.
        unsafe { PostMessageW(address as HWND, WM_REFRESH, 0, 0) };
    }
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    // SAFETY: initialized notification identity for our live owning window.
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = u32::try_from(std::mem::size_of::<NOTIFYICONDATAW>()).unwrap_or(0);
        nid.hWnd = hwnd;
        nid.uID = 1;
        if Shell_NotifyIconW(NIM_DELETE, &raw const nid) == 0 {
            warn!("could not hide tray icon; will retry");
            return;
        }
    }
    NATIVE_TRAY.with_borrow_mut(|tray| *tray = None);
}

unsafe fn update_tray_icon(hwnd: HWND) {
    let presentation = tray_status::current();
    if !presentation.visible {
        if NATIVE_TRAY.with_borrow(Option::is_some) {
            // SAFETY: called on our live hidden window's owning thread.
            unsafe { remove_tray_icon(hwnd) };
        }
        return;
    }
    let light = taskbar_is_light();
    // SAFETY: GetSystemMetrics has no pointer arguments or preconditions.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    NATIVE_TRAY.with_borrow_mut(|slot| {
        if slot.as_ref().is_some_and(|old| {
            old.presentation == presentation && old.light == light && old.size == size
        }) {
            return;
        }
        let Some(icon) = icon::create(&presentation.icon, light, size) else {
            warn!("could not render tray icon; keeping the previous icon");
            return;
        };
        // SAFETY: the icon is owned here, the window belongs to this thread,
        // and Shell_NotifyIconW copies the bounded buffers during the call.
        unsafe {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = u32::try_from(std::mem::size_of::<NOTIFYICONDATAW>()).unwrap_or(0);
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY;
            nid.hIcon = icon.handle();
            copy_tip(&presentation.tooltip, &mut nid.szTip);
            let operation = if slot.is_some() { NIM_MODIFY } else { NIM_ADD };
            if Shell_NotifyIconW(operation, &raw const nid) == 0 {
                let retry = if operation == NIM_ADD {
                    NIM_MODIFY
                } else {
                    NIM_ADD
                };
                if Shell_NotifyIconW(retry, &raw const nid) == 0 {
                    warn!("tray update failed; keeping previous state for the next retry");
                    return;
                }
            }
        }
        // Dropping the previous owned HICON releases it after Shell copied it.
        *slot = Some(NativeTray {
            _icon: icon,
            presentation,
            light,
            size,
        });
    });
}

/// Ampersands in user-assigned names are text, not Windows menu mnemonics.
fn menu_label(text: &str) -> String {
    text.replace('&', "&&")
}

/// Leave a terminator and never split a UTF-16 surrogate pair at the limit.
fn copy_tip(text: &str, target: &mut [u16]) {
    target.fill(0);
    let capacity = target.len().saturating_sub(1);
    let truncated = text.encode_utf16().count() > capacity;
    let available = capacity.saturating_sub(usize::from(truncated));
    let mut used = 0;
    for character in text.chars() {
        let mut buffer = [0; 2];
        let encoded = character.encode_utf16(&mut buffer);
        if used + encoded.len() > available {
            break;
        }
        target[used..used + encoded.len()].copy_from_slice(encoded);
        used += encoded.len();
    }
    if truncated && capacity > 0 {
        target[used] = 0x2026;
    }
}

/// Whether the taskbar renders light (needs the black glyph). Missing value
/// means the Windows default: dark.
fn taskbar_is_light() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.get_value::<u32, _>("SystemUsesLightTheme"))
        .is_ok_and(|v| v == 1)
}

/// Show the context menu at the cursor and run the chosen command.
#[expect(
    clippy::cast_sign_loss,
    reason = "TrackPopupMenu returns the command id it was given, never negative"
)]
unsafe fn show_menu(hwnd: HWND) {
    // SAFETY: menu handles are created and destroyed here; the
    // SetForegroundWindow/WM_NULL bracket is the documented TrackPopupMenu
    // dance for tray menus (without it the menu won't dismiss on outside
    // clicks).
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let devices = tray_status::current().devices;
        // Built fresh on every right-click, so `t!` follows a live language
        // switch with no rebuild plumbing.
        AppendMenuW(
            menu,
            MF_STRING,
            ID_SHOW,
            wide(&rust_i18n::t!("app.show_main_window")).as_ptr(),
        );
        for (index, device) in devices.iter().enumerate() {
            AppendMenuW(
                menu,
                MF_STRING,
                ID_DEVICE_BASE + index,
                wide(&menu_label(&device.label)).as_ptr(),
            );
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(
            menu,
            MF_STRING,
            ID_QUIT,
            wide(&rust_i18n::t!("app.quit_openhid")).as_ptr(),
        );

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&raw mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);

        match cmd as usize {
            ID_SHOW => open_or_focus_gui(),
            ID_QUIT => quit(hwnd),
            command => {
                if let Some(device) = command
                    .checked_sub(ID_DEVICE_BASE)
                    .and_then(|index| devices.get(index))
                {
                    crate::device_selection::request(device.key.clone());
                    open_or_focus_gui();
                }
            }
        }
    }
}

/// Focus the running GUI's window, or launch the sibling GUI binary when no
/// GUI is running (a second launch would just exit on the `openhid.lock`
/// singleton, so spawning blindly does nothing visible).
fn open_or_focus_gui() {
    let pids = gui_pids();
    if pids.is_empty() {
        spawn_gui();
        return;
    }
    if !focus_window_of(&pids) {
        // Running but windowless should not happen (the GUI always has its
        // main window); log rather than spawn a doomed duplicate.
        warn!("GUI process is running but no window was found to focus");
    }
}

/// PIDs of this user's running GUI processes: `OpenHID.exe` (installed /
/// portable layout) or `openhid-desktop.exe` (cargo target dir).
///
/// Matching by *name* rather than by install directory is deliberate: the
/// GUI is a per-user singleton (`openhid.lock` lives under the profile), so
/// whichever copy is running — MSI, portable, dev — it is the only one that
/// *can* run, it is the one talking to this agent (the IPC pipe name is
/// machine-global), and a directory-scoped Show would spawn a sibling that
/// immediately loses the singleton and exits, doing nothing visible. The
/// same-user filter keeps other sessions (fast user switching) out of
/// Show/Quit — their windows are invisible to `EnumWindows` and their
/// processes unkillable anyway, but don't even consider them.
fn gui_pids() -> Vec<u32> {
    use sysinfo::{Pid, Process, ProcessesToUpdate, System};
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let own_user = system
        .process(Pid::from_u32(std::process::id()))
        .and_then(Process::user_id);
    system
        .processes()
        .values()
        .filter(|p| {
            is_gui_process_name(&p.name().to_string_lossy())
                && (own_user.is_none() || p.user_id() == own_user)
        })
        .map(|p| p.pid().as_u32())
        .collect()
}

/// Whether a process image name is one of the GUI binaries.
///
/// `OpenHID.exe` is matched case-*sensitively*: the CLI is `openhid.exe`,
/// which `eq_ignore_ascii_case` would accept, and the dev target dir holds
/// both. Windows reports image names with their on-disk case, so this holds.
fn is_gui_process_name(name: &str) -> bool {
    name == "OpenHID.exe" || name.eq_ignore_ascii_case("openhid-desktop.exe")
}

/// Bring the first visible top-level window owned by one of `pids` to the
/// foreground, restoring it if minimized. Returns whether one was found.
fn focus_window_of(pids: &[u32]) -> bool {
    struct Search<'a> {
        pids: &'a [u32],
        focused: bool,
    }
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: lparam is the &mut Search passed to EnumWindows below and
        // outlives the enumeration; the win32 queries take a valid hwnd.
        unsafe {
            let search = &mut *(lparam as *mut Search<'_>);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &raw mut pid);
            if search.pids.contains(&pid) && IsWindowVisible(hwnd) != 0 {
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                SetForegroundWindow(hwnd);
                search.focused = true;
                return 0; // stop enumerating
            }
            1
        }
    }
    let mut search = Search {
        pids,
        focused: false,
    };
    // SAFETY: the callback only dereferences the &mut Search for the duration
    // of this call.
    unsafe {
        EnumWindows(Some(enum_proc), std::ptr::addr_of_mut!(search) as LPARAM);
    }
    search.focused
}

/// Launch the GUI binary sitting next to the agent.
fn spawn_gui() {
    let Ok(exe) = std::env::current_exe() else {
        warn!("could not resolve the agent's own path — cannot launch the GUI");
        return;
    };
    let Some(dir) = exe.parent() else { return };
    // Dev target dir first: it holds both `openhid.exe` (CLI) and
    // `openhid-desktop.exe`, and the CLI shares `OpenHID.exe`'s name on the
    // case-insensitive filesystem — so `dir.join("OpenHID.exe").exists()`
    // there resolves to the CLI and would launch it. Probing the unambiguous
    // `openhid-desktop.exe` first avoids that; the installed layout has only
    // `OpenHID.exe` and falls through to it.
    let gui = ["openhid-desktop.exe", "OpenHID.exe"]
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists());
    let Some(gui) = gui else {
        warn!(dir = %dir.display(), "no GUI binary found next to the agent");
        return;
    };
    match std::process::Command::new(&gui).spawn() {
        Ok(_) => info!(path = %gui.display(), "tray — launched the GUI"),
        Err(e) => warn!(error = %e, path = %gui.display(), "tray — could not launch the GUI"),
    }
}

/// Quit the whole app: GUI first (its IPC retry loop would otherwise respawn
/// the agent we are about to exit), then the icon, then the agent. Mirrors
/// the macOS Quit semantics; the GUI holds no unsaved state (config writes
/// are immediate).
#[expect(
    clippy::cast_possible_truncation,
    reason = "NOTIFYICONDATAW is a few hundred bytes"
)]
fn quit(hwnd: HWND) {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    for pid in gui_pids() {
        if let Some(process) = system.process(Pid::from_u32(pid)) {
            if process.kill() {
                info!(pid, "tray Quit — terminated the GUI");
            } else {
                warn!(pid, "tray Quit — could not terminate the GUI");
            }
        }
    }
    // SAFETY: removing the icon this thread added.
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        Shell_NotifyIconW(NIM_DELETE, &raw const nid);
    }
    crate::overlay::evict_on_quit();
    info!("tray Quit — requesting graceful agent shutdown");
    let requests = SHUTDOWN_TX.with_borrow(Clone::clone);
    shutdown::request_tray_quit(requests.as_ref(), 0);
}

/// NUL-terminated UTF-16 for win32 W-APIs.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{copy_tip, is_gui_process_name, menu_label};

    #[test]
    fn tray_tooltip_truncates_without_splitting_unicode_or_losing_terminator() {
        for (text, expected) in [
            ("鼠标 🖱 85%", "鼠标 …"),
            ("abc🖱", "abc…"),
            ("🖱abc", "🖱a…"),
            ("ab", "ab"),
        ] {
            let mut tip = [0xffff; 5];
            copy_tip(text, &mut tip);
            let end = tip.iter().position(|&unit| unit == 0).unwrap();
            assert_eq!(String::from_utf16(&tip[..end]).unwrap(), expected);
        }
        copy_tip("x", &mut []);
        let mut tip = [0xffff];
        copy_tip("x", &mut tip);
        assert_eq!(tip, [0]);
    }

    #[test]
    fn the_cli_binary_is_not_the_gui() {
        assert!(is_gui_process_name("OpenHID.exe"));
        assert!(is_gui_process_name("openhid-desktop.exe"));
        assert!(!is_gui_process_name("openhid.exe")); // the CLI
    }
    #[test]
    fn menu_names_preserve_literal_ampersands() {
        assert_eq!(menu_label("Work & home: 50%"), "Work && home: 50%");
        assert_eq!(menu_label("Mouse: 50%"), "Mouse: 50%");
    }
}
