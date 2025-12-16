use crate::{
    Screen, BladeType, BladesState, input::InputState,
    audio::SoundEffects, config::Config,
};
use macroquad::prelude::*;

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

// ===================================
// HELPER STRUCTS
// ===================================

struct BladeRenderInfo {
    x: f32,
    width: f32,
    skew: f32,
    alpha: f32,
    z_order: i32,
}

// ===================================
// UPDATE FUNCTION
// ===================================

pub fn update(
    blades_state: &mut BladesState,
    input_state: &mut InputState,
    sound_effects: &SoundEffects,
    config: &Config,
    current_screen: &mut Screen,
) {
    // Update animations
    blades_state.animation.update(get_frame_time());

    // LEFT/RIGHT: Switch between blades
    if input_state.left {
        if blades_state.current_blade > 0 {
            blades_state.current_blade -= 1;
            blades_state.animation.trigger_blade_transition(blades_state.current_blade);
            sound_effects.play_cursor_move(config);
        } else {
            sound_effects.play_reject(config);
        }
    }

    if input_state.right {
        if blades_state.current_blade < blades_state.blades.len() - 1 {
            blades_state.current_blade += 1;
            blades_state.animation.trigger_blade_transition(blades_state.current_blade);
            sound_effects.play_cursor_move(config);
        } else {
            sound_effects.play_reject(config);
        }
    }

    // UP/DOWN: Navigate tabs within current blade
    if input_state.up {
        if blades_state.blades[blades_state.current_blade].selected_tab > 0 {
            blades_state.blades[blades_state.current_blade].selected_tab -= 1;
            sound_effects.play_cursor_move(config);
        } else {
            sound_effects.play_reject(config);
        }
    }

    if input_state.down {
        let current_blade = &blades_state.blades[blades_state.current_blade];
        if current_blade.selected_tab < current_blade.tabs.len() - 1 {
            blades_state.blades[blades_state.current_blade].selected_tab += 1;
            sound_effects.play_cursor_move(config);
        } else {
            sound_effects.play_reject(config);
        }
    }

    // A button (SELECT): Activate tab content
    if input_state.select {
        handle_blade_selection(blades_state, current_screen, sound_effects, config);
    }

    // B button (BACK): Return to main menu
    if input_state.back {
        *current_screen = Screen::MainMenu;
        blades_state.enabled = false;
        sound_effects.play_back(config);
    }
}

fn handle_blade_selection(
    blades_state: &mut BladesState,
    current_screen: &mut Screen,
    sound_effects: &SoundEffects,
    config: &Config,
) {
    let current_blade = &blades_state.blades[blades_state.current_blade];

    match current_blade.blade_type {
        BladeType::GamesAndApps => {
            match current_blade.selected_tab {
                0 => {  // Library
                    *current_screen = Screen::GameSelection;
                    sound_effects.play_select(config);
                },
                1 => {  // Recently Played
                    // TODO: Implement recently played view
                    sound_effects.play_select(config);
                },
                2 => {  // Installed Apps
                    *current_screen = Screen::Extras;
                    sound_effects.play_select(config);
                },
                _ => {},
            }
        },
        BladeType::SystemSettings => {
            let settings_screen = match current_blade.selected_tab {
                0 => Screen::GeneralSettings,
                1 => Screen::AudioSettings,
                2 => Screen::GuiSettings,
                3 => Screen::Wifi,
                4 => Screen::AssetSettings,
                _ => return,
            };
            *current_screen = settings_screen;
            sound_effects.play_select(config);
        },
        BladeType::SaveDataAndMemory => {
            *current_screen = Screen::SaveData;
            sound_effects.play_select(config);
        },
    }
}

// ===================================
// DRAW FUNCTION
// ===================================

pub fn draw(blades_state: &BladesState, scale_factor: f32) {
    let screen_w = screen_width();
    let screen_h = screen_height();

    // Calculate blade dimensions
    let blade_width = screen_w * BLADE_WIDTH_RATIO;
    let overlap_amount = blade_width * BLADE_OVERLAP_RATIO;

    // Calculate positions for all visible blades
    let blade_positions = calculate_blade_positions(
        blades_state,
        blade_width,
        overlap_amount,
        screen_w,
    );

    // Draw blades from back to front (right to left)
    for i in (0..blades_state.blades.len()).rev() {
        if let Some(render_info) = blade_positions.get(&i) {
            draw_blade(
                &blades_state.blades[i],
                render_info,
                i == blades_state.current_blade,
                &blades_state.animation,
                screen_h,
                scale_factor,
            );
        }
    }

    // Draw instructions overlay
    let instruction_size = 14.0 * scale_factor;
    let instruction_y = screen_h - 80.0 * scale_factor;
    draw_text("LEFT/RIGHT: Switch Blades", 20.0 * scale_factor, instruction_y, instruction_size, LIGHTGRAY);
    draw_text("UP/DOWN: Select Tab", 20.0 * scale_factor, instruction_y + 25.0 * scale_factor, instruction_size, LIGHTGRAY);
    draw_text("A: Select | B: Back", 20.0 * scale_factor, instruction_y + 50.0 * scale_factor, instruction_size, LIGHTGRAY);
}

