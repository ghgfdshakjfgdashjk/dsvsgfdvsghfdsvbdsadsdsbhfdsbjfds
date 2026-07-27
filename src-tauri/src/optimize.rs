use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::process::Command;

use serde::Serialize;

type HKEY = isize;

const HKEY_CURRENT_USER: HKEY = 0x8000_0001u32 as i32 as isize;
const HKEY_LOCAL_MACHINE: HKEY = 0x8000_0002u32 as i32 as isize;

const KEY_READ: u32 = 0x2_0019;
const KEY_WRITE: u32 = 0x2_0006;
const REG_DWORD: u32 = 4;
const REG_SZ: u32 = 1;
const ERROR_SUCCESS: i32 = 0;

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        hKey: HKEY,
        lpSubKey: *const u16,
        ulOptions: u32,
        samDesired: u32,
        phkResult: *mut HKEY,
    ) -> i32;
    fn RegCreateKeyExW(
        hKey: HKEY,
        lpSubKey: *const u16,
        Reserved: u32,
        lpClass: *const u16,
        dwOptions: u32,
        samDesired: u32,
        lpSecurityAttributes: *const c_void,
        phkResult: *mut HKEY,
        lpdwDisposition: *mut u32,
    ) -> i32;
    fn RegQueryValueExW(
        hKey: HKEY,
        lpValueName: *const u16,
        lpReserved: *mut u32,
        lpType: *mut u32,
        lpData: *mut u8,
        lpcbData: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        hKey: HKEY,
        lpValueName: *const u16,
        Reserved: u32,
        dwType: u32,
        lpData: *const u8,
        cbData: u32,
    ) -> i32;
    fn RegCloseKey(hKey: HKEY) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn SystemParametersInfoW(
        uiAction: u32,
        uiParam: u32,
        pvParam: *mut c_void,
        fWinIni: u32,
    ) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        lpOperation: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShowCmd: i32,
    ) -> isize;
}

const SPI_GETMOUSE: u32 = 0x0003;
const SPI_SETMOUSE: u32 = 0x0004;
const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;
const SPI_SETCLIENTAREAANIMATION: u32 = 0x1043;
const SPI_GETKEYBOARDSPEED: u32 = 0x000A;
const SPI_SETKEYBOARDSPEED: u32 = 0x000B;
const SPI_GETKEYBOARDDELAY: u32 = 0x0016;
const SPI_SETKEYBOARDDELAY: u32 = 0x0017;

const KEYBOARD_FASTEST_SPEED: u32 = 31;
const KEYBOARD_SHORTEST_DELAY: u32 = 0;

const KEYBOARD_DEFAULT_SPEED: u32 = 31;
const KEYBOARD_DEFAULT_DELAY: u32 = 1;

const SPIF_UPDATEINIFILE: u32 = 0x01;
const SPIF_SENDCHANGE: u32 = 0x02;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_dword(path: &str, name: &str) -> Option<u32> {
    read_dword_in(HKEY_CURRENT_USER, path, name)
}

fn read_dword_in(hive: HKEY, path: &str, name: &str) -> Option<u32> {
    unsafe {
        let mut key: HKEY = 0;
        if RegOpenKeyExW(hive, wide(path).as_ptr(), 0, KEY_READ, &mut key) != ERROR_SUCCESS {
            return None;
        }

        let mut value: u32 = 0;
        let mut kind: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = RegQueryValueExW(
            key,
            wide(name).as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            &mut value as *mut u32 as *mut u8,
            &mut size,
        );
        RegCloseKey(key);

        if status == ERROR_SUCCESS && kind == REG_DWORD {
            Some(value)
        } else {
            None
        }
    }
}

fn read_string(path: &str, name: &str) -> Option<String> {
    unsafe {
        let mut key: HKEY = 0;
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            KEY_READ,
            &mut key,
        ) != ERROR_SUCCESS
        {
            return None;
        }

        let mut kind: u32 = 0;
        let mut size: u32 = 0;

        let status = RegQueryValueExW(
            key,
            wide(name).as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        );
        if status != ERROR_SUCCESS || kind != REG_SZ || size == 0 {
            RegCloseKey(key);
            return None;
        }

        let mut buffer = vec![0u16; (size as usize / 2) + 1];
        let mut length = size;
        let status = RegQueryValueExW(
            key,
            wide(name).as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            buffer.as_mut_ptr() as *mut u8,
            &mut length,
        );
        RegCloseKey(key);

        if status != ERROR_SUCCESS {
            return None;
        }

        let chars = (length as usize / 2).min(buffer.len());
        let text: String = String::from_utf16_lossy(&buffer[..chars]);
        Some(text.trim_end_matches('\0').to_string())
    }
}

