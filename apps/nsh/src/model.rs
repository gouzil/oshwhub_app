pub use oshwhub_core::search_bom;
use oshwhub_core::{
    error::{AppError, Result},
    model::{FeedPage, FeedQuery, ProjectSummary},
};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct FeedSnapshot {
    pub items: Vec<ProjectSummary>,
    pub loading: bool,
    pub refreshing: bool,
    pub error: Option<AppError>,
    pub refresh_error: Option<AppError>,
    pub page: u32,
    pub has_more: bool,
    pub total: u64,
}

#[derive(Debug)]
pub struct FeedRequest {
    pub query: FeedQuery,
    generation: u64,
    refreshing: bool,
}

#[derive(Debug)]
pub struct FeedController {
    snapshot: FeedSnapshot,
    generation: u64,
    query: String,
    sort: String,
}

impl Default for FeedSnapshot {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            loading: false,
            refreshing: false,
            error: None,
            refresh_error: None,
            page: 0,
            has_more: true,
            total: 0,
        }
    }
}

impl Default for FeedController {
    fn default() -> Self {
        Self {
            snapshot: FeedSnapshot::default(),
            generation: 0,
            query: String::new(),
            sort: "recommend".into(),
        }
    }
}

impl FeedController {
    pub fn snapshot(&self) -> &FeedSnapshot {
        &self.snapshot
    }

    pub fn search(&mut self, query: &str, sort: &str) -> FeedRequest {
        self.generation += 1;
        self.query = query.into();
        self.sort = sort.into();
        self.snapshot = FeedSnapshot::default();
        self.load_page(1, false)
    }

    pub fn load_more(&mut self) -> Option<FeedRequest> {
        if self.snapshot.loading || self.snapshot.refreshing || !self.snapshot.has_more {
            return None;
        }
        Some(self.load_page(self.snapshot.page + 1, false))
    }

    pub fn refresh(&mut self) -> Option<FeedRequest> {
        if self.snapshot.refreshing {
            return None;
        }
        self.generation += 1;
        Some(self.load_page(1, true))
    }

    pub fn load_more_if_visible(&mut self, active: bool) -> Option<FeedRequest> {
        if !active
            || self.snapshot.items.is_empty()
            || self.snapshot.error.is_some()
            || self.snapshot.refresh_error.is_some()
        {
            return None;
        }
        self.load_more()
    }

    fn load_page(&mut self, page: u32, refreshing: bool) -> FeedRequest {
        self.snapshot.loading = !refreshing;
        self.snapshot.refreshing = refreshing;
        self.snapshot.error = None;
        self.snapshot.refresh_error = None;
        FeedRequest {
            query: FeedQuery {
                page,
                page_size: 20,
                query: self.query.clone(),
                sort: self.sort.clone(),
            },
            generation: self.generation,
            refreshing,
        }
    }

    pub fn complete(&mut self, request: FeedRequest, result: Result<FeedPage>) -> bool {
        if request.generation != self.generation {
            return false;
        }
        self.snapshot.loading = false;
        self.snapshot.refreshing = false;
        match result {
            Ok(page) => {
                if request.query.page == 1 {
                    self.snapshot.items.clear();
                }
                let mut seen: HashSet<_> = self
                    .snapshot
                    .items
                    .iter()
                    .map(|item| item.id.clone())
                    .collect();
                self.snapshot.items.extend(
                    page.items
                        .into_iter()
                        .filter(|item| seen.insert(item.id.clone())),
                );
                self.snapshot.page = request.query.page;
                self.snapshot.has_more = page.has_more;
                self.snapshot.total = page.total;
                self.snapshot.error = None;
                self.snapshot.refresh_error = None;
            }
            Err(error) => {
                self.snapshot.error = None;
                self.snapshot.refresh_error = None;
                if request.refreshing {
                    self.snapshot.refresh_error = Some(error);
                } else {
                    self.snapshot.error = Some(error);
                }
            }
        }
        true
    }
}
