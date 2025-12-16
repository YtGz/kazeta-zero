# Xbox 360 Blades Dashboard UI Implementation Plan

## Overview

Add an authentic Xbox 360 Blades-style Dashboard UI to Kazeta-plus BIOS as an alternative interface mode. Features horizontal blade navigation, smooth animations, and full theme customization.

## User Requirements

- **Integration:** Alternative dashboard mode accessible from main menu (not replacing it)
- **Blades:** 3 blades - Games & Apps, System Settings, Save Data & Memory
- **Visual Style:** Authentic Xbox 360 replica (iconic green gradients, blade shapes)
- **Theming:** Video backgrounds, custom blade colors, custom fonts/logos, blade transparency/blur

## Architecture Context

**Current System:**
- Rust BIOS built on macroquad framework
- Screen-based navigation (Screen enum in `types.rs`)
- Main rendering loop in `main.rs` matches on `current_screen`
- Each screen has `update()` and `draw()` functions in `ui/` modules
- Sophisticated AnimationState system with easing functions
- Theme system with colors, fonts, backgrounds, music
- Video background support via FFmpeg

---

## 1. Data Structures

### New Types (add to `/bios/src/types.rs`)

```rust
// Screen variant for Blades UI
pub enum Screen {
    // ... existing variants ...
    BladesDashboard,  // NEW
}

// Blade identification
#[derive(Clone, Debug, PartialEq)]
pub enum BladeType {
    GamesAndApps,
    SystemSettings,
    SaveDataAndMemory,
}

// Tab within a blade
#[derive(Clone, Debug)]
pub struct BladeTab {
    pub name: String,
    pub icon: Option<String>,
}

// Complete blade definition
#[derive(Clone, Debug)]
pub struct Blade {
    pub blade_type: BladeType,
    pub name: String,
    pub tabs: Vec<BladeTab>,
    pub selected_tab: usize,
    pub scroll_offset: usize,
    pub gradient_color: Color,  // Themeable
}

// Animation state for blade transitions
pub struct BladesAnimationState {
    pub horizontal_scroll_time: f32,
    pub target_blade: usize,
    pub blade_transition_progress: f32,
    pub tab_highlight_time: f32,
    pub blade_fade_alpha: f32,
}

// Master state for Blades UI
pub struct BladesState {
    pub blades: Vec<Blade>,
    pub current_blade: usize,
    pub animation: BladesAnimationState,
    pub enabled: bool,
    pub games_list: Vec<(save::CartInfo, PathBuf)>,
    pub game_icon_cache: HashMap<String, Texture2D>,
}
```

### Constants

```rust
const BLADE_WIDTH_RATIO: f32 = 0.35;        // 35% of screen width
const BLADE_OVERLAP_RATIO: f32 = 0.20;      // 20% overlap when stacked
const BLADE_PERSPECTIVE_ANGLE: f32 = 5.0;   // Degrees of tilt
const TAB_HEIGHT: f32 = 40.0;
const TAB_PADDING: f32 = 20.0;
const BLADE_CONTENT_PADDING: f32 = 30.0;
const GLOW_THICKNESS: f32 = 3.0;
const BLADE_TRANSITION_DURATION: f32 = 0.3; // Smooth scrolling
```

---

## 2. Xbox 360 Visual Design

### Authentic Characteristics

- **Blade Width:** 35% of screen width when centered
- **Blade Overlap:** Stack with 20% overlap showing adjacent blades
- **Perspective Effect:** Subtle 3D tilt for non-centered blades
- **Gradient Pattern:** Vertical gradients (darker at edges, lighter in middle)
- **Tab Layout:** Horizontal tabs at top of each blade
- **Glow Effect:** Selected tabs have white/bright pulsing glow
- **Iconic Colors:**
  - Games: `#00CC44` (Xbox green)
  - Settings: `#CC6600` (Orange)
  - Saves: `#6600CC` (Purple)
  - All customizable via themes

### Rendering Layers

1. **Background:** Existing video/image background (unchanged)
2. **Blades:** 3-5 blades rendered horizontally (back to front)
3. **Active Blade Content:** Content of centered blade
4. **UI Overlay:** Clock, battery, logo (unchanged)

---

## 3. Blade Content Design

### Blade 1: Games & Apps

**Tabs:**
- **Library:** Grid view of all games (reuse GameSelection screen components)
- **Recently Played:** Horizontal list of last 10 games with playtime stats
- **Installed Apps:** List of extras (CD Player, Wi-Fi, Bluetooth, etc.)

