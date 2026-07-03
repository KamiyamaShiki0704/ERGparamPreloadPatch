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

This patch was tested against the local Elden Ring executable used during development. It relies on fixed RVAs for:

- GPARAM filecap request: `eldenring.exe+0x001F2420`
- CommonEvent drawparam prime: `eldenring.exe+0x00AB89A0`
- GPARAM resource manager global: `eldenring.exe+0x03D5B0F8`

These offsets may need updating for other game versions.

## License

Licensed under either of:

- MIT license (`LICENSE-MIT`)
- Apache License, Version 2.0 (`LICENSE-APACHE`)
