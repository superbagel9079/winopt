# winopt

winopt is a Windows tuning tool that looks at the PC before it suggests anything. It reads whether the machine is a laptop or a desktop, who made it, its processor, graphics, memory, disks and power plan, and which vendor tools are installed. The suggestions depend on what it finds. A laptop keeps separate power settings on battery and on AC, an ASUS laptop gets G-Helper instead of a second tool that fights Armoury Crate, and an NVIDIA card gets a driver profile picked for that card.

It changes nothing until you click Apply, and Undo puts back the values it found. It reads the machine again every minute and whenever its window comes back to the front.

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

## What it changes

So far, one step: the power plan (Tuning > Power plan). What it does depends on the machine.

| Machine | What Apply does |
|---|---|
| Desktop | Makes High performance the plan in use (creating it when Windows lacks it), sets processor boost to Aggressive, and keeps the PCIe links, USB ports and disks from sleeping |
| Laptop with no maker tool | Keeps the plan in use and changes the same values for when it is plugged in only. The battery values do not change. |
| Laptop with G-Helper or Armoury Crate running | Nothing: the maker's tool already manages power, and two tools would undo each other |
| Machine winopt cannot identify | Nothing |

It goes through the Windows power API (powrprof), the same calls `powercfg` makes, and needs no administrator rights. Every value it changes is saved first in `undo-power.json`, in the app's data folder (`%APPDATA%\app.winopt.client`). Undo writes those values back, makes the previous plan active again and deletes any plan winopt created.

## Build

Needs Rust (1.89 or newer) and the Tauri CLI (`cargo install tauri-cli --version "^2"`). Windows 10 or 11, WebView2 is there already.

```
cd app/src-tauri
cargo tauri dev       # run it
cargo tauri build     # the installer, in target/release/bundle/nsis/
cargo test            # the rules, without the hardware
cargo test -- --ignored --nocapture   # read this machine for real, print the report
cargo test -- --ignored power_api     # write and restore values on a scratch copy of the active plan
```

To work on the page in a plain browser, serve `app/ui` with any static server. It then shows `app/ui/mock.json`, a report saved from a real machine.

## Layout

- `app/src-tauri/src/inventory.rs`: reads the machine. Each part is read separately, so a part that fails is listed under "Not read" and the rest still shows.
- `app/src-tauri/src/profile.rs`: what the readings mean (laptop or desktop, vendors, findings). Pure functions, tested.
- `app/src-tauri/src/power.rs`: the power plan for this machine, and Apply and Undo through the Windows power API.
- `app/ui/`: the page, plain HTML, CSS and JavaScript.
