use std::{
    ffi::c_void,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    time::Duration,
};

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp, CSWorldSceneDrawParamManager},
    fd4::FD4TaskData,
};
use fromsoftware_shared::{
    FromStatic, SharedTaskImpExt,
    game_version::{GameVersion, LANG_ID_EN, LANG_ID_JP},
};
use pelite::pe64::PeView;
use windows::Win32::{
    Foundation::HMODULE,
    System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleA},
};

const WW_262_OFFSETS: GparamOffsets = GparamOffsets {
    profile: "WW 2.6.2.0",
    common_event_preload_fn: 0x00AB89A0,
    gparam_filecap_request_fn: 0x001F2420,
    gparam_resource_manager_global: 0x03D5B0F8,
};

const JP_2621_OFFSETS: GparamOffsets = GparamOffsets {
    // The current JP 1.16.2 executable is known to match the WW addresses.
    profile: "JP 2.6.2.1",
    common_event_preload_fn: 0x00AB89A0,
    gparam_filecap_request_fn: 0x001F2420,
    gparam_resource_manager_global: 0x03D5B0F8,
};

const JP_2611_OFFSETS: GparamOffsets = GparamOffsets {
    // Verified against eldenring.jp.exe product version 2.6.1.1.
    profile: "JP 2.6.1.1",
    common_event_preload_fn: 0x00AB89A0,
    gparam_filecap_request_fn: 0x001F2420,
    gparam_resource_manager_global: 0x03D5B0F8,
};

#[derive(Clone)]
struct Config {
    enabled: bool,
    log_enabled: bool,
    common_event_ids: Vec<u32>,
    start_delay_ms: u64,
    retries_per_id: u32,
    retry_every_frames: u32,
    request_filecap: bool,
    prime_drawparam: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            log_enabled: true,
            common_event_ids: vec![5],
            start_delay_ms: 5000,
            retries_per_id: 120,
            retry_every_frames: 60,
            request_filecap: true,
            prime_drawparam: true,
        }
    }
}

#[derive(Clone)]
struct PendingId {
    id: u32,
    attempts: u32,
    done: bool,
}

#[derive(Clone, Copy)]
struct GparamOffsets {
    profile: &'static str,
    common_event_preload_fn: usize,
    gparam_filecap_request_fn: usize,
    gparam_resource_manager_global: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupportedExe {
    Ww262,
    Jp2611,
    Jp2621,
}

impl GameVersion for SupportedExe {
    const NAME: &'static str = "elden ring";

    fn from_lang_version(lang_id: u16, version: &str) -> Option<Self> {
        match (lang_id, version) {
            (LANG_ID_EN, "2.6.2.0") => Some(Self::Ww262),
            (LANG_ID_JP, "2.6.1.1") => Some(Self::Jp2611),
            (LANG_ID_JP, "2.6.2.1") => Some(Self::Jp2621),
            _ => None,
        }
    }
}

impl SupportedExe {
    const fn offsets(self) -> &'static GparamOffsets {
        match self {
            Self::Ww262 => &WW_262_OFFSETS,
            Self::Jp2611 => &JP_2611_OFFSETS,
            Self::Jp2621 => &JP_2621_OFFSETS,
        }
    }
}

type CommonEventPreloadFn = unsafe extern "system" fn(*mut c_void, u32, f32);
type GparamFilecapRequestFn =
    unsafe extern "system" fn(*mut c_void, *const u16, usize) -> *mut c_void;

/// # Safety
///
/// This is exposed for Windows LoadLibrary. Do not call it directly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DllMain(_hmodule: usize, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    let hmodule = _hmodule;
    std::thread::spawn(move || {
        let config = Config::load(hmodule);
        let log_path = if config.log_enabled {
            sidecar_path(hmodule, "gparam_preload_patch.log")
        } else {
            None
        };
        append_log(
            &log_path,
            &format!(
                "loaded enabled={} log_enabled={} common_event_ids={:?} start_delay_ms={} retries_per_id={} retry_every_frames={} request_filecap={} prime_drawparam={}",
                config.enabled,
                config.log_enabled,
                config.common_event_ids,
                config.start_delay_ms,
                config.retries_per_id,
                config.retry_every_frames,
                config.request_filecap,
                config.prime_drawparam,
            ),
        );

        if !config.enabled {
            append_log(&log_path, "loader disabled by config");
            return;
        }

        if config.common_event_ids.is_empty() {
            append_log(&log_path, "no common_event_ids configured");
            return;
        }

        let offsets = match detect_offsets() {
            Ok(offsets) => offsets,
            Err(message) => {
                append_log(&log_path, &format!("{message}; loader disabled"));
                return;
            }
        };

        append_log(
            &log_path,
            &format!(
                "using offset profile {} filecap_request=0x{:X} common_event_prime=0x{:X} resource_manager_global=0x{:X}",
                offsets.profile,
                offsets.gparam_filecap_request_fn,
                offsets.common_event_preload_fn,
                offsets.gparam_resource_manager_global
            ),
        );

        load_common_event_ids_after_delay(log_path, config, *offsets);
    });

    true
}

