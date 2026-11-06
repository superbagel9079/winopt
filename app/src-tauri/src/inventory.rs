//! What the machine is: read from WMI, the registry and powercfg, never changed.
//!
//! Every section is read on its own: one that fails (a WMI class missing on an edition of
//! Windows, a namespace refused) leaves its part empty and adds a line to `warnings`, so
//! the rest of the machine still shows.

use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use wmi::WMIConnection;

use crate::profile::{self, FormFactor, GpuVendor};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub machine: Machine,
    pub os: Os,
    pub cpu: Cpu,
    pub gpus: Vec<Gpu>,
    pub memory: Memory,
    pub disks: Vec<Disk>,
    pub power: Power,
    pub tools: Tools,
    /// The parts that could not be read, and why.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub manufacturer: String,
    pub model: String,
    pub family: String,
    pub form: FormFactor,
    pub board: String,
    pub bios_version: String,
    pub bios_date: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Os {
    pub caption: String,
    pub version: String,
    pub build: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cpu {
    pub name: String,
    pub manufacturer: String,
    pub cores: u32,
    pub threads: u32,
    pub base_mhz: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    pub vendor: GpuVendor,
    pub discrete: bool,
    pub driver: String,
    pub vram_bytes: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub refresh_hz: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub total_bytes: u64,
    pub modules: Vec<MemoryModule>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryModule {
    pub capacity_bytes: u64,
    pub kind: String,
    pub soldered: bool,
    pub rated_mhz: u32,
    pub configured_mhz: u32,
    pub manufacturer: String,
    pub part: String,
    pub slot: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub name: String,
    pub media: String,
    pub bus: String,
    pub size_bytes: u64,
    pub system: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Power {
    pub plan_guid: String,
    pub plan_name: String,
    pub battery: Option<Battery>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub charge_percent: Option<u32>,
    pub on_ac: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub chocolatey: bool,
    /// ASUS services found, with their start mode ("Auto", "Manual", "Disabled").
    pub asus_services: Vec<(String, String)>,
    pub ghelper_running: bool,
}

/// Reads the whole machine. Slow (WMI, a second or two): call it off the UI thread.
pub fn collect() -> Inventory {
    let mut inv = Inventory::default();
    let cimv2 = match WMIConnection::new() {
        Ok(c) => c,
        Err(e) => {
            inv.warnings.push(format!("WMI unreachable (ROOT\\CIMV2): {e}"));
            return inv;
        }
    };
    let mut battery_present = false;
    let mut pc_system_type = 0u16;
    let mut chassis: Vec<u16> = Vec::new();

    section(&mut inv, "machine", |inv| {
        let cs: Win32ComputerSystem = first(
            &cimv2,
            "SELECT Manufacturer, Model, SystemFamily, PCSystemType, TotalPhysicalMemory FROM Win32_ComputerSystem",
        )?;
        inv.machine.manufacturer = clean(cs.manufacturer);
        inv.machine.model = profile::clean_model(&clean(cs.model));
        inv.machine.family = clean(cs.system_family);
        inv.memory.total_bytes = cs.total_physical_memory.unwrap_or(0);
        pc_system_type = cs.pc_system_type.unwrap_or(0);
        let enclosure: Win32SystemEnclosure = first(&cimv2, "SELECT ChassisTypes FROM Win32_SystemEnclosure")?;
        chassis = enclosure.chassis_types.unwrap_or_default();
        let board: Win32BaseBoard = first(&cimv2, "SELECT Manufacturer, Product FROM Win32_BaseBoard")?;
        inv.machine.board = clean(board.product);
        let bios: Win32Bios = first(&cimv2, "SELECT SMBIOSBIOSVersion, ReleaseDate FROM Win32_BIOS")?;
        inv.machine.bios_version = clean(bios.smbios_bios_version);
        inv.machine.bios_date = bios
            .release_date
            .map(|d| d.chars().take(8).collect())
            .map(format_cim_date)
            .unwrap_or_default();
        Ok(())
    });

    section(&mut inv, "Windows", |inv| {
        let os: Win32OperatingSystem = first(&cimv2, "SELECT Caption, Version, BuildNumber FROM Win32_OperatingSystem")?;
        inv.os = Os {
            caption: clean(os.caption),
            version: clean(os.version),
            build: clean(os.build_number),
        };
        Ok(())
    });

    section(&mut inv, "processor", |inv| {
        let cpu: Win32Processor = first(
            &cimv2,
            "SELECT Name, Manufacturer, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed FROM Win32_Processor",
        )?;
        inv.cpu = Cpu {
            name: clean(cpu.name),
            manufacturer: clean(cpu.manufacturer),
            cores: cpu.number_of_cores.unwrap_or(0),
            threads: cpu.number_of_logical_processors.unwrap_or(0),
            base_mhz: cpu.max_clock_speed.unwrap_or(0),
        };
        Ok(())
    });

    section(&mut inv, "graphics", |inv| {
        let controllers: Vec<Win32VideoController> = cimv2.raw_query(
            "SELECT Name, DriverVersion, PNPDeviceID, CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate FROM Win32_VideoController",
        )?;
        let vram = read_vram_by_device();
        inv.gpus = controllers.into_iter().filter_map(|c| gpu_from(c, &vram)).collect();
        Ok(())
    });

    section(&mut inv, "memory", |inv| {
        let modules: Vec<Win32PhysicalMemory> = cimv2.raw_query(
            "SELECT Capacity, Speed, ConfiguredClockSpeed, SMBIOSMemoryType, FormFactor, Manufacturer, PartNumber, DeviceLocator FROM Win32_PhysicalMemory",
        )?;
        inv.memory.modules = modules.into_iter().map(module_from).collect();
        Ok(())
    });

    section(&mut inv, "battery", |inv| {
        let batteries: Vec<Win32Battery> = cimv2.raw_query("SELECT BatteryStatus, EstimatedChargeRemaining FROM Win32_Battery")?;
        battery_present = !batteries.is_empty();
        inv.power.battery = batteries.into_iter().next().map(|b| Battery {
            charge_percent: b.estimated_charge_remaining.map(u32::from),
            // 2 is "on AC" in Win32_Battery; anything else (1 discharging, 4 low…) is battery.
            on_ac: b.battery_status == Some(2),
        });
        Ok(())
    });

    inv.machine.form = profile::form_factor(&chassis, pc_system_type, battery_present);

    section(&mut inv, "storage", |inv| {
        let storage = WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Storage")?;
        let disks: Vec<MsftPhysicalDisk> =
            storage.raw_query("SELECT FriendlyName, MediaType, BusType, Size, DeviceId FROM MSFT_PhysicalDisk")?;
        let boot: Vec<MsftPartition> = storage.raw_query("SELECT DiskNumber FROM MSFT_Partition WHERE IsBoot = TRUE")?;
        let boot_disks: Vec<String> = boot.iter().filter_map(|p| p.disk_number).map(|n| n.to_string()).collect();
        inv.disks = disks.into_iter().map(|d| disk_from(d, &boot_disks)).collect();
        Ok(())
    });

    section(&mut inv, "tools", |inv| {
        let services: Vec<Win32Service> = cimv2.raw_query(format!(
            "SELECT Name, StartMode FROM Win32_Service WHERE {}",
            profile::ASUS_SERVICES
                .iter()
                .map(|s| format!("Name = '{s}'"))
                .collect::<Vec<_>>()
                .join(" OR ")
        ))?;
        inv.tools.asus_services = services.into_iter().map(|s| (clean(s.name), clean(s.start_mode))).collect();
        let ghelper: Vec<HashMap<String, wmi::Variant>> =
            cimv2.raw_query("SELECT ProcessId FROM Win32_Process WHERE Name = 'GHelper.exe'")?;
        inv.tools.ghelper_running = !ghelper.is_empty();
        Ok(())
    });
    inv.tools.chocolatey = chocolatey_installed();

    section(&mut inv, "power plan", |inv| {
        let (guid, name) = active_power_plan()?;
        inv.power.plan_name = profile::power_plan_name(&guid).map(str::to_owned).unwrap_or(name);
        inv.power.plan_guid = guid;
        Ok(())
    });

    inv
}

type SectionResult = Result<(), Box<dyn std::error::Error>>;

/// Runs one reading; its failure is kept as a warning, the others still run.
fn section(inv: &mut Inventory, what: &str, read: impl FnOnce(&mut Inventory) -> SectionResult) {
    if let Err(e) = read(inv) {
        inv.warnings.push(format!("{what}: {e}"));
    }
}

fn first<T: for<'de> Deserialize<'de>>(con: &WMIConnection, query: &str) -> Result<T, Box<dyn std::error::Error>> {
    con.raw_query::<T>(query)?
        .into_iter()
        .next()
        .ok_or_else(|| format!("no result for `{query}`").into())
}

/// WMI strings come padded, or as OEM placeholders ("To Be Filled By O.E.M.").
fn clean(value: Option<String>) -> String {
    let v = value.unwrap_or_default().trim().to_owned();
    if profile::is_placeholder(&v) {
        String::new()
    } else {
        v
    }
}

/// "20250606" as "2025-06-06".
fn format_cim_date(d: String) -> String {
    if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
    } else {
        d
    }
}

fn gpu_from(c: Win32VideoController, vram: &HashMap<String, u64>) -> Option<Gpu> {
    let pnp = c.pnp_device_id.unwrap_or_default();
    // Remote desktop and "Microsoft Basic Display" adapters are not real graphics cards.
    if !pnp.to_ascii_uppercase().starts_with("PCI\\") {
        return None;
    }
    let name = clean(c.name);
    let vendor = profile::gpu_vendor(&pnp);
    let key = device_key(&pnp);
    Some(Gpu {
        discrete: profile::is_discrete_gpu(vendor, &name),
        vendor,
        name,
        driver: clean(c.driver_version),
        // Some drivers register the device without its subsystem: the longest match wins.
        vram_bytes: vram
            .iter()
            .filter(|(id, _)| key.starts_with(id.as_str()))
            .max_by_key(|(id, _)| id.len())
            .map(|(_, &v)| v),
        width: c.current_horizontal_resolution,
        height: c.current_vertical_resolution,
        refresh_hz: c.current_refresh_rate.filter(|&hz| hz > 1),
    })
}

/// "PCI\VEN_10DE&DEV_2860&SUBSYS_1C631043&REV_A1\4&…" as "pci\ven_10de&dev_2860&subsys_1c631043",
/// the form the display class keys give in MatchingDeviceId.
fn device_key(pnp: &str) -> String {
    pnp.to_ascii_lowercase()
        .split('\\')
        .take(2)
        .collect::<Vec<_>>()
        .join("\\")
        .split("&rev_")
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Win32_VideoController's AdapterRAM stops at 4 GB (a 32-bit field); the driver's own
/// figure is in the display class key, by device.
fn read_vram_by_device() -> HashMap<String, u64> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let mut out = HashMap::new();
    let Ok(class) = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}")
    else {
        return out;
    };
    for sub in class.enum_keys().flatten() {
        let Ok(key) = class.open_subkey(&sub) else { continue };
        let Ok(id) = key.get_value::<String, _>("MatchingDeviceId") else {
            continue;
        };
        if let Ok(size) = key.get_value::<u64, _>("HardwareInformation.qwMemorySize") {
            out.insert(id.to_ascii_lowercase(), size);
        }
    }
    out
}

fn module_from(m: Win32PhysicalMemory) -> MemoryModule {
    let kind = profile::memory_kind(m.smbios_memory_type.unwrap_or(0));
    MemoryModule {
        capacity_bytes: m.capacity.unwrap_or(0),
        soldered: kind.starts_with("LP") || m.form_factor == Some(profile::FORM_FACTOR_ROW_OF_CHIPS),
        kind: kind.to_owned(),
        rated_mhz: m.speed.unwrap_or(0),
        configured_mhz: m.configured_clock_speed.unwrap_or(0),
        manufacturer: clean(m.manufacturer),
        part: clean(m.part_number),
        slot: clean(m.device_locator),
    }
}

fn disk_from(d: MsftPhysicalDisk, boot_disks: &[String]) -> Disk {
    Disk {
        name: clean(d.friendly_name),
        media: profile::disk_media(d.media_type.unwrap_or(0)).to_owned(),
        bus: profile::disk_bus(d.bus_type.unwrap_or(0)).to_owned(),
        size_bytes: d.size.unwrap_or(0),
        system: d.device_id.is_some_and(|id| boot_disks.contains(&id)),
    }
}

fn chocolatey_installed() -> bool {
    let from_env = std::env::var_os("ChocolateyInstall").map(PathBuf::from);
    let default = std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("chocolatey"));
    [from_env, default]
        .into_iter()
        .flatten()
        .any(|root| root.join("bin").join("choco.exe").is_file())
}

