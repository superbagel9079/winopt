mod inventory;
mod power;
mod profile;

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    inventory: inventory::Inventory,
    findings: Vec<profile::Finding>,
    power: power::PowerView,
}

/// What the commands share: the last reading of the machine, and where the undo journal of
/// the power changes is kept.
struct AppState {
    inventory: Mutex<Option<inventory::Inventory>>,
    power_journal: PathBuf,
}

/// Runs work that blocks (WMI, the power API) off the async runtime. WMI wants a thread of
/// its own anyway: its connection cannot move between threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("the task stopped: {e}"))?
}

/// The last inventory, or a fresh one when the machine has not been read yet.
fn current_inventory(state: &AppState) -> inventory::Inventory {
    let cached = state.inventory.lock().expect("inventory lock poisoned").clone();
    cached.unwrap_or_else(inventory::collect)
}

/// Reads the machine and what follows from it.
#[tauri::command]
async fn scan(app: tauri::AppHandle) -> Result<Report, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let inventory = inventory::collect();
        let findings = profile::findings(&inventory);
        let power = power::view(&inventory, &state.power_journal);
        *state.inventory.lock().expect("inventory lock poisoned") = Some(inventory.clone());
        Ok(Report {
            inventory,
            findings,
            power,
        })
    })
    .await
}

/// Applies the power plan that suits this machine, keeping what it changes for Undo.
#[tauri::command]
async fn power_apply(app: tauri::AppHandle) -> Result<power::PowerView, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let inventory = current_inventory(&state);
        power::apply(&inventory, &state.power_journal)?;
        Ok(power::view(&inventory, &state.power_journal))
    })
    .await
}

/// Puts back the power values winopt changed.
#[tauri::command]
async fn power_undo(app: tauri::AppHandle) -> Result<power::PowerView, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        power::undo(&state.power_journal)?;
        Ok(power::view(&current_inventory(&state), &state.power_journal))
    })
    .await
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            app.manage(AppState {
                inventory: Mutex::new(None),
                power_journal: data.join("undo-power.json"),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![scan, power_apply, power_undo])
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
        let journal = std::env::temp_dir().join("winopt-test-no-journal.json");
        let power = super::power::view(&inventory, &journal);
        assert!(inventory.warnings.is_empty(), "parts not read: {:?}", inventory.warnings);
        assert!(!inventory.cpu.name.is_empty());
        assert!(power.error.is_none(), "power not read: {:?}", power.error);
        println!(
            "{}",
            serde_json::to_string_pretty(&super::Report {
                inventory,
                findings,
                power
            })
            .unwrap()
        );
    }
}
