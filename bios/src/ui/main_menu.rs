use crate::{
    Screen, UIFocus, InputState, copy_session_logs_to_sd, render_background, render_ui_overlay, get_current_font, measure_text, text_with_config_color, text_disabled, FLASH_MESSAGE_DURATION, FONT_SIZE, MENU_PADDING, MENU_OPTION_HEIGHT, ShakeTarget, save, StorageMediaState, VideoPlayer,
    audio::SoundEffects,
    config::Config,
    types::{AnimationState, BackgroundState, BatteryInfo, MenuPosition},
    ui::text_with_color,
};
use macroquad::prelude::*;
use rodio::{buffer::SamplesBuffer, Sink};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    sync::atomic::Ordering,
};

pub const MAIN_MENU_OPTIONS: &[&str] = &["DATA", "PLAY", "BLADES", "COPY SESSION LOGS", "SETTINGS", "EXTRAS", "ABOUT"];
pub const MAIN_MENU_OPTIONS_NO_BLADES: &[&str] = &["DATA", "PLAY", "COPY SESSION LOGS", "SETTINGS", "EXTRAS", "ABOUT"];

pub fn update(
    current_screen: &mut Screen,
    main_menu_selection: &mut usize,
    play_option_enabled: &mut bool,
    copy_logs_option_enabled: &mut bool,
    cart_connected: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    input_state: &mut InputState,
    animation_state: &mut AnimationState,
    sound_effects: &SoundEffects,
    config: &Config,
    log_messages: &std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    storage_state: &Arc<Mutex<StorageMediaState>>,
    _fade_start_time: &mut Option<f64>,
    _current_bgm: &mut Option<Sink>,
    _music_cache: &HashMap<String, SamplesBuffer>,
    game_icon_queue: &mut Vec<(String, PathBuf)>,
    available_games: &mut Vec<(save::CartInfo, PathBuf)>,
    game_selection: &mut usize,
    flash_message: &mut Option<(String, f32)>,
    _game_process: &mut Option<std::process::Child>,
) {
    let menu_options = if config.blades_enabled { MAIN_MENU_OPTIONS } else { MAIN_MENU_OPTIONS_NO_BLADES };

    // Update play option enabled status based on cart connection
    *play_option_enabled = cart_connected.load(Ordering::Relaxed);

    // Update copy logs option enabled status based on cart connection
    *copy_logs_option_enabled = cart_connected.load(Ordering::Relaxed);

    // Handle main menu navigation
    if input_state.up {
        if *main_menu_selection == 0 {
            *main_menu_selection = menu_options.len() - 1;
        } else {
            *main_menu_selection = (*main_menu_selection - 1) % menu_options.len();
        }
        animation_state.trigger_transition(&config.cursor_transition_speed);
        sound_effects.play_cursor_move(&config);
    }
    if input_state.down {
        *main_menu_selection = (*main_menu_selection + 1) % menu_options.len();
        animation_state.trigger_transition(&config.cursor_transition_speed);
        sound_effects.play_cursor_move(&config);
    }
    if input_state.select {
        let selected_option = menu_options[*main_menu_selection];
        match selected_option {
            "DATA" => {
                // Trigger a refresh the next time the data screen is entered.
                if let Ok(mut state) = storage_state.lock() {
                    state.needs_memory_refresh = true;
                }

                *current_screen = Screen::SaveData;
                input_state.ui_focus = UIFocus::Grid;
                sound_effects.play_select(&config);
            },
            "PLAY" => {
                if *play_option_enabled {
                    sound_effects.play_select(&config);
                    log_messages.lock().unwrap().clear();

                    match save::find_all_game_files() {
                        Ok((game_paths, mut debug_log)) => {
                            log_messages.lock().unwrap().append(&mut debug_log);

                            let mut games: Vec<(save::CartInfo, PathBuf)> = Vec::new();
                            let parse_errors: Vec<String> = Vec::new();

                            for path in &game_paths {
                                // Handle .kzp vs .kzi parsing
                                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                    if ext == "kzi" {
                                        // Standard parsing for KZI
                                        if let Ok(info) = save::parse_kzi_file(path) {
                                            games.push((info, path.clone()));
                                        }
                                    } else if ext == "kzp" {
                                        // Logic for KZP (Compressed Package)
                                        let filename = path.file_stem().unwrap().to_string_lossy().to_string();
                                        let info = save::CartInfo {
                                            name: Some(filename.clone()),
                                            id: filename,
                                            exec: String::from("internal"),
                                            icon: String::from("icon.png"),
                                            runtime: Some(String::from("erofs")),
                                            ..Default::default()
                                        };
                                        games.push((info, path.clone()));
                                    }
                                }
                            }

                            match games.len() {
                                0 => {
                                    let mut logs = log_messages.lock().unwrap();
                                    logs.push(format!("[Info] Found {} potential game file(s), but none could be parsed.", game_paths.len()));
                                    logs.push("--- ERRORS ---".to_string());
                                    logs.extend(parse_errors);
                                    *current_screen = Screen::Debug;
                                },
                                1 => {
                                    *available_games = games;
                                    *game_selection = 0;
                                    *current_screen = Screen::GameSelection;
                                },
                                _ => {
                                    println!("[Debug] Found {} games. Switching to selection screen.", games.len());
                                    game_icon_queue.clear();
                                    for (cart_info, game_path) in &games {
                                        let is_package = game_path.extension().map_or(false, |e| e == "kzp");
                                        let icon_path = if is_package {
                                            let sidecar_png = game_path.with_extension("png");
                                            let sidecar_jpg = game_path.with_extension("jpg");

                                            if sidecar_png.exists() {
                                                sidecar_png
                                            } else if sidecar_jpg.exists() {
                                                sidecar_jpg
                                            } else {
                                                PathBuf::from("::KZP_PLACEHOLDER::")
                                            }
                                        } else {
                                            game_path.parent().unwrap().join(&cart_info.icon)
                                        };
                                        game_icon_queue.push((cart_info.id.clone(), icon_path));
                                    }
                                    *available_games = games;
                                    *game_selection = 0;
                                    *current_screen = Screen::GameSelection;
                                }
                            }
                        },
                        Err(e) => {
                            let error_msg = format!("[Error] Error scanning for cartridges: {}", e);
                            println!("[Error] {}", &error_msg);
                            log_messages.lock().unwrap().push(error_msg);
                            *current_screen = Screen::Debug;
                        }
                    }
                } else {
                    sound_effects.play_reject(&config);
                    animation_state.trigger_play_option_shake();
                }
            },
            "BLADES" => {
                *current_screen = Screen::BladesDashboard;
                sound_effects.play_select(config);
            },
            "COPY SESSION LOGS" => {
                if *copy_logs_option_enabled {
                    sound_effects.play_select(&config);
                    match copy_session_logs_to_sd() {
                        Ok(path) => {
                            *flash_message = Some((format!("SUCCESS: {}", path), FLASH_MESSAGE_DURATION));
                        }
                        Err(e) => {
                            *flash_message = Some((format!("ERROR: {}", e), FLASH_MESSAGE_DURATION));
                        }
                    }
                } else {
                    sound_effects.play_reject(&config);
                    animation_state.trigger_copy_log_option_shake();
                }
            },
            "SETTINGS" => {
                *current_screen = Screen::GeneralSettings;
                sound_effects.play_select(&config);
            },
            "EXTRAS" => {
                *current_screen = Screen::Extras;
                sound_effects.play_select(&config);
            },
            "ABOUT" => {
                *current_screen = Screen::About;
                sound_effects.play_select(&config);
            },
            _ => {}
        }
    }
}

