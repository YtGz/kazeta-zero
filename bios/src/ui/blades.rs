use crate::audio::SoundEffects;
use crate::config::Config;
use crate::input::InputState;
use crate::save;
use crate::types::{Blade, BladeTab, BladeType, Screen};
use crate::ui::get_current_font;

use macroquad::prelude::*;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::path::PathBuf;

// ===================================
// CONSTANTS
// ===================================
const BLADE_WIDTH_RATIO: f32 = 0.35;
const BLADE_OVERLAP_RATIO: f32 = 0.20;
const BLADE_PERSPECTIVE_ANGLE: f32 = 5.0;
const TAB_HEIGHT: f32 = 40.0;
const TAB_PADDING: f32 = 20.0;
const BLADE_CONTENT_PADDING: f32 = 30.0;
const GLOW_THICKNESS: f32 = 3.0;
const BLADE_TRANSITION_DURATION: f32 = 0.3;

// ===================================
// DATA STRUCTURES
// ===================================

pub enum BladeAction {
    None,
    LaunchGame((save::CartInfo, PathBuf)),
    GoToScreen(Screen),
}

pub struct BladesAnimationState {
    pub horizontal_scroll_time: f32,
    pub source_blade: usize,
    pub target_blade: usize,
    pub blade_transition_progress: f32,
    pub tab_highlight_time: f32,
    pub blade_fade_alpha: f32,
}

pub struct BladesState {
    pub blades: Vec<Blade>,
    pub current_blade: usize,
    pub animation: BladesAnimationState,
    pub enabled: bool,
    pub games_list: Vec<(save::CartInfo, PathBuf)>,
    pub game_icon_cache: HashMap<String, Texture2D>,
    pub game_list_selection: usize,
}

struct BladeRenderInfo {
    x: f32,
    width: f32,
    alpha: f32,
}

// ===================================
// IMPLEMENTATIONS
// ===================================

impl BladesAnimationState {
    const TAB_GLOW_SPEED: f32 = 3.0;

    pub fn new() -> Self {
        BladesAnimationState {
            horizontal_scroll_time: 0.0,
            source_blade: 0,
            target_blade: 0,
            blade_transition_progress: 1.0,
            tab_highlight_time: 0.0,
            blade_fade_alpha: 1.0,
        }
    }

    pub fn trigger_blade_transition(&mut self, source: usize, target: usize) {
        self.source_blade = source;
        self.target_blade = target;
        self.horizontal_scroll_time = BLADE_TRANSITION_DURATION;
        self.blade_transition_progress = 0.0;
    }

    pub fn update(&mut self, delta_time: f32) {
        if self.horizontal_scroll_time > 0.0 {
            self.horizontal_scroll_time = (self.horizontal_scroll_time - delta_time).max(0.0);
            self.blade_transition_progress =
                1.0 - (self.horizontal_scroll_time / BLADE_TRANSITION_DURATION);
        }
        self.tab_highlight_time =
            (self.tab_highlight_time + delta_time * Self::TAB_GLOW_SPEED) % (2.0 * PI);
    }

    pub fn get_tab_glow_alpha(&self) -> f32 {
        0.5 + (self.tab_highlight_time.sin() * 0.25) + 0.25
    }

    pub fn get_eased_progress(&self) -> f32 {
        let t = self.blade_transition_progress;
        if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
    }
}

