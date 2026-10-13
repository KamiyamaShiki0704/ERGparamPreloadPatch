//! Independently verified fixed WW 1.17.1 Gparam candidate; real resource acceptance pending.
//! The loader retains this DLL until the resident task pins it; native registration/allocator
//! serialization and future DrawParamUpdate full resource-graph lifetime remain experimental.
#![deny(unsafe_op_in_unsafe_fn)]
use shikifsrs_elden_ring::{
    TaskPhase, Ww171File, gparam::CommonEventId, gparam_resident::ResidentGparamSlot,
    gparam_task::GparamPreloadConfig,
};
use shikifsrs_runtime::memory::CurrentProcess;
use std::{
    ffi::c_void,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use windows::Win32::{
    Foundation::HMODULE,
    System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW},
};
static PRELOADER: ResidentGparamSlot = ResidentGparamSlot::new();
static STARTED: AtomicBool = AtomicBool::new(false);

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

impl Config {
    fn load(hmodule: usize) -> Self {
        config_path(hmodule)
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|contents| Self::parse(&contents))
            .unwrap_or_default()
    }
    fn parse(contents: &str) -> Self {
        let mut config = Self::default();
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
    fn sdk_config(&self) -> shikifsrs_runtime::Result<GparamPreloadConfig> {
        if self.common_event_ids.len() > 256 {
            return Err(shikifsrs_runtime::Error::InvalidProfile(
                "more than 256 common event IDs",
            ));
        }
        Ok(GparamPreloadConfig {
            enabled: self.enabled,
            ids: self
                .common_event_ids
                .iter()
                .copied()
                .map(CommonEventId::new)
                .collect::<shikifsrs_runtime::Result<_>>()?,
            start_delay_ms: self.start_delay_ms,
            attempts_per_id: self.retries_per_id,
            retry_every_frames: self.retry_every_frames,
            request_filecap: self.request_filecap,
            prime_drawparam: self.prime_drawparam,
        })
    }
}

/// # Safety
/// Loader/registration and every future resource callback obey the module header contract.
unsafe fn install(config: Config, log_path: Option<PathBuf>) -> Result<(), String> {
    if !config.enabled || config.common_event_ids.is_empty() {
        append_log(&log_path, "disabled or no common_event_ids configured");
        return Ok(());
    }
    let mut schedule = config.sdk_config().map_err(|error| error.to_string())?;
    let path = std::env::current_exe().map_err(|error| error.to_string())?;
    let bytes: Arc<[u8]> = fs::read(path).map_err(|error| error.to_string())?.into();
    // Reject unknown files before any loaded game-memory access.
    let file = Ww171File::verify(&bytes).map_err(|error| error.to_string())?;
    // Retain the original delay before task registration; the SDK must not wait it a second time.
    std::thread::sleep(Duration::from_millis(schedule.start_delay_ms));
    schedule.start_delay_ms = 0;
    // SAFETY: None queries the process executable handle; this borrowed handle is never freed.
    let base = unsafe { GetModuleHandleW(None) }
        .map_err(|error| error.to_string())?
        .0 as u64;
    let memory = CurrentProcess;
    loop {
        let catalog = file
            .bind(&memory, base)
            .map_err(|error| error.to_string())?;
        if let Some(group) = catalog
            .observe_phase(TaskPhase::DRAW_PARAMETER_UPDATE)
            .map_err(|error| error.to_string())?
            && let Some(id) = group.native_group_id()
            && catalog
                .registered_group(id, 4096)
                .map_err(|error| error.to_string())?
                .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // SAFETY: Experimental supported registration phase and future exclusive DrawParamUpdate
    // resource/manager/allocator graph contract. Group presence alone does not prove that contract.
    let installed = unsafe { PRELOADER.install_current(bytes, base, schedule, None) }
        .map_err(|error| error.to_string())?;
    let control = installed.registration.map_err(|error| {
        format!(
            "module pinned at {:#x}; registration failed: {error}",
            installed.module_handle
        )
    })?;
    append_log(
        &log_path,
        "WW 1.17.1 DrawParamUpdate scheduler registered (experimental)",
    );
    let mut previous = Vec::new();
    loop {
        if let Some(report) = control.last_report() {
            for (index, entry) in report.submissions.iter().enumerate() {
                if previous.get(index) != Some(entry) {
                    append_log(
                        &log_path,
                        &format!(
                            "common_event id={} attempt={} state={:?} request={:?} prime={:?} error={:?}",
                            entry.id.value(),
                            entry.attempts,
                            entry.state,
                            entry.file_request,
                            entry.prime,
                            entry.last_error
                        ),
                    );
                }
            }
            previous = report.submissions;
            if report.settled || report.blocked {
                append_log(
                    &log_path,
                    if report.blocked {
                        "submission uncertain; receipts retained, no automatic retry"
                    } else {
                        "scheduler settled; native submission does not prove resource ready"
                    },
                );
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // The static resident slot retains registration; exiting this worker never unregisters/frees it.
    Ok(())
}

/// # Safety
/// Windows invokes the standard DLL ABI. Host retains this DLL until PIN and obeys the
/// module header's experimental native phase/lifetime contracts; hot-unloading is unsupported.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(hmodule: usize, reason: u32, _reserved: *mut c_void) -> i32 {
    if reason == 1 && !STARTED.swap(true, Ordering::AcqRel) {
        let _ = std::thread::Builder::new().name("gparam-preload-init".into()).spawn(move || {
            let config = Config::load(hmodule);
            let log_path = if config.log_enabled { sidecar_path(hmodule, "gparam_preload_patch.log") } else { None };
            append_log(&log_path, &format!("loaded enabled={} common_event_ids={:?} start_delay_ms={} retries_per_id={} retry_every_frames={} request_filecap={} prime_drawparam={}",
                config.enabled, config.common_event_ids, config.start_delay_ms, config.retries_per_id,
                config.retry_every_frames, config.request_filecap, config.prime_drawparam));
            // SAFETY: Host contract is propagated from DLL load to this worker and resident callback.
            if let Err(error) = unsafe { install(config, log_path.clone()) } {
                append_log(&log_path, &format!("rejected: {error}"));
            }
        });
    }
    1
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
    let mut buffer = vec![0u16; 32768];
    let len =
        unsafe { GetModuleFileNameW(Some(HMODULE(hmodule as *mut c_void)), &mut buffer) } as usize;
    if len == 0 || len >= buffer.len() {
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

#[cfg(test)]
mod tests;
