use crate::controllers::{BluetoothScanState, CONTROLLER_MENU_OPTIONS, MAX_PLAYERS};
use crate::ipc::{OverlayScreen, ToastStyle};
use crate::state::OverlayState;
use macroquad::prelude::*;

/// Helper to get theme from state
fn theme(state: &OverlayState) -> &crate::themes::Theme {
    state.theme_config.theme()
}

pub async fn render(state: &OverlayState) {
    clear_background(BLANK);

    // Render overlay menu if visible
    if state.visible {
        render_overlay_menu(state);
    }

    // Always render toasts (even when overlay is hidden)
    render_toasts(state);

    // Render performance HUD if enabled
    if state.performance.is_visible() {
        render_performance_hud(state);
    }

    next_frame().await;
}

fn render_overlay_menu(state: &OverlayState) {
    let t = theme(state);
    // Semi-transparent background overlay
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        t.background_overlay,
    );

    match state.current_screen {
        OverlayScreen::Main => render_main_menu(state),
        OverlayScreen::Settings => render_settings_screen(state),
        OverlayScreen::Achievements => render_achievements_screen(state),
        OverlayScreen::Performance => render_performance(state),
        OverlayScreen::Playtime => render_playtime(state),
        OverlayScreen::Controllers => render_controllers_menu(state),
        OverlayScreen::BluetoothPairing => render_bluetooth_screen(state),
        OverlayScreen::ControllerAssign => render_assign_screen(state),
        OverlayScreen::GamepadTester => render_gamepad_tester(state),
        OverlayScreen::HotkeySettings => render_hotkey_settings(state),
        OverlayScreen::MenuCustomization => render_menu_customization(state),
        OverlayScreen::ThemeSelection => render_theme_selection(state),
        OverlayScreen::QuitConfirm => render_quit_confirm(state),
    }
}

fn render_main_menu(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 600.0;
    let menu_height = 420.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    // Menu background
    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 3.0, t.panel_border);

    // Title
    let title = "KAZETA OVERLAY";
    let title_size = 40;
    let title_dims = measure_text(title, None, title_size, 1.0);
    draw_text(
        title,
        menu_x + (menu_width - title_dims.width) / 2.0,
        menu_y + 60.0,
        title_size as f32,
        t.cursor,
    );

    // Menu options from config
    let visible_items = state.menu_config.config().get_visible_items();
    let option_start_y = menu_y + 120.0;
    let option_height = 50.0;
    const MAX_VISIBLE: usize = 6;

    // Use scroll offset from state
    let scroll_offset = state.main_menu_scroll_offset;
    let visible_count = visible_items.len();

    for i in 0..MAX_VISIBLE.min(visible_count.saturating_sub(scroll_offset)) {
        let item_idx = scroll_offset + i;
        if item_idx >= visible_count {
            break;
        }
        let menu_item_id = visible_items[item_idx];
        let y = option_start_y + (i as f32 * option_height);
        let option = menu_item_id.display_name();
        
        // Quit option gets special red coloring
        let is_quit = menu_item_id == crate::menu_config::MenuItemId::Quit;
        let is_selected = item_idx == state.selected_option;
        let color = if is_selected {
            if is_quit { t.error } else { t.cursor }
        } else {
            if is_quit { Color::new(t.error.r * 0.7, t.error.g * 0.3, t.error.b * 0.3, 1.0) } else { t.text }
        };

        // Selection indicator
        if is_selected {
            draw_text("►", menu_x + 40.0, y + 30.0, 30.0, if is_quit { t.error } else { t.cursor });
        }

        draw_text(option, menu_x + 80.0, y + 30.0, 28.0, color);
    }

    // Scroll indicators
    if scroll_offset > 0 {
        draw_text("▲", menu_x + menu_width - 25.0, option_start_y + 10.0, 16.0, t.text_secondary);
    }
    if scroll_offset + MAX_VISIBLE < visible_count {
        draw_text("▼", menu_x + menu_width - 25.0, option_start_y + (MAX_VISIBLE as f32 * option_height) - 15.0, 16.0, t.text_secondary);
    }

    // Playtime display (if game is active)
    if let Some(session) = &state.playtime.current_session {
        let elapsed = session.start_time.elapsed();
        let hours = elapsed.as_secs() / 3600;
        let mins = (elapsed.as_secs() % 3600) / 60;

        let session_text = if hours > 0 {
            format!("Session: {}h {}m", hours, mins)
        } else {
            format!("Session: {}m", mins)
        };

        draw_text(&session_text, menu_x + 20.0, menu_y + menu_height - 60.0, 18.0, LIGHTGRAY);

        // Show total playtime for this game
        if let Some(entry) = state.playtime.database.entries.get(&session.cart_id) {
            let total_hours = entry.total_seconds / 3600;
            let total_mins = (entry.total_seconds % 3600) / 60;
            let total_text = if total_hours > 0 {
                format!("Total: {}h {}m • {} plays", total_hours, total_mins, entry.play_count)
            } else {
                format!("Total: {}m • {} plays", total_mins, entry.play_count)
            };
            draw_text(&total_text, menu_x + 240.0, menu_y + menu_height - 60.0, 18.0, GRAY);
        }
    }

    // Controls hint
    draw_text(
        "Use D-Pad to navigate • A to select • Guide button to close",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        18.0,
        LIGHTGRAY,
    );
}

fn render_settings_screen(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 700.0;
    let menu_height = 500.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 3.0, t.panel_border);

    draw_text(
        "SETTINGS",
        menu_x + 20.0,
        menu_y + 40.0,
        36.0,
        t.cursor,
    );

    let options = ["Menu Customization", "Theme Selection"];
    let option_start_y = menu_y + 100.0;
    let option_height = 50.0;
    const MAX_VISIBLE: usize = 6;

    // Use scroll offset from state
    let scroll_offset = state.settings_scroll_offset;
    let visible_count = options.len();

    for i in 0..MAX_VISIBLE.min(visible_count.saturating_sub(scroll_offset)) {
        let item_idx = scroll_offset + i;
        if item_idx >= visible_count {
            break;
        }
        let option = options[item_idx];
        let y = option_start_y + (i as f32 * option_height);
        let is_selected = item_idx == state.settings_selected_option;
        let color = if is_selected { t.cursor } else { t.text };

        // Selection indicator
        if is_selected {
            draw_text("►", menu_x + 40.0, y + 30.0, 30.0, t.cursor);
        }

        draw_text(option, menu_x + 80.0, y + 30.0, 28.0, color);
    }

    // Scroll indicators (only show if needed)
    if scroll_offset > 0 {
        draw_text("▲", menu_x + menu_width - 25.0, option_start_y + 10.0, 16.0, t.text_secondary);
    }
    if scroll_offset + MAX_VISIBLE < visible_count {
        draw_text("▼", menu_x + menu_width - 25.0, option_start_y + (MAX_VISIBLE as f32 * option_height) - 15.0, 16.0, t.text_secondary);
    }

    draw_text(
        "Press B to go back",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        18.0,
        LIGHTGRAY,
    );
}

