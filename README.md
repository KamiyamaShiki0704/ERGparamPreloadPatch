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

Cargo fetches the independently implemented runtime crates from the [ShikiFSRS](https://github.com/KamiyamaShiki0704/ShikiFSRS) repository, pinned to `fb9f4524da1fb648e0a696f2d47aa08d3acf35cc`. `Cargo.lock` records the immutable revision.

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

- WW `2.7.1.0` (game version `1.17.1`; local build awaiting runtime validation)
- WW `2.7.0.0`
- WW `2.6.2.0`
- JP `2.6.1.1`
- JP `2.6.2.1`

The JP `2.6.1.1` and JP `2.6.2.1` executables are known to share the same addresses as WW `2.6.2.0`. WW `2.7.0.0` uses updated addresses for the CommonEvent prime function and GPARAM resource-manager global. Other JP or game versions may use different offsets.

Version `0.1.2` also includes the updated WW `2.7.0.0` task-runtime RVA map required to register the loader's `DrawParamUpdate` callback. Earlier `0.1.1` builds recognized the new GPARAM profile but could abort during loading when the task runtime rejected the executable version.

The loader keeps `global_hinstance` and `register_task` in its own per-executable profiles and uses those addresses to register the pinned upstream `RecurringTask`. This preserves the listed legacy profiles without depending on `fromsoftware-rs`' internal current-version RVA dispatcher.

The verified offsets are:

For WW `2.7.1.0`, the three GPARAM addresses below and `global_hinstance` (`0x03D89708`) are unchanged from WW `2.7.0.0`. Task registration moved from `0x00EB3DE0` to `0x00EB3E50`. The pinned Cargo revision remains unchanged because this DLL manages task RVAs locally. JP `2.7.1.x` has not been verified.

| Profile | GPARAM filecap request | CommonEvent drawparam prime | GPARAM resource manager global |
| --- | --- | --- | --- |
| WW `2.7.1.0` | `0x001F2420` | `0x00AB9CB0` | `0x03D5F158` |
| WW `2.7.0.0` | `0x001F2420` | `0x00AB9CB0` | `0x03D5F158` |
| WW `2.6.2.0` | `0x001F2420` | `0x00AB89A0` | `0x03D5B0F8` |
| JP `2.6.1.1` | `0x001F2420` | `0x00AB89A0` | `0x03D5B0F8` |
| JP `2.6.2.1` | `0x001F2420` | `0x00AB89A0` | `0x03D5B0F8` |

Unsupported versions need their own offset profile before the loader can safely run.

## License

Licensed under either of:

- MIT license (`LICENSE-MIT`)
- Apache License, Version 2.0 (`LICENSE-APACHE`)

## Building with ShikiFSRS

Cargo pins `KamiyamaShiki0704/ShikiFSRS` to `fb9f4524da1fb648e0a696f2d47aa08d3acf35cc`. `.cargo/config.toml` enables Git CLI fetching; no token is stored in this repository.

The migration was checked offline against ER WW 1.17.1 (EXE 2.7.1.0). Native engine ownership, phase/lifetime requirements and unexecuted gameplay behavior remain experimental and require manual acceptance. Existing ignored game/asset fixtures were not executed.
