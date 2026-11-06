mod inventory;
mod profile;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    inventory: inventory::Inventory,
    findings: Vec<profile::Finding>,
}

/// Reads the machine and what follows from it. WMI wants a thread of its own (its
/// connection cannot move between threads), so the reading runs on a blocking one.
#[tauri::command]
async fn scan() -> Result<Report, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let inventory = inventory::collect();
        let findings = profile::findings(&inventory);
        Report { inventory, findings }
    })
    .await
    .map_err(|e| format!("the scan stopped: {e}"))
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![scan])
        .run(tauri::generate_context!())
        .expect("winopt could not start its window");
}

#[cfg(test)]
mod tests {
    /// Reads this machine for real and prints the report: `cargo test -- --ignored --nocapture`.
    /// Its output is what ui/mock.json holds, to see the page in a plain browser.
    #[test]
    #[ignore = "reads the real machine"]
    fn scan_this_machine() {
        let inventory = super::inventory::collect();
        let findings = super::profile::findings(&inventory);
        assert!(inventory.warnings.is_empty(), "parts not read: {:?}", inventory.warnings);
        assert!(!inventory.cpu.name.is_empty());
        println!("{}", serde_json::to_string_pretty(&super::Report { inventory, findings }).unwrap());
    }
}
