//! What the readings mean for this machine: its kind, its parts, and what tuning has to
//! take into account. Pure functions over what `inventory` read, so they are tested
//! without the hardware.

use serde::Serialize;

use crate::inventory::Inventory;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FormFactor {
    Laptop,
    Desktop,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

/// ASUS services that Armoury Crate and MyASUS install.
pub const ASUS_SERVICES: &[&str] = &[
    "ArmouryCrateService",
    "ArmouryCrateControlInterface",
    "AsusAppService",
    "ASUSOptimization",
    "ASUSSystemAnalysis",
    "ASUSSystemDiagnosis",
    "LightingService",
];

/// SMBIOS memory form factor 23: chips soldered on the board.
pub const FORM_FACTOR_ROW_OF_CHIPS: u16 = 23;

// SMBIOS chassis types (DMTF DSP0134, 7.4.1).
const LAPTOP_CHASSIS: &[u16] = &[8, 9, 10, 11, 12, 14, 18, 21, 30, 31, 32];
const DESKTOP_CHASSIS: &[u16] = &[3, 4, 5, 6, 7, 13, 15, 16, 24, 34, 35, 36];

/// Laptop or desktop, from the chassis first, then Win32_ComputerSystem.PCSystemType
/// (2 is mobile), then the presence of a battery. A UPS shows as a battery on some
/// desktops, so the battery only decides when nothing else does.
pub fn form_factor(chassis: &[u16], pc_system_type: u16, battery: bool) -> FormFactor {
    if chassis.iter().any(|c| LAPTOP_CHASSIS.contains(c)) {
        return FormFactor::Laptop;
    }
    if chassis.iter().any(|c| DESKTOP_CHASSIS.contains(c)) {
        return FormFactor::Desktop;
    }
    match pc_system_type {
        2 => FormFactor::Laptop,
        1 | 3 | 4 | 5 | 7 => FormFactor::Desktop,
        _ if battery => FormFactor::Laptop,
        _ => FormFactor::Unknown,
    }
}

pub fn gpu_vendor(pnp_device_id: &str) -> GpuVendor {
    let id = pnp_device_id.to_ascii_uppercase();
    if id.contains("VEN_10DE") {
        GpuVendor::Nvidia
    } else if id.contains("VEN_1002") || id.contains("VEN_1022") {
        GpuVendor::Amd
    } else if id.contains("VEN_8086") {
        GpuVendor::Intel
    } else {
        GpuVendor::Other
    }
}

/// A graphics card of its own, rather than one inside the processor. Intel's and AMD's
/// integrated parts carry no model number of a card ("Arc Graphics", "Radeon 780M").
pub fn is_discrete_gpu(vendor: GpuVendor, name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    match vendor {
        GpuVendor::Nvidia => true,
        GpuVendor::Amd => n.contains(" rx ") || n.contains("radeon pro") || n.contains("radeon vii"),
        GpuVendor::Intel => n.contains("arc(tm) a") || n.contains("arc(tm) b") || n.contains("arc a") || n.contains("arc b"),
        GpuVendor::Other => false,
    }
}

/// SMBIOS memory type (DSP0134, 7.18.2) as its usual name.
pub fn memory_kind(smbios_type: u32) -> &'static str {
    match smbios_type {
        20 => "DDR",
        21 => "DDR2",
        24 => "DDR3",
        26 => "DDR4",
        27 => "LPDDR",
        28 => "LPDDR2",
        29 => "LPDDR3",
        30 => "LPDDR4",
        34 => "DDR5",
        35 => "LPDDR5",
        _ => "",
    }
}

/// MSFT_PhysicalDisk.MediaType.
pub fn disk_media(media_type: u16) -> &'static str {
    match media_type {
        3 => "HDD",
        4 => "SSD",
        5 => "SCM",
        _ => "",
    }
}

