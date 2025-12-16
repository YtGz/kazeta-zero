use anyhow::{Context, Result};
use md5::{Md5, Digest};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use crate::types::ConsoleId;

/// Hash a ROM file for RetroAchievements identification
/// Different consoles may require different hashing methods
pub fn hash_rom(path: &Path, console_id: ConsoleId) -> Result<String> {
    let mut file = File::open(path)
        .context("Failed to open ROM file")?;
    
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .context("Failed to read ROM file")?;

    // Apply console-specific preprocessing
    let data = preprocess_rom(&buffer, console_id);

    // Hash the data
    let hash = Md5::digest(&data);
    Ok(format!("{:x}", hash))
}

/// Preprocess ROM data based on console requirements
/// Some consoles need headers stripped, etc.
fn preprocess_rom(data: &[u8], console_id: ConsoleId) -> Vec<u8> {
    match console_id {
        ConsoleId::NES => strip_nes_header(data),
        ConsoleId::SNES => strip_snes_header(data),
        ConsoleId::Nintendo64 => byteswap_n64(data),
        _ => data.to_vec(),
    }
}

/// Strip iNES header from NES ROMs (16 bytes)
fn strip_nes_header(data: &[u8]) -> Vec<u8> {
    if data.len() > 16 && &data[0..4] == b"NES\x1a" {
        data[16..].to_vec()
    } else {
        data.to_vec()
    }
}

/// Strip header from SNES ROMs if present
fn strip_snes_header(data: &[u8]) -> Vec<u8> {
    // SNES ROMs can have a 512-byte copier header
    let header_size = data.len() % 1024;
    if header_size == 512 && data.len() > 512 {
        data[512..].to_vec()
    } else {
        data.to_vec()
    }
}

/// Byteswap N64 ROMs to big-endian if needed
/// N64 ROMs can be in different byte orders (z64, n64, v64)
fn byteswap_n64(data: &[u8]) -> Vec<u8> {
    if data.len() < 4 {
        return data.to_vec();
    }

    // Check for byte order magic
    match &data[0..4] {
        // Big-endian (z64) - no swap needed
        [0x80, 0x37, 0x12, 0x40] => data.to_vec(),
        
        // Little-endian (n64) - swap every 4 bytes
        [0x40, 0x12, 0x37, 0x80] => {
            let mut result = Vec::with_capacity(data.len());
            for chunk in data.chunks(4) {
                if chunk.len() == 4 {
                    result.push(chunk[3]);
                    result.push(chunk[2]);
                    result.push(chunk[1]);
                    result.push(chunk[0]);
                } else {
                    result.extend_from_slice(chunk);
                }
            }
            result
        }
        
        // Byte-swapped (v64) - swap every 2 bytes
        [0x37, 0x80, 0x40, 0x12] => {
            let mut result = Vec::with_capacity(data.len());
            for chunk in data.chunks(2) {
                if chunk.len() == 2 {
                    result.push(chunk[1]);
                    result.push(chunk[0]);
                } else {
                    result.extend_from_slice(chunk);
                }
            }
            result
        }
        
        // Unknown format - return as-is
        _ => data.to_vec(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nes_header_strip() {
        let nes_rom = b"NES\x1a\x02\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00ROM_DATA_HERE";
        let stripped = strip_nes_header(nes_rom);
        assert_eq!(&stripped[..], b"ROM_DATA_HERE");
    }

    #[test]
    fn test_no_header_passthrough() {
        let gba_rom = b"GBA_ROM_DATA";
        let result = preprocess_rom(gba_rom, ConsoleId::GameBoyAdvance);
        assert_eq!(result, gba_rom.to_vec());
    }
}

