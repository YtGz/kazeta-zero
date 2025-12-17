use crate::{StorageMediaState, Arc, Mutex};

pub struct DialogOption {
    pub text: String,
    pub value: String,
    pub disabled: bool,
}

pub struct Dialog {
    pub id: String,
    pub desc: Option<String>,
    pub options: Vec<DialogOption>,
    pub selection: usize,
}

pub fn create_confirm_delete_dialog() -> Dialog {
    Dialog {
        id: "confirm_delete".to_string(),
        desc: Some("PERMANENTLY DELETE THIS SAVE DATA?".to_string()),
        options: vec![
            DialogOption {
                text: "DELETE".to_string(),
                value: "DELETE".to_string(),
                disabled: false,
            },
            DialogOption {
                text: "CANCEL".to_string(),
                value: "CANCEL".to_string(),
                disabled: false,
            }
        ],
        selection: 1,
    }
}

pub fn create_copy_storage_dialog(storage_state: &Arc<Mutex<StorageMediaState>>) -> Dialog {
    let mut options = Vec::new();
    if let Ok(state) = storage_state.lock() {
        for drive in state.media.iter() {
            if drive.id == state.media[state.selected].id {
                continue;
            }
            options.push(DialogOption {
                text: format!("{} ({} MB Free)", drive.id.clone(), drive.free),
                value: drive.id.clone(),
                disabled: false,
            });
        }
    }
    options.push(DialogOption {
        text: "CANCEL".to_string(),
        value: "CANCEL".to_string(),
        disabled: false,
    });

    Dialog {
        id: "copy_storage_select".to_string(),
        desc: Some("WHERE TO COPY THIS SAVE DATA?".to_string()),
        options,
        selection: 0,
    }
}

pub fn create_main_dialog(storage_state: &Arc<Mutex<StorageMediaState>>) -> Dialog {
    let has_external_devices = if let Ok(state) = storage_state.lock() {
        state.media.len() > 1
    } else {
        false
    };

    let options = vec![
        DialogOption {
            text: "COPY".to_string(),
            value: "COPY".to_string(),
            disabled: !has_external_devices,
        },
        DialogOption {
            text: "DELETE".to_string(),
            value: "DELETE".to_string(),
            disabled: false,
        },
        DialogOption {
            text: "CANCEL".to_string(),
            value: "CANCEL".to_string(),
            disabled: false,
        },
    ];

    Dialog {
        id: "main".to_string(),
        desc: None,
        options,
        selection: 0,
    }
}

pub fn create_save_exists_dialog() -> Dialog {
    Dialog {
        id: "save_exists".to_string(),
        desc: Some("THIS SAVE DATA ALREADY EXISTS AT THE SELECTED DESTINATION".to_string()),
        options: vec![
            DialogOption {
                text: "OK".to_string(),
                value: "OK".to_string(),
                disabled: false,
            }
        ],
        selection: 0,
    }
}

pub fn create_error_dialog(message: String) -> Dialog {
    Dialog {
        id: "error".to_string(),
        desc: Some(message),
        options: vec![
            DialogOption {
                text: "OK".to_string(),
                value: "OK".to_string(),
                disabled: false,
            }
        ],
        selection: 0,
    }
}

pub fn create_player_count_dialog(max_players: u8) -> Dialog {
    let mut options = Vec::new();

    // Create options for 1 to max_players
    for i in 1..=max_players {
        let text = if i == 1 {
            "1 PLAYER".to_string()
        } else {
            format!("{} PLAYERS", i)
        };

        options.push(DialogOption {
            text,
            value: i.to_string(),
            disabled: false,
        });
    }

    // Add cancel option
    options.push(DialogOption {
        text: "CANCEL".to_string(),
        value: "CANCEL".to_string(),
        disabled: false,
    });

    Dialog {
        id: "player_count_select".to_string(),
        desc: Some("SELECT NUMBER OF PLAYERS".to_string()),
        options,
        selection: if max_players >= 2 { 1 } else { 0 }, // Default to 2 players if available
    }
}

/// Create a dialog for selecting a save file slot for a player
/// `existing_saves` is a list of existing save slot identifiers (e.g., ["p1", "p2"])
/// `player_num` is the player number (1-4) selecting their save
/// `rom_name` is used to display which game's saves we're looking at
/// `can_import` adds an option to pull in an embedded save for this player
pub fn create_save_slot_dialog(existing_saves: &[String], player_num: u8, rom_name: &str, can_import: bool) -> Dialog {
    let mut options = Vec::new();

    // Option to create a new save for this player
    let new_slot = format!("p{}", player_num);
    let new_slot_exists = existing_saves.contains(&new_slot);

    if new_slot_exists {
        // If save exists, show it as "PLAYER X SAVE (CONTINUE)"
        options.push(DialogOption {
            text: format!("PLAYER {} SAVE (CONTINUE)", player_num),
            value: new_slot.clone(),
            disabled: false,
        });
    } else {
        // New save option
        options.push(DialogOption {
            text: format!("PLAYER {} SAVE (NEW)", player_num),
            value: new_slot.clone(),
            disabled: false,
        });
    }

    // Show other existing saves that the player could use
    for save_id in existing_saves {
        if save_id == &new_slot {
            continue; // Already added above
        }
        options.push(DialogOption {
            text: format!("USE {} SAVE", save_id.to_uppercase()),
            value: save_id.clone(),
            disabled: false,
        });
    }

    if can_import {
        options.push(DialogOption {
            text: "IMPORT PROVIDED SAVE".to_string(),
            value: "IMPORT".to_string(),
            disabled: false,
        });
    }

    // Add cancel option
    options.push(DialogOption {
        text: "CANCEL".to_string(),
        value: "CANCEL".to_string(),
        disabled: false,
    });

    let desc = if player_num == 1 {
        format!("SELECT SAVE FILE\n{}", rom_name.to_uppercase())
    } else {
        format!("PLAYER {} - SELECT SAVE FILE\n{}", player_num, rom_name.to_uppercase())
    };

    Dialog {
        id: format!("save_slot_select_p{}", player_num),
        desc: Some(desc),
        options,
        selection: 0,
    }
}

/// Scan the save directory for existing save files for a given cart/ROM
/// Returns a list of save slot identifiers (e.g., ["p1", "p2"] or just [])
pub fn find_existing_save_slots(save_dir: &std::path::Path, rom_name: &str) -> Vec<String> {
    let mut slots = Vec::new();

    if let Ok(entries) = std::fs::read_dir(save_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "sav" {
                    if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                        // Check if this save belongs to our ROM
                        // Format: {rom_name}_p1.sav, {rom_name}_p2.sav, or just {rom_name}.sav
                        if filename == rom_name {
                            // Single-player save (no suffix)
                            slots.push("default".to_string());
                        } else if filename.starts_with(&format!("{}_", rom_name)) {
                            // Multiplayer save with suffix
                            let suffix = &filename[rom_name.len() + 1..];
                            slots.push(suffix.to_string());
                        }
                    }
                }
            }
        }
    }

    slots.sort();
    slots
}
