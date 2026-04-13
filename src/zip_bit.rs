use std::collections::BTreeSet;

// Module for manipulating bit 11 (UTF-8 flag) of the General Purpose Bit Flag in ZIP entries.

// Signature constants
const LOCAL_FILE_HEADER_SIG: u32 = 0x04034b50; // PK\x03\x04
const CENTRAL_DIR_SIG: u32 = 0x02014b50; // PK\x01\x02
const EOCD_SIG: u32 = 0x06054b50; // PK\x05\x06
const ZIP64_EOCD_SIG: u32 = 0x06064b50; // PK\x06\x06
const ZIP64_EOCD_LOCATOR_SIG: u32 = 0x07064b50; // PK\x06\x07
const ZIP64_EXTRA_FIELD_ID: u16 = 0x0001;

/// bit 11 = 0x0800: indicates that the filename and comment are encoded in UTF-8
const BIT11: u16 = 0x0800;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Status,
    Detail,
    Set,
    Clear,
    Toggle,
}

#[derive(Debug, Clone, PartialEq)]
enum EntrySelection {
    All,
    Selected(BTreeSet<usize>),
}

impl EntrySelection {
    fn parse(raw: Option<&str>, total_entries: usize) -> Result<Self, String> {
        let Some(raw) = raw else {
            return Ok(Self::All);
        };

        if raw.trim().is_empty() {
            return Err("entry selector must not be empty".to_string());
        }

        let mut entries = BTreeSet::new();
        for part in raw.split(',') {
            let token = part.trim();
            if token.is_empty() {
                return Err("entry selector contains an empty item".to_string());
            }

            if let Some((start, end)) = token.split_once('-') {
                let start = parse_entry_no(start.trim(), total_entries)?;
                let end = parse_entry_no(end.trim(), total_entries)?;
                if start > end {
                    return Err(format!("entry range '{token}' is descending"));
                }
                entries.extend(start..=end);
            } else {
                entries.insert(parse_entry_no(token, total_entries)?);
            }
        }

        Ok(Self::Selected(entries))
    }

    fn includes(&self, entry_no: usize) -> bool {
        match self {
            Self::All => true,
            Self::Selected(entries) => entries.contains(&entry_no),
        }
    }

    fn count(&self, total_entries: usize) -> usize {
        match self {
            Self::All => total_entries,
            Self::Selected(entries) => entries.len(),
        }
    }

    fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }
}