impl BladesState {
    pub fn new() -> Self {
        let mut blades = Vec::new();
        blades.push(Blade {
            blade_type: BladeType::GamesAndApps,
            name: "GAMES & APPS".to_string(),
            tabs: vec![
                BladeTab { name: "LIBRARY".to_string(), icon: None },
                BladeTab { name: "RECENTLY PLAYED".to_string(), icon: None },
                BladeTab { name: "INSTALLED APPS".to_string(), icon: None },
            ],
            selected_tab: 0, scroll_offset: 0, gradient_color: WHITE,
        });
        blades.push(Blade {
            blade_type: BladeType::SystemSettings,
            name: "SYSTEM SETTINGS".to_string(),
            tabs: vec![
                BladeTab { name: "GENERAL".to_string(), icon: None },
                BladeTab { name: "AUDIO".to_string(), icon: None },
                BladeTab { name: "GUI".to_string(), icon: None },
                BladeTab { name: "NETWORK".to_string(), icon: None },
                BladeTab { name: "ASSETS".to_string(), icon: None },
            ],
            selected_tab: 0, scroll_offset: 0, gradient_color: WHITE,
        });
        blades.push(Blade {
            blade_type: BladeType::SaveDataAndMemory,
            name: "SAVE DATA & MEMORY".to_string(),
            tabs: vec![
                BladeTab { name: "INTERNAL STORAGE".to_string(), icon: None },
                BladeTab { name: "EXTERNAL STORAGE".to_string(), icon: None },
                BladeTab { name: "MANAGE SAVES".to_string(), icon: None },
            ],
            selected_tab: 0, scroll_offset: 0, gradient_color: WHITE,
        });

        BladesState {
            blades,
            current_blade: 0,
            animation: BladesAnimationState::new(),
            enabled: false,
            games_list: Vec::new(),
            game_icon_cache: HashMap::new(),
            game_list_selection: 0,
        }
    }
}

// ===================================
// UPDATE & DRAW
// ===================================

