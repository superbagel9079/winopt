//! The power plan winopt sets for this machine, and how it is applied and undone.
//!
//! `decide` says what suits the machine (pure, tested without the hardware). `view`
//! compares that with what is in effect now. `apply` changes it through the Windows power
//! API (powrprof, the same calls `powercfg` makes, without its localized text) and keeps
//! every previous value in a journal file, which `undo` puts back.

use std::path::Path;

use serde::{Deserialize, Serialize};
use windows::core::GUID;
use windows::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HLOCAL, WIN32_ERROR};
use windows::Win32::System::Power::{
    PowerDeleteScheme, PowerDuplicateScheme, PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerReadFriendlyName,
    PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex,
};

use crate::inventory::Inventory;
use crate::profile::{self, FormFactor};

const HIGH_PERFORMANCE: GUID = GUID::from_u128(0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c);

/// One power setting: where Windows keeps it and how its values read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Setting {
    pub label: &'static str,
    subgroup: GUID,
    guid: GUID,
    values: Values,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Values {
    Choices(&'static [(u32, &'static str)]),
    /// A delay in seconds, 0 meaning never.
    Seconds,
}

pub const PROCESSOR_BOOST: Setting = Setting {
    label: "Processor boost",
    subgroup: GUID::from_u128(0x54533251_82be_4824_96c1_47b60b740d00),
    guid: GUID::from_u128(0xbe337238_0d82_4146_a960_4f3749d470c7),
    values: Values::Choices(&[
        (0, "Disabled"),
        (1, "Enabled"),
        (2, "Aggressive"),
        (3, "Efficient enabled"),
        (4, "Efficient aggressive"),
        (5, "Aggressive at guaranteed"),
        (6, "Efficient aggressive at guaranteed"),
    ]),
};

pub const PCIE_LINK_POWER: Setting = Setting {
    label: "PCIe link power saving",
    subgroup: GUID::from_u128(0x501a4d13_42af_4429_9fd1_a8218c268e20),
    guid: GUID::from_u128(0xee12f906_d277_404b_b6da_e5fa1a576df5),
    values: Values::Choices(&[(0, "Off"), (1, "Moderate"), (2, "Maximum")]),
};

pub const USB_SUSPEND: Setting = Setting {
    label: "USB selective suspend",
    subgroup: GUID::from_u128(0x2a737441_1930_4402_8d77_b2bebba308a3),
    guid: GUID::from_u128(0x48e6b7a6_50f5_4782_a5d4_53bb8f07e226),
    values: Values::Choices(&[(0, "Disabled"), (1, "Enabled")]),
};

pub const DISK_SLEEP: Setting = Setting {
    label: "Turn off disks after",
    subgroup: GUID::from_u128(0x0012ee47_9041_4b5d_9b77_535fba8b1442),
    guid: GUID::from_u128(0x6738e2c4_e8a5_4a42_b16a_e040e769756e),
    values: Values::Seconds,
};

impl Setting {
    /// A value as Windows' power options would name it.
    pub fn describe(&self, value: u32) -> String {
        match self.values {
            Values::Choices(choices) => choices
                .iter()
                .find(|(v, _)| *v == value)
                .map(|(_, name)| (*name).to_owned())
                .unwrap_or_else(|| format!("value {value}")),
            Values::Seconds if value == 0 => "Never".to_owned(),
            Values::Seconds if value.is_multiple_of(60) => format!("{} min", value / 60),
            Values::Seconds => format!("{value} s"),
        }
    }
}

/// Plugged in (AC) or on battery (DC): Windows keeps a value for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Ac,
    Dc,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::Ac => "plugged in",
            Source::Dc => "on battery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scheme {
    /// The plan in use stays; only its values change.
    Active,
    /// Windows' High performance plan becomes the one in use.
    HighPerformance,
}

/// The settings to set: which one, plugged in or on battery, and its value.
pub type Targets = Vec<(Setting, Source, u32)>;

/// What suits the machine, before looking at the values in effect.
#[derive(Debug, PartialEq)]
pub struct Decision {
    pub title: String,
    pub reason: String,
    /// `None` when winopt leaves power alone on this machine.
    pub plan: Option<(Scheme, Targets)>,
}