/// Get color for achievement rarity display
fn get_rarity_color(rarity_percent: f32) -> Color {
    if rarity_percent < 5.0 {
        GOLD  // Ultra rare
    } else if rarity_percent < 10.0 {
        Color::new(0.9, 0.4, 0.9, 1.0)  // Purple - very rare
    } else if rarity_percent < 25.0 {
        Color::new(0.3, 0.6, 1.0, 1.0)  // Blue - rare
    } else if rarity_percent < 50.0 {
        GREEN  // Uncommon
    } else {
        GRAY  // Common
    }
}

fn render_achievements_screen(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 600.0;
    let menu_height = 340.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    // Background panel
    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, t.panel_border);

    let tracker = &state.achievements;

    // Title with game name
    let title = if tracker.game_title.is_empty() {
        "ACHIEVEMENTS".to_string()
    } else {
        format!("🏆 {}", tracker.game_title)
    };
    draw_text(&title, menu_x + 15.0, menu_y + 28.0, 24.0, t.cursor);

    // Filter and sort indicators
    if !tracker.achievements.is_empty() {
        let filter_text = format!("Filter: {}", tracker.filter.name());
        let sort_text = format!("Sort: {}", tracker.sort_mode.name());
        draw_text(&filter_text, menu_x + menu_width - 280.0, menu_y + 28.0, 14.0, t.text_secondary);
        draw_text(&sort_text, menu_x + menu_width - 140.0, menu_y + 28.0, 14.0, t.text_secondary);
    }

    // Progress bar and stats
    if !tracker.achievements.is_empty() {
        let earned = tracker.earned_count();
        let total = tracker.total_count();
        let filtered_count = tracker.filtered_count();
        let progress = if total > 0 { earned as f32 / total as f32 } else { 0.0 };

        // Progress text (show filtered count if filtering)
        let progress_text = if filtered_count < total {
            format!("{}/{} ({:.0}%) • {} / {} pts • Showing: {}",
                earned, total, progress * 100.0,
                tracker.earned_points(), tracker.total_points(),
                filtered_count)
        } else {
            format!("{}/{} ({:.0}%) • {} / {} pts",
                earned, total, progress * 100.0,
                tracker.earned_points(), tracker.total_points())
        };
        draw_text(&progress_text, menu_x + 15.0, menu_y + 50.0, 16.0, t.text_secondary);

        // Progress bar
        let bar_x = menu_x + 15.0;
        let bar_y = menu_y + 58.0;
        let bar_width = menu_width - 30.0;
        let bar_height = 6.0;

        // Bar background
        draw_rectangle(bar_x, bar_y, bar_width, bar_height, Color::new(0.2, 0.2, 0.2, 1.0));
        // Bar fill
        let fill_color = if progress >= 1.0 { GOLD } else { GREEN };
        draw_rectangle(bar_x, bar_y, bar_width * progress, bar_height, fill_color);

        // Achievement list (use filtered achievements)
        let list_y = menu_y + 75.0;
        let item_height = 40.0;
        let max_visible = 6;

        for i in 0..max_visible {
            let filtered_idx = tracker.scroll_offset + i;
            if filtered_idx >= filtered_count {
                break;  // No more achievements to display
            }

            // Get achievement from filtered list
            let achievement = match tracker.get_filtered_achievement(filtered_idx) {
                Some(ach) => ach,
                None => continue,
            };

            let y = list_y + (i as f32 * item_height);
            let is_selected = filtered_idx == tracker.selected_index;

            // Selection background
            if is_selected {
                draw_rectangle(
                    menu_x + 10.0, y,
                    menu_width - 20.0, item_height - 2.0,
                    Color::new(0.3, 0.3, 0.4, 0.8),
                );
            }

            // Earned indicator
            let status_icon = if achievement.earned_hardcore {
                "⭐" // Hardcore
            } else if achievement.earned {
                "✓"  // Normal
            } else {
                "○"  // Locked
            };
            let status_color = if achievement.earned { GREEN } else { GRAY };
            draw_text(status_icon, menu_x + 18.0, y + 26.0, 22.0, status_color);

            // Achievement title
            let title_color = if achievement.earned { t.text } else { t.text_disabled };
            let title_text = if achievement.title.len() > 40 {
                format!("{}...", &achievement.title[..37])
            } else {
                achievement.title.clone()
            };
            draw_text(&title_text, menu_x + 45.0, y + 22.0, 18.0, title_color);

            // Points (with rarity indicator if available)
            let points_x = menu_x + menu_width - 130.0;
            let points_text = format!("{} pts", achievement.points);
            let points_color = if achievement.earned { GOLD } else { DARKGRAY };
            draw_text(&points_text, points_x, y + 22.0, 16.0, points_color);

            // Rarity display (if available)
            if let Some(rarity) = achievement.rarity_percent {
                let rarity_color = get_rarity_color(rarity);
                let rarity_text = format!("{:.1}%", rarity);
                draw_text(&rarity_text, menu_x + menu_width - 65.0, y + 22.0, 14.0, rarity_color);
            }

            // Progress bar for multi-step achievements (on same line as description)
            if let Some(progress) = &achievement.progress {
                let prog_percent = progress.current as f32 / progress.target as f32;
                let prog_text = format!("{}/{}", progress.current, progress.target);

                // Progress bar
                let prog_bar_x = menu_x + 45.0;
                let prog_bar_y = y + 28.0;
                let prog_bar_width = 100.0;
                let prog_bar_height = 4.0;

                // Background
                draw_rectangle(prog_bar_x, prog_bar_y, prog_bar_width, prog_bar_height, Color::new(0.2, 0.2, 0.2, 1.0));
                // Fill
                let fill_width = prog_bar_width * prog_percent;
                draw_rectangle(prog_bar_x, prog_bar_y, fill_width, prog_bar_height, GREEN);

                // Progress text
                draw_text(&prog_text, prog_bar_x + prog_bar_width + 5.0, y + 32.0, 11.0, LIGHTGRAY);
            }

            // Description (smaller, only for selected if no progress bar)
            if is_selected && !achievement.description.is_empty() && achievement.progress.is_none() {
                let desc = if achievement.description.len() > 60 {
                    format!("{}...", &achievement.description[..57])
                } else {
                    achievement.description.clone()
                };
                draw_text(&desc, menu_x + 45.0, y + 36.0, 12.0, Color::new(0.6, 0.6, 0.6, 1.0));
            }
        }

        // Scroll indicators (use filtered count)
        if tracker.scroll_offset > 0 {
            draw_text("▲", menu_x + menu_width - 25.0, list_y + 10.0, 16.0, LIGHTGRAY);
        }
        if tracker.scroll_offset + max_visible < filtered_count {
            draw_text("▼", menu_x + menu_width - 25.0, list_y + (max_visible as f32 * item_height) - 15.0, 16.0, LIGHTGRAY);
        }
    } else {
        // No achievements loaded
        draw_text(
            "No RetroAchievements data",
            menu_x + menu_width / 2.0 - 100.0,
            menu_y + 120.0,
            20.0,
            GRAY,
        );
        draw_text(
            "Start a game with RA support to view achievements",
            menu_x + 30.0,
            menu_y + 160.0,
            16.0,
            DARKGRAY,
        );
    }

    // Controls hint
    draw_text(
        "D-Pad: Navigate • LB/RB: Filter • LT/RT: Sort • B: Back",
        menu_x + 15.0,
        menu_y + menu_height - 12.0,
        14.0,
        LIGHTGRAY,
    );
}

