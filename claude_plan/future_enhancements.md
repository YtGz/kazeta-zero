# Future Enhancements for Kazeta Overlay & Input Systems

## Input Daemon Enhancements

### Hotkey System
- [ ] Customizable hotkey configuration (JSON/TOML config file)
- [ ] Multiple hotkey profiles (per-game, global)
- [ ] Macro support (record and playback button sequences)
- [ ] Hotkey chaining (combo sequences like fighting games)
- [ ] Context-aware hotkeys (different bindings when overlay is open vs game)

### Controller Features
- [ ] Controller rumble/vibration support
- [ ] Battery level monitoring for wireless controllers
- [ ] Controller LED color customization
- [ ] Gyro/accelerometer support for motion controls
- [ ] Touchpad support (PS4/PS5 controllers)
- [ ] Per-controller dead zone configuration
- [ ] Controller firmware update notifications
- [ ] Auto-sleep for inactive wireless controllers

### Advanced Input
- [ ] Keyboard remapping (custom key bindings)
- [ ] Mouse support for menu navigation
- [ ] Turbo button functionality
- [ ] Sticky keys / accessibility features
- [ ] Input recording and playback (TAS tools)
- [ ] Input display overlay (show button presses on screen)
- [ ] Analog stick calibration tool

## Overlay UI/UX Enhancements

### Visual Improvements
- [ ] Multiple theme support (dark, light, retro, custom)
- [ ] Animated transitions between screens
- [ ] Custom background images/patterns
- [ ] Icon support for menu items
- [ ] Font selection and sizing options
- [ ] Sound effects for menu navigation
- [ ] Smooth scrolling animations
- [ ] Grid vs list view options

### Save State Management
- [ ] Save state creation and loading
- [ ] Screenshot thumbnails for save states
- [ ] Save state browser with metadata (timestamp, playtime)
- [ ] Quick save/load hotkeys
- [ ] Multiple save state slots per game
- [ ] Save state auto-backup
- [ ] Cloud sync for save states (optional)
- [ ] Save state comparison (view changes between states)

### Media Capture
- [ ] Screenshot capture with overlay hidden
- [ ] Video recording (gameplay clips)
- [ ] Screenshot gallery viewer
- [ ] Share screenshots to external services
- [ ] GIF creation from recorded clips
- [ ] Configurable video quality/bitrate
- [ ] Replay buffer (save last N seconds)

### Game Management
- [ ] Recently played games list
- [ ] Favorites/bookmarks
- [ ] Game search/filter
- [ ] Play time tracking per game
- [ ] Game notes/annotations
- [ ] Game tags/categories
- [ ] Launch count statistics
- [ ] Sort games by various criteria

### Performance Overlay (✓ Completed)
- [x] FPS counter
- [x] Frame time graph
- [x] CPU usage monitoring
- [x] Memory usage monitoring
- [x] Toggle hotkey (F3)
- [ ] Network stats (latency, bandwidth)
- [ ] Disk I/O monitoring
- [ ] Temperature monitoring (if available)
- [ ] Customizable HUD position
- [ ] Graph history (frame time over time)
- [ ] Export performance data to CSV

### Settings & Configuration
- [ ] Audio settings (volume control, mute)
- [ ] Display settings (resolution, aspect ratio, filters)
- [ ] Input latency display
- [ ] Language selection
- [ ] Accessibility options (colorblind modes, large text)
- [ ] Auto-update settings
- [ ] Data privacy settings
- [ ] Performance presets (quality vs performance)

## RetroAchievements Integration