**Actions:**
- Select game to launch
- View game details
- Access extras menu

### Blade 2: System Settings

**Tabs:** Map directly to existing settings screens
- **General** → Screen::GeneralSettings
- **Audio** → Screen::AudioSettings
- **GUI** → Screen::GuiSettings
- **Network** → Screen::Wifi/Bluetooth
- **Assets** → Screen::AssetSettings

**Actions:**
- Navigate to full settings screen
- Return to Blades after configuration

### Blade 3: Save Data & Memory

**Tabs:**
- **Internal Storage:** Filter save data by internal drive
- **External Storage:** Filter by external drives
- **Manage Saves:** Full save data management

**Actions:**
- View/copy/delete saves
- Navigate to full SaveData screen
- Return to Blades

---

## 4. Rendering Implementation

### Blade Positioning Algorithm

```rust
fn calculate_blade_offset(
    blade_index: usize,
    current_blade: usize,
    animation: &BladesAnimationState,
    scale_factor: f32,
) -> BladeRenderInfo {
    let screen_center = screen_width() / 2.0;
    let blade_width = screen_width() * BLADE_WIDTH_RATIO * scale_factor;
    let overlap_width = blade_width * BLADE_OVERLAP_RATIO;

    // Calculate position relative to current blade
    let position_delta = (blade_index as i32) - (current_blade as i32);

    // Base X position
    let base_x = screen_center - (blade_width / 2.0) +
                 (position_delta as f32 * (blade_width - overlap_width));

    // Apply smooth transition animation
    let eased_progress = animation.get_eased_progress();
    let animated_x = if animation.horizontal_scroll_time > 0.0 {
        // Interpolate between old and new position
        lerp(old_x, base_x, eased_progress)
    } else {
        base_x
    };

    // Perspective skew for depth
    let perspective_skew = if position_delta == 0 {
        0.0
    } else {
        BLADE_PERSPECTIVE_ANGLE * position_delta.signum() as f32
    };

    // Fade distant blades
    let alpha = if position_delta.abs() > 1 { 0.3 } else { 1.0 };

    BladeRenderInfo { x: animated_x, width: blade_width, skew: perspective_skew, alpha, ... }
}
```

### Blade Rendering

```rust
fn render_blade(blade: &Blade, render_info: &BladeRenderInfo, ...) {
    // 1. Draw vertical gradient background
    draw_vertical_gradient_rect(
        render_info.x,
        0.0,
        render_info.width,
        screen_height(),
        gradient_start,
        gradient_end,
        render_info.alpha,
    );

    // 2. Draw beveled edge highlights (Xbox 360 style)
    draw_line(/* edge highlight */);

    // 3. Draw tabs with glow effect
    render_blade_tabs(blade, render_info, ...);
}

fn render_blade_tabs(blade: &Blade, ...) {
    for (i, tab) in blade.tabs.iter().enumerate() {
        let is_selected = i == blade.selected_tab;

        // Draw pulsing glow underline for selected tab
        if is_selected {
            let glow_alpha = animation.get_tab_glow_alpha();
            draw_line(/* underline with pulsing glow */);
        }

        // Draw tab text (white if selected, gray otherwise)
        draw_text_ex(&tab.name, ...);
    }
}
```

---

## 5. Navigation & Input

### Input Mapping

```rust
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
    if input_state.left && blades_state.current_blade > 0 {
        blades_state.current_blade -= 1;
        blades_state.animation.trigger_blade_transition(blades_state.current_blade);
        sound_effects.play_cursor_move(config);
    }

    if input_state.right && blades_state.current_blade < blades_state.blades.len() - 1 {
        blades_state.current_blade += 1;
        blades_state.animation.trigger_blade_transition(blades_state.current_blade);
        sound_effects.play_cursor_move(config);
    }

    // UP/DOWN: Navigate tabs within current blade
    if input_state.up { /* previous tab */ }
    if input_state.down { /* next tab */ }

    // A button: Activate selected tab content
    if input_state.select {
        handle_blade_selection(blades_state, current_screen, ...);
    }

    // B button: Return to main menu
    if input_state.back {
        *current_screen = Screen::MainMenu;
        blades_state.enabled = false;
        sound_effects.play_back(config);
    }
}
```

### Selection Handling