fn render_controllers_menu(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 600.0;
    let menu_height = 380.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    // Menu background
    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, t.panel_border);

    // Title
    draw_text("🎮 CONTROLLERS", menu_x + 20.0, menu_y + 40.0, 32.0, t.cursor);

    // Connected controller count
    let controller_count = state.controllers.controllers.len();
    let status_text = format!("{} controller(s) connected", controller_count);
    draw_text(&status_text, menu_x + 20.0, menu_y + 70.0, 18.0, LIGHTGRAY);

    // Menu options
    let option_start_y = menu_y + 110.0;
    let option_height = 45.0;
    const MAX_VISIBLE: usize = 6;
    let menu_len = CONTROLLER_MENU_OPTIONS.len();
    let scroll_offset = 0; // Controllers menu is short, no scrolling needed for now

    for i in 0..MAX_VISIBLE.min(menu_len.saturating_sub(scroll_offset)) {
        let item_idx = scroll_offset + i;
        if item_idx >= menu_len {
            break;
        }
        let option = CONTROLLER_MENU_OPTIONS[item_idx];
        let y = option_start_y + (i as f32 * option_height);
        let is_selected = item_idx == state.controllers.selected_menu_item;
        let color = if is_selected { t.cursor } else { t.text };

        // Selection indicator
        if is_selected {
            draw_rectangle(
                menu_x + 15.0, y - 5.0,
                menu_width - 30.0, option_height - 5.0,
                Color::new(t.accent.r * 0.2, t.accent.g * 0.2, t.accent.b * 0.2, 0.6),
            );
            draw_text("►", menu_x + 25.0, y + 25.0, 24.0, t.cursor);
        }

        draw_text(option, menu_x + 60.0, y + 25.0, 24.0, color);

        // Show sub-info for some options
        match i {
            0 => {
                // Bluetooth - show paired count if any
                let paired = state.controllers.bluetooth_devices.iter().filter(|d| d.is_paired).count();
                if paired > 0 {
                    draw_text(&format!("{} paired", paired), menu_x + menu_width - 120.0, y + 25.0, 18.0, GRAY);
                }
            }
            1 => {
                // Assign - show assignment summary
                let assigned = state.controllers.player_assignments.iter().filter(|a| a.is_some()).count();
                draw_text(&format!("{}/4 assigned", assigned), menu_x + menu_width - 120.0, y + 25.0, 18.0, GRAY);
            }
            _ => {}
        }
    }

    // Success/error message
    if let Some((msg, _)) = &state.controllers.success_message {
        draw_text(msg, menu_x + 20.0, menu_y + menu_height - 50.0, 18.0, GREEN);
    }
    if let Some(msg) = &state.controllers.error_message {
        draw_text(msg, menu_x + 20.0, menu_y + menu_height - 50.0, 18.0, RED);
    }

    // Controls hint
    draw_text(
        "D-Pad: Navigate • A: Select • B: Back",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        16.0,
        LIGHTGRAY,
    );
}

fn render_bluetooth_screen(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 600.0;
    let menu_height = 400.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, t.panel_border);

    // Title
    draw_text("📶 BLUETOOTH CONTROLLERS", menu_x + 20.0, menu_y + 40.0, 28.0, t.cursor);

    // Scan state indicator
    let scan_status = match &state.controllers.bluetooth_state {
        BluetoothScanState::Idle => ("Press X to scan", GRAY),
        BluetoothScanState::Scanning => ("Scanning...", YELLOW),
        BluetoothScanState::Pairing(mac) => (&format!("Pairing: {}...", mac) as &str, ORANGE),
        BluetoothScanState::Connecting(mac) => (&format!("Connecting: {}...", mac) as &str, BLUE),
        BluetoothScanState::Error(err) => (err.as_str(), RED),
    };
    // Need to handle the lifetime issue differently
    let (scan_text, scan_color) = scan_status;
    draw_text(scan_text, menu_x + 20.0, menu_y + 70.0, 16.0, scan_color);

    // Device list
    let list_y = menu_y + 100.0;
    let item_height = 50.0;
    let max_visible = 5;

    if state.controllers.bluetooth_devices.is_empty() {
        draw_text(
            "No devices found",
            menu_x + menu_width / 2.0 - 80.0,
            menu_y + 180.0,
            20.0,
            t.text_disabled,
        );
        draw_text(
            "Press X to scan for Bluetooth controllers",
            menu_x + 60.0,
            menu_y + 220.0,
            16.0,
            t.text_secondary,
        );
    } else {
        let device_count = state.controllers.bluetooth_devices.len();
        let scroll_offset = state.controllers.bt_scroll_offset;

        for i in 0..max_visible.min(device_count.saturating_sub(scroll_offset)) {
            let device_idx = scroll_offset + i;
            if device_idx >= device_count {
                break;
            }
            let device = &state.controllers.bluetooth_devices[device_idx];
            let y = list_y + (i as f32 * item_height);
            let is_selected = device_idx == state.controllers.bt_selected_index;

            // Selection background
            if is_selected {
                draw_rectangle(
                    menu_x + 15.0, y,
                    menu_width - 30.0, item_height - 5.0,
                    Color::new(0.3, 0.3, 0.4, 0.6),
                );
            }

            // Device icon based on state
            let icon = if device.is_connected {
                "🎮"
            } else if device.is_paired {
                "🔗"
            } else {
                "📡"
            };
            draw_text(icon, menu_x + 25.0, y + 30.0, 24.0, WHITE);

            // Device name
            let name_color = if device.is_connected { t.success } else if device.is_paired { t.text } else { t.text_disabled };
            draw_text(&device.name, menu_x + 60.0, y + 25.0, 20.0, name_color);

            // MAC address (smaller)
            draw_text(&device.mac_address, menu_x + 60.0, y + 42.0, 12.0, DARKGRAY);

            // Status
            let status = if device.is_connected {
                "Connected"
            } else if device.is_paired {
                "Paired"
            } else {
                "Available"
            };
            let status_color = if device.is_connected { GREEN } else if device.is_paired { YELLOW } else { GRAY };
                draw_text(status, menu_x + menu_width - 110.0, y + 30.0, 16.0, status_color);
        }

        // Scroll indicators
        if scroll_offset > 0 {
            draw_text("▲", menu_x + menu_width - 25.0, list_y + 10.0, 16.0, t.text_secondary);
        }
        if scroll_offset + max_visible < device_count {
            draw_text("▼", menu_x + menu_width - 25.0, list_y + (max_visible as f32 * item_height) - 15.0, 16.0, t.text_secondary);
        }
    }

    // Controls hint
    draw_text(
        "D-Pad: Navigate • A: Pair/Connect • X: Scan • B: Back",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        14.0,
        LIGHTGRAY,
    );
}

