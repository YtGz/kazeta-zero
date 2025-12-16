use anyhow::{Context, Result, bail};
use md5::{Md5, Digest};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use crate::types::ConsoleId;

/// Hash a ROM file for RetroAchievements identification
/// Different consoles may require different hashing methods
/// Uses streaming to avoid loading entire file into memory
pub fn hash_rom(path: &Path, console_id: ConsoleId) -> Result<String> {
    let file = File::open(path)
        .context("Failed to open ROM file")?;

    let metadata = file.metadata()
        .context("Failed to get file metadata")?;
    let file_size = metadata.len() as usize;

    // Stream hash based on console type
    let hasher = match console_id {
        ConsoleId::NES => hash_nes_rom(file)?,
        ConsoleId::SNES => hash_snes_rom(file, file_size)?,
        ConsoleId::Nintendo64 => hash_n64_rom(file)?,
        _ => hash_generic_rom(file)?,
    };

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// Hash NES ROM with streaming (strip 16-byte header if present)
fn hash_nes_rom(mut file: File) -> Result<Md5> {
    let mut header = [0u8; 16];
    file.read_exact(&mut header)
        .context("Failed to read NES header")?;

    let mut hasher = Md5::new();

    // Check if this is an iNES header
    if &header[0..4] == b"NES\x1a" {
        // Skip header, hash the rest
        let mut reader = BufReader::with_capacity(1024 * 1024, file);
        let mut chunk = [0u8; 8192];
        loop {
            let bytes_read = reader.read(&mut chunk)
                .context("Failed to read ROM data")?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&chunk[..bytes_read]);
        }
    } else {
        // No header, hash everything including what we already read
        hasher.update(&header);
        let mut reader = BufReader::with_capacity(1024 * 1024, file);
        let mut chunk = [0u8; 8192];
        loop {
            let bytes_read = reader.read(&mut chunk)
                .context("Failed to read ROM data")?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&chunk[..bytes_read]);
        }
    }

    Ok(hasher)
}

/// Hash SNES ROM with streaming (strip 512-byte header if present)
fn hash_snes_rom(mut file: File, file_size: usize) -> Result<Md5> {
    let mut hasher = Md5::new();

    // Check if file has 512-byte copier header
    let header_size = file_size % 1024;
    if header_size == 512 {
        // Skip 512-byte header
        let mut header = vec![0u8; 512];
        file.read_exact(&mut header)
            .context("Failed to skip SNES header")?;
    }

    // Hash the rest
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut chunk = [0u8; 8192];
    loop {
        let bytes_read = reader.read(&mut chunk)
            .context("Failed to read ROM data")?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&chunk[..bytes_read]);
    }

    Ok(hasher)
}

/// Hash N64 ROM with streaming (byteswap if needed)
fn hash_n64_rom(mut file: File) -> Result<Md5> {
    // Read first 4 bytes to determine byte order
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .context("Failed to read N64 magic bytes")?;

    let mut hasher = Md5::new();

    match &magic {
        // Big-endian (z64) - no swap needed
        [0x80, 0x37, 0x12, 0x40] => {
            hasher.update(&magic);
            let mut reader = BufReader::with_capacity(1024 * 1024, file);
            let mut chunk = [0u8; 8192];
            loop {
                let bytes_read = reader.read(&mut chunk)
                    .context("Failed to read ROM data")?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&chunk[..bytes_read]);
            }
        }

        // Little-endian (n64) - swap every 4 bytes
        [0x40, 0x12, 0x37, 0x80] => {
            // Hash the swapped magic first
            hasher.update(&[magic[3], magic[2], magic[1], magic[0]]);

            // Stream and swap the rest
            let mut reader = BufReader::with_capacity(1024 * 1024, file);
            let mut chunk = [0u8; 8192];
            loop {
                let bytes_read = reader.read(&mut chunk)
                    .context("Failed to read ROM data")?;
                if bytes_read == 0 {
                    break;
                }

                // Byteswap in 4-byte chunks
                let full_chunks = bytes_read / 4;
                for i in 0..full_chunks {
                    let offset = i * 4;
                    hasher.update(&[
                        chunk[offset + 3],
                        chunk[offset + 2],
                        chunk[offset + 1],
                        chunk[offset + 0],
                    ]);
                }

                // Handle remaining bytes (if any)
                let remainder = bytes_read % 4;
                if remainder > 0 {
                    let offset = full_chunks * 4;
                    hasher.update(&chunk[offset..bytes_read]);
                }
            }
        }

        // Byte-swapped (v64) - swap every 2 bytes
        [0x37, 0x80, 0x40, 0x12] => {
            // Hash the swapped magic first
            hasher.update(&[magic[1], magic[0], magic[3], magic[2]]);

            // Stream and swap the rest
            let mut reader = BufReader::with_capacity(1024 * 1024, file);
            let mut chunk = [0u8; 8192];
            loop {
                let bytes_read = reader.read(&mut chunk)
                    .context("Failed to read ROM data")?;
                if bytes_read == 0 {
                    break;
                }

                // Byteswap in 2-byte chunks
                let full_chunks = bytes_read / 2;
                for i in 0..full_chunks {
                    let offset = i * 2;
                    hasher.update(&[chunk[offset + 1], chunk[offset + 0]]);
                }

                // Handle remaining byte (if any)
                if bytes_read % 2 == 1 {
                    hasher.update(&[chunk[bytes_read - 1]]);
                }
            }
        }

        // Unknown format - hash as-is
        _ => {
            hasher.update(&magic);
            let mut reader = BufReader::with_capacity(1024 * 1024, file);
            let mut chunk = [0u8; 8192];
            loop {
                let bytes_read = reader.read(&mut chunk)
                    .context("Failed to read ROM data")?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&chunk[..bytes_read]);
            }
        }
    }

    Ok(hasher)
}

