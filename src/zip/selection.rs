use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum EntrySelection {
    All,
    Selected(BTreeSet<usize>),
}

impl EntrySelection {
    pub(super) fn parse(raw: Option<&str>, total_entries: usize) -> Result<Self, String> {
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

    pub(super) fn includes(&self, entry_no: usize) -> bool {
        match self {
            Self::All => true,
            Self::Selected(entries) => entries.contains(&entry_no),
        }
    }

    pub(super) fn count(&self, total_entries: usize) -> usize {
        match self {
            Self::All => total_entries,
            Self::Selected(entries) => entries.len(),
        }
    }

    pub(super) fn is_all(&self) -> bool {
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