fn render_assign_screen(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 600.0;
    let menu_height = 380.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, t.panel_border);

    // Title
    draw_text("👥 ASSIGN CONTROLLERS", menu_x + 20.0, menu_y + 40.0, 28.0, t.cursor);
    draw_text(
        "Use Left/Right to change assignment",
        menu_x + 20.0,
        menu_y + 68.0,
        16.0,
        LIGHTGRAY,
    );

    // Player assignment slots
    let slot_start_y = menu_y + 100.0;
    let slot_height = 60.0;

    for player in 0..MAX_PLAYERS {
        let y = slot_start_y + (player as f32 * slot_height);
        let is_selected = player == state.controllers.assign_selected_player;

        // Selection background
        if is_selected {
            draw_rectangle(
                menu_x + 15.0, y,
                menu_width - 30.0, slot_height - 5.0,
                Color::new(0.3, 0.3, 0.4, 0.6),
            );
        }

        // Player label with color
        let player_colors = [
            Color::new(0.2, 0.6, 1.0, 1.0),  // P1: Blue
            Color::new(1.0, 0.3, 0.3, 1.0),  // P2: Red
            Color::new(0.3, 1.0, 0.3, 1.0),  // P3: Green
            Color::new(1.0, 1.0, 0.3, 1.0),  // P4: Yellow
        ];
        let player_color = player_colors[player];
        draw_text(&format!("P{}", player + 1), menu_x + 30.0, y + 35.0, 28.0, player_color);

        // Assigned controller name
        let controller_name = state.controllers.get_player_controller(player + 1)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "< Not Assigned >".to_string());
        
        let name_color = if state.controllers.player_assignments[player].is_some() {
            t.text
        } else {
            t.text_disabled
        };
        draw_text(&controller_name, menu_x + 100.0, y + 35.0, 20.0, name_color);

        // Left/Right arrows if selected
        if is_selected {
            draw_text("◄", menu_x + menu_width - 80.0, y + 35.0, 24.0, t.cursor);
            draw_text("►", menu_x + menu_width - 40.0, y + 35.0, 24.0, t.cursor);
        }
    }

    // Available controllers summary
    let unassigned_count = state.controllers.controllers.iter()
        .filter(|c| c.assigned_player.is_none())
        .count();
    if unassigned_count > 0 {
        draw_text(
            &format!("{} unassigned controller(s) available", unassigned_count),
            menu_x + 20.0,
            menu_y + menu_height - 50.0,
            16.0,
            YELLOW,
        );
    }

    // Controls hint
    draw_text(
        "Up/Down: Select Player • Left/Right: Change • A: Quick Assign • B: Back",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        14.0,
        LIGHTGRAY,
    );
}

