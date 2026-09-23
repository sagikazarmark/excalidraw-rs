//! The two published pagination contracts. They are deliberately not unified.
use crate::{Error, de};
use serde::Deserialize;

/// Offset pagination, used by collections, scenes, users and invites.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PageRequest {
    /// 1..=100. `None` omits the parameter; the documented server default is 10.
    pub limit: Option<u32>,
    /// 0..=9007199254740991. `None` omits the parameter; the default is 0.
    pub offset: Option<u64>,
}

impl PageRequest {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if let Some(limit) = self.limit
            && !(1..=100).contains(&limit)
        {
            return Err(Error::invalid(
                "pagination limit",
                format!("must be 1..=100, got {limit}"),
            ));
        }
        if let Some(offset) = self.offset
            && offset > de::MAX_SAFE_INTEGER
        {
            return Err(Error::invalid(
                "pagination offset",
                format!("must be 0..={}, got {offset}", de::MAX_SAFE_INTEGER),
            ));
        }
        Ok(())
    }
}

/// One page of an offset-paginated collection.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub limit: u64,
    pub offset: u64,
    pub has_next_page: bool,
    pub data: Vec<T>,
}

impl<T> Page<T> {
    /// The request that would fetch the following page, when one is reported.
    ///
    /// Offset pagination over a mutating collection can repeat or skip items:
    /// consecutive pages are not a snapshot.
    ///
    /// An empty page ends the walk even when it reports `hasNextPage`: its
    /// following offset is its own, so requesting it would fetch the same page
    /// again, and a loop that followed it would never make progress.
    pub fn next_request(&self) -> Option<PageRequest> {
        if !self.has_next_page || self.data.is_empty() {
            return None;
        }
        Some(PageRequest {
            limit: u32::try_from(self.limit).ok(),
            offset: Some(self.offset.saturating_add(self.data.len() as u64)),
        })
    }
}

/// The request that opens a walk, or `None` when `max_items` is already spent.
///
/// The zero-budget case of the rule [`absorb`] enforces after every page: a
/// walk that can keep nothing must not fetch its first page either.
#[cfg_attr(not(feature = "client-core"), allow(dead_code))]
pub(crate) fn first(start: PageRequest, max_items: usize) -> Option<PageRequest> {
    (max_items > 0).then_some(start)
}

/// Absorb one page into `collected` and report the request that should follow.
///
/// `None` ends the walk: either the server reported no next page, or the
/// `max_items` budget is spent. Checking the budget *after* absorbing is what
/// stops the walk fetching a page it cannot use — the page that completes the
/// budget is the last one requested, not the second to last.
///
/// Sans-io on purpose: both clients share this rule, so the boundary condition
/// has one implementation and one test.
///
/// Only the clients call it, and `default = []` builds none of them, so the
/// default build sees it as dead. `ApiKey`'s secret carries the same
/// annotation for the same reason.
#[cfg_attr(not(feature = "client-core"), allow(dead_code))]
pub(crate) fn absorb<T>(
    page: Page<T>,
    collected: &mut Vec<T>,
    max_items: usize,
) -> Option<PageRequest> {
    let next = page.next_request();
    for item in page.data {
        if collected.len() >= max_items {
            return None;
        }
        collected.push(item);
    }
    if collected.len() >= max_items {
        return None;
    }
    next
}

de::open_string_enum! {
    /// Query filter for `GET /logs`. The log entry's own `operation` field is an
    /// unconstrained string and is not decoded through this type.
    LogOperation {
        Create => "create",
        Read => "read",
        Update => "update",
        Delete => "delete",
    }
}

/// Query parameters for `GET /logs`, which does not use offset pagination.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogQuery {
    pub limit: Option<u32>,
    pub offset: Option<u64>,
    pub cursor: Option<String>,
    /// Mutually exclusive with `cursor`. The published artifact documents no
    /// exclusion, but sending both returns `500` (observed 2026-09-21), so
    /// `LogQuery::validate` rejects the combination before a request is built.
    pub page: Option<String>,
    pub user: Option<String>,
    pub action: Option<String>,
    pub operation: Option<LogOperation>,
    /// Example format `2024-01-01`. No pattern is published.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

impl LogQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        PageRequest {
            limit: self.limit,
            offset: self.offset,
        }
        .validate()?;
        if self.cursor.is_some() && self.page.is_some() {
            return Err(Error::invalid(
                "log pagination",
                "cursor and page are mutually exclusive; sending both returns 500",
            ));
        }
        if let Some(LogOperation::Unknown(value)) = &self.operation {
            return Err(Error::invalid(
                "log operation filter",
                format!("must be create, read, update or delete, got {value}"),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(data: Vec<u8>, has_next_page: bool) -> Page<u8> {
        Page {
            limit: 10,
            offset: 20,
            has_next_page,
            data,
        }
    }

    #[test]
    fn an_empty_page_is_never_followed_even_when_it_reports_a_next() {
        assert_eq!(page(vec![], true).next_request(), None);
        assert_eq!(
            page(vec![1, 2], true).next_request(),
            Some(PageRequest::new().limit(10).offset(22))
        );
        assert_eq!(page(vec![1, 2], false).next_request(), None);
    }

    #[test]
    fn a_spent_budget_opens_no_walk() {
        let start = PageRequest::new().limit(1);
        assert_eq!(first(start, 0), None);
        assert_eq!(first(start, 1), Some(start));
    }
}