/// Settings that save energy by putting parts to sleep, at the cost of a delay when they
/// wake: worth it on battery, not when plugged in.
fn plugged_in_targets() -> Targets {
    vec![
        (PROCESSOR_BOOST, Source::Ac, 2),
        (PCIE_LINK_POWER, Source::Ac, 0),
        (USB_SUSPEND, Source::Ac, 0),
        (DISK_SLEEP, Source::Ac, 0),
    ]
}

pub fn decide(inv: &Inventory) -> Decision {
    match inv.machine.form {
        FormFactor::Unknown => Decision {
            title: "Power left as it is".into(),
            reason: "winopt could not tell whether this PC is a laptop or a desktop, so it does not touch the power settings.".into(),
            plan: None,
        },
        FormFactor::Laptop => match profile::vendor_power_tool(inv) {
            Some(tool) => Decision {
                title: format!("{tool} manages power"),
                reason: format!(
                    "{tool} sets the performance mode and the power limits of this laptop. If winopt changed the Windows power plan as well, the two would undo each other, so winopt leaves it alone."
                ),
                plan: None,
            },
            None => Decision {
                title: "Faster when plugged in".into(),
                reason: "This laptop keeps its plan. winopt changes only the values used while it is plugged in: the processor boosts sooner, and the PCIe links, USB ports and disks stay awake instead of sleeping and taking a moment to wake up. The battery values do not change.".into(),
                plan: Some((Scheme::Active, plugged_in_targets())),
            },
        },
        FormFactor::Desktop => Decision {
            title: "High performance plan".into(),
            reason: "A desktop has no battery to save. winopt switches to the High performance plan, lets the processor boost aggressively, and keeps the PCIe links, USB ports and disks awake instead of sleeping and taking a moment to wake up.".into(),
            plan: Some((Scheme::HighPerformance, plugged_in_targets())),
        },
    }
}

// --- what the page shows -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeView {
    pub setting: &'static str,
    pub when: &'static str,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerView {
    pub title: String,
    pub reason: String,
    /// Whether winopt sets power on this machine at all (false: a vendor tool does it).
    pub actionable: bool,
    pub plan_now: String,
    /// The plan in use after Apply, when it is not the one in use now.
    pub plan_after: Option<String>,
    pub changes: Vec<ChangeView>,
    /// When winopt applied its changes (seconds since 1970), if they are still in place.
    pub applied_at: Option<u64>,
    /// What went wrong while reading the values, if anything.
    pub error: Option<String>,
}

/// The decision for this machine, set against the values in effect now.
pub fn view(inv: &Inventory, journal: &Path) -> PowerView {
    let decision = decide(inv);
    let applied_at = Journal::load(journal).ok().flatten().map(|j| j.applied_at);
    let mut out = PowerView {
        title: decision.title,
        reason: decision.reason,
        actionable: decision.plan.is_some(),
        plan_now: String::new(),
        plan_after: None,
        changes: Vec::new(),
        applied_at,
        error: None,
    };
    let active = match active_scheme() {
        Ok(guid) => guid,
        Err(e) => {
            out.error = Some(e);
            return out;
        }
    };
    out.plan_now = plan_name(&active);
    let Some((scheme, targets)) = decision.plan else { return out };
    if scheme == Scheme::HighPerformance && active != HIGH_PERFORMANCE {
        out.plan_after = Some("High performance".into());
    }
    for (setting, source, target) in targets {
        // A setting this PC does not have (no PCIe power management, say) is skipped.
        let Ok(current) = read_value(&active, &setting, source) else {
            continue;
        };
        if current != target {
            out.changes.push(ChangeView {
                setting: setting.label,
                when: source.label(),
                before: setting.describe(current),
                after: setting.describe(target),
            });
        }
    }
    out
}

// --- applying and undoing -------------------------------------------------------------------

/// Everything `apply` changed, as it was before: what `undo` needs.
#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    applied_at: u64,
    scheme_before: String,
    scheme_used: String,
    /// A plan winopt created (High performance missing from this Windows), deleted by undo.
    created: Option<String>,
    values: Vec<SavedValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SavedValue {
    subgroup: String,
    setting: String,
    source: Source,
    before: u32,
}

impl Journal {
    fn load(path: &Path) -> Result<Option<Journal>, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| format!("the undo journal {} is unreadable: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("reading the undo journal {}: {e}", path.display())),
        }
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("writing the undo journal {}: {e}", path.display()))
    }
}

