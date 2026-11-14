use std::ops::Range;

use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumePlan {
    received: Vec<Range<u64>>,
    total: u64,
}

impl ResumePlan {
    pub fn new(total: u64) -> Self {
        Self {
            received: Vec::new(),
            total,
        }
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn received_bytes(&self) -> u64 {
        self.received
            .iter()
            .map(|range| range.end - range.start)
            .sum()
    }

    pub fn received(&self) -> &[Range<u64>] {
        &self.received
    }

    pub fn is_complete(&self) -> bool {
        self.received_bytes() == self.total
    }

    pub fn add(&mut self, start: u64, end: u64) -> Result<(), ResumeError> {
        if start >= end || end > self.total {
            return Err(ResumeError::OutOfBounds { start, end });
        }
        self.received.push(start..end);
        self.received.sort_by_key(|range| range.start);
        let mut merged: Vec<Range<u64>> = Vec::with_capacity(self.received.len());
        for range in self.received.drain(..) {
            if let Some(last) = merged.last_mut()
                && range.start <= last.end
            {
                last.end = last.end.max(range.end);
                continue;
            }
            merged.push(range);
        }
        self.received = merged;
        Ok(())
    }

    pub fn missing(&self, chunk_size: u64) -> Vec<(u64, u64)> {
        if chunk_size == 0 || self.total == 0 {
            return Vec::new();
        }
        let mut missing = Vec::new();
        let mut cursor = 0_u64;
        for range in &self.received {
            if range.start > cursor {
                push_ranges(&mut missing, cursor, range.start, chunk_size);
            }
            cursor = cursor.max(range.end);
        }
        if cursor < self.total {
            push_ranges(&mut missing, cursor, self.total, chunk_size);
        }
        missing
    }

    pub fn next_offset(&self) -> Option<u64> {
        if self.is_complete() {
            return None;
        }
        let mut cursor = 0;
        for range in &self.received {
            if cursor < range.start {
                return Some(cursor);
            }
            cursor = cursor.max(range.end);
        }
        (cursor < self.total).then_some(cursor)
    }

    pub fn accepts(&self, offset: u64, length: usize) -> bool {
        let Ok(length) = u64::try_from(length) else {
            return false;
        };
        let Some(end) = offset.checked_add(length) else {
            return false;
        };
        end <= self.total
            && !self
                .received
                .iter()
                .any(|range| offset < range.end && end > range.start)
    }
}

fn push_ranges(ranges: &mut Vec<(u64, u64)>, start: u64, end: u64, chunk_size: u64) {
    let mut cursor = start;
    while cursor < end {
        let next = cursor.saturating_add(chunk_size).min(end);
        ranges.push((cursor, next));
        cursor = next;
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResumeError {
    #[error("resume range {start}..{end} is outside the attachment")]
    OutOfBounds { start: u64, end: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_ranges_cover_gaps() {
        let mut plan = ResumePlan::new(100);
        assert_eq!(
            plan.missing(30),
            vec![(0, 30), (30, 60), (60, 90), (90, 100)]
        );
        plan.add(30, 60).expect("add");
        assert_eq!(plan.received_bytes(), 30);
        assert_eq!(plan.next_offset(), Some(0));
        assert_eq!(plan.missing(30), vec![(0, 30), (60, 90), (90, 100)]);
        plan.add(0, 30).expect("add");
        plan.add(60, 100).expect("add");
        assert!(plan.is_complete());
        assert_eq!(plan.next_offset(), None);
    }

    #[test]
    fn merges_overlapping_ranges_and_rejects_out_of_bounds() {
        let mut plan = ResumePlan::new(10);
        plan.add(0, 5).expect("add");
        plan.add(3, 8).expect("add");
        assert_eq!(plan.received().len(), 1);
        assert_eq!(plan.received()[0], 0..8);
        assert!(plan.add(8, 11).is_err());
        assert!(plan.add(5, 5).is_err());
    }

    #[test]
    fn accepts_only_missing_bytes() {
        let mut plan = ResumePlan::new(10);
        plan.add(0, 5).expect("add");
        assert!(!plan.accepts(0, 1));
        assert!(!plan.accepts(4, 2));
        assert!(plan.accepts(5, 2));
        assert!(!plan.accepts(9, 2));
    }
}