fn write_string(path: &str, name: &str, value: &str) -> Result<(), String> {
    unsafe {
        let mut key: HKEY = 0;
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_WRITE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if status != ERROR_SUCCESS {
            return Err(format!("couldn't open {path} (code {status})"));
        }

        let data = wide(value);
        let status = RegSetValueExW(
            key,
            wide(name).as_ptr(),
            0,
            REG_SZ,
            data.as_ptr() as *const u8,
            (data.len() * 2) as u32,
        );
        RegCloseKey(key);

        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("couldn't write {name} (code {status})"))
        }
    }
}

fn write_dword(path: &str, name: &str, value: u32) -> Result<(), String> {
    unsafe {
        let mut key: HKEY = 0;
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_WRITE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if status != ERROR_SUCCESS {
            return Err(format!("couldn't open {path} (code {status})"));
        }

        let status = RegSetValueExW(
            key,
            wide(name).as_ptr(),
            0,
            REG_DWORD,
            &value as *const u32 as *const u8,
            std::mem::size_of::<u32>() as u32,
        );
        RegCloseKey(key);

        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("couldn't write {name} (code {status})"))
        }
    }
}

const BACKUP_KEY: &str = r"Software\BootsAutoClicker\Backup";

fn mouse_params() -> Option<[i32; 3]> {
    unsafe {
        let mut params = [0i32; 3];
        let ok = SystemParametersInfoW(SPI_GETMOUSE, 0, params.as_mut_ptr() as *mut c_void, 0);
        if ok == 0 {
            return None;
        }
        Some(params)
    }
}

