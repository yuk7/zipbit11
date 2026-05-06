#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StatusSummary {
    pub(super) total_entries: usize,
    set_entries: usize,
    aggregate: AggregateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum AggregateStatus {
    Set,
    Partial,
    Clear,
    Empty,
}

impl AggregateStatus {
    pub(super) fn from_counts(total_entries: usize, set_entries: usize) -> Self {
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
    pub(super) fn from_counts(total_entries: usize, set_entries: usize) -> Self {
        Self {
            total_entries,
            set_entries,
            aggregate: AggregateStatus::from_counts(total_entries, set_entries),
        }
    }

    pub(super) fn aggregate_label(self) -> &'static str {
        self.aggregate.label()
    }

    pub(super) fn detail_label(self) -> String {
        format!(
            "{} ({}/{})",
            self.aggregate_label(),
            self.set_entries,
            self.total_entries
        )
    }
}