/// MSFT_PhysicalDisk.BusType.
pub fn disk_bus(bus_type: u16) -> &'static str {
    match bus_type {
        3 => "ATA",
        7 => "USB",
        8 => "RAID",
        11 => "SATA",
        12 => "SD",
        17 => "NVMe",
        _ => "",
    }
}

/// The built-in plans, whatever the language of Windows.
pub fn power_plan_name(guid: &str) -> Option<&'static str> {
    match guid.to_ascii_lowercase().as_str() {
        "381b4222-f694-41f0-9685-ff5bb260df2e" => Some("Balanced"),
        "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" => Some("High performance"),
        "a1841308-3541-4fab-bc81-f71556f20b4a" => Some("Power saver"),
        "e9a42b02-d5df-448d-aa00-03f14749eb61" => Some("Ultimate Performance"),
        _ => None,
    }
}

/// The GUID and name in `powercfg /getactivescheme`, in any language:
/// "<label>: 381b4222-…-ff5bb260df2e  (Balanced)".
pub fn parse_active_scheme(text: &str) -> Option<(String, String)> {
    let line = text.lines().find(|l| l.contains('('))?;
    let guid = line
        .split(|c: char| !(c.is_ascii_hexdigit() || c == '-'))
        .find(|w| w.len() == 36 && w.matches('-').count() == 4)?
        .to_ascii_lowercase();
    let name = line.rsplit_once('(')?.1.trim_end().trim_end_matches(')').trim().to_owned();
    Some((guid, name))
}

/// The model name without the doubled code ASUS writes in it:
/// "ROG Zephyrus G16 GU605MI_GU605MI" is "ROG Zephyrus G16 GU605MI".
pub fn clean_model(model: &str) -> String {
    let mut words: Vec<&str> = model.split_whitespace().collect();
    if let Some(last) = words.last_mut() {
        if let Some((a, b)) = last.split_once('_') {
            if a.eq_ignore_ascii_case(b) {
                *last = a;
            }
        }
    }
    words.join(" ")
}

/// What OEMs leave in fields they did not fill.
pub fn is_placeholder(value: &str) -> bool {
    let v = value.to_ascii_lowercase();
    v.is_empty()
        || v.contains("to be filled")
        || v.contains("system product name")
        || v.contains("system manufacturer")
        || v == "default string"
        || v == "not applicable"
        || v == "o.e.m."
}

// --- findings -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// What tuning will take into account.
    Note,
    /// Something worth changing, outside what winopt does itself.
    Advice,
    /// Something that holds the machine back.
    Warning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: &'static str,
    pub area: &'static str,
    pub level: Level,
    pub title: String,
    pub detail: String,
}

fn finding(id: &'static str, area: &'static str, level: Level, title: impl Into<String>, detail: impl Into<String>) -> Finding {
    Finding {
        id,
        area,
        level,
        title: title.into(),
        detail: detail.into(),
    }
}

pub fn is_asus(inv: &Inventory) -> bool {
    inv.machine.manufacturer.to_ascii_lowercase().contains("asus")
}

/// What this machine's tuning has to take into account, most important first.
pub fn findings(inv: &Inventory) -> Vec<Finding> {
    let mut out = Vec::new();
    power_findings(inv, &mut out);
    asus_findings(inv, &mut out);
    gpu_findings(inv, &mut out);
    memory_findings(inv, &mut out);
    storage_findings(inv, &mut out);
    if !inv.tools.chocolatey {
        out.push(finding("choco-missing", "Tools", Level::Note,
            "Chocolatey is not installed",
            "Tools winopt installs (NVIDIA Profile Inspector, G-Helper…) come through Chocolatey: it will be installed first, when one is needed."));
    }
    out.sort_by_key(|f| match f.level {
        Level::Warning => 0,
        Level::Advice => 1,
        Level::Note => 2,
    });
    out
}

