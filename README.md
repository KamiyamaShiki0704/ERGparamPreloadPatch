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

- WW `2.6.2.0`
- JP `2.6.1.1`
- JP `2.6.2.1`

The JP `2.6.1.1` and JP `2.6.2.1` executables are known to share the same addresses as WW `2.6.2.0`. Other JP or game versions may use different offsets.

The required offsets are:

- GPARAM filecap request: `eldenring.exe+0x001F2420`
- CommonEvent drawparam prime: `eldenring.exe+0x00AB89A0`
- GPARAM resource manager global: `eldenring.exe+0x03D5B0F8`

Unsupported versions need their own offset profile before the loader can safely run.

## License

Licensed under either of:

- MIT license (`LICENSE-MIT`)
- Apache License, Version 2.0 (`LICENSE-APACHE`)