fn render_gamepad_tester(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 620.0;
    let menu_height = 340.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, t.panel_border);

    // Title with controller selector
    draw_text("🕹️ GAMEPAD TESTER", menu_x + 20.0, menu_y + 35.0, 26.0, t.cursor);

    if state.controllers.controllers.is_empty() {
        draw_text(
            "No controllers connected",
            menu_x + menu_width / 2.0 - 100.0,
            menu_y + 150.0,
            22.0,
            GRAY,
        );
        draw_text(
            "Connect a controller to test",
            menu_x + menu_width / 2.0 - 110.0,
            menu_y + 190.0,
            18.0,
            DARKGRAY,
        );
    } else {
        // Controller name with navigation
        let controller_name = state.controllers.controllers
            .get(state.controllers.tester_selected_controller)
            .map(|c| c.name.as_str())
            .unwrap_or("Unknown");
        
        let nav_text = format!("◄ {} ({}/{}) ►", 
            controller_name,
            state.controllers.tester_selected_controller + 1,
            state.controllers.controllers.len()
        );
        draw_text(&nav_text, menu_x + 180.0, menu_y + 35.0, 18.0, LIGHTGRAY);

        let btn = &state.controllers.tester_button_state;

        // Layout constants
        let left_x = menu_x + 80.0;
        let right_x = menu_x + 420.0;
        let center_y = menu_y + 170.0;

        // === Left side: D-Pad and Left Stick ===
        
        // D-Pad
        let dpad_x = left_x;
        let dpad_y = center_y - 40.0;
        let btn_size = 28.0;
        
        draw_text("D-PAD", dpad_x - 10.0, dpad_y - 45.0, 14.0, GRAY);
        // Up
        draw_button(dpad_x, dpad_y - btn_size, btn_size, btn.dpad_up, "▲");
        // Down
        draw_button(dpad_x, dpad_y + btn_size, btn_size, btn.dpad_down, "▼");
        // Left
        draw_button(dpad_x - btn_size, dpad_y, btn_size, btn.dpad_left, "◄");
        // Right
        draw_button(dpad_x + btn_size, dpad_y, btn_size, btn.dpad_right, "►");

        // Left Stick
        let ls_x = left_x + 100.0;
        let ls_y = center_y + 40.0;
        draw_text("L STICK", ls_x - 15.0, ls_y - 50.0, 14.0, GRAY);
        draw_stick(ls_x, ls_y, 35.0, btn.left_stick_x, btn.left_stick_y, btn.ls_press);

        // === Center: Triggers and special buttons ===
        
        let center_x = menu_x + menu_width / 2.0;
        
        // Triggers at top
        let trigger_y = menu_y + 70.0;
        draw_text("LT", center_x - 100.0, trigger_y, 14.0, GRAY);
        draw_trigger(center_x - 80.0, trigger_y + 5.0, 60.0, 15.0, btn.lt);
        draw_text("RT", center_x + 50.0, trigger_y, 14.0, GRAY);
        draw_trigger(center_x + 20.0, trigger_y + 5.0, 60.0, 15.0, btn.rt);

        // Bumpers
        let bumper_y = trigger_y + 30.0;
        draw_button(center_x - 80.0, bumper_y, 30.0, btn.lb, "LB");
        draw_button(center_x + 50.0, bumper_y, 30.0, btn.rb, "RB");

        // Select/Start/Guide
        let special_y = center_y;
        draw_button(center_x - 60.0, special_y, 25.0, btn.select, "⊡");
        draw_button(center_x, special_y - 20.0, 30.0, btn.guide, "⬡");
        draw_button(center_x + 35.0, special_y, 25.0, btn.start, "≡");
        draw_text("SEL", center_x - 65.0, special_y + 35.0, 10.0, DARKGRAY);
        draw_text("GUIDE", center_x - 15.0, special_y + 15.0, 10.0, DARKGRAY);
        draw_text("START", center_x + 25.0, special_y + 35.0, 10.0, DARKGRAY);

        // === Right side: Face buttons and Right Stick ===
        
        // Face buttons (ABXY)
        let face_x = right_x;
        let face_y = center_y - 40.0;
        
        draw_text("BUTTONS", face_x - 20.0, face_y - 45.0, 14.0, GRAY);
        // A (bottom)
        draw_button_colored(face_x, face_y + btn_size, btn_size, btn.a, "A", GREEN);
        // B (right)
        draw_button_colored(face_x + btn_size, face_y, btn_size, btn.b, "B", RED);
        // X (left)
        draw_button_colored(face_x - btn_size, face_y, btn_size, btn.x, "X", BLUE);
        // Y (top)
        draw_button_colored(face_x, face_y - btn_size, btn_size, btn.y, "Y", YELLOW);

        // Right Stick
        let rs_x = right_x - 100.0;
        let rs_y = center_y + 40.0;
        draw_text("R STICK", rs_x - 15.0, rs_y - 50.0, 14.0, GRAY);
        draw_stick(rs_x, rs_y, 35.0, btn.right_stick_x, btn.right_stick_y, btn.rs_press);

        // Last input indicator
        let elapsed = state.controllers.tester_last_input_time.elapsed();
        if elapsed.as_secs() < 2 {
            draw_text("● Input detected", menu_x + 20.0, menu_y + menu_height - 50.0, 14.0, GREEN);
        }
    }

    // Controls hint
    draw_text(
        "Left/Right: Switch Controller • B: Back • Press buttons to test",
        menu_x + 20.0,
        menu_y + menu_height - 18.0,
        14.0,
        LIGHTGRAY,
    );
}

// Helper functions for gamepad tester rendering

fn draw_button(x: f32, y: f32, size: f32, pressed: bool, label: &str) {
    let color = if pressed { GREEN } else { Color::new(0.3, 0.3, 0.3, 1.0) };
    let border = if pressed { WHITE } else { GRAY };
    
    draw_rectangle(x - size/2.0, y - size/2.0, size, size, color);
    draw_rectangle_lines(x - size/2.0, y - size/2.0, size, size, 2.0, border);
    
    let label_size = if label.len() > 1 { 12.0 } else { 16.0 };
    let dims = measure_text(label, None, label_size as u16, 1.0);
    draw_text(label, x - dims.width/2.0, y + dims.height/4.0, label_size, WHITE);
}

fn draw_button_colored(x: f32, y: f32, size: f32, pressed: bool, label: &str, color: Color) {
    let bg_color = if pressed { color } else { Color::new(0.2, 0.2, 0.2, 1.0) };
    let border = if pressed { WHITE } else { Color::new(color.r * 0.5, color.g * 0.5, color.b * 0.5, 1.0) };
    
    draw_circle(x, y, size/2.0, bg_color);
    draw_circle_lines(x, y, size/2.0, 2.0, border);
    
    let dims = measure_text(label, None, 14, 1.0);
    draw_text(label, x - dims.width/2.0, y + dims.height/4.0, 14.0, WHITE);
}

fn draw_stick(x: f32, y: f32, radius: f32, stick_x: f32, stick_y: f32, pressed: bool) {
    // Outer circle (deadzone area)
    draw_circle_lines(x, y, radius, 2.0, GRAY);
    
    // Stick position
    let stick_radius = radius * 0.4;
    let pos_x = x + stick_x * (radius - stick_radius);
    let pos_y = y - stick_y * (radius - stick_radius); // Invert Y for display
    
    let stick_color = if pressed { GREEN } else { Color::new(0.6, 0.6, 0.6, 1.0) };
    draw_circle(pos_x, pos_y, stick_radius, stick_color);
    draw_circle_lines(pos_x, pos_y, stick_radius, 2.0, WHITE);
}

fn draw_trigger(x: f32, y: f32, width: f32, height: f32, value: f32) {
    // Background
    draw_rectangle(x, y, width, height, Color::new(0.2, 0.2, 0.2, 1.0));
    // Fill
    let fill_color = if value > 0.1 { GREEN } else { Color::new(0.3, 0.3, 0.3, 1.0) };
    draw_rectangle(x, y, width * value, height, fill_color);
    // Border
    draw_rectangle_lines(x, y, width, height, 1.0, GRAY);
}

