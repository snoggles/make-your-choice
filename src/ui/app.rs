slint::include_modules!();

use crate::logic::region::{get_regions, get_group_name};
use crate::logic::hosts::HostsManager;
use crate::logic::settings::UserSettings;
use crate::logic::ping::ping_host;
use crate::logic::utils::{load_versinf, fetch_git_identity};
use crate::ui::dark_mode::{detect_theme, Theme};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use slint::{ModelRc, VecModel, Color, Model, Timer, TimerMode};
use futures::stream::{FuturesUnordered, StreamExt};

pub async fn run() -> anyhow::Result<()> {
    std::env::set_var("SLINT_STYLE", "qt");
    let main_window = MainWindow::new()?;
    
    // Load settings
    let _settings = UserSettings::load().unwrap_or_default();
    let regions = Arc::new(get_regions());
    let hosts_manager = Arc::new(HostsManager::new("https://discord.gg/xEMyAA8gn8".to_string()));

    // Set theme
    let theme = detect_theme();
    main_window.set_is_dark(matches!(theme, Theme::Dark));

    // Version and Identity
    let (version, _notes) = load_versinf();
    main_window.set_version(version.clone().into());
    
    let dev_main = main_window.as_weak();
    slint::spawn_local(async move {
        if let Some(login) = fetch_git_identity().await {
            if let Some(main) = dev_main.upgrade() {
                main.set_developer(login.into());
            }
        }
    }).unwrap();

    // Prepare server list
    let server_entries = Rc::new(VecModel::<ServerEntry>::default());
    let mut name_to_index = std::collections::HashMap::new();
    
    // Group regions by category
    let regions_list = regions.clone();
    let group_order = vec![
        ("Europe", "Europe"),
        ("Americas", "The Americas"),
        ("Asia", "Asia (Excl. Cn)"),
        ("Oceania", "Oceania"),
        ("China", "Mainland China"),
    ];

    for (group_key, group_label) in group_order {
        // Add header
        server_entries.push(ServerEntry {
            name: group_label.into(),
            latency: "".into(),
            latency_color: Color::from_rgb_u8(0, 0, 0),
            checked: false,
            is_group: true,
            is_stable: false,
        });

        // Add regions in group
        for (name, info) in regions_list.as_ref() {
            if get_group_name(name) == group_key {
                let index = server_entries.row_count();
                name_to_index.insert(name.clone(), index);
                server_entries.push(ServerEntry {
                    name: name.clone().into(),
                    latency: "…".into(),
                    latency_color: Color::from_rgb_u8(128, 128, 128),
                    checked: false,
                    is_group: false,
                    is_stable: info.stable,
                });
            }
        }
    }

    let name_to_index = Arc::new(name_to_index);

    main_window.set_servers(ModelRc::from(server_entries.clone()));

    // Callbacks
    let server_entries_clone = server_entries.clone();
    main_window.on_toggle_server(move |index| {
        if let Some(mut entry) = server_entries_clone.row_data(index as usize) {
            entry.checked = !entry.checked;
            server_entries_clone.set_row_data(index as usize, entry);
        }
    });

    let server_entries_apply = server_entries.clone();
    let hosts_manager_apply = hosts_manager.clone();
    let regions_apply = regions.clone();
    
    main_window.on_apply_clicked(move || {
        let mut selected = HashSet::new();
        for i in 0..server_entries_apply.row_count() {
            let entry = server_entries_apply.row_data(i).unwrap();
            if !entry.is_group && entry.checked {
                selected.insert(entry.name.to_string());
            }
        }

        if selected.is_empty() {
            // In a real app we'd show a dialog, for now just print
            println!("No servers selected");
            return;
        }

        let result = hosts_manager_apply.apply_gatekeep(
            &regions_apply,
            &selected,
            crate::logic::region::BlockMode::Both,
            true
        );

        match result {
            Ok(_) => println!("Successfully applied"),
            Err(e) => eprintln!("Error applying: {}", e),
        }
    });

    main_window.on_revert_clicked(move || {
        let _ = hosts_manager.revert();
    });

    main_window.on_repo_clicked(|| {
        let _ = open::that("https://github.com/laewliet/make-your-choice");
    });

    main_window.on_discord_clicked(|| {
        let _ = open::that("https://discord.gg/xEMyAA8gn8");
    });

    main_window.on_check_updates_clicked(|| {
        let _ = open::that("https://github.com/laewliet/make-your-choice/releases");
    });

    // Ping update loop
    let (tx, rx) = std::sync::mpsc::channel::<(String, i64)>();

    // Background ping loop
    let regions_bg = regions.clone();
    tokio::spawn(async move {
        loop {
            let mut tasks = FuturesUnordered::new();
            for (name, info) in regions_bg.as_ref() {
                let name: String = name.clone();
                let host = info.hosts[0].clone();
                tasks.push(tokio::spawn(async move {
                    (name, ping_host(&host).await)
                }));
            }

            while let Some(res) = tasks.next().await {
                if let Ok(data) = res {
                    let _ = tx.send(data);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });

    // UI update timer (Throttled polling)
    let server_entries_ui = server_entries.clone();
    let name_to_index_ui = name_to_index.clone();
    let update_timer = Timer::default();
    update_timer.start(TimerMode::Repeated, std::time::Duration::from_millis(500), move || {
        // Drain all pending updates from the channel
        while let Ok((name, latency)) = rx.try_recv() {
            if let Some(&index) = name_to_index_ui.get(&name) {
                if let Some(mut entry) = server_entries_ui.row_data(index) {
                    if latency >= 0 {
                        entry.latency = format!("{} ms", latency).into();
                        entry.latency_color = match latency {
                            l if l < 80 => Color::from_rgb_u8(0, 128, 0),
                            l if l < 130 => Color::from_rgb_u8(255, 165, 0),
                            l if l < 250 => Color::from_rgb_u8(220, 20, 60),
                            _ => Color::from_rgb_u8(128, 0, 128),
                        };
                    } else {
                        entry.latency = "disconnected".into();
                        entry.latency_color = Color::from_rgb_u8(128, 128, 128);
                    }
                    server_entries_ui.set_row_data(index, entry);
                }
            }
        }
    });

    main_window.run()?;
    Ok(())
}