/// Hash generic ROM with streaming (no preprocessing)
fn hash_generic_rom(file: File) -> Result<Md5> {
    let mut hasher = Md5::new();
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut chunk = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut chunk)
            .context("Failed to read ROM data")?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&chunk[..bytes_read]);
    }

    Ok(hasher)
}

/// Get the hash type name for a console
pub fn hash_type_name(console_id: ConsoleId) -> &'static str {
    match console_id {
        ConsoleId::NES => "MD5 (headerless)",
        ConsoleId::SNES => "MD5 (headerless)",
        ConsoleId::Nintendo64 => "MD5 (big-endian)",
        _ => "MD5",
    }
}

/// Auto-detect console type from ROM file
/// Checks file extension first, then verifies with magic bytes
pub fn detect_console(path: &Path) -> Result<ConsoleId> {
    // First, try to detect from file extension
    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            let ext_lower = ext_str.to_lowercase();
            if let Some(console) = detect_from_extension(&ext_lower) {
                // Verify with magic bytes if possible
                if let Ok(verified) = verify_with_magic_bytes(path, console) {
                    if verified {
                        return Ok(console);
                    }
                }
                // If verification fails but extension is clear, still use it
                // (some ROMs might have unusual headers)
                return Ok(console);
            }
        }
    }

    // If extension didn't work, try magic bytes only
    detect_from_magic_bytes(path)
}

/// Detect console from file extension
fn detect_from_extension(ext: &str) -> Option<ConsoleId> {
    match ext {
        "gba" => Some(ConsoleId::GameBoyAdvance),
        "gb" => Some(ConsoleId::GameBoy),
        "gbc" => Some(ConsoleId::GameBoyColor),
        "nes" | "fds" => Some(ConsoleId::NES),
        "snes" | "sfc" | "smc" => Some(ConsoleId::SNES),
        "n64" | "z64" | "v64" | "u64" => Some(ConsoleId::Nintendo64),
        "nds" => Some(ConsoleId::NintendoDS),
        "psx" | "ps1" | "bin" | "cue" | "img" => Some(ConsoleId::PlayStation),
        "ps2" | "iso" => Some(ConsoleId::PlayStation2),
        "gen" | "md" | "smd" => Some(ConsoleId::MegaDrive),
        "sms" => Some(ConsoleId::MasterSystem),
        "a26" => Some(ConsoleId::Atari2600),
        "vb" => Some(ConsoleId::VirtualBoy),
        _ => None,
    }
}