fn calculate_blade_positions(
    blades_state: &BladesState,
    blade_width: f32,
    overlap_amount: f32,
    screen_width: f32,
) -> std::collections::HashMap<usize, BladeRenderInfo> {
    use std::collections::HashMap;

    let mut positions = HashMap::new();
    let blade_spacing = blade_width - overlap_amount;

    // Calculate transition offset
    let transition_progress = blades_state.animation.blade_transition_progress;
    let transition_offset = (1.0 - transition_progress) * blade_spacing;

    // Base position: center the current blade
    let center_x = screen_width / 2.0;
    let base_x = center_x - (blade_width / 2.0);

    for i in 0..blades_state.blades.len() {
        let offset_from_current = i as f32 - blades_state.current_blade as f32;
        let target_x = base_x + (offset_from_current * blade_spacing) + transition_offset;

        // Calculate perspective skew (blades on the sides are more skewed)
        let distance_from_center = (i as f32 - blades_state.current_blade as f32).abs();
        let skew = BLADE_PERSPECTIVE_ANGLE * distance_from_center;

        // Calculate alpha based on position
        let alpha = if i == blades_state.current_blade {
            1.0
        } else if distance_from_center <= 1.0 {
            0.7
        } else {
            0.4
        };

        // Z-order: current blade is on top
        let z_order = if i == blades_state.current_blade {
            100
        } else if i < blades_state.current_blade {
            50 - (blades_state.current_blade - i) as i32
        } else {
            50 - (i - blades_state.current_blade) as i32
        };

        positions.insert(i, BladeRenderInfo {
            x: target_x,
            width: blade_width,
            skew,
            alpha,
            z_order,
        });
    }

    positions
}

fn draw_blade(
    blade: &crate::Blade,
    render_info: &BladeRenderInfo,
    is_current: bool,
    animation: &crate::BladesAnimationState,
    screen_height: f32,
    scale_factor: f32,
) {
    let x = render_info.x;
    let width = render_info.width;
    let height = screen_height;

    // Draw blade background with gradient
    let base_color = blade.gradient_color;
    let dark_color = Color::new(
        base_color.r * 0.3,
        base_color.g * 0.3,
        base_color.b * 0.3,
        base_color.a * render_info.alpha,
    );
    let bright_color = Color::new(
        base_color.r,
        base_color.g,
        base_color.b,
        base_color.a * render_info.alpha,
    );

    // Draw main blade rectangle with vertical gradient
    draw_rectangle_gradient(
        x,
        0.0,
        width,
        height,
        dark_color,
        bright_color,
    );

    // Draw glow on current blade
    if is_current {
        let glow_alpha = 0.3 + 0.2 * (animation.tab_highlight_time.sin() * 0.5 + 0.5);
        let glow_color = Color::new(1.0, 1.0, 1.0, glow_alpha);
        draw_rectangle(x - GLOW_THICKNESS, 0.0, GLOW_THICKNESS, height, glow_color);
        draw_rectangle(x + width, 0.0, GLOW_THICKNESS, height, glow_color);
    }

    // Draw blade title
    let title_size = 32.0 * scale_factor;
    let title_x = x + BLADE_CONTENT_PADDING * scale_factor;
    let title_y = 60.0 * scale_factor;
    draw_text(&blade.name, title_x, title_y, title_size, WHITE);

    // Draw separator line
    let sep_y = title_y + 20.0 * scale_factor;
    draw_line(
        title_x,
        sep_y,
        x + width - BLADE_CONTENT_PADDING * scale_factor,
        sep_y,
        2.0,
        Color::new(1.0, 1.0, 1.0, 0.5 * render_info.alpha),
    );

    // Draw tabs
    let tab_start_y = sep_y + 40.0 * scale_factor;
    draw_blade_tabs(
        blade,
        is_current,
        animation,
        title_x,
        tab_start_y,
        scale_factor,
        render_info.alpha,
    );
}

fn draw_blade_tabs(
    blade: &crate::Blade,
    is_current: bool,
    animation: &crate::BladesAnimationState,
    start_x: f32,
    start_y: f32,
    scale_factor: f32,
    base_alpha: f32,
) {
    let tab_height = TAB_HEIGHT * scale_factor;
    let tab_padding = TAB_PADDING * scale_factor;

    for (i, tab) in blade.tabs.iter().enumerate() {
        let y = start_y + (i as f32 * (tab_height + tab_padding));
        let is_selected = is_current && i == blade.selected_tab;

        // Calculate highlight alpha
        let highlight_alpha = if is_selected {
            0.6 + 0.4 * (animation.tab_highlight_time.sin() * 0.5 + 0.5)
        } else {
            0.0
        };

        // Draw tab background highlight
        if is_selected {
            let highlight_color = Color::new(1.0, 1.0, 1.0, highlight_alpha * base_alpha);
            draw_rectangle(
                start_x - 10.0 * scale_factor,
                y - 5.0 * scale_factor,
                300.0 * scale_factor,
                tab_height,
                highlight_color,
            );
        }

        // Draw tab text
        let tab_size = 20.0 * scale_factor;
        let text_color = if is_selected {
            Color::new(1.0, 1.0, 0.0, base_alpha) // Yellow for selected
        } else {
            Color::new(0.9, 0.9, 0.9, base_alpha * 0.7) // Light gray for others
        };

        let prefix = if is_selected { "▶ " } else { "  " };
        draw_text(&format!("{}{}", prefix, tab.name), start_x, y + tab_height / 2.0, tab_size, text_color);
    }
}

fn draw_rectangle_gradient(x: f32, y: f32, w: f32, h: f32, top_color: Color, bottom_color: Color) {
    // Draw gradient using thin horizontal strips
    let strips = 50;
    let strip_height = h / strips as f32;

    for i in 0..strips {
        let t = i as f32 / strips as f32;
        let color = Color::new(
            top_color.r + (bottom_color.r - top_color.r) * t,
            top_color.g + (bottom_color.g - top_color.g) * t,
            top_color.b + (bottom_color.b - top_color.b) * t,
            top_color.a + (bottom_color.a - top_color.a) * t,
        );
        draw_rectangle(x, y + i as f32 * strip_height, w, strip_height, color);
    }
}