fn render_hotkey_settings(_state: &OverlayState) {
    let menu_width = 620.0;
    let menu_height = 400.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    draw_rectangle(menu_x, menu_y, menu_width, menu_height, Color::new(0.1, 0.1, 0.15, 0.98));
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, YELLOW);

    // Title
    draw_text("⌨️ HOTKEY SETTINGS", menu_x + 20.0, menu_y + 40.0, 28.0, YELLOW);

    // Coming soon message
    draw_text(
        "Configure custom hotkey bindings",
        menu_x + 20.0,
        menu_y + 80.0,
        18.0,
        LIGHTGRAY,
    );

    let info_y = menu_y + 140.0;
    let line_height = 30.0;

    // Show current default bindings
    draw_text("Current Hotkeys:", menu_x + 20.0, info_y, 20.0, WHITE);

    draw_text("• Toggle Overlay:", menu_x + 30.0, info_y + line_height, 16.0, LIGHTGRAY);
    draw_text("F12, Ctrl+O, Guide Button", menu_x + 200.0, info_y + line_height, 16.0, GREEN);

    draw_text("• Performance HUD:", menu_x + 30.0, info_y + line_height * 2.0, 16.0, LIGHTGRAY);
    draw_text("F3", menu_x + 200.0, info_y + line_height * 2.0, 16.0, GREEN);

    draw_text("• Quick Save:", menu_x + 30.0, info_y + line_height * 3.0, 16.0, LIGHTGRAY);
    draw_text("F5", menu_x + 200.0, info_y + line_height * 3.0, 16.0, GREEN);

    draw_text("• Quick Load:", menu_x + 30.0, info_y + line_height * 4.0, 16.0, LIGHTGRAY);
    draw_text("F9", menu_x + 200.0, info_y + line_height * 4.0, 16.0, GREEN);

    // Coming soon notice
    draw_text(
        "🚧 Custom hotkey configuration coming soon! 🚧",
        menu_x + 80.0,
        menu_y + menu_height - 60.0,
        16.0,
        ORANGE,
    );

    // Controls hint
    draw_text(
        "B: Back to Controllers",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        16.0,
        LIGHTGRAY,
    );
}

fn render_menu_customization(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 700.0;
    let menu_height = 500.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, t.panel_border);

    // Title
    draw_text("⚙️ MENU CUSTOMIZATION", menu_x + 20.0, menu_y + 40.0, 28.0, t.cursor);

    // Instructions
    draw_text(
        "Left/Right: Toggle visibility • LB/RB: Move up/down",
        menu_x + 20.0,
        menu_y + 70.0,
        16.0,
        LIGHTGRAY,
    );

    // Menu items list
    let all_items = state.menu_config.config().get_all_items();
    let item_start_y = menu_y + 110.0;
    let item_height = 45.0;
    let max_visible = 8;

    // Use scroll offset from state
    let scroll_offset = state.menu_customization_scroll_offset;

    for i in 0..max_visible.min(all_items.len() - scroll_offset) {
        let item_idx = scroll_offset + i;
        if item_idx >= all_items.len() {
            break;
        }
        let item_id = all_items[item_idx];
        let y = item_start_y + (i as f32 * item_height);
        let is_selected = item_idx == state.menu_customization_selected;
        
        // Get item config
        let item_config = state.menu_config.config().items.iter()
            .find(|item| item.id == item_id)
            .unwrap();

        // Selection background
        if is_selected {
            draw_rectangle(
                menu_x + 15.0, y,
                menu_width - 30.0, item_height - 5.0,
                Color::new(0.3, 0.3, 0.4, 0.6),
            );
            draw_text("►", menu_x + 25.0, y + 28.0, 24.0, t.cursor);
        }

        // Visibility indicator
        let visibility_icon = if item_config.visible { "✓" } else { "✗" };
        let visibility_color = if item_config.visible { t.success } else { t.error };
        draw_text(visibility_icon, menu_x + 60.0, y + 28.0, 20.0, visibility_color);

        // Item name
        let name_color = if item_config.visible { t.text } else { t.text_disabled };
        draw_text(item_id.display_name(), menu_x + 100.0, y + 28.0, 22.0, name_color);

        // Order indicator
        let order_text = format!("#{:02}", item_config.order + 1);
        draw_text(&order_text, menu_x + menu_width - 100.0, y + 28.0, 18.0, LIGHTGRAY);

        // Move indicators
        if is_selected {
            if item_idx > 0 {
                draw_text("▲", menu_x + menu_width - 60.0, y + 28.0, 18.0, LIGHTGRAY);
            }
            if item_idx < all_items.len() - 1 {
                draw_text("▼", menu_x + menu_width - 40.0, y + 28.0, 18.0, LIGHTGRAY);
            }
        }
    }

    // Scroll indicators
    if scroll_offset > 0 {
        draw_text("▲", menu_x + menu_width - 25.0, item_start_y + 10.0, 16.0, LIGHTGRAY);
    }
    if scroll_offset + max_visible < all_items.len() {
        draw_text("▼", menu_x + menu_width - 25.0, item_start_y + (max_visible as f32 * item_height) - 15.0, 16.0, LIGHTGRAY);
    }

    // Controls hint
    draw_text(
        "D-Pad: Navigate • Left/Right: Toggle • LB/RB: Reorder • B: Back",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        14.0,
        LIGHTGRAY,
    );
}