fn power_findings(inv: &Inventory, out: &mut Vec<Finding>) {
    let plan = inv.power.plan_name.as_str();
    match inv.machine.form {
        FormFactor::Laptop => {
            out.push(finding("laptop-power", "Power", Level::Note,
                "Laptop: power stays per source",
                "Plugged in, performance can be raised; on battery, the plan and the processor's limits are left to save it. No Ultimate Performance plan forced on a laptop."));
            if plan == "Ultimate Performance" || plan == "High performance" {
                out.push(finding("laptop-plan-high", "Power", Level::Advice,
                    format!("{plan} plan on a laptop"),
                    "It also applies on battery, and keeps the processor from resting: battery life and heat suffer. Balanced, with the vendor's performance mode plugged in, gives the same speed on AC."));
            }
        }
        FormFactor::Desktop if plan == "Power saver" => {
            out.push(finding(
                "desktop-saver",
                "Power",
                Level::Warning,
                "Power saver plan on a desktop",
                "The processor is held back for no gain: a desktop has no battery to save.",
            ));
        }
        _ => {}
    }
}

fn asus_findings(inv: &Inventory, out: &mut Vec<Finding>) {
    if !is_asus(inv) || inv.machine.form != FormFactor::Laptop {
        return;
    }
    let armoury = inv
        .tools
        .asus_services
        .iter()
        .any(|(name, mode)| name.starts_with("ArmouryCrate") && mode != "Disabled");
    if inv.tools.ghelper_running {
        out.push(finding("ghelper", "ASUS", Level::Note,
            "G-Helper is running",
            "It sets the performance mode, the GPU mode and the charge limit: winopt leaves those to it and does not change the power plan behind its back."));
        if armoury {
            out.push(finding("ghelper-armoury", "ASUS", Level::Warning,
                "G-Helper and Armoury Crate both active",
                "Both drive the same firmware settings and undo each other's. Keep one: G-Helper alone means Armoury Crate's services stopped."));
        }
    } else if armoury {
        out.push(finding("armoury", "ASUS", Level::Advice,
            "Armoury Crate is installed",
            "G-Helper does the same (performance modes, GPU mode, charge limit, fans) in a few MB, without the background services. Offered as a later step, through Chocolatey."));
    } else {
        out.push(finding("asus-no-tool", "ASUS", Level::Advice,
            "No ASUS control tool found",
            "Without Armoury Crate or G-Helper, the performance mode and the charge limit stay where the firmware left them. G-Helper sets them in a few MB."));
    }
}

fn gpu_findings(inv: &Inventory, out: &mut Vec<Finding>) {
    if let Some(gpu) = inv.gpus.iter().find(|g| g.vendor == GpuVendor::Nvidia) {
        out.push(finding("nvidia", "Graphics", Level::Note,
            gpu.name.clone(),
            "A driver profile can be applied with NVIDIA Profile Inspector (shader cache, low latency, power management), chosen for this card."));
    }
    let hybrid = inv.gpus.iter().any(|g| g.discrete) && inv.gpus.iter().any(|g| !g.discrete);
    if hybrid && inv.machine.form == FormFactor::Laptop {
        out.push(finding("hybrid-gpu", "Graphics", Level::Note,
            "Two graphics chips (hybrid)",
            "The integrated chip runs the desktop and the card takes over in games. Per-application GPU choice and the MUX switch (if the laptop has one) matter more than global settings."));
    }
}

fn memory_findings(inv: &Inventory, out: &mut Vec<Finding>) {
    let modules = &inv.memory.modules;
    let slow = modules
        .iter()
        .any(|m| !m.soldered && m.configured_mhz > 0 && m.rated_mhz > m.configured_mhz);
    if slow {
        let m = modules.iter().find(|m| m.rated_mhz > m.configured_mhz).expect("checked above");
        out.push(finding(
            "ram-below-rated",
            "Memory",
            Level::Advice,
            format!("Memory at {} MT/s, rated {}", m.configured_mhz, m.rated_mhz),
            "XMP or EXPO is probably off. It is turned on in the BIOS (Windows cannot), and is often worth several percent in games.",
        ));
    }
    let socketed: Vec<_> = modules.iter().filter(|m| !m.soldered).collect();
    if socketed.len() == 1 && modules.len() == 1 && inv.machine.form == FormFactor::Desktop {
        out.push(finding(
            "ram-single",
            "Memory",
            Level::Advice,
            "A single memory module",
            "One module is one channel: half the memory bandwidth. A second identical module doubles it.",
        ));
    }
}

