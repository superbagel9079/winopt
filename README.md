# winopt

**Windows tuning that starts from the machine it runs on.** Other tools apply the same list of tweaks to every PC. winopt first reads what this one is: laptop or desktop, who made it, its processor, graphics, memory, disks, power plan and the vendor tools already there. What it suggests follows from that: a laptop keeps its power per source, an ASUS laptop gets G-Helper rather than a second tool fighting Armoury Crate, an NVIDIA card gets a driver profile chosen for it.

For now winopt **reads and explains, it changes nothing**. The tuning itself comes next.

## What it reads

| Part | What | From |
|---|---|---|
| The machine | maker, model, laptop or desktop, board, BIOS | WMI (`Win32_ComputerSystem`, `Win32_SystemEnclosure`, `Win32_BIOS`) |
| Processor | model, cores, threads, base clock | `Win32_Processor` |
| Graphics | each card, integrated or discrete, driver, memory, display | `Win32_VideoController`, the display class key in the registry (the real VRAM size) |
| Memory | installed and usable, modules, type, rated and actual speed | `Win32_PhysicalMemory` |
| Storage | each disk, NVMe / SATA, SSD / HDD, the one Windows is on | `MSFT_PhysicalDisk`, `MSFT_Partition` |
| Power | the active plan, battery and source | `powercfg`, `Win32_Battery` |
| Tools | Chocolatey, G-Helper, ASUS services | the file system, `Win32_Service`, `Win32_Process` |

From these, `profile.rs` draws what tuning this machine has to take into account (laptop power, ASUS tools in conflict, hybrid graphics, memory below its rated speed, Windows on a hard disk…).

## Build

Needs Rust (1.89 or newer) and the Tauri CLI (`cargo install tauri-cli --version "^2"`). Windows 10 or 11, WebView2 is there already.

```
cd app/src-tauri
cargo tauri dev       # run it
cargo tauri build     # the installer, in target/release/bundle/nsis/
cargo test            # the rules, without the hardware
cargo test -- --ignored --nocapture   # read this machine for real, print the report
```

The page alone, in a browser, shows `app/ui/mock.json` (a report saved from a real machine): serve `app/ui` with any static server.

## Layout

- `app/src-tauri/src/inventory.rs`: reads the machine. Each part is read on its own; one that fails is listed under "Not read" and the others still show.
- `app/src-tauri/src/profile.rs`: what the readings mean (laptop or desktop, vendors, findings). Pure functions, tested.
- `app/ui/`: the page, plain HTML, CSS and JavaScript.
