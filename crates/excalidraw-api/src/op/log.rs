//! Audit log retrieval.
use crate::{Error, JsonOperation, LogQuery, Method, Request, model::LogPage};

/// `GET /logs` — workspace activity.
///
/// This endpoint does not use the offset pagination of the other list routes: it
/// returns `nextCursor`/`hasMore`, plus page counts that the schema marks
/// required while documenting them as page-mode only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetLogs {
    pub query: LogQuery,
}

impl JsonOperation for GetLogs {
    type Output = LogPage;
    fn request(&self) -> Result<Request, Error> {
        self.query.validate()?;
        Ok(Request::new(Method::Get, "/logs")
            .maybe_query("limit", self.query.limit)
            .maybe_query("offset", self.query.offset)
            .maybe_query("cursor", self.query.cursor.as_deref())
            .maybe_query("page", self.query.page.as_deref())
            .maybe_query("user", self.query.user.as_deref())
            .maybe_query("action", self.query.action.as_deref())
            .maybe_query(
                "operation",
                self.query.operation.as_ref().map(|o| o.as_str()),
            )
            .maybe_query("dateFrom", self.query.date_from.as_deref())
            .maybe_query("dateTo", self.query.date_to.as_deref()))
    }
}