fn storage_findings(inv: &Inventory, out: &mut Vec<Finding>) {
    if inv.disks.iter().any(|d| d.system && d.media == "HDD") {
        out.push(finding(
            "system-hdd",
            "Storage",
            Level::Warning,
            "Windows is on a hard disk",
            "No setting makes up for it: an SSD is the biggest gain this machine can have.",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{Battery, Disk, Gpu, MemoryModule};

    fn ids(inv: &Inventory) -> Vec<&'static str> {
        findings(inv).iter().map(|f| f.id).collect()
    }

    fn module(kind: &str, rated: u32, configured: u32) -> MemoryModule {
        MemoryModule {
            capacity_bytes: 16 << 30,
            kind: kind.into(),
            soldered: kind.starts_with("LP"),
            rated_mhz: rated,
            configured_mhz: configured,
            manufacturer: String::new(),
            part: String::new(),
            slot: String::new(),
        }
    }

    fn gpu(vendor: GpuVendor, name: &str) -> Gpu {
        Gpu {
            name: name.into(),
            vendor,
            discrete: is_discrete_gpu(vendor, name),
            driver: String::new(),
            vram_bytes: None,
            width: None,
            height: None,
            refresh_hz: None,
        }
    }

    /// The ASUS ROG Zephyrus G16 (GU605MI) this was written on.
    fn zephyrus() -> Inventory {
        let mut inv = Inventory::default();
        inv.machine.manufacturer = "ASUSTeK COMPUTER INC.".into();
        inv.machine.form = form_factor(&[10], 2, true);
        inv.gpus = vec![
            gpu(GpuVendor::Nvidia, "NVIDIA GeForce RTX 4070 Laptop GPU"),
            gpu(GpuVendor::Intel, "Intel(R) Arc(TM) Graphics"),
        ];
        inv.memory.modules = vec![module("LPDDR5", 7467, 7467); 8];
        inv.power.plan_name = "Balanced".into();
        inv.power.battery = Some(Battery {
            charge_percent: Some(83),
            on_ac: true,
        });
        inv.tools.chocolatey = true;
        inv
    }

    #[test]
    fn form_factor_prefers_the_chassis() {
        assert_eq!(form_factor(&[10], 1, false), FormFactor::Laptop);
        assert_eq!(form_factor(&[3], 2, true), FormFactor::Desktop);
        assert_eq!(form_factor(&[], 2, false), FormFactor::Laptop);
        assert_eq!(form_factor(&[2], 0, true), FormFactor::Laptop);
        assert_eq!(form_factor(&[2], 0, false), FormFactor::Unknown);
    }

    #[test]
    fn gpu_vendor_and_kind() {
        assert_eq!(gpu_vendor("PCI\\VEN_10DE&DEV_2860"), GpuVendor::Nvidia);
        assert_eq!(gpu_vendor("pci\\ven_1002&dev_744c"), GpuVendor::Amd);
        assert_eq!(gpu_vendor("PCI\\VEN_8086&DEV_7D55"), GpuVendor::Intel);
        assert!(!is_discrete_gpu(GpuVendor::Intel, "Intel(R) Arc(TM) Graphics"));
        assert!(is_discrete_gpu(GpuVendor::Intel, "Intel(R) Arc(TM) A770 Graphics"));
        assert!(!is_discrete_gpu(GpuVendor::Amd, "AMD Radeon 780M Graphics"));
        assert!(is_discrete_gpu(GpuVendor::Amd, "AMD Radeon RX 7900 XTX"));
    }

    #[test]
    fn active_scheme_in_english_and_french() {
        let en = "Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)\r\n";
        assert_eq!(
            parse_active_scheme(en),
            Some(("381b4222-f694-41f0-9685-ff5bb260df2e".into(), "Balanced".into()))
        );
        let fr = "GUID du mode de gestion de l'alimentation : 8C5E7FDA-E8BF-4A96-9A85-A6E23A8C635C  (Performances élevées)";
        let (guid, name) = parse_active_scheme(fr).unwrap();
        assert_eq!(guid, "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c");
        assert_eq!(name, "Performances élevées");
        assert_eq!(power_plan_name(&guid), Some("High performance"));
        assert_eq!(parse_active_scheme("nothing here"), None);
    }

    #[test]
    fn asus_laptop_without_a_tool() {
        let found = ids(&zephyrus());
        assert!(found.contains(&"laptop-power"));
        assert!(found.contains(&"asus-no-tool"));
        assert!(found.contains(&"nvidia"));
        assert!(found.contains(&"hybrid-gpu"));
        assert!(!found.contains(&"ram-below-rated"), "soldered LPDDR has no XMP to turn on");
        assert!(!found.contains(&"choco-missing"));
    }

    #[test]
    fn ghelper_and_armoury_crate_together() {
        let mut inv = zephyrus();
        inv.tools.ghelper_running = true;
        inv.tools.asus_services = vec![("ArmouryCrateService".into(), "Auto".into())];
        let found = ids(&inv);
        assert_eq!(found[0], "ghelper-armoury", "the conflict comes first");
        assert!(found.contains(&"ghelper"));
        inv.tools.asus_services[0].1 = "Disabled".into();
        assert!(!ids(&inv).contains(&"ghelper-armoury"));
    }

    #[test]
    fn asus_desktop_gets_no_laptop_tools() {
        let mut inv = zephyrus();
        inv.machine.form = FormFactor::Desktop;
        let found = ids(&inv);
        assert!(!found
            .iter()
            .any(|id| ["asus-no-tool", "armoury", "ghelper", "laptop-power", "hybrid-gpu"].contains(id)));
    }

    #[test]
    fn high_performance_plan_on_a_laptop() {
        let mut inv = zephyrus();
        inv.power.plan_name = "Ultimate Performance".into();
        assert!(ids(&inv).contains(&"laptop-plan-high"));
    }

    #[test]
    fn desktop_memory_and_disk() {
        let mut inv = Inventory::default();
        inv.machine.form = FormFactor::Desktop;
        inv.power.plan_name = "Power saver".into();
        inv.memory.modules = vec![module("DDR4", 3200, 2133)];
        inv.disks = vec![Disk {
            name: "WD Blue".into(),
            media: "HDD".into(),
            bus: "SATA".into(),
            size_bytes: 1 << 40,
            system: true,
        }];
        let found = ids(&inv);
        assert_eq!(&found[..2], &["desktop-saver", "system-hdd"]);
        assert!(found.contains(&"ram-below-rated"));
        assert!(found.contains(&"ram-single"));
        assert!(found.contains(&"choco-missing"));
    }

    #[test]
    fn placeholders_are_blank() {
        assert!(is_placeholder("To Be Filled By O.E.M."));
        assert!(is_placeholder("System Product Name"));
        assert!(!is_placeholder("ROG Zephyrus G16"));
    }

    #[test]
    fn doubled_model_codes() {
        assert_eq!(clean_model("ROG Zephyrus G16 GU605MI_GU605MI"), "ROG Zephyrus G16 GU605MI");
        assert_eq!(clean_model("TUF Gaming FX507ZV4_FX507ZV4"), "TUF Gaming FX507ZV4");
        assert_eq!(clean_model("Legion 5 15ACH6H"), "Legion 5 15ACH6H");
        assert_eq!(clean_model("Some_Model"), "Some_Model");
    }
}
