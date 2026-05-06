use super::bytes::{has_range, read_u16, read_u32, read_u64};
use super::{EOCD_SIG, ZIP64_EOCD_LOCATOR_SIG, ZIP64_EOCD_SIG};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CentralDirectoryInfo {
    pub(super) total_entries: usize,
    pub(super) cd_offset: usize,
}

/// Scan backwards through the data to locate the End of Central Directory record.
pub(super) fn find_eocd(data: &[u8]) -> Result<usize, String> {
    if data.len() < 22 {
        return Err("file is too small to be a valid ZIP archive".to_string());
    }

    let search_from = data.len().saturating_sub(22 + 65535);
    let mut i = data.len() - 22;
    loop {
        if read_u32(data, i) == EOCD_SIG {
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

pub(super) fn read_central_directory_info(
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