### Achievement Features
- [ ] Achievement progress tracking with percentage
- [ ] Achievement hints/guides
- [ ] Leaderboard support
- [ ] Rich presence (show what you're playing to friends)
- [ ] Challenge mode (hardcore achievements)
- [ ] Achievement sound effects and animations
- [ ] Achievement rarity display
- [ ] Unlock time tracking
- [ ] Achievement export (badges, cards)
- [ ] Achievement difficulty ratings

### Social Features
- [ ] Friends list integration
- [ ] Achievement comparison with friends
- [ ] Global leaderboards
- [ ] Recent achievements feed
- [ ] Profile customization
- [ ] Achievement sharing (social media)
- [ ] Community challenges
- [ ] Multiplayer session invites

## System Integration

### Platform Features
- [ ] Discord Rich Presence integration
- [ ] Steam overlay integration (when running in Steam)
- [ ] System tray integration (minimize to tray)
- [ ] Desktop notifications (achievements, friends online)
- [ ] Auto-start on system boot (optional)
- [ ] Suspend/resume game state
- [ ] Power management (prevent sleep during gameplay)

### Network Features
- [ ] Netplay/online multiplayer support
- [ ] Lobby browser for online games
- [ ] Voice chat integration
- [ ] Spectator mode
- [ ] NAT traversal / relay servers
- [ ] Rollback netcode for low latency
- [ ] Network quality indicator

### Storage & Backup
- [ ] Automatic save backup (local)
- [ ] Cloud save sync
- [ ] Save migration tools
- [ ] Export/import saves
- [ ] Compression for save states
- [ ] Save state versioning
- [ ] Automatic cleanup of old saves

## Technical/Architecture Enhancements

### Performance
- [ ] GPU acceleration for overlay rendering
- [ ] Multi-threaded rendering pipeline
- [ ] Lazy loading for large game libraries
- [ ] Memory usage optimization
- [ ] Reduce overlay latency
- [ ] Frame pacing improvements
- [ ] Background asset preloading

### Modularity
- [ ] Plugin system for custom features
- [ ] Scripting API (Lua/JavaScript)
- [ ] Custom shader support
- [ ] Mod loader integration
- [ ] Extensible theme engine
- [ ] Custom widget system
- [ ] Hook system for game events

### Debugging & Development
- [ ] Debug overlay (show internal state)
- [ ] Performance profiler
- [ ] Input lag tester
- [ ] Memory leak detector
- [ ] Crash reporter with stack traces
- [ ] Telemetry (opt-in analytics)
- [ ] Developer console
- [ ] Log viewer in overlay

### Platform Support
- [ ] Windows port
- [ ] Steam Deck optimization
- [ ] Android/mobile support
- [ ] Web-based remote control
- [ ] Wayland native support (currently X11)
- [ ] Controller support for more platforms
- [ ] Headless mode (server/testing)

## Priority Ranking

### High Priority (Next Sprint)
1. Save state management (core functionality)
2. Screenshot capture
3. Game library browser improvements
4. Audio settings in overlay
5. Controller battery indicators

### Medium Priority (Future Releases)
1. Video recording
2. Achievement progress tracking
3. Discord Rich Presence
4. Custom themes
5. Netplay support

### Low Priority (Nice to Have)
1. Scripting API
2. Plugin system
3. Advanced analytics
4. Mobile companion app
5. Web-based remote control

## Implementation Notes

### Dependencies
- Additional libraries may be needed for:
  - Video encoding (ffmpeg already available)
  - Cloud sync (AWS SDK, Google Drive API)
  - Voice chat (WebRTC, Opus)
  - Discord integration (discord-rpc)

### Architecture Considerations
- Save state format compatibility
- Performance impact of new features
- Security for online features
- Privacy concerns with telemetry
- Backward compatibility with older saves

### Testing Requirements
- Controller compatibility testing (various brands)
- Performance benchmarking with new features
- Network testing for multiplayer
- Cross-platform testing
- Accessibility testing

## Completed Features ✓

- [x] Basic overlay system with IPC
- [x] Controller input handling (gilrs)
- [x] Keyboard input support
- [x] RetroAchievements display
- [x] Controller menu (Bluetooth, assignment, tester)
- [x] Performance overlay (FPS, CPU, memory)
- [x] Toast notification system
- [x] Theme color customization
- [x] Multiple hotkey support (Guide, F12, Ctrl+O, F3)