pub fn draw(
    selected_option: usize,
    play_option_enabled: bool,
    copy_logs_option_enabled: bool,
    animation_state: &AnimationState,
    logo_cache: &HashMap<String, Texture2D>,
    background_cache: &HashMap<String, Texture2D>,
    font_cache: &HashMap<String, Font>,
    config: &Config,
    background_state: &mut BackgroundState,
    video_cache: &mut HashMap<String, VideoPlayer>,
    battery_info: &Option<BatteryInfo>,
    current_time_str: &str,
    gcc_adapter_poll_rate: &Option<u32>,
    scale_factor: f32,
    flash_message: Option<&str>,
) {
    let menu_options = if config.blades_enabled { MAIN_MENU_OPTIONS } else { MAIN_MENU_OPTIONS_NO_BLADES };
    render_background(background_cache, video_cache, config, background_state);
    render_ui_overlay(logo_cache, font_cache, config, battery_info, current_time_str, gcc_adapter_poll_rate, scale_factor);

    let font_size = (FONT_SIZE as f32 * scale_factor) as u16;
    let menu_padding = MENU_PADDING * scale_factor;
    let menu_option_height = MENU_OPTION_HEIGHT * scale_factor;
    let margin_x = 30.0 * scale_factor;
    let margin_y = 45.0 * scale_factor;

    let current_font = get_current_font(font_cache, config);

    let (start_x, start_y, is_centered) = match config.menu_position {
        MenuPosition::Center => (screen_width() / 2.0, (screen_height() * 0.3).max(margin_y), true),
        MenuPosition::TopLeft => (margin_x, margin_y, false),
        MenuPosition::TopRight => (screen_width() - margin_x, margin_y, false),
        MenuPosition::BottomLeft => (margin_x, screen_height() - (menu_options.len() as f32 * menu_option_height), false),
        MenuPosition::BottomRight => (screen_width() - margin_x, screen_height() - (menu_options.len() as f32 * menu_option_height), false),
    };

    for (i, &option) in menu_options.iter().enumerate() {
        let y_pos = start_y + (i as f32 * menu_option_height);
        let text_dims = measure_text(option, Some(current_font), font_size, 1.0);
        let mut x_pos = if is_centered {
            start_x - (text_dims.width / 2.0)
        } else if start_x > screen_width() / 2.0 {
            start_x - text_dims.width
        } else {
            start_x
        };

        if i == 1 && !play_option_enabled && i == selected_option {
            x_pos += animation_state.calculate_shake_offset(ShakeTarget::PlayOption);
        }
        let (is_selected, is_disabled) = (i == selected_option, match option {
            "PLAY" => !play_option_enabled,
            "COPY SESSION LOGS" => !copy_logs_option_enabled,
            _ => false,
        });

        if is_selected && config.cursor_style == "BOX" {
            let cursor_color = animation_state.get_cursor_color(config);
            let cursor_scale = animation_state.get_cursor_scale();
            let base_width = text_dims.width + (menu_padding * 2.0);
            let base_height = text_dims.height + (menu_padding * 2.0);
            let scaled_width = base_width * cursor_scale;
            let scaled_height = base_height * cursor_scale;
            let offset_x = (scaled_width - base_width) / 2.0;
            let offset_y = (scaled_height - base_height) / 2.0;
            let rect_x = x_pos - menu_padding;
            let rect_y = y_pos - text_dims.height - menu_padding;
            draw_rectangle_lines(rect_x - offset_x, rect_y - offset_y, scaled_width, scaled_height, 4.0 * scale_factor, cursor_color);
        }

        if is_selected && config.cursor_style == "TEXT" {
            let mut highlight_color = animation_state.get_cursor_color(config);
            if is_disabled {
                highlight_color.r *= 0.5;
                highlight_color.g *= 0.5;
                highlight_color.b *= 0.5;
                highlight_color.a = 1.0;
            }
            text_with_color(font_cache, config, option, x_pos, y_pos, font_size, highlight_color);
        } else if is_disabled {
            text_disabled(font_cache, config, option, x_pos, y_pos, font_size);
        } else {
            text_with_config_color(font_cache, config, option, x_pos, y_pos, font_size);
        }
    }

    if let Some(message) = flash_message {
        let font_size = (FONT_SIZE as f32 * scale_factor) as u16;
        let current_font = get_current_font(font_cache, config);
        let dims = measure_text(message, Some(current_font), font_size, 1.0);
        let x = screen_width() / 2.0 - dims.width / 2.0;
        let y = screen_height() - (60.0 * scale_factor);
        draw_rectangle(x - (10.0 * scale_factor), y - dims.height, dims.width + (20.0 * scale_factor), dims.height + (10.0 * scale_factor), Color::new(0.0, 0.0, 0.0, 0.7));
        text_with_config_color(font_cache, config, message, x, y, font_size);
    }
}
