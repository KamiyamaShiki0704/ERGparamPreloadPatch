# ERGparamPreloadPatch

Runtime loader for extra Elden Ring CommonEvent GPARAM files.

The DLL registers configured `m00_00_XXXX_CommonEvent.gparam` resources through the game's GPARAM filecap path so event scripts can call ids beyond the default preloaded range, for example `ActivateGparamOverride(5, ...)` or `ActivateGparamOverride(6, ...)`.

## Build

```powershell
cargo build --release
```

The output DLL is:

```text
target\release\gparam_preload_patch.dll
```

The repository vendors the required `fromsoftware-rs` crates under `vendor/fromsoftware-rs` so the DLL can be built as a standalone workspace.

## Install

Place these files together in the mod DLL loading folder:

```text
gparam_preload_patch.dll
gparam_preload_patch.ini
```

The sample config is in `config/gparam_preload_patch.ini`.

## Configuration

`common_event_ids` controls which CommonEvent GPARAM ids are registered. Each id maps to:

```text
gparam:/m00_00_XXXX_CommonEvent.gparam
```

`log_enabled=true` writes `gparam_preload_patch.log` next to the DLL.

## Notes

This patch uses per-executable offset profiles and disables itself when the running Elden Ring executable is not recognized.

Currently supported profiles:

- WW `2.7.0.0`
- WW `2.6.2.0`
- JP `2.6.1.1`
- JP `2.6.2.1`

The JP `2.6.1.1` and JP `2.6.2.1` executables are known to share the same addresses as WW `2.6.2.0`. WW `2.7.0.0` uses updated addresses for the CommonEvent prime function and GPARAM resource-manager global. Other JP or game versions may use different offsets.

The verified offsets are:

| Profile | GPARAM filecap request | CommonEvent drawparam prime | GPARAM resource manager global |
| --- | --- | --- | --- |
| WW `2.7.0.0` | `0x001F2420` | `0x00AB9CB0` | `0x03D5F158` |
| WW `2.6.2.0` | `0x001F2420` | `0x00AB89A0` | `0x03D5B0F8` |
| JP `2.6.1.1` | `0x001F2420` | `0x00AB89A0` | `0x03D5B0F8` |
| JP `2.6.2.1` | `0x001F2420` | `0x00AB89A0` | `0x03D5B0F8` |

Unsupported versions need their own offset profile before the loader can safely run.

## License

Licensed under either of:

- MIT license (`LICENSE-MIT`)
- Apache License, Version 2.0 (`LICENSE-APACHE`)