/// Applies the decision for this machine. Values already changed by an earlier Apply keep
/// their first "before", so Undo always returns to the state winopt found.
pub fn apply(inv: &Inventory, journal_path: &Path) -> Result<(), String> {
    let Some((scheme, targets)) = decide(inv).plan else {
        return Err("winopt leaves power alone on this machine".into());
    };
    let active = active_scheme()?;
    let mut journal = Journal::load(journal_path)?.unwrap_or(Journal {
        applied_at: 0,
        scheme_before: guid_text(&active),
        scheme_used: guid_text(&active),
        created: None,
        values: Vec::new(),
    });
    let target = match scheme {
        Scheme::Active => active,
        Scheme::HighPerformance if scheme_exists(&HIGH_PERFORMANCE) => HIGH_PERFORMANCE,
        Scheme::HighPerformance => {
            // Windows on Modern Standby machines ships without it; a copy of its template works.
            let copy = duplicate_scheme(&HIGH_PERFORMANCE)?;
            journal.created = Some(guid_text(&copy));
            copy
        }
    };
    journal.scheme_used = guid_text(&target);

    for (setting, source, value) in targets {
        let Ok(before) = read_value(&target, &setting, source) else {
            continue;
        };
        if before == value {
            continue;
        }
        if let Err(e) = write_value(&target, &setting, source, value) {
            // Leave nothing half done: put back what this Apply already changed.
            let _ = restore(&journal);
            return Err(e);
        }
        let saved = SavedValue {
            subgroup: guid_text(&setting.subgroup),
            setting: guid_text(&setting.guid),
            source,
            before,
        };
        if !journal
            .values
            .iter()
            .any(|v| v.subgroup == saved.subgroup && v.setting == saved.setting && v.source == source)
        {
            journal.values.push(saved);
        }
    }
    // Setting the plan again makes Windows take the new values into account.
    set_active_scheme(&target)?;
    journal.applied_at = now();
    journal.save(journal_path)
}

/// Puts back everything the journal recorded, then forgets it.
pub fn undo(journal_path: &Path) -> Result<(), String> {
    let Some(journal) = Journal::load(journal_path)? else {
        return Err("there is nothing to undo".into());
    };
    restore(&journal)?;
    std::fs::remove_file(journal_path).map_err(|e| format!("removing the undo journal {}: {e}", journal_path.display()))
}

fn restore(journal: &Journal) -> Result<(), String> {
    let used = parse_guid(&journal.scheme_used)?;
    let mut first_error = None;
    for v in &journal.values {
        let result = (|| {
            let setting = Setting {
                label: "",
                subgroup: parse_guid(&v.subgroup)?,
                guid: parse_guid(&v.setting)?,
                values: Values::Seconds,
            };
            write_value(&used, &setting, v.source, v.before)
        })();
        if let Err(e) = result {
            first_error.get_or_insert(e);
        }
    }
    set_active_scheme(&parse_guid(&journal.scheme_before)?)?;
    if let Some(created) = &journal.created {
        delete_scheme(&parse_guid(created)?)?;
    }
    first_error.map_or(Ok(()), Err)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn guid_text(guid: &GUID) -> String {
    format!("{:032x}", guid.to_u128())
}

fn parse_guid(text: &str) -> Result<GUID, String> {
    u128::from_str_radix(text, 16)
        .map(GUID::from_u128)
        .map_err(|e| format!("bad GUID {text:?} in the undo journal: {e}"))
}

// --- the Windows power API ------------------------------------------------------------------

fn check(code: WIN32_ERROR, what: &str) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        return Ok(());
    }
    let reason = std::io::Error::from_raw_os_error(code.0 as i32);
    Err(format!("{what}: {reason}"))
}

/// A GUID that powrprof allocated, copied out and freed.
fn take_guid(ptr: *mut GUID) -> GUID {
    // SAFETY: powrprof returned a valid, LocalAlloc'ed GUID; it is read once, then freed.
    let guid = unsafe { *ptr };
    unsafe { LocalFree(Some(HLOCAL(ptr.cast()))) };
    guid
}

fn active_scheme() -> Result<GUID, String> {
    let mut ptr: *mut GUID = std::ptr::null_mut();
    check(unsafe { PowerGetActiveScheme(None, &mut ptr) }, "reading the active power plan")?;
    Ok(take_guid(ptr))
}

