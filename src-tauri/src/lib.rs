mod desktop;
mod storage;

use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use storage::{PersonalProfile, Storage};
use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};

struct AppState {
    storage: Mutex<Option<Storage>>,
    database_error: Option<&'static str>,
    exit_requested: AtomicBool,
}

#[derive(Serialize)]
struct AppStatus {
    database_ready: bool,
    error_code: Option<&'static str>,
}

#[tauri::command]
fn get_app_status(state: State<'_, AppState>) -> AppStatus {
    AppStatus {
        database_ready: state.storage.lock().map(|s| s.is_some()).unwrap_or(false),
        error_code: state.database_error,
    }
}

#[tauri::command]
fn get_profile(state: State<'_, AppState>) -> Result<PersonalProfile, &'static str> {
    let storage = state.storage.lock().map_err(|_| "DB_UNAVAILABLE")?;
    storage
        .as_ref()
        .ok_or(state.database_error.unwrap_or("DB_UNAVAILABLE"))?
        .profile()
        .map_err(|error| error.code())
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), &'static str> {
    desktop::show_window(&app, "settings", true)
}

#[tauri::command]
fn open_panel(app: AppHandle) -> Result<(), &'static str> {
    desktop::show_window(&app, "panel", true)
}

#[tauri::command]
fn show_pet(app: AppHandle) -> Result<(), &'static str> {
    desktop::show_window(&app, "pet", false)
}

#[tauri::command]
fn hide_pet(app: AppHandle) -> Result<(), &'static str> {
    desktop::hide_window(&app, "pet")
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    request_exit(&app);
}

pub(crate) fn request_exit(app: &AppHandle) {
    app.state::<AppState>()
        .exit_requested
        .store(true, Ordering::SeqCst);
    app.exit(0);
}

pub fn run() {
    let app = tauri::Builder::default()
        // The lock is acquired before setup/migrations; the second process never opens SQLite.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Err(code) = desktop::show_window(app, "panel", true) {
                eprintln!("error_code={code}");
            }
        }))
        .invoke_handler(tauri::generate_handler![
            get_profile,
            get_app_status,
            open_settings,
            open_panel,
            show_pet,
            hide_pet,
            quit_app
        ])
        .setup(|app| {
            let database = app
                .path()
                .app_data_dir()
                .map_err(|_| "DB_IO_FAILED")
                .and_then(|directory| {
                    Storage::open(&directory.join("marketing-mate.sqlite3"))
                        .map_err(|error| error.code())
                });
            let (storage, database_error) = match database {
                Ok(storage) => (Some(storage), None),
                Err(code) => {
                    eprintln!("error_code={code}");
                    (None, Some(code))
                }
            };
            app.manage(AppState {
                storage: Mutex::new(storage),
                database_error,
                exit_requested: AtomicBool::new(false),
            });
            desktop::initialize(app.handle())?;
            if database_error.is_some() {
                desktop::show_window(app.handle(), "settings", true)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Closing a window hides it; explicit tray/panel Quit ends the process.
                api.prevent_close();
                if window.hide().is_err() {
                    eprintln!("error_code=WINDOW_UNAVAILABLE");
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("MARKETING MATE desktop initialization failed");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            if !app
                .state::<AppState>()
                .exit_requested
                .load(Ordering::SeqCst)
            {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            if let Ok(mut database) = app.state::<AppState>().storage.lock() {
                if let Some(database) = database.take() {
                    if let Err(error) = database.close() {
                        eprintln!("error_code={}", error.code());
                    }
                }
            }
        }
        _ => {}
    });
}