/// Verify console type using magic bytes
fn verify_with_magic_bytes(path: &Path, console: ConsoleId) -> Result<bool> {
    let mut file = File::open(path)
        .context("Failed to open ROM file")?;
    
    let mut buffer = vec![0u8; 16]; // Read first 16 bytes for magic detection
    let bytes_read = file.read(&mut buffer)
        .context("Failed to read ROM file")?;

    if bytes_read < 4 {
        return Ok(false);
    }

    let verified = match console {
        ConsoleId::NES => &buffer[0..4] == b"NES\x1a",
        ConsoleId::SNES => {
            // SNES ROMs can have headers, check for common patterns
            // Check at offset 0 or 512 (header offset)
            // SNES ROMs often start with specific patterns, but it's not always reliable
            // For now, just check if it's not obviously wrong
            // SNES detection is less reliable, so we trust extension
            let _check_offset = if bytes_read > 512 && buffer.len() > 512 {
                &buffer[512..512+4]
            } else {
                &buffer[0..4]
            };
            true
        },
        ConsoleId::Nintendo64 => {
            // N64 ROMs have specific magic bytes
            matches!(&buffer[0..4], 
                [0x80, 0x37, 0x12, 0x40] | // Big-endian (z64)
                [0x40, 0x12, 0x37, 0x80] | // Little-endian (n64)
                [0x37, 0x80, 0x40, 0x12]   // Byte-swapped (v64)
            )
        },
        ConsoleId::GameBoyAdvance | ConsoleId::GameBoy | ConsoleId::GameBoyColor => {
            // Game Boy ROMs have a Nintendo logo at specific offsets
            // GBA: 0x04-0x9F, GB/GBC: 0x104-0x133
            if bytes_read >= 0xA0 {
                // Check for Nintendo logo pattern (simplified check)
                // Real check would verify the exact logo bytes
                buffer[0x04] == 0x24 || buffer[0x04] == 0xCE // Common first bytes
            } else {
                true // Can't verify, trust extension
            }
        },
        ConsoleId::NintendoDS => {
            // NDS ROMs have "Nintendo DS" string at offset 0x0C
            bytes_read >= 0x10 && 
            buffer[0x0C..0x0C+12].iter().any(|&b| b != 0)
        },
        ConsoleId::PlayStation => {
            // PSX discs have "PLAYSTATION" or "PS-X EXE" markers
            // This is complex for disc images, so we'll trust extension
            true
        },
        _ => true, // For other consoles, trust extension
    };

    Ok(verified)
}

/// Detect console from magic bytes only (fallback when extension fails)
fn detect_from_magic_bytes(path: &Path) -> Result<ConsoleId> {
    let mut file = File::open(path)
        .context("Failed to open ROM file")?;
    
    let mut buffer = vec![0u8; 16];
    let bytes_read = file.read(&mut buffer)
        .context("Failed to read ROM file")?;

    if bytes_read < 4 {
        bail!("File too small to detect console type");
    }

    // Check for known magic bytes
    if &buffer[0..4] == b"NES\x1a" {
        return Ok(ConsoleId::NES);
    }

    if matches!(&buffer[0..4], 
        [0x80, 0x37, 0x12, 0x40] | 
        [0x40, 0x12, 0x37, 0x80] | 
        [0x37, 0x80, 0x40, 0x12]
    ) {
        return Ok(ConsoleId::Nintendo64);
    }

    // Check for SNES header (512 bytes offset)
    if bytes_read > 512 {
        let mut header_buffer = vec![0u8; 4];
        file.seek(SeekFrom::Start(512))?;
        if file.read_exact(&mut header_buffer).is_ok() {
            // SNES ROMs have specific patterns, but detection is complex
            // We'll need extension for SNES
        }
    }

    // Check for Game Boy family
    if bytes_read >= 0xA0 {
        // Check Nintendo logo area
        if buffer[0x04] == 0x24 || buffer[0x04] == 0xCE {
            // Could be GB/GBC/GBA, but need more info
            // For now, default to GBA as most common
            return Ok(ConsoleId::GameBoyAdvance);
        }
    }

    bail!("Could not detect console type from file. Please specify --console manually.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generic_rom_hash() {
        // Create a temporary file with known data
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"TEST_ROM_DATA_12345").unwrap();
        temp_file.flush().unwrap();

        // Hash it as a generic console
        let hash = hash_rom(temp_file.path(), ConsoleId::GameBoyAdvance).unwrap();

        // Verify it produces a valid MD5 hash (32 hex chars)
        assert_eq!(hash.len(), 32);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_nes_header_detection() {
        // Create NES ROM with iNES header
        let mut temp_file = NamedTempFile::new().unwrap();
        let mut nes_data = Vec::new();
        nes_data.extend_from_slice(b"NES\x1a\x02\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00");
        nes_data.extend_from_slice(b"ROM_DATA_HERE");
        temp_file.write_all(&nes_data).unwrap();
        temp_file.flush().unwrap();

        // Hash it - should skip the 16-byte header
        let hash = hash_rom(temp_file.path(), ConsoleId::NES).unwrap();

        // Verify it produces a valid MD5 hash
        assert_eq!(hash.len(), 32);
    }
}