pub fn update(
    blades_state: &mut BladesState,
    input_state: &mut InputState,
    sound_effects: &SoundEffects,
    config: &Config,
) -> BladeAction {
    if !blades_state.enabled {
        blades_state.enabled = true;
        if let Ok((game_paths, _)) = save::find_all_game_files() {
            let mut games: Vec<(save::CartInfo, PathBuf)> = Vec::new();
            for path in &game_paths {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "kzi" {
                        if let Ok(info) = save::parse_kzi_file(path) {
                            games.push((info, path.clone()));
                        }
                    } else if ext == "kzp" {
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
            blades_state.games_list = games;
        }
    }

    blades_state.animation.update(get_frame_time());

    let num_blades = blades_state.blades.len();
    if input_state.right && blades_state.current_blade < num_blades - 1 {
        let source = blades_state.current_blade;
        blades_state.current_blade += 1;
        blades_state.animation.trigger_blade_transition(source, blades_state.current_blade);
        sound_effects.play_cursor_move(config);
    }
    if input_state.left && blades_state.current_blade > 0 {
        let source = blades_state.current_blade;
        blades_state.current_blade -= 1;
        blades_state.animation.trigger_blade_transition(source, blades_state.current_blade);
        sound_effects.play_cursor_move(config);
    }

    let current_blade = &mut blades_state.blades[blades_state.current_blade];
    
    let mut switched_tabs = false;
    if input_state.up {
        if current_blade.blade_type == BladeType::GamesAndApps && current_blade.selected_tab == 0 {
            if blades_state.game_list_selection > 0 {
                blades_state.game_list_selection -= 1;
                sound_effects.play_cursor_move(config);
            }
        } else if current_blade.selected_tab > 0 {
            current_blade.selected_tab -= 1;
            switched_tabs = true;
        }
    }
    if input_state.down {
        if current_blade.blade_type == BladeType::GamesAndApps && current_blade.selected_tab == 0 {
            if blades_state.game_list_selection < blades_state.games_list.len() - 1 {
                blades_state.game_list_selection += 1;
                sound_effects.play_cursor_move(config);
            }
        } else if current_blade.selected_tab < current_blade.tabs.len() - 1 {
            current_blade.selected_tab += 1;
            switched_tabs = true;
        }
    }
    if switched_tabs {
        sound_effects.play_cursor_move(config);
        blades_state.game_list_selection = 0;
    }

    if input_state.select {
        sound_effects.play_select(config);
        match current_blade.blade_type {
            BladeType::GamesAndApps => {
                if current_blade.selected_tab == 0 { // Library
                    if let Some(game) = blades_state.games_list.get(blades_state.game_list_selection) {
                        return BladeAction::LaunchGame(game.clone());
                    }
                }
            }
            BladeType::SystemSettings => {
                let screen = match current_blade.selected_tab {
                    0 => Screen::GeneralSettings,
                    1 => Screen::AudioSettings,
                    2 => Screen::GuiSettings,
                    3 => Screen::Wifi,
                    4 => Screen::AssetSettings,
                    _ => Screen::BladesDashboard, // Should not happen
                };
                return BladeAction::GoToScreen(screen);
            }
            BladeType::SaveDataAndMemory => {
                return BladeAction::GoToScreen(Screen::SaveData);
            }
        }
    }

    if input_state.back {
        sound_effects.play_back(config);
        blades_state.enabled = false;
        return BladeAction::GoToScreen(Screen::MainMenu);
    }
    
    BladeAction::None
}

pub fn draw(blades_state: &BladesState, font_cache: &HashMap<String, Font>, config: &Config, _frame_t: f64) {
    clear_background(BLACK);

    let scale_factor = screen_height() / 360.0;

    let blade_indices: Vec<usize> = (0..blades_state.blades.len()).collect();
    let mut blade_render_infos: Vec<_> = blade_indices.iter().map(|&i| {
        let info = calculate_blade_offset(i, &blades_state.animation, scale_factor, blades_state.blades.len());
        (i, info)
    }).collect();

    blade_render_infos.sort_by(|(a_idx, _), (b_idx, _)| {
        let dist_a = (*a_idx as i32 - blades_state.current_blade as i32).abs();
        let dist_b = (*b_idx as i32 - blades_state.current_blade as i32).abs();
        dist_b.cmp(&dist_a)
    });

    for (i, render_info) in blade_render_infos {
        render_blade(&blades_state.blades[i], blades_state, &render_info, &blades_state.animation, font_cache, config, scale_factor);
    }
}

// ===================================
// RENDER HELPERS
// ===================================

fn calculate_blade_offset(
    blade_index: usize,
    animation: &BladesAnimationState,
    scale_factor: f32,
    num_blades: usize,
) -> BladeRenderInfo {
    let screen_center = screen_width() / 2.0;
    let blade_width = screen_width() * BLADE_WIDTH_RATIO * scale_factor;
    let overlap_width = blade_width * BLADE_OVERLAP_RATIO;
    
    let eased_progress = animation.get_eased_progress();
    let effective_blade_pos = lerp(animation.source_blade as f32, animation.target_blade as f32, eased_progress);
    
    let position_delta = blade_index as f32 - effective_blade_pos;
    
    let base_x = screen_center - (blade_width / 2.0) + (position_delta * (blade_width - overlap_width));
    
    let alpha = 1.0 - (position_delta.abs() / (num_blades as f32 / 2.0)).powf(2.0);

    BladeRenderInfo { x: base_x, width: blade_width, alpha }
}

use crate::utils::string_to_color;

fn render_blade(blade: &Blade, blades_state: &BladesState, render_info: &BladeRenderInfo, animation: &BladesAnimationState, font_cache: &HashMap<String, Font>, config: &Config, scale_factor: f32) {
    // Metallic base with per-blade accent strip (pulled from config colors)
    let accent_color = match blade.blade_type {
        BladeType::GamesAndApps => string_to_color(&config.blade_games_color),
        BladeType::SystemSettings => string_to_color(&config.blade_settings_color),
        BladeType::SaveDataAndMemory => string_to_color(&config.blade_saves_color),
    };
    let mut base_top = Color::new(0.38, 0.38, 0.40, render_info.alpha * 0.95);
    let mut base_bottom = Color::new(0.22, 0.22, 0.24, render_info.alpha * 0.95);
    base_top.a *= config.blade_transparency;
    base_bottom.a *= config.blade_transparency;

    let is_active = blades_state.current_blade == blades_state.blades.iter().position(|b| b.blade_type == blade.blade_type).unwrap();
    draw_blade_panel(
        render_info.x,
        render_info.width,
        screen_height(),
        base_top,
        base_bottom,
        scale_factor,
        is_active,
        accent_color,
    );
    
    render_blade_tabs(blade, render_info, animation, font_cache, config, scale_factor, accent_color);
    render_blade_title(blade, render_info, font_cache, config, scale_factor, accent_color);

    if is_active {
        if blade.blade_type == BladeType::GamesAndApps && blade.selected_tab == 0 {
            // Only show the game library on the Games & Apps blade, Library tab
            render_games_blade_content(blades_state, render_info, font_cache, config, scale_factor, accent_color);
        }
    }
}

fn render_blade_tabs(blade: &Blade, render_info: &BladeRenderInfo, animation: &BladesAnimationState, font_cache: &HashMap<String, Font>, config: &Config, scale_factor: f32, _accent: Color) {
    let font = get_current_font(font_cache, config);
    let font_size = (20.0 * scale_factor) as u16;
    let mut y_pos = 110.0 * scale_factor;

    for (i, tab) in blade.tabs.iter().enumerate() {
        let is_selected = i == blade.selected_tab;
        let text_color = if is_selected { WHITE } else { GRAY };

        draw_text_ex(
            &tab.name,
            render_info.x + (TAB_PADDING * scale_factor),
            y_pos,
            TextParams { font: Some(font), font_size, color: text_color, ..Default::default() }
        );
        
        if is_selected {
            let glow_alpha = animation.get_tab_glow_alpha();
            let mut glow_color = WHITE;
            glow_color.a = glow_alpha;
            draw_line(
                render_info.x, y_pos + (5.0 * scale_factor),
                render_info.x + render_info.width, y_pos + (5.0 * scale_factor),
                GLOW_THICKNESS * scale_factor,
                glow_color
            );
        }
        y_pos += TAB_HEIGHT * scale_factor;
    }
}

fn render_games_blade_content(blades_state: &BladesState, render_info: &BladeRenderInfo, font_cache: &HashMap<String, Font>, config: &Config, scale_factor: f32, accent: Color) {
    let font = get_current_font(font_cache, config);
    let font_size = (18.0 * scale_factor) as u16;
    let row_height = 34.0 * scale_factor;
    let container_x = render_info.x + (18.0 * scale_factor);
    let container_w = render_info.width - (36.0 * scale_factor);
    let container_y = 120.0 * scale_factor;
    let container_h = screen_height() - container_y - (40.0 * scale_factor);
    let header_h = 36.0 * scale_factor;
    let content_left = container_x + (14.0 * scale_factor);
    let row_width = container_w - (28.0 * scale_factor);

    let panel_bg = Color::new(0.08, 0.08, 0.1, render_info.alpha * 0.85);
    let header_bg = Color::new(0.12, 0.12, 0.14, render_info.alpha * 0.95);
    let mut accent_line = accent;
    accent_line.a = render_info.alpha * 0.9;
    let base_bg = Color::new(0.0, 0.0, 0.0, 0.22 * render_info.alpha);

    // Container
    draw_rectangle(container_x, container_y, container_w, container_h, panel_bg);
    draw_rectangle(container_x, container_y, container_w, header_h, header_bg);
    draw_rectangle(container_x, container_y + header_h - (3.0 * scale_factor), container_w, 3.0 * scale_factor, accent_line);

    // Header title
    let header_label = "Game Library";
    let label_dims = measure_text(header_label, Some(font), font_size, 1.0);
    draw_text_ex(
        header_label,
        container_x + (12.0 * scale_factor),
        container_y + header_h / 2.0 + label_dims.height / 2.5,
        TextParams { font: Some(font), font_size, color: WHITE, ..Default::default() }
    );

    let y_pos = container_y + header_h + (8.0 * scale_factor);

    if blades_state.games_list.is_empty() {
        let message = "No games detected";
        let dims = measure_text(message, Some(font), font_size, 1.0);
        draw_text_ex(
            message,
            container_x + (12.0 * scale_factor),
            y_pos + dims.height,
            TextParams { font: Some(font), font_size, color: GRAY, ..Default::default() },
        );
        return;
    }

    for (i, (cart_info, _)) in blades_state.games_list.iter().enumerate() {
        let row_y = y_pos + (i as f32 * row_height);
        let is_selected = i == blades_state.game_list_selection;

        // Row background
        let mut bg = base_bg;
        if is_selected {
            bg.a = 0.45 * render_info.alpha;
        }
        draw_rectangle(content_left, row_y, row_width, row_height - (6.0 * scale_factor), bg);

        // Accent strip for selection
        if is_selected {
            let strip_width = 4.0 * scale_factor;
            draw_rectangle(content_left, row_y, strip_width, row_height - (6.0 * scale_factor), accent);
        }

        let text_color = if is_selected { WHITE } else { GRAY };
        let game_name = cart_info.name.as_deref().unwrap_or("Unknown Game");
        let text_y = row_y + row_height / 2.0 + (font_size as f32 * 0.35);

        draw_text_ex(
            game_name,
            content_left + (12.0 * scale_factor),
            text_y,
            TextParams { font: Some(font), font_size, color: text_color, ..Default::default() }
        );
    }
}

fn draw_blade_panel(x: f32, width: f32, height: f32, top: Color, bottom: Color, scale_factor: f32, is_active: bool, accent_color: Color) {
    // Flat metallic panel with subtle vertical gradient, no curvature for now.
    let step = (3.0 * scale_factor).max(1.0);
    let accent_strip = 8.0 * scale_factor;
    let rim_color = Color::new(1.0, 1.0, 1.0, 0.12 * bottom.a);

    let mut y = 0.0;
    while y < height {
        let t = y / height;
        let r = top.r + t * (bottom.r - top.r);
        let g = top.g + t * (bottom.g - top.g);
        let b = top.b + t * (bottom.b - top.b);
        let a = top.a + t * (bottom.a - top.a);
        draw_rectangle(x, y, width, step, Color::new(r, g, b, a));
        y += step;
    }

    // Accent strip along the left edge to differentiate blades
    draw_rectangle(
        x,
        0.0,
        accent_strip,
        height,
        Color::new(accent_color.r, accent_color.g, accent_color.b, bottom.a * 0.8),
    );

    // Highlight rim on active blade
    if is_active {
        draw_rectangle_lines(
            x,
            0.0,
            width,
            height,
            3.0 * scale_factor,
            rim_color,
        );
    }
}

fn render_blade_title(blade: &Blade, render_info: &BladeRenderInfo, font_cache: &HashMap<String, Font>, config: &Config, scale_factor: f32, accent: Color) {
    let font = get_current_font(font_cache, config);
    let font_size = (28.0 * scale_factor) as u16;
    let title = &blade.name;
    let dims = measure_text(title, Some(font), font_size, 1.0);
    let tab_height = 38.0 * scale_factor;
    let tab_x = render_info.x + (18.0 * scale_factor);
    let tab_y = 40.0 * scale_factor;
    let tab_w = render_info.width - (36.0 * scale_factor);

    // Tab background (darker gray) with accent bottom border
    draw_rectangle(
        tab_x,
        tab_y,
        tab_w,
        tab_height,
        Color::new(0.12, 0.12, 0.14, render_info.alpha * 0.9),
    );
    draw_rectangle(
        tab_x,
        tab_y + tab_height - (3.0 * scale_factor),
        tab_w,
        3.0 * scale_factor,
        Color::new(accent.r, accent.g, accent.b, render_info.alpha * 0.9),
    );

    let x = tab_x + (16.0 * scale_factor);
    let y = tab_y + tab_height / 2.0 + dims.height / 2.5;

    let mut title_color = WHITE;
    title_color.a = render_info.alpha;
    draw_text_ex(
        title,
        x,
        y,
        TextParams { font: Some(font), font_size, color: title_color, ..Default::default() }
    );
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start * (1.0 - t) + end * t
}