fn render_theme_selection(state: &OverlayState) {
    let t = theme(state);
    let menu_width = 700.0;
    let menu_height = 500.0;
    let menu_x = (screen_width() - menu_width) / 2.0;
    let menu_y = (screen_height() - menu_height) / 2.0;

    draw_rectangle(menu_x, menu_y, menu_width, menu_height, t.panel_background);
    draw_rectangle_lines(menu_x, menu_y, menu_width, menu_height, 2.0, t.panel_border);

    // Title
    draw_text("🎨 THEME SELECTION", menu_x + 20.0, menu_y + 40.0, 28.0, t.cursor);

    // Instructions
    draw_text(
        "Select a theme and press A to apply",
        menu_x + 20.0,
        menu_y + 70.0,
        16.0,
        t.text_secondary,
    );

    // Theme list
    let themes = crate::theme_config::ThemeConfigManager::available_themes();
    let theme_start_y = menu_y + 110.0;
    let theme_height = 60.0;
    let max_visible = 6;

    // Use scroll offset from state
    let scroll_offset = state.theme_selection_scroll_offset;

    for i in 0..max_visible.min(themes.len() - scroll_offset) {
        let theme_idx = scroll_offset + i;
        if theme_idx >= themes.len() {
            break;
        }
        let theme_name = &themes[theme_idx];
        let y = theme_start_y + (i as f32 * theme_height);
        let is_selected = theme_idx == state.theme_selected;
        let is_current = theme_name == state.theme_config.theme_name();

        // Get theme preview
        let preview_theme = crate::themes::Theme::by_name(theme_name).unwrap_or_else(|| crate::themes::Theme::dark());

        // Selection background
        if is_selected {
            draw_rectangle(
                menu_x + 15.0, y,
                menu_width - 30.0, theme_height - 5.0,
                Color::new(t.accent.r * 0.2, t.accent.g * 0.2, t.accent.b * 0.2, 0.6),
            );
            draw_text("►", menu_x + 25.0, y + 35.0, 24.0, t.cursor);
        }

        // Theme name
        let name_color = if is_selected { t.cursor } else { t.text };
        draw_text(theme_name, menu_x + 60.0, y + 25.0, 24.0, name_color);

        // Current theme indicator
        if is_current {
            draw_text("(Current)", menu_x + 60.0, y + 45.0, 16.0, t.accent);
        }

        // Color preview swatches
        let swatch_size = 20.0;
        let swatch_y = y + 20.0;
        let swatch_start_x = menu_x + menu_width - 200.0;
        let swatch_spacing = 25.0;

        // Background
        draw_rectangle(swatch_start_x, swatch_y, swatch_size, swatch_size, preview_theme.panel_background);
        draw_rectangle_lines(swatch_start_x, swatch_y, swatch_size, swatch_size, 1.0, preview_theme.panel_border);
        
        // Text
        draw_rectangle(swatch_start_x + swatch_spacing, swatch_y, swatch_size, swatch_size, preview_theme.text);
        
        // Cursor/Accent
        draw_rectangle(swatch_start_x + swatch_spacing * 2.0, swatch_y, swatch_size, swatch_size, preview_theme.cursor);
        
        // Accent
        draw_rectangle(swatch_start_x + swatch_spacing * 3.0, swatch_y, swatch_size, swatch_size, preview_theme.accent);
    }

    // Scroll indicators
    if scroll_offset > 0 {
        draw_text("▲", menu_x + menu_width - 25.0, theme_start_y + 10.0, 16.0, t.text_secondary);
    }
    if scroll_offset + max_visible < themes.len() {
        draw_text("▼", menu_x + menu_width - 25.0, theme_start_y + (max_visible as f32 * theme_height) - 15.0, 16.0, t.text_secondary);
    }

    // Controls hint
    draw_text(
        "D-Pad: Navigate • A: Apply Theme • B: Back",
        menu_x + 20.0,
        menu_y + menu_height - 20.0,
        14.0,
        t.text_secondary,
    );
}

fn render_performance(state: &OverlayState) {
    let t = theme(state);
    let panel_width = 600.0;
    let panel_height = 400.0;
    let panel_x = (screen_width() - panel_width) / 2.0;
    let panel_y = (screen_height() - panel_height) / 2.0;

    // Panel background
    draw_rectangle(panel_x, panel_y, panel_width, panel_height, t.panel_background);
    draw_rectangle_lines(panel_x, panel_y, panel_width, panel_height, 3.0, t.panel_border);

    // Title
    draw_text("PERFORMANCE", panel_x + 20.0, panel_y + 40.0, 32.0, t.cursor);

    // Performance stats
    let stats_y = panel_y + 80.0;
    let cpu = state.performance.get_cpu_usage();
    let ram = state.performance.get_ram_usage();
    let fps = state.performance.get_average_fps();

    draw_text(
        &format!("CPU: {:.1}%", cpu),
        panel_x + 40.0,
        stats_y,
        24.0,
        t.text,
    );
    draw_text(
        &format!("RAM: {:.1}%", ram),
        panel_x + 40.0,
        stats_y + 40.0,
        24.0,
        t.text,
    );
    draw_text(
        &format!("FPS: {:.0}", fps),
        panel_x + 40.0,
        stats_y + 80.0,
        24.0,
        t.text,
    );

    // Back hint
    draw_text(
        "Press B to go back",
        panel_x + 20.0,
        panel_y + panel_height - 20.0,
        16.0,
        t.text_secondary,
    );
}

fn render_playtime(state: &OverlayState) {
    let t = theme(state);
    let panel_width = 600.0;
    let panel_height = 400.0;
    let panel_x = (screen_width() - panel_width) / 2.0;
    let panel_y = (screen_height() - panel_height) / 2.0;

    // Panel background
    draw_rectangle(panel_x, panel_y, panel_width, panel_height, t.panel_background);
    draw_rectangle_lines(panel_x, panel_y, panel_width, panel_height, 3.0, t.panel_border);

    // Title
    draw_text("PLAYTIME", panel_x + 20.0, panel_y + 40.0, 32.0, t.cursor);

    // Current session
    if let Some(session_time) = state.playtime.get_current_session_duration() {
        let hours = session_time.as_secs() / 3600;
        let minutes = (session_time.as_secs() % 3600) / 60;
        let seconds = session_time.as_secs() % 60;

        draw_text(
            &format!("Current Session: {:02}:{:02}:{:02}", hours, minutes, seconds),
            panel_x + 40.0,
            panel_y + 100.0,
            24.0,
            t.text,
        );
    } else {
        draw_text(
            "No active session",
            panel_x + 40.0,
            panel_y + 100.0,
            24.0,
            t.text_secondary,
        );
    }

    // Back hint
    draw_text(
        "Press B to go back",
        panel_x + 20.0,
        panel_y + panel_height - 20.0,
        16.0,
        t.text_secondary,
    );
}

