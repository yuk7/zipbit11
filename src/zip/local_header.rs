use super::bytes::{read_u16, read_u32, read_u64};
use super::ZIP64_EXTRA_FIELD_ID;

pub(super) fn read_lfh_offset_from_cd(
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
