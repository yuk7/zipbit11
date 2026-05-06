mod bytes;
mod eocd;
mod local_header;
mod selection;
mod status;

use bytes::{has_range, read_u16, read_u32, write_u16};
use eocd::{find_eocd, read_central_directory_info};
use local_header::read_lfh_offset_from_cd;
use selection::EntrySelection;
use status::StatusSummary;

const LOCAL_FILE_HEADER_SIG: u32 = 0x04034b50;
const CENTRAL_DIR_SIG: u32 = 0x02014b50;
const EOCD_SIG: u32 = 0x06054b50;
const ZIP64_EOCD_SIG: u32 = 0x06064b50;
const ZIP64_EOCD_LOCATOR_SIG: u32 = 0x07064b50;
const ZIP64_EXTRA_FIELD_ID: u16 = 0x0001;

/// bit 11 = 0x0800: indicates that the filename and comment are encoded in UTF-8.
const BIT11: u16 = 0x0800;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Status,
    Detail,
    Set,
    Clear,
    Toggle,
}

/// Run the requested operation on the given ZIP file.
pub fn process(path: &str, mode: Mode, selection: Option<&str>) -> Result<(), String> {
    let mut data = std::fs::read(path).map_err(|e| format!("cannot read '{}': {}", path, e))?;

    let eocd_offset = find_eocd(&data)?;
    let cd_info = read_central_directory_info(&data, eocd_offset)?;
    let total_entries = cd_info.total_entries;
    let cd_offset = cd_info.cd_offset;

    if cd_offset > data.len() {
        return Err(format!(
            "Central Directory offset {:#x} is out of bounds",
            cd_offset
        ));
    }

    let selection = match mode {
        Mode::Status => EntrySelection::All,
        Mode::Detail | Mode::Set | Mode::Clear | Mode::Toggle => {
            EntrySelection::parse(selection, total_entries)?
        }
    };
    let selected_entries = selection.count(total_entries);

    if mode == Mode::Detail {
        println!("File: {}", path);
    }

    let mut modified = false;
    let mut pos = cd_offset;
    let mut set_entries = 0usize;
    let mut selected_set_entries = 0usize;
    let mut detail_rows = Vec::new();

    for i in 0..total_entries {
        if !has_range(&data, pos, 46) {
            return Err(format!(
                "file ended unexpectedly at Central Directory entry {}",
                i + 1
            ));
        }

        let sig = read_u32(&data, pos);
        if sig != CENTRAL_DIR_SIG {
            return Err(format!(
                "invalid Central Directory signature (offset: {:#x})",
                pos
            ));
        }

        let fname_len = read_u16(&data, pos + 28) as usize;
        let extra_len = read_u16(&data, pos + 30) as usize;
        let comment_len = read_u16(&data, pos + 32) as usize;
        let entry_len = 46usize
            .checked_add(fname_len)
            .and_then(|n| n.checked_add(extra_len))
            .and_then(|n| n.checked_add(comment_len))
            .ok_or_else(|| {
                format!(
                    "size overflow while parsing Central Directory entry {}",
                    i + 1
                )
            })?;
        if !has_range(&data, pos, entry_len) {
            return Err(format!(
                "file ended unexpectedly in Central Directory entry {}",
                i + 1
            ));
        }

        let cd_flag_offset = pos + 8;
        let current_flags = read_u16(&data, cd_flag_offset);
        let bit11_set = (current_flags & BIT11) != 0;
        if bit11_set {
            set_entries += 1;
        }
        let is_selected = selection.includes(i + 1);
        if is_selected && bit11_set {
            selected_set_entries += 1;
        }

        let fname_start = pos + 46;
        let fname_end = fname_start.checked_add(fname_len);
        let fname = if let Some(end) = fname_end {
            if end <= data.len() {
                String::from_utf8_lossy(&data[fname_start..end]).into_owned()
            } else {
                "<invalid filename>".to_string()
            }
        } else {
            "<invalid filename>".to_string()
        };

        match mode {
            Mode::Status => {}
            Mode::Detail => {
                if is_selected {
                    let mark = if bit11_set { "✓ set" } else { "✗ clear" };
                    detail_rows.push(format!(" {:<4}  {:<6}  {}", i + 1, mark, fname));
                }
            }
            _ => {
                if !is_selected {
                    pos = advance_cd_pos(pos, entry_len, i + 1)?;
                    continue;
                }

                let new_bit11 = match mode {
                    Mode::Set => true,
                    Mode::Clear => false,
                    Mode::Toggle => !bit11_set,
                    Mode::Status | Mode::Detail => unreachable!(),
                };

                let new_flags = with_bit11(current_flags, new_bit11);
                let lfh_offset = read_lfh_offset_from_cd(&data, pos, fname_len, extra_len, i + 1)?;

                if !has_range(&data, lfh_offset, 8) {
                    return Err(format!(
                        "Local File Header offset {:#x} for '{}' is out of bounds",
                        lfh_offset, fname
                    ));
                }

                let lfh_sig = read_u32(&data, lfh_offset);
                if lfh_sig != LOCAL_FILE_HEADER_SIG {
                    return Err(format!(
                        "invalid Local File Header signature for '{}' (offset: {:#x})",
                        fname, lfh_offset
                    ));
                }

                let lfh_flags = read_u16(&data, lfh_offset + 6);
                let new_lfh_flags = with_bit11(lfh_flags, new_bit11);

                if new_flags != current_flags || new_lfh_flags != lfh_flags {
                    write_u16(&mut data, cd_flag_offset, new_flags);
                    write_u16(&mut data, lfh_offset + 6, new_lfh_flags);
                    modified = true;
                }
            }
        }

        pos = advance_cd_pos(pos, entry_len, i + 1)?;
    }

    let summary = StatusSummary::from_counts(total_entries, set_entries);
    let selected_summary = StatusSummary::from_counts(selected_entries, selected_set_entries);

    if mode == Mode::Status {
        println!("File: {}", path);
        println!("Entries: {}", summary.total_entries);
        println!("bit11: {}", summary.aggregate_label());
        return Ok(());
    }

    if mode == Mode::Detail {
        if selection.is_all() {
            println!("Entries: {}", summary.total_entries);
        } else {
            println!(
                "Entries: {} selected of {}",
                selected_summary.total_entries, summary.total_entries
            );
        }
        println!("bit11: {}", selected_summary.detail_label());
        println!();
        println!(" {:<4}  {:<6}  Filename", "No.", "bit11");
        println!(" {}", "-".repeat(60));
        for row in detail_rows {
            println!("{row}");
        }
        return Ok(());
    }

    if modified {
        std::fs::write(path, &data).map_err(|e| format!("failed to write '{}': {}", path, e))?;
        let action = match mode {
            Mode::Set => "set bit 11 in",
            Mode::Clear => "cleared bit 11 in",
            Mode::Toggle => "toggled bit 11 in",
            Mode::Status | Mode::Detail => unreachable!(),
        };
        if selection.is_all() {
            println!(
                "{} '{}' ({} entries processed)",
                action, path, total_entries
            );
        } else {
            println!(
                "{} '{}' ({} selected of {} entries)",
                action, path, selected_entries, total_entries
            );
        }
    } else if selection.is_all() {
        println!(
            "no change needed: '{}' (already in the desired state)",
            path
        );
    } else {
        println!(
            "no change needed: '{}' ({} selected entries already in the desired state)",
            path, selected_entries
        );
    }

    Ok(())
}

#[inline]
fn with_bit11(flags: u16, enabled: bool) -> u16 {
    if enabled {
        flags | BIT11
    } else {
        flags & !BIT11
    }
}

fn advance_cd_pos(pos: usize, entry_len: usize, entry_no: usize) -> Result<usize, String> {
    pos.checked_add(entry_len).ok_or_else(|| {
        format!(
            "offset overflow while advancing Central Directory entry {}",
            entry_no
        )
    })
}

#[cfg(test)]
mod tests;