fn set_active_scheme(scheme: &GUID) -> Result<(), String> {
    check(unsafe { PowerSetActiveScheme(None, Some(scheme)) }, "making the power plan active")
}

fn scheme_exists(scheme: &GUID) -> bool {
    let mut size = 0u32;
    let code = unsafe { PowerReadFriendlyName(None, Some(scheme), None, None, None, &mut size) };
    code == ERROR_SUCCESS
}

fn duplicate_scheme(source: &GUID) -> Result<GUID, String> {
    let mut ptr: *mut GUID = std::ptr::null_mut();
    check(
        unsafe { PowerDuplicateScheme(None, source, &mut ptr) },
        "creating the High performance plan",
    )?;
    Ok(take_guid(ptr))
}

fn delete_scheme(scheme: &GUID) -> Result<(), String> {
    check(unsafe { PowerDeleteScheme(None, scheme) }, "deleting the plan winopt created")
}

/// The plan's name as Windows shows it, in the language of Windows.
fn plan_name(scheme: &GUID) -> String {
    let mut size = 0u32;
    if unsafe { PowerReadFriendlyName(None, Some(scheme), None, None, None, &mut size) } != ERROR_SUCCESS || size < 2 {
        return guid_text(scheme);
    }
    let mut buffer = vec![0u8; size as usize];
    if unsafe { PowerReadFriendlyName(None, Some(scheme), None, None, Some(buffer.as_mut_ptr()), &mut size) } != ERROR_SUCCESS {
        return guid_text(scheme);
    }
    let wide: Vec<u16> = buffer
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[lo, hi]| u16::from_le_bytes([lo, hi]))
        .take_while(|&c| c != 0)
        .collect();
    String::from_utf16_lossy(&wide)
}

fn read_value(scheme: &GUID, setting: &Setting, source: Source) -> Result<u32, String> {
    let mut value = 0u32;
    let code = match source {
        Source::Ac => unsafe { PowerReadACValueIndex(None, Some(scheme), Some(&setting.subgroup), Some(&setting.guid), &mut value) },
        Source::Dc => {
            WIN32_ERROR(unsafe { PowerReadDCValueIndex(None, Some(scheme), Some(&setting.subgroup), Some(&setting.guid), &mut value) })
        }
    };
    check(code, &format!("reading {} ({})", setting.label, source.label()))?;
    Ok(value)
}