```rust
fn handle_blade_selection(blades_state: &mut BladesState, current_screen: &mut Screen, ...) {
    let current_blade = &blades_state.blades[blades_state.current_blade];

    match current_blade.blade_type {
        BladeType::GamesAndApps => {
            // Launch selected game or navigate to game grid
        },
        BladeType::SystemSettings => {
            // Navigate to appropriate settings screen
            let settings_screen = match current_blade.selected_tab {
                0 => Screen::GeneralSettings,
                1 => Screen::AudioSettings,
                2 => Screen::GuiSettings,
                3 => Screen::Wifi,
                4 => Screen::AssetSettings,
                _ => return,
            };
            *current_screen = settings_screen;
        },
        BladeType::SaveDataAndMemory => {
            *current_screen = Screen::SaveData;
        },
    }
}
```

---

## 6. Animation System

### Blade Transition Animation

```rust
impl BladesAnimationState {
    pub fn trigger_blade_transition(&mut self, target: usize) {
        self.target_blade = target;
        self.horizontal_scroll_time = Self::BLADE_TRANSITION_DURATION;
        self.blade_transition_progress = 0.0;
    }

    pub fn update(&mut self, delta_time: f32) {
        // Update horizontal scrolling
        if self.horizontal_scroll_time > 0.0 {
            self.horizontal_scroll_time = (self.horizontal_scroll_time - delta_time).max(0.0);
            self.blade_transition_progress = 1.0 - (self.horizontal_scroll_time / Self::BLADE_TRANSITION_DURATION);
        }

        // Update tab glow (pulsing sine wave)
        self.tab_highlight_time = (self.tab_highlight_time + delta_time * 3.0) % (2.0 * PI);
    }

    pub fn get_eased_progress(&self) -> f32 {
        let t = self.blade_transition_progress;
        // Cubic ease-in-out (smooth like Xbox 360)
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    }

    pub fn get_tab_glow_alpha(&self) -> f32 {
        // Pulsing between 0.5 and 1.0
        0.5 + (self.tab_highlight_time.sin() * 0.25) + 0.25
    }
}
```

---

## 7. Theme Integration

### Config Extensions (add to `/bios/src/config.rs`)

```rust
pub struct Config {
    // ... existing fields ...

    // Blades-specific settings
    pub blades_enabled: bool,
    pub blade_games_color: String,      // Hex: "#00CC44"
    pub blade_settings_color: String,   // Hex: "#CC6600"
    pub blade_saves_color: String,      // Hex: "#6600CC"
    pub blade_transparency: f32,        // 0.0 to 1.0
    pub blade_blur_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            blades_enabled: false,
            blade_games_color: "#00CC44".to_string(),
            blade_settings_color: "#CC6600".to_string(),
            blade_saves_color: "#6600CC".to_string(),
            blade_transparency: 0.95,
            blade_blur_enabled: false,
        }
    }
}
```

### Theme.toml Support

```toml
[blades]
enabled = true
games_color = "#00CC44"      # Xbox green
settings_color = "#CC6600"   # Orange
saves_color = "#6600CC"      # Purple
transparency = 0.95
blur_enabled = false
```

---

## 8. Implementation Sequence

### Phase 1: Core Infrastructure (Week 1)

**Tasks:**
- [ ] Add blade types and enums to `/bios/src/types.rs`
  - BladeType, BladeTab, Blade, BladesAnimationState, BladesState
  - Screen::BladesDashboard variant
- [ ] Create `/bios/src/ui/blades.rs` module
  - BladesState::new() initialization
  - BladesAnimationState with update() and easing
  - Placeholder update() and draw() functions
- [ ] Modify `/bios/src/ui/mod.rs`
  - Add `pub mod blades;` declaration
- [ ] Modify `/bios/src/main.rs`
  - Initialize BladesState in main
  - Add Screen::BladesDashboard match arm
  - Add "Blades Dashboard" option to main menu
- [ ] Test: Can enter/exit Blades mode

### Phase 2: Visual Rendering (Week 2)

**Tasks:**
- [ ] Implement blade positioning in `/bios/src/ui/blades.rs`
  - calculate_blade_offset() function
  - BladeRenderInfo struct
- [ ] Implement blade rendering
  - render_blade() with vertical gradients
  - render_blade_tabs() with glow effects
  - Helper: draw_vertical_gradient_rect()
- [ ] Apply theme colors to blades
- [ ] Test: Blades render with correct colors and positioning

### Phase 3: Navigation & Animation (Week 2-3)

