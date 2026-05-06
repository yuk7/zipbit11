use super::*;
use crate::zip::eocd::CentralDirectoryInfo;
use crate::zip::status::AggregateStatus;
use std::collections::BTreeSet;
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
    assert_eq!(
        err,
        "entry number 4 is out of range (archive has 3 entries)"
    );
}

#[test]
fn reads_standard_eocd_info() {
    let mut data = vec![0u8; 200];
    let eocd = 178;
    write_u32(&mut data, eocd, EOCD_SIG);
    write_u16(&mut data, eocd + 4, 0);
    write_u16(&mut data, eocd + 6, 0);
    write_u16(&mut data, eocd + 8, 3);
    write_u16(&mut data, eocd + 10, 3);
    write_u32(&mut data, eocd + 12, 123);
    write_u32(&mut data, eocd + 16, 40);
    write_u16(&mut data, eocd + 20, 0);

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

    write_u32(&mut data, zip64_eocd, ZIP64_EOCD_SIG);
    write_u64(&mut data, zip64_eocd + 4, 44);
    write_u16(&mut data, zip64_eocd + 12, 45);
    write_u16(&mut data, zip64_eocd + 14, 45);
    write_u32(&mut data, zip64_eocd + 16, 0);
    write_u32(&mut data, zip64_eocd + 20, 0);
    write_u64(&mut data, zip64_eocd + 24, 70_000);
    write_u64(&mut data, zip64_eocd + 32, 70_000);
    write_u64(&mut data, zip64_eocd + 40, 1234);
    write_u64(&mut data, zip64_eocd + 48, 50);

    write_u32(&mut data, locator, ZIP64_EOCD_LOCATOR_SIG);
    write_u32(&mut data, locator + 4, 0);
    write_u64(&mut data, locator + 8, zip64_eocd as u64);
    write_u32(&mut data, locator + 16, 1);

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
        assert!(
            result.is_ok(),
            "status should ignore LFH lookup: {result:?}"
        );
    });
}

#[test]
fn detail_does_not_require_local_file_header_offset() {
    let data = make_zip_with_missing_zip64_lfh_extra(true);
    with_temp_zip("detail-no-lfh", &data, |path| {
        let result = process(path.to_str().unwrap(), Mode::Detail, None);
        assert!(
            result.is_ok(),
            "detail should ignore LFH lookup: {result:?}"
        );
    });
}

#[test]
fn set_updates_only_selected_entries() {
    let data = make_simple_zip(&[(b"one.txt", 0, 0), (b"two.txt", 0, 0), (b"three.txt", 0, 0)]);

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