fn write_value(scheme: &GUID, setting: &Setting, source: Source, value: u32) -> Result<(), String> {
    let code = match source {
        Source::Ac => unsafe { PowerWriteACValueIndex(None, scheme, Some(&setting.subgroup), Some(&setting.guid), value) },
        Source::Dc => WIN32_ERROR(unsafe { PowerWriteDCValueIndex(None, scheme, Some(&setting.subgroup), Some(&setting.guid), value) }),
    };
    check(code, &format!("changing {} ({})", setting.label, source.label()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::form_factor;

    fn machine(form: FormFactor) -> Inventory {
        let mut inv = Inventory::default();
        inv.machine.form = form;
        inv
    }

    #[test]
    fn desktop_gets_high_performance_and_awake_devices() {
        let d = decide(&machine(FormFactor::Desktop));
        let (scheme, targets) = d.plan.expect("a desktop gets a plan");
        assert_eq!(scheme, Scheme::HighPerformance);
        assert!(targets.iter().all(|(_, source, _)| *source == Source::Ac));
        assert!(targets.contains(&(PCIE_LINK_POWER, Source::Ac, 0)));
        assert!(targets.contains(&(DISK_SLEEP, Source::Ac, 0)));
    }

    #[test]
    fn laptop_keeps_its_plan_and_its_battery_values() {
        let d = decide(&machine(FormFactor::Laptop));
        let (scheme, targets) = d.plan.expect("a laptop with no vendor tool gets a plan");
        assert_eq!(scheme, Scheme::Active);
        assert!(
            targets.iter().all(|(_, source, _)| *source == Source::Ac),
            "battery values are never touched"
        );
    }

    #[test]
    fn laptop_with_a_vendor_tool_is_left_alone() {
        let mut inv = machine(form_factor(&[10], 2, true));
        inv.machine.manufacturer = "ASUSTeK COMPUTER INC.".into();
        inv.tools.ghelper_running = true;
        let d = decide(&inv);
        assert!(d.plan.is_none());
        assert_eq!(d.title, "G-Helper manages power");

        inv.tools.ghelper_running = false;
        inv.tools.asus_services = vec![("ArmouryCrateService".into(), "Auto".into())];
        assert_eq!(decide(&inv).title, "Armoury Crate manages power");

        inv.tools.asus_services[0].1 = "Disabled".into();
        assert!(decide(&inv).plan.is_some(), "a disabled Armoury Crate manages nothing");
    }

    #[test]
    fn unknown_machine_is_left_alone() {
        assert!(decide(&machine(FormFactor::Unknown)).plan.is_none());
    }

    #[test]
    fn values_read_as_windows_names_them() {
        assert_eq!(PROCESSOR_BOOST.describe(2), "Aggressive");
        assert_eq!(PROCESSOR_BOOST.describe(42), "value 42");
        assert_eq!(DISK_SLEEP.describe(0), "Never");
        assert_eq!(DISK_SLEEP.describe(1200), "20 min");
        assert_eq!(DISK_SLEEP.describe(90), "90 s");
    }

    #[test]
    fn guids_survive_the_journal() {
        let text = guid_text(&HIGH_PERFORMANCE);
        assert_eq!(text, "8c5e7fdae8bf4a969a85a6e23a8c635c");
        assert_eq!(parse_guid(&text).unwrap(), HIGH_PERFORMANCE);
        assert!(parse_guid("not a guid").is_err());
    }

    /// The whole cycle as on a desktop: Apply makes High performance active (creating it
    /// if missing) and changes values, Undo puts everything back. It changes the active plan
    /// for a moment: `cargo test -- --ignored apply_then_undo`.
    #[test]
    #[ignore = "switches this machine's active power plan for a moment"]
    fn apply_then_undo_restores_everything() {
        let journal = std::env::temp_dir().join(format!("winopt-test-undo-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&journal);
        let active = active_scheme().unwrap();
        let before: Vec<_> = plugged_in_targets()
            .iter()
            .map(|(s, src, _)| read_value(&active, s, *src).ok())
            .collect();

        apply(&machine(FormFactor::Desktop), &journal).unwrap();
        let used = active_scheme().unwrap();
        let saved = Journal::load(&journal).unwrap().expect("apply writes its journal");
        assert_eq!(guid_text(&used), saved.scheme_used, "the plan Apply set up is the one in use");
        for (setting, source, value) in plugged_in_targets() {
            if let Ok(now) = read_value(&used, &setting, source) {
                assert_eq!(now, value, "{} after Apply", setting.label);
            }
        }

        undo(&journal).unwrap();
        assert_eq!(active_scheme().unwrap(), active, "the previous plan is in use again");
        let after: Vec<_> = plugged_in_targets()
            .iter()
            .map(|(s, src, _)| read_value(&active, s, *src).ok())
            .collect();
        assert_eq!(after, before, "the previous plan's values are untouched");
        if let Some(created) = saved.created {
            assert!(!scheme_exists(&parse_guid(&created).unwrap()), "the plan winopt created is gone");
        }
        assert!(!journal.exists(), "undo removes its journal");
    }

    /// Writes and restores values on a throwaway copy of the active plan, which is never
    /// made active and is deleted at the end: `cargo test -- --ignored power_api`.
    #[test]
    #[ignore = "changes power plans on this machine (a scratch copy only)"]
    fn power_api_round_trip_on_a_scratch_plan() {
        let active = active_scheme().unwrap();
        let scratch = duplicate_scheme(&active).unwrap();
        let result = std::panic::catch_unwind(|| {
            for setting in [PROCESSOR_BOOST, USB_SUSPEND, DISK_SLEEP] {
                let before = read_value(&scratch, &setting, Source::Ac).unwrap();
                let other = if before == 0 { 1 } else { 0 };
                write_value(&scratch, &setting, Source::Ac, other).unwrap();
                assert_eq!(read_value(&scratch, &setting, Source::Ac).unwrap(), other, "{}", setting.label);
                write_value(&scratch, &setting, Source::Ac, before).unwrap();
                assert_eq!(read_value(&scratch, &setting, Source::Ac).unwrap(), before);
            }
            assert!(!plan_name(&scratch).is_empty());
        });
        delete_scheme(&scratch).unwrap();
        assert_eq!(active_scheme().unwrap(), active, "the active plan never changed");
        result.unwrap();
    }
}