fn write_mouse_params(mut params: [i32; 3]) -> Result<(), String> {
    unsafe {
        let ok = SystemParametersInfoW(
            SPI_SETMOUSE,
            0,
            params.as_mut_ptr() as *mut c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
        if ok == 0 {
            Err("Windows refused the change".into())
        } else {
            Ok(())
        }
    }
}

fn pointer_precision() -> Option<bool> {

    mouse_params().map(|params| params[2] != 0)
}

fn set_pointer_precision(on: bool) -> Result<(), String> {
    let current = mouse_params().ok_or("couldn't read the current mouse settings")?;

    if !on {

        if current[2] != 0 {
            let _ = write_dword(BACKUP_KEY, "MouseThreshold1", current[0] as u32);
            let _ = write_dword(BACKUP_KEY, "MouseThreshold2", current[1] as u32);
            let _ = write_dword(BACKUP_KEY, "MouseSpeed", current[2] as u32);
        }
        return write_mouse_params([current[0], current[1], 0]);
    }

    let restored = match read_dword(BACKUP_KEY, "MouseSpeed") {
        Some(speed) if speed != 0 => [
            read_dword(BACKUP_KEY, "MouseThreshold1").unwrap_or(6) as i32,
            read_dword(BACKUP_KEY, "MouseThreshold2").unwrap_or(10) as i32,
            speed as i32,
        ],
        _ => [6, 10, 1],
    };

    write_mouse_params(restored)
}

fn keyboard_fastest() -> Option<bool> {
    unsafe {
        let mut speed: u32 = 0;
        let mut delay: u32 = 0;

        let got_speed = SystemParametersInfoW(
            SPI_GETKEYBOARDSPEED,
            0,
            &mut speed as *mut u32 as *mut c_void,
            0,
        );
        let got_delay = SystemParametersInfoW(
            SPI_GETKEYBOARDDELAY,
            0,
            &mut delay as *mut u32 as *mut c_void,
            0,
        );

        if got_speed == 0 || got_delay == 0 {
            return None;
        }
        Some(speed >= KEYBOARD_FASTEST_SPEED && delay <= KEYBOARD_SHORTEST_DELAY)
    }
}

fn set_keyboard_fastest(fastest: bool) -> Result<(), String> {
    let (speed, delay) = if fastest {
        (KEYBOARD_FASTEST_SPEED, KEYBOARD_SHORTEST_DELAY)
    } else {
        (KEYBOARD_DEFAULT_SPEED, KEYBOARD_DEFAULT_DELAY)
    };

    unsafe {

        let a = SystemParametersInfoW(
            SPI_SETKEYBOARDSPEED,
            speed,
            std::ptr::null_mut(),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
        let b = SystemParametersInfoW(
            SPI_SETKEYBOARDDELAY,
            delay,
            std::ptr::null_mut(),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );

        if a == 0 || b == 0 {
            Err("Windows refused the change".into())
        } else {
            Ok(())
        }
    }
}

fn client_animations() -> Option<bool> {
    unsafe {
        let mut on: i32 = 0;
        let ok = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut on as *mut i32 as *mut c_void,
            0,
        );
        if ok == 0 {
            return None;
        }
        Some(on != 0)
    }
}

fn set_client_animations(on: bool) -> Result<(), String> {
    unsafe {
        let ok = SystemParametersInfoW(
            SPI_SETCLIENTAREAANIMATION,
            0,
            (on as usize) as *mut c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
        if ok == 0 {
            Err("Windows refused the change".into())
        } else {
            Ok(())
        }
    }
}

const SCHEME_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
const SCHEME_HIGH: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn powercfg(args: &[&str]) -> Option<String> {
    let output = Command::new("powercfg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn active_scheme() -> Option<String> {
    let text = powercfg(&["/getactivescheme"])?;

    text.split_whitespace()
        .find(|word| word.len() == 36 && word.matches('-').count() == 4)
        .map(|guid| guid.to_lowercase())
}

fn set_scheme(guid: &str) -> Result<(), String> {
    let status = Command::new("powercfg")
        .args(["/setactive", guid])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("couldn't run powercfg: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("powercfg wouldn't switch scheme".into())
    }
}

enum Backing {

    Reg {
        keys: &'static [(&'static str, &'static str)],
        on_value: u32,

        default_on: bool,
    },

    RegSet {

        entries: &'static [(&'static str, &'static str, u32, u32)],
    },

    RegStr {
        keys: &'static [(&'static str, &'static str)],
        on_value: &'static str,
        off_value: &'static str,
    },
    PointerPrecision,
    Animations,
    KeyboardRepeat,
}

struct Tweak {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    backing: Backing,
}

static TWEAKS: &[Tweak] = &[
    Tweak {
        id: "gameMode",
        label: "Turn on Game Mode",
        detail: "Stops Windows Update installing drivers and sending restart \
                 notifications mid-session, and prioritises the game's CPU time.",
        backing: Backing::Reg {
            keys: &[(r"Software\Microsoft\GameBar", "AutoGameModeEnabled")],
            on_value: 1,
            default_on: true,
        },
    },
    Tweak {
        id: "gameBar",
        label: "Turn off Xbox Game Bar",
        detail: "The overlay on Win+G. It hooks input and rendering system-wide, \
                 so switching it off removes a layer between your clicks and the game.",
        backing: Backing::Reg {
            keys: &[
                (r"Software\Microsoft\GameBar", "UseNexusForGameBarEnabled"),
                (r"Software\Microsoft\GameBar", "ShowStartupPanel"),
            ],
            on_value: 0,
            default_on: true,
        },
    },
    Tweak {
        id: "gameDvr",
        label: "Turn off background recording",
        detail: "Game DVR keeps the last few minutes of gameplay buffered at all \
                 times. Constant encoding for a feature most people never use.",
        backing: Backing::Reg {
            keys: &[
                (r"System\GameConfigStore", "GameDVR_Enabled"),
                (
                    r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
                    "AppCaptureEnabled",
                ),
            ],
            on_value: 0,
            default_on: true,
        },
    },
    Tweak {
        id: "pointerPrecision",
        label: "Turn off mouse acceleration",
        detail: "\"Enhance pointer precision\" scales your movement by how fast you \
                 move, so the same physical distance lands somewhere different each \
                 time. The one setting here that changes where a click actually goes.",
        backing: Backing::PointerPrecision,
    },
    Tweak {
        id: "animations",
        label: "Turn off window animations",
        detail: "Fades and slides when windows open, close and minimise. Small, but \
                 it's GPU work happening exactly when you're switching to a game.",
        backing: Backing::Animations,
    },
    Tweak {
        id: "notifications",
        label: "Turn off notification banners",
        detail: "Toasts steal focus and appear over whatever you're clicking. Off \
                 here means they still arrive, they just don't pop up.",
        backing: Backing::Reg {
            keys: &[(
                r"Software\Microsoft\Windows\CurrentVersion\PushNotifications",
                "ToastEnabled",
            )],
            on_value: 0,
            default_on: true,
        },
    },
    Tweak {
        id: "transparency",
        label: "Turn off transparency effects",
        detail: "The acrylic blur behind Start, the taskbar and this app. Continuous \
                 compositor work for something purely decorative.",
        backing: Backing::Reg {
            keys: &[(
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "EnableTransparency",
            )],
            on_value: 0,
            default_on: true,
        },
    },
    Tweak {
        id: "stickyKeys",
        label: "Turn off the Sticky Keys prompt",
        detail: "The dialog that ambushes you when Shift is tapped five times — which \
                 is to say, mid-fight. Turns off the shortcut, not the feature.",
        backing: Backing::RegStr {
            keys: &[(r"Control Panel\Accessibility\StickyKeys", "Flags")],
            on_value: "506",
            off_value: "510",
        },
    },
    Tweak {
        id: "filterKeys",
        label: "Turn off the Filter Keys prompt",
        detail: "The same ambush, triggered by holding right Shift for eight seconds.",
        backing: Backing::RegStr {
            keys: &[(r"Control Panel\Accessibility\Keyboard Response", "Flags")],
            on_value: "118",
            off_value: "126",
        },
    },
    Tweak {
        id: "minAnimate",
        label: "Turn off minimise and maximise animations",
        detail: "The zoom when a window minimises. Frames spent animating a window \
                 you're leaving are frames the game doesn't get.",
        backing: Backing::RegStr {
            keys: &[(r"Control Panel\Desktop\WindowMetrics", "MinAnimate")],
            on_value: "0",
            off_value: "1",
        },
    },
    Tweak {
        id: "aeroShake",
        label: "Turn off Aero Shake",
        detail: "Wobbling a window minimises every other window. Easy to trigger by \
                 accident, and nobody has ever meant to.",
        backing: Backing::Reg {
            keys: &[(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
                "DisallowShaking",
            )],
            on_value: 1,
            default_on: true,
        },
    },
    Tweak {
        id: "fullscreenOpt",
        label: "Turn off fullscreen optimisations",
        detail: "Windows runs \"fullscreen\" games as a borderless window behind the \
                 compositor. Convenient for alt-tabbing, and it adds a frame of latency. \
                 Off means exclusive fullscreen actually is exclusive.",
        backing: Backing::RegSet {
            entries: &[
                (r"System\GameConfigStore", "GameDVR_DXGIHonorFSEWindowsCompatible", 1, 0),
                (r"System\GameConfigStore", "GameDVR_FSEBehavior", 2, 0),
                (r"System\GameConfigStore", "GameDVR_HonorUserFSEBehaviorMode", 1, 0),
                (r"System\GameConfigStore", "GameDVR_EFSEFeatureFlags", 0, 1),
            ],
        },
    },
    Tweak {
        id: "backgroundApps",
        label: "Turn off background apps",
        detail: "Store apps that keep running when closed — Mail, Photos, Xbox and \
                 friends — polling and updating while you play.",
        backing: Backing::RegSet {
            entries: &[(
                r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications",
                "GlobalUserDisabled",
                1,
                0,
            )],
        },
    },
    Tweak {
        id: "visualEffects",
        label: "Visual effects for performance",
        detail: "The shadows, fades and slides throughout Windows. Turning them off is \
                 the single most visible change here, and the one you'll notice most on \
                 an older machine.",
        backing: Backing::RegSet {
            entries: &[(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects",
                "VisualFXSetting",
                2,
                0,
            )],
        },
    },
    Tweak {
        id: "storageSense",
        label: "Turn on Storage Sense",
        detail: "Windows clears temp files and empties the Recycle Bin of anything older \
                 than 30 days, on its own. The one entry here that switches something on \
                 rather than off — it does the disk cleanup below for you, on a schedule.",
        backing: Backing::RegSet {
            entries: &[

                (r"Software\Microsoft\Windows\CurrentVersion\StorageSense\Parameters\StoragePolicy", "01", 1, 0),
                (r"Software\Microsoft\Windows\CurrentVersion\StorageSense\Parameters\StoragePolicy", "04", 1, 1),
                (r"Software\Microsoft\Windows\CurrentVersion\StorageSense\Parameters\StoragePolicy", "08", 1, 0),
                (r"Software\Microsoft\Windows\CurrentVersion\StorageSense\Parameters\StoragePolicy", "32", 30, 0),
            ],
        },
    },
    Tweak {
        id: "menuDelay",
        label: "Open menus instantly",
        detail: "Windows waits 400 ms before a submenu appears. There's no reason for \
                 it and removing it makes the whole desktop feel quicker.",
        backing: Backing::RegStr {
            keys: &[(r"Control Panel\Desktop", "MenuShowDelay")],
            on_value: "0",
            off_value: "400",
        },
    },
    Tweak {
        id: "keyboardRepeat",
        label: "Fastest key repeat",
        detail: "Shortest delay before a held key starts repeating, and the fastest \
                 repeat rate. Directly relevant here — it's what governs held-key spam.",
        backing: Backing::KeyboardRepeat,
    },
    Tweak {
        id: "suggestedContent",
        label: "Turn off suggested content",
        detail: "App suggestions in Start, tips, and the \"recommended\" panels in \
                 Settings. Fetched from Microsoft on a timer — advertising, in practice.",
        backing: Backing::RegSet {
            entries: &[
                (r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-338388Enabled", 0, 1),
                (r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-338389Enabled", 0, 1),
                (r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-353694Enabled", 0, 1),
                (r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-353696Enabled", 0, 1),
                (r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SystemPaneSuggestionsEnabled", 0, 1),
                (r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SilentInstalledAppsEnabled", 0, 1),
            ],
        },
    },
    Tweak {
        id: "searchHighlights",
        label: "Turn off search highlights",
        detail: "The rotating illustrations and trivia in the Start menu search box. \
                 Fetched from the internet, on a timer, forever.",
        backing: Backing::Reg {
            keys: &[(
                r"Software\Microsoft\Windows\CurrentVersion\SearchSettings",
                "IsDynamicSearchBoxEnabled",
            )],
            on_value: 0,
            default_on: true,
        },
    },
];

fn run_elevated(script: &str) -> Result<(), String> {
    unsafe {
        let verb = wide("runas");
        let file = wide("powershell.exe");
        let args = wide(&format!(
            "-NoProfile -ExecutionPolicy Bypass -Command \"{script}\""
        ));

        let result = ShellExecuteW(
            0,
            verb.as_ptr(),
            file.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            1,
        );

        match result {

            5 => Err("Administrator permission was declined.".into()),
            r if r > 32 => Ok(()),
            r => Err(format!("Couldn't start an elevated shell (code {r})")),
        }
    }
}

struct AdminTweak {
    id: &'static str,
    label: &'static str,
    detail: &'static str,

    path: &'static str,
    name: &'static str,

    on_value: u32,
    default_value: u32,

    apply: &'static str,
    revert: &'static str,
    reboot: bool,
}

static ADMIN_TWEAKS: &[AdminTweak] = &[
    AdminTweak {
        id: "gpuScheduling",
        label: "Hardware-accelerated GPU scheduling",
        detail: "Lets the GPU manage its own work queue instead of the driver doing it \
                 on the CPU. Usually a small latency win. Needs a restart.",
        path: r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers",
        name: "HwSchMode",
        on_value: 2,
        default_value: 1,

        apply: r"Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' -Name HwSchMode -Value 2 -Type DWord -Force",
        revert: r"Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' -Name HwSchMode -Value 1 -Type DWord -Force",
        reboot: true,
    },
    AdminTweak {
        id: "powerThrottling",
        label: "Turn off power throttling",
        detail: "Windows slows background processes to save power. Harmless on a \
                 desktop, and it's exactly what makes a clicker run slower when it \
                 isn't the focused window. Costs battery on a laptop.",
        path: r"SYSTEM\CurrentControlSet\Control\Power\PowerThrottling",
        name: "PowerThrottlingOff",
        on_value: 1,
        default_value: 0,
        apply: r"New-Item -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling' -Force | Out-Null; Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling' -Name PowerThrottlingOff -Value 1 -Type DWord -Force",
        revert: r"Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling' -Name PowerThrottlingOff -Value 0 -Type DWord -Force",
        reboot: false,
    },
    AdminTweak {
        id: "networkThrottling",
        label: "Turn off network throttling",
        detail: "Windows caps non-multimedia network traffic at about ten packets a \
                 millisecond to protect audio streaming. On a modern connection that \
                 ceiling is pure added latency for anything online.",
        path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile",
        name: "NetworkThrottlingIndex",
        on_value: 0xFFFF_FFFF,
        default_value: 10,
        apply: r"Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name NetworkThrottlingIndex -Value 0xFFFFFFFF -Type DWord -Force",
        revert: r"Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name NetworkThrottlingIndex -Value 10 -Type DWord -Force",
        reboot: true,
    },
    AdminTweak {
        id: "systemResponsiveness",
        label: "Give background tasks less reserved CPU",
        detail: "Windows holds back a fifth of the CPU for background work. Lowering \
                 that to a tenth gives the foreground more. Going to zero is a popular \
                 tweak and a common cause of audio crackle, so this stops at ten.",
        path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile",
        name: "SystemResponsiveness",
        on_value: 10,
        default_value: 20,
        apply: r"Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name SystemResponsiveness -Value 10 -Type DWord -Force",
        revert: r"Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name SystemResponsiveness -Value 20 -Type DWord -Force",
        reboot: true,
    },
    AdminTweak {
        id: "gamesTask",
        label: "Raise the priority of games",
        detail: "Windows has a scheduling profile named Games that most titles register \
                 under. This sets its GPU and CPU priority to the top of the range \
                 instead of the middling defaults.",
        path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games",
        name: "GPU Priority",
        on_value: 8,
        default_value: 2,
        apply: r"$k='HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games'; New-Item -Path $k -Force | Out-Null; Set-ItemProperty -Path $k -Name 'GPU Priority' -Value 8 -Type DWord -Force; Set-ItemProperty -Path $k -Name 'Priority' -Value 6 -Type DWord -Force; Set-ItemProperty -Path $k -Name 'Scheduling Category' -Value 'High' -Type String -Force; Set-ItemProperty -Path $k -Name 'SFIO Priority' -Value 'High' -Type String -Force",
        revert: r"$k='HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games'; Set-ItemProperty -Path $k -Name 'GPU Priority' -Value 2 -Type DWord -Force; Set-ItemProperty -Path $k -Name 'Priority' -Value 2 -Type DWord -Force; Set-ItemProperty -Path $k -Name 'Scheduling Category' -Value 'Medium' -Type String -Force; Set-ItemProperty -Path $k -Name 'SFIO Priority' -Value 'Normal' -Type String -Force",
        reboot: true,
    },
    AdminTweak {
        id: "reservedStorage",
        label: "Turn off reserved storage",
        detail: "Windows sets aside about 7 GB purely so updates always have room. \
                 WARNING: with it off, a future feature update can fail for lack of \
                 space, and you'd have to free some by hand before it will install. \
                 The single biggest disk win here, and the one most likely to bite.",

        path: "",
        name: "",
        on_value: 1,
        default_value: 0,
        apply: "DISM.exe /Online /Set-ReservedStorageState /State:Disabled",
        revert: "DISM.exe /Online /Set-ReservedStorageState /State:Enabled",
        reboot: false,
    },
    AdminTweak {
        id: "nagle",
        label: "Turn off Nagle's algorithm",
        detail: "Windows batches small network packets together before sending. Off \
                 means they go out immediately — lower latency in games. On a poor or \
                 congested connection it can be neutral or slightly worse. Fully \
                 reversible either way.",
        path: "",
        name: "",
        on_value: 1,
        default_value: 0,
        apply: r"Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | ForEach-Object { Set-ItemProperty -Path $_.PSPath -Name TcpAckFrequency -Value 1 -Type DWord -Force; Set-ItemProperty -Path $_.PSPath -Name TCPNoDelay -Value 1 -Type DWord -Force }",
        revert: r"Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' | ForEach-Object { Remove-ItemProperty -Path $_.PSPath -Name TcpAckFrequency -ErrorAction SilentlyContinue; Remove-ItemProperty -Path $_.PSPath -Name TCPNoDelay -ErrorAction SilentlyContinue }",
        reboot: true,
    },
    AdminTweak {
        id: "fastStartup",
        label: "Turn off Fast Startup",
        detail: "Shutting down doesn't actually shut down — Windows hibernates the \
                 kernel and drivers, then restores them. That's why some problems \
                 survive a shutdown but vanish after a restart. Off means a shutdown is \
                 a real one; boots take a few seconds longer.",
        path: r"SYSTEM\CurrentControlSet\Control\Session Manager\Power",
        name: "HiberbootEnabled",
        on_value: 0,
        default_value: 1,
        apply: r"Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Power' -Name HiberbootEnabled -Value 0 -Type DWord -Force",
        revert: r"Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Power' -Name HiberbootEnabled -Value 1 -Type DWord -Force",
        reboot: false,
    },
    AdminTweak {
        id: "hibernation",
        label: "Turn off hibernation",
        detail: "Frees a hidden file about the size of your RAM — often 8 to 32 GB. \
                 Fast Startup depends on that file, so this turns it off too — expect \
                 the row above to follow.",
        path: r"SYSTEM\CurrentControlSet\Control\Power",
        name: "HibernateEnabled",
        on_value: 0,
        default_value: 1,
        apply: "powercfg /hibernate off",
        revert: "powercfg /hibernate on",
        reboot: false,
    },
];

struct Cleanup {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    script: &'static str,

    destructive: bool,
}

static CLEANUPS: &[Cleanup] = &[
    Cleanup {
        id: "tempFiles",
        label: "Clear temp files",
        detail: "Your temp folder and Windows'. Anything still in use is skipped.",
        script: r"Remove-Item -Path $env:TEMP\* -Recurse -Force -ErrorAction SilentlyContinue; Remove-Item -Path $env:WINDIR\Temp\* -Recurse -Force -ErrorAction SilentlyContinue",
        destructive: false,
    },
    Cleanup {
        id: "updateCache",
        label: "Clear Windows Update cache",
        detail: "Installers Windows keeps after updating. Often several gigabytes, and \
                 rebuilt automatically when it next needs them.",
        script: r"Stop-Service -Name wuauserv -Force -ErrorAction SilentlyContinue; Remove-Item -Path $env:WINDIR\SoftwareDistribution\Download\* -Recurse -Force -ErrorAction SilentlyContinue; Start-Service -Name wuauserv -ErrorAction SilentlyContinue",
        destructive: false,
    },
    Cleanup {
        id: "shaderCache",
        label: "Clear GPU shader caches",
        detail: "Compiled shaders kept by DirectX and your graphics driver. Often \
                 several gigabytes. Games rebuild what they need — the first launch \
                 after this may stutter briefly, then it's back to normal.",
        script: r"Remove-Item -Path $env:LOCALAPPDATA\D3DSCache\* -Recurse -Force -ErrorAction SilentlyContinue; Remove-Item -Path $env:LOCALAPPDATA\NVIDIA\DXCache\* -Recurse -Force -ErrorAction SilentlyContinue; Remove-Item -Path $env:LOCALAPPDATA\NVIDIA\GLCache\* -Recurse -Force -ErrorAction SilentlyContinue; Remove-Item -Path $env:LOCALAPPDATA\AMD\DxCache\* -Recurse -Force -ErrorAction SilentlyContinue",
        destructive: false,
    },
    Cleanup {
        id: "deliveryOptimization",
        label: "Clear Delivery Optimization cache",
        detail: "Update data Windows keeps around to share with other PCs on your \
                 network. Can be gigabytes and is never needed again.",
        script: r"Delete-DeliveryOptimizationCache -Force -ErrorAction SilentlyContinue; Remove-Item -Path $env:WINDIR\SoftwareDistribution\DeliveryOptimization\* -Recurse -Force -ErrorAction SilentlyContinue",
        destructive: false,
    },
    Cleanup {
        id: "recycleBin",
        label: "Empty the Recycle Bin",
        detail: "Every drive at once.",
        script: "Clear-RecycleBin -Force -ErrorAction SilentlyContinue",
        destructive: true,
    },
    Cleanup {
        id: "diskCleanup",
        label: "Disk Cleanup, including system files",
        detail: "Opens the Windows tool with the system-file options already unlocked, \
                 so you choose what goes.",
        script: "Start-Process cleanmgr.exe -ArgumentList '/d C:'",
        destructive: false,
    },
    Cleanup {
        id: "componentStore",
        label: "Compact the component store",
        detail: "Superseded copies of Windows components. Frees a lot and is Microsoft's \
                 own tool — but it takes several minutes and afterwards the updates it \
                 cleans up can no longer be uninstalled.",
        script: "DISM.exe /Online /Cleanup-Image /StartComponentCleanup",
        destructive: true,
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminTweakState {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub optimised: bool,
    pub readable: bool,
    pub reboot: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupState {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub destructive: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakState {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub optimised: bool,

    pub readable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Optimizations {
    pub tweaks: Vec<TweakState>,

    pub power_plan: String,
    pub admin: Vec<AdminTweakState>,
    pub cleanups: Vec<CleanupState>,
}

fn find(id: &str) -> Option<&'static Tweak> {
    TWEAKS.iter().find(|t| t.id == id)
}

fn is_optimised(tweak: &Tweak) -> Option<bool> {
    match &tweak.backing {
        Backing::Reg {
            keys,
            on_value,
            default_on,
        } => {

            Some(keys.iter().all(|(path, name)| {
                read_dword(path, name).unwrap_or(u32::from(*default_on)) == *on_value
            }))
        }

        Backing::RegSet { entries } => Some(entries.iter().all(|(path, name, on, _)| {

            read_dword(path, name) == Some(*on)
        })),
        Backing::RegStr {
            keys, on_value, ..
        } => Some(keys.iter().all(|(path, name)| {
            read_string(path, name).as_deref() == Some(*on_value)
        })),
        Backing::PointerPrecision => pointer_precision().map(|on| !on),
        Backing::Animations => client_animations().map(|on| !on),
        Backing::KeyboardRepeat => keyboard_fastest(),
    }
}

fn apply(tweak: &Tweak, optimised: bool) -> Result<(), String> {
    match &tweak.backing {

        Backing::Reg { keys, on_value, .. } => {

            let value = if optimised {
                *on_value
            } else {
                u32::from(*on_value == 0)
            };
            for (path, name) in keys.iter() {
                write_dword(path, name, value)?;
            }
            Ok(())
        }
        Backing::RegSet { entries } => {
            for (path, name, on, off) in entries.iter() {
                write_dword(path, name, if optimised { *on } else { *off })?;
            }
            Ok(())
        }
        Backing::RegStr {
            keys,
            on_value,
            off_value,
        } => {
            let value = if optimised { on_value } else { off_value };
            for (path, name) in keys.iter() {
                write_string(path, name, value)?;
            }
            Ok(())
        }
        Backing::PointerPrecision => set_pointer_precision(!optimised),
        Backing::Animations => set_client_animations(!optimised),
        Backing::KeyboardRepeat => set_keyboard_fastest(optimised),
    }
}

pub fn snapshot() -> Optimizations {
    let tweaks = TWEAKS
        .iter()
        .map(|tweak| {
            let state = is_optimised(tweak);
            TweakState {
                id: tweak.id.into(),
                label: tweak.label.into(),
                detail: tweak.detail.into(),
                optimised: state.unwrap_or(false),
                readable: state.is_some(),
            }
        })
        .collect();

    let power_plan = match active_scheme().as_deref() {
        Some(SCHEME_HIGH) => "high",
        Some(SCHEME_BALANCED) => "balanced",
        Some(_) => "other",
        None => "unknown",
    }
    .to_string();

    let admin = ADMIN_TWEAKS
        .iter()
        .map(|tweak| {

            let readable = !tweak.path.is_empty();
            let current = if readable {
                read_dword_in(HKEY_LOCAL_MACHINE, tweak.path, tweak.name)
            } else {
                None
            };

            AdminTweakState {
                id: tweak.id.into(),
                label: tweak.label.into(),
                detail: tweak.detail.into(),

                optimised: readable && current.unwrap_or(tweak.default_value) == tweak.on_value,
                readable,
                reboot: tweak.reboot,
            }
        })
        .collect();

    let cleanups = CLEANUPS
        .iter()
        .map(|job| CleanupState {
            id: job.id.into(),
            label: job.label.into(),
            detail: job.detail.into(),
            destructive: job.destructive,
        })
        .collect();

    Optimizations {
        tweaks,
        power_plan,
        admin,
        cleanups,
    }
}

pub fn set_admin_tweak(id: &str, optimised: bool) -> Result<(), String> {
    let tweak = ADMIN_TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("no tweak called {id}"))?;

    run_elevated(if optimised { tweak.apply } else { tweak.revert })
}

pub fn run_cleanup(id: &str) -> Result<(), String> {
    let job = CLEANUPS
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("no cleanup called {id}"))?;

    run_elevated(job.script)
}

pub fn set_tweak(id: &str, optimised: bool) -> Result<(), String> {
    let tweak = find(id).ok_or_else(|| format!("no tweak called {id}"))?;
    apply(tweak, optimised)
}

pub fn set_power_plan(plan: &str) -> Result<(), String> {
    let guid = match plan {
        "high" => SCHEME_HIGH,
        "balanced" => SCHEME_BALANCED,
        other => return Err(format!("unknown power plan {other}")),
    };
    set_scheme(guid)
}

fn shell_open(target: &str, args: Option<&str>) -> Result<(), String> {
    unsafe {
        let operation = wide("open");
        let file = wide(target);
        let parameters = args.map(wide);

        let result = ShellExecuteW(
            0,
            operation.as_ptr(),
            file.as_ptr(),
            parameters
                .as_ref()
                .map(|p| p.as_ptr())
                .unwrap_or(std::ptr::null()),
            std::ptr::null(),
            1,
        );

        if result > 32 {
            Ok(())
        } else {
            Err(format!("Windows couldn't open {target} (code {result})"))
        }
    }
}

pub fn open_external(url: &str) -> Result<(), String> {
    shell_open(url, None)
}

pub fn launch(target: &str) -> Result<(), String> {
    match target {

        "gameMode" => shell_open("ms-settings:gaming-gamemode", None),
        "gameBar" => shell_open("ms-settings:gaming-xboxnetworking", None),
        "graphics" => shell_open("ms-settings:display-advancedgraphics", None),
        "power" => shell_open("ms-settings:powersleep", None),
        "mouse" => shell_open("ms-settings:mousetouchpad", None),
        "startup" => shell_open("ms-settings:startupapps", None),
        "storage" => shell_open("ms-settings:storagesense", None),

        "taskManager" => shell_open("taskmgr.exe", None),
        "resourceMonitor" => shell_open("resmon.exe", None),
        "diskCleanup" => shell_open("cleanmgr.exe", None),

        "winutil" => shell_open(
            "powershell.exe",
            Some(
                "-NoProfile -Command \"Start-Process powershell.exe \
                 -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass \
                 -Command irm christitus.com/win | iex'\"",
            ),
        ),

        other => Err(format!("unknown tool {other}")),
    }
}
