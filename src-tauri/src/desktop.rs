use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, LogicalPosition, Manager,
};

pub fn show_window(app: &AppHandle, label: &str, focus: bool) -> Result<(), &'static str> {
    let window = app.get_webview_window(label).ok_or("WINDOW_UNAVAILABLE")?;
    window.show().map_err(|_| "WINDOW_UNAVAILABLE")?;
    window.unminimize().map_err(|_| "WINDOW_UNAVAILABLE")?;
    if focus {
        window.set_focus().map_err(|_| "WINDOW_UNAVAILABLE")?;
    }
    Ok(())
}

pub fn hide_window(app: &AppHandle, label: &str) -> Result<(), &'static str> {
    app.get_webview_window(label)
        .ok_or("WINDOW_UNAVAILABLE")?
        .hide()
        .map_err(|_| "WINDOW_UNAVAILABLE")
}

pub fn initialize(app: &AppHandle) -> tauri::Result<()> {
    let pet = app
        .get_webview_window("pet")
        .expect("configured pet window");
    // Showing the companion must not interrupt the user's keyboard focus.
    pet.set_focusable(false)?;
    if let Some(monitor) = pet.primary_monitor()? {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let left = f64::from(area.position.x) / scale;
        let top = f64::from(area.position.y) / scale;
        let width = f64::from(area.size.width) / scale;
        let height = f64::from(area.size.height) / scale;
        pet.set_position(LogicalPosition::new(
            left + (width - 192.0 - 24.0).max(0.0),
            top + (height - 192.0 - 24.0).max(0.0),
        ))?;
    }

    let panel = MenuItem::with_id(app, "panel", "패널 열기", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "설정", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "캐릭터 표시", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "캐릭터 숨기기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&panel, &settings, &show, &hide, &quit])?;
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 4..28 {
        for x in 4..28 {
            let offset = (y * 32 + x) * 4;
            let is_mark = (8..11).contains(&x) || (21..24).contains(&x);
            let color = if is_mark {
                [244, 243, 236, 255]
            } else {
                [58, 91, 73, 255]
            };
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
    TrayIconBuilder::with_id("marketing-mate-tray")
        .icon(Image::new_owned(pixels, 32, 32))
        .tooltip("MARKETING MATE · Phase 1")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let result = match event.id.as_ref() {
                "panel" => show_window(app, "panel", true),
                "settings" => show_window(app, "settings", true),
                "show" => show_window(app, "pet", false),
                "hide" => hide_window(app, "pet"),
                "quit" => {
                    crate::request_exit(app);
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(code) = result {
                eprintln!("error_code={code}");
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Err(code) = show_window(tray.app_handle(), "panel", true) {
                    eprintln!("error_code={code}");
                }
            }
        })
        .build(app)?;
    pet.show()?;
    Ok(())
}