impl Config {
    fn load(hmodule: usize) -> Self {
        let mut config = Self::default();

        let Some(path) = config_path(hmodule) else {
            return config;
        };
        let Ok(contents) = fs::read_to_string(path) else {
            return config;
        };

        for raw_line in contents.lines() {
            let line = raw_line
                .split_once('#')
                .map_or(raw_line, |(value, _)| value)
                .trim();
            if line.is_empty() {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            match (key.trim(), value.trim()) {
                ("enabled", value) => {
                    if let Some(parsed) = parse_bool(value) {
                        config.enabled = parsed;
                    }
                }
                ("log_enabled", value) | ("enable_log", value) | ("write_log", value) => {
                    if let Some(parsed) = parse_bool(value) {
                        config.log_enabled = parsed;
                    }
                }
                ("common_event_ids", value) => {
                    config.common_event_ids = parse_id_list(value);
                }
                ("start_delay_ms", value) | ("direct_preload_delay_ms", value) => {
                    if let Ok(parsed) = value.parse::<u64>() {
                        config.start_delay_ms = parsed;
                    }
                }
                ("retries_per_id", value) | ("direct_preload_retries", value) => {
                    if let Ok(parsed) = value.parse::<u32>() {
                        config.retries_per_id = parsed.max(1);
                    }
                }
                ("retry_every_frames", value) => {
                    if let Ok(parsed) = value.parse::<u32>() {
                        config.retry_every_frames = parsed.max(1);
                    }
                }
                ("request_filecap", value) => {
                    if let Some(parsed) = parse_bool(value) {
                        config.request_filecap = parsed;
                    }
                }
                ("prime_drawparam", value) | ("direct_preload", value) => {
                    if let Some(parsed) = parse_bool(value) {
                        config.prime_drawparam = parsed;
                    }
                }
                // Backward compatible with the previous single-id test ini.
                ("direct_preload_id", value) => {
                    if let Ok(parsed) = value.parse::<u32>() {
                        config.common_event_ids = vec![parsed];
                    }
                }
                _ => {}
            }
        }

        config.common_event_ids.sort_unstable();
        config.common_event_ids.dedup();
        config
    }
}

fn load_common_event_ids_after_delay(
    log_path: Option<PathBuf>,
    config: Config,
    offsets: GparamOffsets,
) {
    std::thread::sleep(Duration::from_millis(config.start_delay_ms));

    let Ok(cs_task) = CSTaskImp::wait_for_instance(Duration::MAX) else {
        append_log(&log_path, "failed: CSTaskImp instance not found");
        return;
    };

    append_log(
        &log_path,
        "common_event_loader scheduler installed on DrawParamUpdate",
    );

    let mut frame = 0u32;
    let mut pending = config
        .common_event_ids
        .iter()
        .copied()
        .map(|id| PendingId {
            id,
            attempts: 0,
            done: false,
        })
        .collect::<Vec<_>>();
    let retry_every_frames = config.retry_every_frames;
    let retries_per_id = config.retries_per_id;
    let request_filecap = config.request_filecap;
    let prime_drawparam = config.prime_drawparam;
    let mut finished = false;

    cs_task.run_recurring(
        move |_: &FD4TaskData| {
            if finished {
                return;
            }

            frame = frame.wrapping_add(1);
            if frame % retry_every_frames != 1 {
                return;
            }

            let mut all_done = true;
            for pending_id in &mut pending {
                if pending_id.done {
                    continue;
                }

                if pending_id.attempts >= retries_per_id {
                    append_log(
                        &log_path,
                        &format!(
                            "common_event id={} stopped after {} attempts",
                            pending_id.id, retries_per_id
                        ),
                    );
                    pending_id.done = true;
                    continue;
                }

                pending_id.attempts = pending_id.attempts.saturating_add(1);
                match unsafe {
                    load_common_event_id(pending_id.id, request_filecap, prime_drawparam, offsets)
                } {
                    Ok(message) => {
                        append_log(
                            &log_path,
                            &format!(
                                "common_event id={} attempt={} {message}",
                                pending_id.id, pending_id.attempts
                            ),
                        );
                        pending_id.done = true;
                    }
                    Err(message) => {
                        append_log(
                            &log_path,
                            &format!(
                                "common_event id={} attempt={} failed: {message}",
                                pending_id.id, pending_id.attempts
                            ),
                        );
                        all_done = false;
                    }
                }
            }

            if all_done || pending.iter().all(|item| item.done) {
                append_log(&log_path, "common_event_loader finished");
                finished = true;
            }
        },
        CSTaskGroupIndex::DrawParamUpdate,
    );
}

unsafe fn load_common_event_id(
    id: u32,
    request_filecap: bool,
    prime_drawparam: bool,
    offsets: GparamOffsets,
) -> Result<String, String> {
    let exe = unsafe { GetModuleHandleA(None) }
        .map_err(|err| format!("GetModuleHandleA(None) failed: {err:?}"))?;
    let base = exe.0 as usize;

    let request_message = if request_filecap {
        unsafe { request_common_event_filecap(base, id, offsets) }?
    } else {
        "filecap_request skipped".to_string()
    };

    if !prime_drawparam {
        return Ok(format!("{request_message}; prime_drawparam skipped"));
    }

    let fn_addr = base + offsets.common_event_preload_fn;
    let manager = unsafe { CSWorldSceneDrawParamManager::instance() }
        .map_err(|_| "CSWorldSceneDrawParamManager instance not found".to_string())?;
    let manager_addr = manager as *const CSWorldSceneDrawParamManager as *mut c_void;
    let preload: CommonEventPreloadFn = unsafe { std::mem::transmute(fn_addr) };

    unsafe {
        preload(manager_addr, id, 0.0);
    }

    Ok(format!(
        "{request_message}; primed eldenring.exe+0x{:X} addr=0x{fn_addr:X} manager=0x{:X}",
        offsets.common_event_preload_fn, manager_addr as usize
    ))
}

unsafe fn request_common_event_filecap(
    base: usize,
    id: u32,
    offsets: GparamOffsets,
) -> Result<String, String> {
    let manager_ptr_addr = base + offsets.gparam_resource_manager_global;
    let resource_manager = unsafe { *(manager_ptr_addr as *const *mut c_void) };
    if resource_manager.is_null() {
        return Err(format!(
            "gparam resource manager global eldenring.exe+0x{:X} is null",
            offsets.gparam_resource_manager_global
        ));
    }

    let request_addr = base + offsets.gparam_filecap_request_fn;
    let request: GparamFilecapRequestFn = unsafe { std::mem::transmute(request_addr) };
    let path = format!("gparam:/m00_00_{id:04}_CommonEvent.gparam");
    let mut wide_path = path.encode_utf16().collect::<Vec<_>>();
    wide_path.push(0);
    let filecap = unsafe { request(resource_manager, wide_path.as_ptr(), 0) };

    if filecap.is_null() {
        return Err(format!(
            "filecap_request path={path} result=0x0 resource_manager=0x{:X}",
            resource_manager as usize
        ));
    }

    Ok(format!(
        "filecap_request path={path} fn=eldenring.exe+0x{:X} addr=0x{request_addr:X} resource_manager=0x{:X} result=0x{:X}",
        offsets.gparam_filecap_request_fn, resource_manager as usize, filecap as usize
    ))
}

fn detect_offsets() -> Result<&'static GparamOffsets, String> {
    let exe = unsafe { GetModuleHandleA(None) }
        .map_err(|err| format!("GetModuleHandleA(None) failed: {err:?}"))?;
    let module = unsafe { PeView::module(exe.0 as *const u8) };
    SupportedExe::detect(&module)
        .map(|version| version.offsets())
        .map_err(|err| format!("unsupported Elden Ring executable: {err}"))
}

fn parse_id_list(value: &str) -> Vec<u32> {
    value
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                None
            } else {
                trimmed.parse::<u32>().ok()
            }
        })
        .collect()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn config_path(hmodule: usize) -> Option<PathBuf> {
    sidecar_path(hmodule, "gparam_preload_patch.ini")
}

fn sidecar_path(hmodule: usize, file_name: &str) -> Option<PathBuf> {
    let mut buffer = [0u16; 260];
    let len =
        unsafe { GetModuleFileNameW(Some(HMODULE(hmodule as *mut c_void)), &mut buffer) } as usize;
    if len == 0 {
        return None;
    }

    let mut path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
    path.set_file_name(file_name);
    Some(path)
}

fn append_log(path: &Option<PathBuf>, message: &str) {
    let Some(path) = path else {
        return;
    };

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}