fn render_quit_confirm(state: &OverlayState) {
    let t = theme(state);
    let dialog_width = 450.0;
    let dialog_height = 220.0;
    let dialog_x = (screen_width() - dialog_width) / 2.0;
    let dialog_y = (screen_height() - dialog_height) / 2.0;

    // Dialog background with red tint
    draw_rectangle(dialog_x, dialog_y, dialog_width, dialog_height, Color::new(t.error.r * 0.15, t.error.g * 0.08, t.error.b * 0.08, 0.98));
    draw_rectangle_lines(dialog_x, dialog_y, dialog_width, dialog_height, 3.0, t.error);

    // Warning icon and title
    draw_text("⚠️ QUIT GAME?", dialog_x + 130.0, dialog_y + 50.0, 32.0, t.error);

    // Message
    draw_text(
        "Are you sure you want to quit?",
        dialog_x + 85.0,
        dialog_y + 90.0,
        22.0,
        t.text,
    );
    draw_text(
        "Unsaved progress will be lost.",
        dialog_x + 95.0,
        dialog_y + 115.0,
        18.0,
        GRAY,
    );

    // Buttons
    let button_y = dialog_y + 155.0;
    let cancel_selected = state.quit_confirm_selected == 0;
    let quit_selected = state.quit_confirm_selected == 1;

    // Cancel button (left)
    let cancel_bg = if cancel_selected {
        Color::new(0.3, 0.3, 0.3, 1.0)
    } else {
        Color::new(0.2, 0.2, 0.2, 1.0)
    };
    let cancel_border = if cancel_selected { t.cursor } else { GRAY };

    draw_rectangle(dialog_x + 50.0, button_y, 150.0, 35.0, cancel_bg);
    draw_rectangle_lines(dialog_x + 50.0, button_y, 150.0, 35.0, if cancel_selected { 3.0 } else { 2.0 }, cancel_border);

    // Selection indicator for Cancel
    if cancel_selected {
        draw_text("►", dialog_x + 25.0, button_y + 25.0, 24.0, t.cursor);
    }

    draw_text("CANCEL", dialog_x + 80.0, button_y + 25.0, 20.0, WHITE);

    // Quit button (right)
    let quit_bg = if quit_selected {
        Color::new(0.7, 0.2, 0.2, 1.0)
    } else {
        Color::new(0.6, 0.2, 0.2, 1.0)
    };
    let quit_border = if quit_selected { Color::new(1.0, 0.4, 0.4, 1.0) } else { RED };

    draw_rectangle(dialog_x + 250.0, button_y, 150.0, 35.0, quit_bg);
    draw_rectangle_lines(dialog_x + 250.0, button_y, 150.0, 35.0, if quit_selected { 3.0 } else { 2.0 }, quit_border);

    // Selection indicator for Quit
    if quit_selected {
        draw_text("►", dialog_x + 225.0, button_y + 25.0, 24.0, Color::new(1.0, 0.4, 0.4, 1.0));
    }

    draw_text("QUIT", dialog_x + 290.0, button_y + 25.0, 20.0, WHITE);

    // Input hints at bottom
    draw_text(
        "Use D-Pad/Stick to select • A to confirm • B to cancel",
        dialog_x + 35.0,
        dialog_y + dialog_height - 10.0,
        14.0,
        GRAY,
    );
}

fn render_toasts(state: &OverlayState) {
    let toasts = state.toasts.get_visible_toasts();
    if toasts.is_empty() {
        return;
    }

    let toast_width = 400.0;
    let toast_height = 70.0;
    let toast_margin = 10.0;
    let base_x = screen_width() - toast_width - 20.0;
    let base_y = 20.0;

    for (i, toast) in toasts.iter().enumerate() {
        let y = base_y + (i as f32 * (toast_height + toast_margin));

        // Calculate fade based on remaining time
        use std::time::Instant;
        let elapsed = Instant::now().duration_since(toast.created_at);
        let remaining = toast.duration.saturating_sub(elapsed);
        let alpha = if remaining < std::time::Duration::from_millis(500) {
            remaining.as_millis() as f32 / 500.0
        } else {
            1.0
        };

        // Background color based on style
        let bg_color = match toast.style {
            ToastStyle::Info => Color::new(0.2, 0.4, 0.8, 0.95 * alpha),
            ToastStyle::Success => Color::new(0.2, 0.8, 0.4, 0.95 * alpha),
            ToastStyle::Warning => Color::new(0.9, 0.7, 0.2, 0.95 * alpha),
            ToastStyle::Error => Color::new(0.9, 0.2, 0.2, 0.95 * alpha),
        };

        // Draw toast background
        draw_rectangle(base_x, y, toast_width, toast_height, bg_color);
        draw_rectangle_lines(
            base_x,
            y,
            toast_width,
            toast_height,
            2.0,
            Color::new(1.0, 1.0, 1.0, alpha),
        );

        // Draw message (word wrap if needed)
        let text_x = base_x + 15.0;
        let text_y = y + toast_height / 2.0 + 5.0;

        draw_text(
            &toast.message,
            text_x,
            text_y,
            22.0,
            Color::new(1.0, 1.0, 1.0, alpha),
        );
    }
}

fn render_performance_hud(state: &OverlayState) {
    let hud_width = 200.0;
    let hud_height = 110.0;
    let hud_x = 10.0;
    let hud_y = 10.0;
    let padding = 8.0;

    // Semi-transparent background
    draw_rectangle(
        hud_x,
        hud_y,
        hud_width,
        hud_height,
        Color::new(0.0, 0.0, 0.0, 0.75),
    );

    // Border
    draw_rectangle_lines(
        hud_x,
        hud_y,
        hud_width,
        hud_height,
        1.5,
        Color::new(0.5, 0.5, 0.5, 0.9),
    );

    let text_x = hud_x + padding;
    let mut text_y = hud_y + padding + 14.0;
    let line_height = 18.0;

    // Title
    draw_text("PERFORMANCE", text_x, text_y, 14.0, YELLOW);
    text_y += line_height + 2.0;

    // FPS
    let fps = state.performance.fps();
    let fps_color = if fps >= 58.0 {
        GREEN
    } else if fps >= 45.0 {
        YELLOW
    } else {
        RED
    };
    draw_text(
        &format!("FPS: {:.1}", fps),
        text_x,
        text_y,
        16.0,
        fps_color,
    );
    text_y += line_height;

    // Frame time
    let frame_time = state.performance.avg_frame_time_ms();
    draw_text(
        &format!("Frame: {:.2}ms", frame_time),
        text_x,
        text_y,
        16.0,
        WHITE,
    );
    text_y += line_height;

    // CPU usage
    let cpu_usage = state.performance.cpu_usage();
    let cpu_color = if cpu_usage < 70.0 {
        GREEN
    } else if cpu_usage < 90.0 {
        YELLOW
    } else {
        RED
    };
    draw_text(
        &format!("CPU: {:.1}%", cpu_usage),
        text_x,
        text_y,
        16.0,
        cpu_color,
    );
    text_y += line_height;

    // Memory usage
    let mem_used = state.performance.memory_used_mb();
    let mem_total = state.performance.memory_total_mb();
    let mem_percent = state.performance.memory_usage_percent();
    let mem_color = if mem_percent < 70.0 {
        GREEN
    } else if mem_percent < 90.0 {
        YELLOW
    } else {
        RED
    };
    draw_text(
        &format!("MEM: {:.0}/{:.0}MB", mem_used, mem_total),
        text_x,
        text_y,
        16.0,
        mem_color,
    );

    // Hint at bottom
    draw_text(
        "F3: Toggle",
        hud_x + hud_width - 65.0,
        hud_y + hud_height - 5.0,
        10.0,
        DARKGRAY,
    );
}