fn parse_entry_no(token: &str, total_entries: usize) -> Result<usize, String> {
    let entry_no = token
        .parse::<usize>()
        .map_err(|_| format!("invalid entry number: '{token}'"))?;

    if entry_no == 0 {
        return Err("entry numbers start at 1".to_string());
    }
    if entry_no > total_entries {
        return Err(format!(
            "entry number {} is out of range (archive has {} entries)",
            entry_no, total_entries
        ));
    }

    Ok(entry_no)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CentralDirectoryInfo {
    total_entries: usize,
    cd_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StatusSummary {
    total_entries: usize,
    set_entries: usize,
    aggregate: AggregateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AggregateStatus {
    Set,
    Partial,
    Clear,
    Empty,
}

impl AggregateStatus {
    fn from_counts(total_entries: usize, set_entries: usize) -> Self {
        if total_entries == 0 {
            Self::Empty
        } else if set_entries == total_entries {
            Self::Set
        } else if set_entries == 0 {
            Self::Clear
        } else {
            Self::Partial
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Set => "✓ set",
            Self::Partial => "△ partial",
            Self::Clear => "✗ clear",
            Self::Empty => "- empty",
        }
    }
}

impl StatusSummary {
    fn from_counts(total_entries: usize, set_entries: usize) -> Self {
        Self {
            total_entries,
            set_entries,
            aggregate: AggregateStatus::from_counts(total_entries, set_entries),
        }
    }

    fn aggregate_label(self) -> &'static str {
        self.aggregate.label()
    }

    fn detail_label(self) -> String {
        format!(
            "{} ({}/{})",
            self.aggregate_label(),
            self.set_entries,
            self.total_entries
        )
    }
}

/// Run the requested operation on the given ZIP file.
pub fn process(path: &str, mode: Mode, selection: Option<&str>) -> Result<(), String> {
    let mut data = std::fs::read(path).map_err(|e| format!("cannot read '{}': {}", path, e))?;

    // Locate the End of Central Directory record
    let eocd_offset = find_eocd(&data)?;

    // Read entry count and Central Directory offset from EOCD / ZIP64 EOCD
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
        // Boundary check
        if !has_range(&data, pos, 46) {
            return Err(format!(
                "file ended unexpectedly at Central Directory entry {}",
                i + 1
            ));
        }

        // Signature check
        let sig = read_u32(&data, pos);
        if sig != CENTRAL_DIR_SIG {
            return Err(format!(
                "invalid Central Directory signature (offset: {:#x})",
                pos
            ));
        }

        // Read Central Directory entry fields
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

        // Read filename (for display)
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
            Mode::Status => {
                // Summary-only mode: collect counts without printing each entry.
            }
            Mode::Detail => {
                if is_selected {
                    let mark = if bit11_set { "✓ set" } else { "✗ clear" };
                    detail_rows.push(format!(" {:<4}  {:<6}  {}", i + 1, mark, fname));
                }
            }
            _ => {
                if !is_selected {
                    pos = pos.checked_add(entry_len).ok_or_else(|| {
                        format!(
                            "offset overflow while advancing Central Directory entry {}",
                            i + 1
                        )
                    })?;
                    continue;
                }

                let new_bit11 = match mode {
                    Mode::Set => true,
                    Mode::Clear => false,
                    Mode::Toggle => !bit11_set,
                    Mode::Status | Mode::Detail => unreachable!(),
                };

                let new_flags = if new_bit11 {
                    current_flags | BIT11
                } else {
                    current_flags & !BIT11
                };

                let lfh_offset =
                    read_lfh_offset_from_cd(&data, pos, fname_len, extra_len, i + 1)?;

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
                let new_lfh_flags = if new_bit11 {
                    lfh_flags | BIT11
                } else {
                    lfh_flags & !BIT11
                };

                if new_flags != current_flags || new_lfh_flags != lfh_flags {
                    write_u16(&mut data, cd_flag_offset, new_flags);
                    write_u16(&mut data, lfh_offset + 6, new_lfh_flags);
                    modified = true;
                }
            }
        }

        // Advance to the next entry
        pos = pos.checked_add(entry_len).ok_or_else(|| {
            format!(
                "offset overflow while advancing Central Directory entry {}",
                i + 1
            )
        })?;
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
    } else {
        if selection.is_all() {
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
    }

    Ok(())
}

/// Scan backwards through the data to locate the End of Central Directory (EOCD) record.
fn find_eocd(data: &[u8]) -> Result<usize, String> {
    // EOCD minimum size is 22 bytes
    if data.len() < 22 {
        return Err("file is too small to be a valid ZIP archive".to_string());
    }

    // ZIP comment can be at most 65535 bytes, so search the last 65557 bytes
    let search_from = data.len().saturating_sub(22 + 65535);

    // Scan backwards from the earliest possible EOCD position
    let mut i = data.len() - 22;
    loop {
        if read_u32(data, i) == EOCD_SIG {
            // Verify that the comment length matches the expected total size
            let comment_len = read_u16(data, i + 20) as usize;
            if i + 22 + comment_len == data.len() {
                return Ok(i);
            }
        }
        if i == search_from {
            break;
        }
        i -= 1;
    }

    Err("End of Central Directory record not found; not a valid ZIP archive".to_string())
}

fn read_central_directory_info(
    data: &[u8],
    eocd_offset: usize,
) -> Result<CentralDirectoryInfo, String> {
    let disk_number = read_u16(data, eocd_offset + 4);
    let cd_start_disk = read_u16(data, eocd_offset + 6);
    let entries_this_disk = read_u16(data, eocd_offset + 8);
    let total_entries_16 = read_u16(data, eocd_offset + 10);
    let cd_size_32 = read_u32(data, eocd_offset + 12);
    let cd_offset_32 = read_u32(data, eocd_offset + 16);

    if disk_number != 0 || cd_start_disk != 0 {
        return Err("multi-disk ZIP archives are not supported".to_string());
    }

    let needs_zip64 = entries_this_disk == 0xFFFF
        || total_entries_16 == 0xFFFF
        || cd_size_32 == 0xFFFF_FFFF
        || cd_offset_32 == 0xFFFF_FFFF;

    if !needs_zip64 {
        if entries_this_disk != total_entries_16 {
            return Err(
                "entry count mismatch between EOCD fields; multi-disk ZIP may be unsupported"
                    .to_string(),
            );
        }
        return Ok(CentralDirectoryInfo {
            total_entries: total_entries_16 as usize,
            cd_offset: cd_offset_32 as usize,
        });
    }

    read_zip64_central_directory_info(data, eocd_offset)
}

fn read_zip64_central_directory_info(
    data: &[u8],
    eocd_offset: usize,
) -> Result<CentralDirectoryInfo, String> {
    if eocd_offset < 20 {
        return Err("ZIP64 EOCD locator not found before EOCD".to_string());
    }

    let locator_offset = eocd_offset - 20;
    if read_u32(data, locator_offset) != ZIP64_EOCD_LOCATOR_SIG {
        return Err("ZIP64 EOCD locator signature not found".to_string());
    }

    let zip64_eocd_disk = read_u32(data, locator_offset + 4);
    let zip64_eocd_offset = read_u64(data, locator_offset + 8);
    let total_disks = read_u32(data, locator_offset + 16);

    if zip64_eocd_disk != 0 || total_disks != 1 {
        return Err("multi-disk ZIP64 archives are not supported".to_string());
    }

    let zip64_eocd_offset = usize::try_from(zip64_eocd_offset)
        .map_err(|_| "ZIP64 EOCD offset is too large for this platform".to_string())?;

    if !has_range(data, zip64_eocd_offset, 56) {
        return Err("ZIP64 EOCD record is out of bounds".to_string());
    }
    if read_u32(data, zip64_eocd_offset) != ZIP64_EOCD_SIG {
        return Err("invalid ZIP64 EOCD signature".to_string());
    }

    let record_size = read_u64(data, zip64_eocd_offset + 4);
    let record_size = usize::try_from(record_size)
        .map_err(|_| "ZIP64 EOCD record size is too large for this platform".to_string())?;
    let total_record_len = 12usize
        .checked_add(record_size)
        .ok_or_else(|| "ZIP64 EOCD record length overflow".to_string())?;

    if !has_range(data, zip64_eocd_offset, total_record_len) {
        return Err("ZIP64 EOCD record is truncated".to_string());
    }
    if record_size < 44 {
        return Err("ZIP64 EOCD record is too short".to_string());
    }

    let disk_number = read_u32(data, zip64_eocd_offset + 16);
    let cd_start_disk = read_u32(data, zip64_eocd_offset + 20);
    let entries_this_disk = read_u64(data, zip64_eocd_offset + 24);
    let total_entries = read_u64(data, zip64_eocd_offset + 32);
    let _cd_size = read_u64(data, zip64_eocd_offset + 40);
    let cd_offset = read_u64(data, zip64_eocd_offset + 48);

    if disk_number != 0 || cd_start_disk != 0 {
        return Err("multi-disk ZIP64 archives are not supported".to_string());
    }
    if entries_this_disk != total_entries {
        return Err(
            "entry count mismatch in ZIP64 EOCD; multi-disk ZIP64 may be unsupported".to_string(),
        );
    }

    let total_entries = usize::try_from(total_entries)
        .map_err(|_| "ZIP64 total entry count is too large for this platform".to_string())?;
    let cd_offset = usize::try_from(cd_offset)
        .map_err(|_| "ZIP64 Central Directory offset is too large for this platform".to_string())?;

    Ok(CentralDirectoryInfo {
        total_entries,
        cd_offset,
    })
}

fn read_lfh_offset_from_cd(
    data: &[u8],
    cd_pos: usize,
    fname_len: usize,
    extra_len: usize,
    entry_no: usize,
) -> Result<usize, String> {
    let lfh_offset_32 = read_u32(data, cd_pos + 42);
    if lfh_offset_32 != 0xFFFF_FFFF {
        return Ok(lfh_offset_32 as usize);
    }

    let compressed_size_32 = read_u32(data, cd_pos + 20);
    let uncompressed_size_32 = read_u32(data, cd_pos + 24);

    let extra_start = cd_pos + 46 + fname_len;
    let extra_end = extra_start + extra_len;
    let mut cursor = extra_start;

    while cursor + 4 <= extra_end {
        let field_id = read_u16(data, cursor);
        let field_size = read_u16(data, cursor + 2) as usize;
        cursor += 4;
        if cursor + field_size > extra_end {
            return Err(format!(
                "truncated extra field in Central Directory entry {}",
                entry_no
            ));
        }

        if field_id == ZIP64_EXTRA_FIELD_ID {
            return read_lfh_offset_from_zip64_extra(
                data,
                cursor,
                field_size,
                compressed_size_32,
                uncompressed_size_32,
                entry_no,
            );
        }
        cursor += field_size;
    }

    Err(format!(
        "ZIP64 extra field missing for Central Directory entry {}",
        entry_no
    ))
}

fn read_lfh_offset_from_zip64_extra(
    data: &[u8],
    field_start: usize,
    field_size: usize,
    compressed_size_32: u32,
    uncompressed_size_32: u32,
    entry_no: usize,
) -> Result<usize, String> {
    let mut cursor = 0usize;

    if uncompressed_size_32 == 0xFFFF_FFFF {
        cursor = cursor.checked_add(8).ok_or_else(|| {
            format!(
                "overflow while parsing ZIP64 extra field in entry {}",
                entry_no
            )
        })?;
    }
    if compressed_size_32 == 0xFFFF_FFFF {
        cursor = cursor.checked_add(8).ok_or_else(|| {
            format!(
                "overflow while parsing ZIP64 extra field in entry {}",
                entry_no
            )
        })?;
    }
    if cursor + 8 > field_size {
        return Err(format!(
            "ZIP64 extra field does not contain Local Header offset for entry {}",
            entry_no
        ));
    }

    let value_offset = field_start + cursor;
    let lfh_offset_64 = read_u64(data, value_offset);
    usize::try_from(lfh_offset_64)
        .map_err(|_| format!("Local Header offset is too large in entry {}", entry_no))
}

#[inline]
fn has_range(data: &[u8], offset: usize, len: usize) -> bool {
    match offset.checked_add(len) {
        Some(end) => end <= data.len(),
        None => false,
    }
}

// ── Utilities ───────────────────────────────────────────────

#[inline]
fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

#[inline]
fn read_u64(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
}

#[inline]
fn write_u16(data: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    data[offset] = bytes[0];
    data[offset + 1] = bytes[1];
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
        buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
        buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(buf: &mut [u8], offset: usize, value: u64) {
        buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn make_zip_with_missing_zip64_lfh_extra(bit11_set: bool) -> Vec<u8> {
        let filename = b"sample.txt";
        let cd_len = 46 + filename.len();
        let eocd_offset = cd_len;
        let mut data = vec![0u8; cd_len + 22];

        write_u32(&mut data, 0, CENTRAL_DIR_SIG);
        write_u16(&mut data, 4, 20);
        write_u16(&mut data, 6, 20);
        write_u16(&mut data, 8, if bit11_set { BIT11 } else { 0 });
        write_u16(&mut data, 28, filename.len() as u16);
        write_u16(&mut data, 30, 0);
        write_u16(&mut data, 32, 0);
        write_u32(&mut data, 42, 0xFFFF_FFFF);
        data[46..46 + filename.len()].copy_from_slice(filename);

        write_u32(&mut data, eocd_offset, EOCD_SIG);
        write_u16(&mut data, eocd_offset + 4, 0);
        write_u16(&mut data, eocd_offset + 6, 0);
        write_u16(&mut data, eocd_offset + 8, 1);
        write_u16(&mut data, eocd_offset + 10, 1);
        write_u32(&mut data, eocd_offset + 12, cd_len as u32);
        write_u32(&mut data, eocd_offset + 16, 0);
        write_u16(&mut data, eocd_offset + 20, 0);

        data
    }

    fn make_simple_zip(entries: &[(&[u8], u16, u16)]) -> Vec<u8> {
        let mut data = Vec::new();
        let mut local_offsets = Vec::new();

        for (name, lfh_flags, _) in entries {
            local_offsets.push(data.len() as u32);
            data.extend_from_slice(&LOCAL_FILE_HEADER_SIG.to_le_bytes());
            data.extend_from_slice(&20u16.to_le_bytes());
            data.extend_from_slice(&lfh_flags.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&(name.len() as u16).to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(name);
        }

        let cd_offset = data.len() as u32;

        for ((name, _, cd_flags), lfh_offset) in entries.iter().zip(local_offsets.iter()) {
            data.extend_from_slice(&CENTRAL_DIR_SIG.to_le_bytes());
            data.extend_from_slice(&20u16.to_le_bytes());
            data.extend_from_slice(&20u16.to_le_bytes());
            data.extend_from_slice(&cd_flags.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&(name.len() as u16).to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&lfh_offset.to_le_bytes());
            data.extend_from_slice(name);
        }

        let cd_size = data.len() as u32 - cd_offset;
        data.extend_from_slice(&EOCD_SIG.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        data.extend_from_slice(&cd_size.to_le_bytes());
        data.extend_from_slice(&cd_offset.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());

        data
    }

    fn read_bit11_pairs(data: &[u8]) -> Vec<(bool, bool)> {
        let eocd_offset = find_eocd(data).unwrap();
        let cd_info = read_central_directory_info(data, eocd_offset).unwrap();
        let mut pos = cd_info.cd_offset;
        let mut pairs = Vec::new();

        for entry_no in 0..cd_info.total_entries {
            let fname_len = read_u16(data, pos + 28) as usize;
            let extra_len = read_u16(data, pos + 30) as usize;
            let comment_len = read_u16(data, pos + 32) as usize;
            let entry_len = 46 + fname_len + extra_len + comment_len;
            let cd_flags = read_u16(data, pos + 8);
            let lfh_offset =
                read_lfh_offset_from_cd(data, pos, fname_len, extra_len, entry_no + 1).unwrap();
            let lfh_flags = read_u16(data, lfh_offset + 6);
            pairs.push(((lfh_flags & BIT11) != 0, (cd_flags & BIT11) != 0));
            pos += entry_len;
        }

        pairs
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "zipbit11-{name}-{}-{nanos}.zip",
            std::process::id()
        ))
    }

    fn with_temp_zip<F>(name: &str, data: &[u8], test: F)
    where
        F: FnOnce(&Path),
    {
        let path = unique_test_path(name);
        std::fs::write(&path, data).unwrap();
        test(&path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn aggregate_status_from_counts() {
        assert_eq!(AggregateStatus::from_counts(3, 3), AggregateStatus::Set);
        assert_eq!(AggregateStatus::from_counts(3, 1), AggregateStatus::Partial);
        assert_eq!(AggregateStatus::from_counts(3, 0), AggregateStatus::Clear);
        assert_eq!(AggregateStatus::from_counts(0, 0), AggregateStatus::Empty);
    }

    #[test]
    fn status_summary_labels_match_expected_symbols() {
        assert_eq!(StatusSummary::from_counts(3, 3).aggregate_label(), "✓ set");
        assert_eq!(
            StatusSummary::from_counts(3, 1).aggregate_label(),
            "△ partial"
        );
        assert_eq!(
            StatusSummary::from_counts(3, 0).aggregate_label(),
            "✗ clear"
        );
    }

    #[test]
    fn detail_summary_includes_counts() {
        assert_eq!(
            StatusSummary::from_counts(5, 2).detail_label(),
            "△ partial (2/5)"
        );
    }

    #[test]
    fn parses_entry_selector_with_ranges_and_duplicates() {
        let selection = EntrySelection::parse(Some("3, 1-2,2"), 3).unwrap();
        assert_eq!(
            selection,
            EntrySelection::Selected(BTreeSet::from([1usize, 2, 3]))
        );
    }

    #[test]
    fn rejects_descending_entry_range() {
        let err = EntrySelection::parse(Some("3-1"), 3).unwrap_err();
        assert_eq!(err, "entry range '3-1' is descending");
    }

    #[test]
    fn rejects_out_of_range_entry_number() {
        let err = EntrySelection::parse(Some("4"), 3).unwrap_err();
        assert_eq!(err, "entry number 4 is out of range (archive has 3 entries)");
    }

    #[test]
    fn reads_standard_eocd_info() {
        let mut data = vec![0u8; 200];
        let eocd = 178;
        write_u32(&mut data, eocd, EOCD_SIG);
        write_u16(&mut data, eocd + 4, 0); // disk number
        write_u16(&mut data, eocd + 6, 0); // central dir start disk
        write_u16(&mut data, eocd + 8, 3); // entries on this disk
        write_u16(&mut data, eocd + 10, 3); // total entries
        write_u32(&mut data, eocd + 12, 123); // cd size
        write_u32(&mut data, eocd + 16, 40); // cd offset
        write_u16(&mut data, eocd + 20, 0); // comment length

        let info = read_central_directory_info(&data, eocd).unwrap();
        assert_eq!(
            info,
            CentralDirectoryInfo {
                total_entries: 3,
                cd_offset: 40
            }
        );
    }

    #[test]
    fn reads_zip64_eocd_info() {
        let mut data = vec![0u8; 142];
        let zip64_eocd = 40;
        let locator = 100;
        let eocd = 120;

        // ZIP64 EOCD record
        write_u32(&mut data, zip64_eocd, ZIP64_EOCD_SIG);
        write_u64(&mut data, zip64_eocd + 4, 44); // record size (without 12-byte lead)
        write_u16(&mut data, zip64_eocd + 12, 45); // version made by
        write_u16(&mut data, zip64_eocd + 14, 45); // version needed
        write_u32(&mut data, zip64_eocd + 16, 0); // disk number
        write_u32(&mut data, zip64_eocd + 20, 0); // central dir start disk
        write_u64(&mut data, zip64_eocd + 24, 70_000); // entries on this disk
        write_u64(&mut data, zip64_eocd + 32, 70_000); // total entries
        write_u64(&mut data, zip64_eocd + 40, 1234); // central dir size
        write_u64(&mut data, zip64_eocd + 48, 50); // central dir offset

        // ZIP64 EOCD locator (must be directly before EOCD)
        write_u32(&mut data, locator, ZIP64_EOCD_LOCATOR_SIG);
        write_u32(&mut data, locator + 4, 0); // disk with ZIP64 EOCD
        write_u64(&mut data, locator + 8, zip64_eocd as u64); // ZIP64 EOCD offset
        write_u32(&mut data, locator + 16, 1); // total disks

        // EOCD with ZIP64 sentinels
        write_u32(&mut data, eocd, EOCD_SIG);
        write_u16(&mut data, eocd + 4, 0);
        write_u16(&mut data, eocd + 6, 0);
        write_u16(&mut data, eocd + 8, 0xFFFF);
        write_u16(&mut data, eocd + 10, 0xFFFF);
        write_u32(&mut data, eocd + 12, 0xFFFF_FFFF);
        write_u32(&mut data, eocd + 16, 0xFFFF_FFFF);
        write_u16(&mut data, eocd + 20, 0);

        let info = read_central_directory_info(&data, eocd).unwrap();
        assert_eq!(
            info,
            CentralDirectoryInfo {
                total_entries: 70_000,
                cd_offset: 50
            }
        );
    }

    #[test]
    fn rejects_zip64_without_locator() {
        let mut data = vec![0u8; 60];
        let eocd = 38;
        write_u32(&mut data, eocd, EOCD_SIG);
        write_u16(&mut data, eocd + 4, 0);
        write_u16(&mut data, eocd + 6, 0);
        write_u16(&mut data, eocd + 8, 0xFFFF);
        write_u16(&mut data, eocd + 10, 0xFFFF);
        write_u32(&mut data, eocd + 12, 0xFFFF_FFFF);
        write_u32(&mut data, eocd + 16, 0xFFFF_FFFF);
        write_u16(&mut data, eocd + 20, 0);

        let err = read_central_directory_info(&data, eocd).unwrap_err();
        assert!(err.contains("ZIP64 EOCD locator"));
    }

    #[test]
    fn status_does_not_require_local_file_header_offset() {
        let data = make_zip_with_missing_zip64_lfh_extra(false);
        with_temp_zip("status-no-lfh", &data, |path| {
            let result = process(path.to_str().unwrap(), Mode::Status, None);
            assert!(result.is_ok(), "status should ignore LFH lookup: {result:?}");
        });
    }

    #[test]
    fn detail_does_not_require_local_file_header_offset() {
        let data = make_zip_with_missing_zip64_lfh_extra(true);
        with_temp_zip("detail-no-lfh", &data, |path| {
            let result = process(path.to_str().unwrap(), Mode::Detail, None);
            assert!(result.is_ok(), "detail should ignore LFH lookup: {result:?}");
        });
    }

    #[test]
    fn set_updates_only_selected_entries() {
        let data = make_simple_zip(&[
            (b"one.txt", 0, 0),
            (b"two.txt", 0, 0),
            (b"three.txt", 0, 0),
        ]);

        with_temp_zip("set-selected", &data, |path| {
            process(path.to_str().unwrap(), Mode::Set, Some("2-3")).unwrap();
            let written = std::fs::read(path).unwrap();
            assert_eq!(
                read_bit11_pairs(&written),
                vec![(false, false), (true, true), (true, true)]
            );
        });
    }

    #[test]
    fn clear_updates_selected_entries_only() {
        let data = make_simple_zip(&[
            (b"one.txt", BIT11, BIT11),
            (b"two.txt", BIT11, BIT11),
            (b"three.txt", BIT11, BIT11),
        ]);

        with_temp_zip("clear-selected", &data, |path| {
            process(path.to_str().unwrap(), Mode::Clear, Some("1,3")).unwrap();
            let written = std::fs::read(path).unwrap();
            assert_eq!(
                read_bit11_pairs(&written),
                vec![(false, false), (true, true), (false, false)]
            );
        });
    }

    #[test]
    fn set_repairs_selected_entry_when_local_header_is_out_of_sync() {
        let data = make_simple_zip(&[(b"broken.txt", 0, BIT11)]);

        with_temp_zip("set-repair", &data, |path| {
            process(path.to_str().unwrap(), Mode::Set, Some("1")).unwrap();
            let written = std::fs::read(path).unwrap();
            assert_eq!(read_bit11_pairs(&written), vec![(true, true)]);
        });
    }
}
