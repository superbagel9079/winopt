# winopt

winopt is a Windows tuning tool that looks at the PC before it suggests anything. It reads whether the machine is a laptop or a desktop, who made it, its processor, graphics, memory, disks and power plan, and which vendor tools are installed. The suggestions depend on what it finds. A laptop keeps separate power settings on battery and on AC, an ASUS laptop gets G-Helper instead of a second tool that fights Armoury Crate, and an NVIDIA card gets a driver profile picked for that card.

This version only reads and explains. It does not change anything on the machine yet. It reads the machine again every minute and whenever its window comes back to the front.

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

`profile.rs` turns these readings into findings: power on a laptop, ASUS tools in conflict, hybrid graphics, memory below its rated speed, Windows on a hard disk.

## Build

Needs Rust (1.89 or newer) and the Tauri CLI (`cargo install tauri-cli --version "^2"`). Windows 10 or 11, WebView2 is there already.

```
cd app/src-tauri
cargo tauri dev       # run it
cargo tauri build     # the installer, in target/release/bundle/nsis/
cargo test            # the rules, without the hardware
cargo test -- --ignored --nocapture   # read this machine for real, print the report
```

To work on the page in a plain browser, serve `app/ui` with any static server. It then shows `app/ui/mock.json`, a report saved from a real machine.

## Layout

- `app/src-tauri/src/inventory.rs`: reads the machine. Each part is read separately, so a part that fails is listed under "Not read" and the rest still shows.
- `app/src-tauri/src/profile.rs`: what the readings mean (laptop or desktop, vendors, findings). Pure functions, tested.
- `app/ui/`: the page, plain HTML, CSS and JavaScript.