/// powercfg, since Win32_PowerPlan refuses some sessions (remote desktop on LTSC).
fn active_power_plan() -> Result<(String, String), Box<dyn std::error::Error>> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new("powercfg")
        .arg("/getactivescheme")
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if !out.status.success() {
        return Err(format!("powercfg /getactivescheme exited with {}", out.status).into());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    profile::parse_active_scheme(&text).ok_or_else(|| format!("unexpected powercfg output: {}", text.trim()).into())
}

// --- WMI rows -------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32ComputerSystem {
    manufacturer: Option<String>,
    model: Option<String>,
    system_family: Option<String>,
    #[serde(rename = "PCSystemType")]
    pc_system_type: Option<u16>,
    total_physical_memory: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32SystemEnclosure {
    chassis_types: Option<Vec<u16>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32BaseBoard {
    product: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32Bios {
    #[serde(rename = "SMBIOSBIOSVersion")]
    smbios_bios_version: Option<String>,
    release_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32OperatingSystem {
    caption: Option<String>,
    version: Option<String>,
    build_number: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32Processor {
    name: Option<String>,
    manufacturer: Option<String>,
    number_of_cores: Option<u32>,
    number_of_logical_processors: Option<u32>,
    max_clock_speed: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32VideoController {
    name: Option<String>,
    driver_version: Option<String>,
    #[serde(rename = "PNPDeviceID")]
    pnp_device_id: Option<String>,
    current_horizontal_resolution: Option<u32>,
    current_vertical_resolution: Option<u32>,
    current_refresh_rate: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32PhysicalMemory {
    capacity: Option<u64>,
    speed: Option<u32>,
    configured_clock_speed: Option<u32>,
    #[serde(rename = "SMBIOSMemoryType")]
    smbios_memory_type: Option<u32>,
    form_factor: Option<u16>,
    manufacturer: Option<String>,
    part_number: Option<String>,
    device_locator: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32Battery {
    battery_status: Option<u16>,
    estimated_charge_remaining: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MsftPhysicalDisk {
    friendly_name: Option<String>,
    media_type: Option<u16>,
    bus_type: Option<u16>,
    size: Option<u64>,
    device_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MsftPartition {
    disk_number: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32Service {
    name: Option<String>,
    start_mode: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_key_matches_the_registry_form() {
        assert_eq!(
            device_key("PCI\\VEN_10DE&DEV_2860&SUBSYS_1C631043&REV_A1\\4&6AD973D&0&0008"),
            "pci\\ven_10de&dev_2860&subsys_1c631043"
        );
        assert_eq!(
            device_key("PCI\\VEN_8086&DEV_7D55&SUBSYS_37781043&REV_08\\3&1"),
            "pci\\ven_8086&dev_7d55&subsys_37781043"
        );
    }

    #[test]
    fn cim_dates_are_iso() {
        assert_eq!(format_cim_date("20250606".into()), "2025-06-06");
        assert_eq!(format_cim_date("garbage".into()), "garbage");
    }
}
