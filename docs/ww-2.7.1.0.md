# WW 2.7.1.0 compatibility evidence

Inspected 2026-09-08. Game version 1.17.1; local EXE file version 2.7.1.0.
EXE SHA-256: `1a3547101327f65d0c76da2f9190ac0aa66871ea42bae2aecc61e11a8b597891`.

| Target | WW 2.7.0.0 RVA | WW 2.7.1.0 RVA |
| --- | --- | --- |
| GPARAM filecap request | 0x001F2420 | 0x001F2420 |
| CommonEvent prime | 0x00AB9CB0 | 0x00AB9CB0 |
| GPARAM resource-manager global | 0x03D5F158 | 0x03D5F158 |
| System HINSTANCE global | 0x03D89708 | 0x03D89708 |
| Task registration | 0x00EB3DE0 | 0x00EB3E50 |

Evidence:

- CommonEvent loop at 0x613D00 sets type 8 at 0x613D07, loads the manager at 0x613D2A, calls filecap request at 0x613D62, and compares the loop counter with 5 at 0x613D91.
- Prime loop call at 0xAB96CB still targets 0xAB9CB0. The prime function sets type 8 at 0xAB9CE8 and passes the transition float from xmm2.
- HINSTANCE getter at 0xDEC0C0 and setter at 0xDEC6B0 still reference 0x03D89708.
- Native task caller at 0xEB7466 passes task in r8 and group in edx, then calls 0xEB3E50 at 0xEB746B.
- Old task RVA 0xEB3DE0 is now a short indexed getter beginning with `cmp edx, 6`, not task registration.

Reproduce with Python plus `pefile` and `capstone`:

```powershell
python tools/inspect_ww271.py 'F:\SteamLibrary\steamapps\common\ELDEN RING\Game\eldenring.exe'
cargo test --workspace --lib --locked
cargo build --release --locked
```

All three profile tests and the executable relationship checks passed. Static CRT imports were checked. This is static/build verification, not in-game validation. JP 2.7.1.x is not enabled. Existing Cargo revision and legacy profiles are retained; the DLL uses its own task RVAs rather than the upstream current-version dispatcher.