**Tasks:**
- [ ] Implement update() in `/bios/src/ui/blades.rs`
  - Left/Right blade switching
  - Up/Down tab navigation
  - A button selection
  - B button back to menu
- [ ] Implement horizontal scrolling animation
  - Smooth easing between blades
  - Tab glow pulsing effect
- [ ] Add sound effects for all interactions
- [ ] Test: Smooth blade navigation and transitions

### Phase 4: Games & Apps Blade (Week 3)

**Tasks:**
- [ ] Implement render_games_blade_content()
  - Library tab: Game grid (reuse GameSelection components)
  - Recently Played tab: Horizontal list
  - Installed Apps tab: Extras menu items
- [ ] Populate games_list from save::find_all_game_files()
- [ ] Implement game icon caching
- [ ] Implement game launching from Blades UI
- [ ] Test: Can browse and launch games from Blades

### Phase 5: Settings & Saves Blades (Week 4)

**Tasks:**
- [ ] Implement render_settings_blade_content()
  - Navigate to existing settings screens
- [ ] Implement render_saves_blade_content()
  - Navigate to SaveData screen
- [ ] Modify `/bios/src/ui/settings.rs`
  - Add "return to Blades" option
- [ ] Modify `/bios/src/ui/data.rs`
  - Add "return to Blades" option
- [ ] Test: Can access all settings and saves from Blades

### Phase 6: Theming & Polish (Week 4-5)

**Tasks:**
- [ ] Add blades config to `/bios/src/config.rs`
- [ ] Add blades parsing to `/bios/src/theme.rs`
- [ ] Create Blades settings page
  - Toggle blades mode
  - Color customization per blade
  - Transparency slider
  - Blur toggle
- [ ] Apply theme customizations to blade rendering
- [ ] Polish animations (timing, curves)
- [ ] Add blade transparency effect
- [ ] Optional: Implement background blur
- [ ] Test: All theme options work correctly

### Phase 7: Testing & Optimization (Week 5)

**Tasks:**
- [ ] Performance profiling
- [ ] Edge case testing (0 games, 100+ games, long names)
- [ ] Input handling validation
- [ ] Animation smoothness check
- [ ] Memory usage optimization (icon caching)
- [ ] Documentation

---

## Critical Files

### Must Create:
1. **`/bios/src/ui/blades.rs`** - Core blades module (NEW FILE)
   - All rendering, navigation, and animation logic
   - ~500-800 lines estimated

### Must Modify:
1. **`/bios/src/types.rs`** - Add blade data structures
2. **`/bios/src/ui/mod.rs`** - Export blades module
3. **`/bios/src/main.rs`** - Integrate BladesDashboard screen
4. **`/bios/src/config.rs`** - Add blades configuration
5. **`/bios/src/theme.rs`** - Parse blades theme settings
6. **`/bios/src/ui/settings.rs`** - Add Blades settings page and return navigation
7. **`/bios/src/ui/data.rs`** - Add return-to-Blades option

---

## Technical Considerations

### Perspective Rendering

**Options:**
1. Simple: Vertical rectangles without skew (recommended start)
2. Advanced: Textured quads with UV mapping for perspective
3. Shader: Custom 3D perspective shader (complex)

**Recommendation:** Start simple, add perspective in polish phase if needed

### Background Blur

**Options:**
1. Skip: No blur (simplest)
2. Fake: Darken with semi-transparent overlay
3. Real: Gaussian blur shader

**Recommendation:** Start with fake blur (darkening), add real blur in polish if desired

### Performance

**Optimizations:**
- Only render visible blades (skip off-screen)
- Cache blade backgrounds as textures
- Use render targets for static content
- Lazy-load game icons

---

## Success Criteria

✅ Blades render with authentic Xbox 360 visual style
✅ Smooth horizontal scrolling between blades (0.3s transition)
✅ Tab navigation within blades works correctly
✅ Can access all games, settings, and saves from Blades UI
✅ Theme system supports custom blade colors
✅ Video backgrounds continue to work behind blades
✅ Pulsing glow effect on selected tabs
✅ Sound effects for all navigation actions
✅ Return to main menu with B button
✅ Performance is smooth (60 FPS target)

---

## Estimated Effort

**Phase 1-2 (Core + Visuals):** 5-7 days
**Phase 3-4 (Navigation + Games):** 5-7 days
**Phase 5-6 (Settings/Saves + Theming):** 5-7 days
**Phase 7 (Testing + Optimization):** 3-5 days

**Total:** 18-26 days (3-5 weeks, 1 developer full-time)
