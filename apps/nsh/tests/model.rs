#[path = "../src/model.rs"]
mod model;

use model::{FeedController, search_bom};
use oshwhub_core::{
    error::AppError,
    model::{BomItem, FeedPage, ProjectSummary},
};

fn project(id: &str) -> ProjectSummary {
    ProjectSummary {
        id: id.into(),
        name: id.into(),
        path: id.into(),
        description: String::new(),
        cover: None,
        author: "作者".into(),
        author_avatar: None,
        likes: 0,
        views: 0,
        tags: Vec::new(),
        updated_at: None,
        url: format!("https://oshwhub.com/author/{id}"),
    }
}

fn page(ids: &[&str], number: u32, has_more: bool) -> FeedPage {
    FeedPage {
        items: ids.iter().map(|id| project(id)).collect(),
        page: number,
        total: 50,
        has_more,
    }
}

fn ids(feed: &FeedController) -> Vec<&str> {
    feed.snapshot()
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect()
}

#[test]
fn default_request_uses_page_one_and_blocks_duplicate_loading() {
    let mut feed = FeedController::default();
    let request = feed.load_more().unwrap();
    assert_eq!(request.query.page, 1);
    assert_eq!(request.query.page_size, 20);
    assert_eq!(request.query.query, "");
    assert_eq!(request.query.sort, "recommend");
    assert!(feed.snapshot().loading);
    assert!(!feed.snapshot().refreshing);
    assert!(feed.load_more().is_none());
    assert!(feed.complete(request, Ok(page(&["first"], 1, true))));
    assert_eq!(ids(&feed), ["first"]);
    assert!(!feed.snapshot().loading);
    assert_eq!(feed.snapshot().total, 50);
}

#[test]
fn newer_search_ignores_stale_successes_and_errors_without_clearing_loading() {
    for fail in [false, true] {
        let mut feed = FeedController::default();
        let old = feed.search("old", "recommend");
        let current = feed.search("new", "new");
        assert_eq!(current.query.query, "new");
        assert_eq!(current.query.sort, "new");
        assert_eq!(current.query.page, 1);
        let stale = if fail {
            Err(AppError::new("offline", "过期错误"))
        } else {
            Ok(page(&["old"], 1, false))
        };
        assert!(!feed.complete(old, stale));
        assert!(feed.snapshot().loading);
        assert!(feed.snapshot().error.is_none());
        assert!(feed.complete(current, Ok(page(&["new"], 1, true))));
        assert_eq!(ids(&feed), ["new"]);
        assert_eq!(feed.snapshot().page, 1);
    }
}

#[test]
fn failed_page_retries_the_same_number_and_deduplicates_in_original_order() {
    let mut feed = FeedController::default();
    let first = feed.search("", "recommend");
    feed.complete(first, Ok(page(&["a", "a", "b"], 1, true)));
    let second = feed.load_more().unwrap();
    assert_eq!(second.query.page, 2);
    feed.complete(second, Err(AppError::new("offline", "离线")));
    assert_eq!(feed.snapshot().page, 1);
    assert_eq!(ids(&feed), ["a", "b"]);
    assert_eq!(feed.snapshot().error.as_ref().unwrap().message, "离线");
    assert!(feed.snapshot().refresh_error.is_none());
    assert!(!feed.snapshot().loading);

    let retry = feed.load_more().unwrap();
    assert_eq!(retry.query.page, 2);
    assert!(feed.snapshot().error.is_none());
    let mut response = page(&["b", "c", "c"], 2, false);
    response.items[0].name = "重复项目不覆盖旧卡片".into();
    feed.complete(retry, Ok(response));
    assert_eq!(ids(&feed), ["a", "b", "c"]);
    assert_eq!(feed.snapshot().items[1].name, "b");
    assert_eq!(feed.snapshot().page, 2);
    assert!(!feed.snapshot().has_more);
    assert!(feed.load_more().is_none());
}

#[test]
fn search_resets_cards_errors_and_pagination_for_the_selected_sort() {
    let mut feed = FeedController::default();
    let first = feed.search("old", "recommend");
    feed.complete(first, Ok(page(&["old"], 1, false)));
    let refresh = feed.refresh().unwrap();
    feed.complete(refresh, Err(AppError::new("offline", "刷新离线")));
    let next = feed.search(" 电源 ", "like");
    assert_eq!(next.query.query, " 电源 ");
    assert_eq!(next.query.sort, "like");
    assert_eq!(next.query.page, 1);
    assert_eq!(next.query.page_size, 20);
    assert!(ids(&feed).is_empty());
    assert_eq!(feed.snapshot().total, 0);
    assert_eq!(feed.snapshot().page, 0);
    assert!(feed.snapshot().has_more);
    assert!(feed.snapshot().loading);
    assert!(!feed.snapshot().refreshing);
    assert!(feed.snapshot().error.is_none());
    assert!(feed.snapshot().refresh_error.is_none());
    feed.complete(next, Ok(page(&[], 1, false)));
    assert_eq!(feed.snapshot().page, 1);
    assert!(!feed.snapshot().has_more);
}

#[test]
fn refresh_preserves_cards_then_replaces_page_one_and_resumes_current_search() {
    let mut feed = FeedController::default();
    let first = feed.search("电源", "like");
    feed.complete(first, Ok(page(&["old"], 1, true)));
    let second = feed.load_more().unwrap();
    feed.complete(second, Ok(page(&["next"], 2, true)));
    let fresh = feed.refresh().unwrap();
    assert_eq!(fresh.query.page, 1);
    assert_eq!(fresh.query.query, "电源");
    assert_eq!(fresh.query.sort, "like");
    assert_eq!(ids(&feed), ["old", "next"]);
    assert_eq!(feed.snapshot().page, 2);
    assert!(feed.snapshot().refreshing);
    assert!(!feed.snapshot().loading);
    assert!(feed.refresh().is_none());
    assert!(feed.load_more().is_none());
    assert!(feed.load_more_if_visible(true).is_none());
    let mut response = page(&["fresh", "fresh"], 1, true);
    response.total = 63;
    feed.complete(fresh, Ok(response));
    assert_eq!(ids(&feed), ["fresh"]);
    assert_eq!(feed.snapshot().page, 1);
    assert_eq!(feed.snapshot().total, 63);
    assert!(feed.snapshot().has_more);
    assert!(!feed.snapshot().refreshing);
    let next = feed.load_more().unwrap();
    assert_eq!(next.query.page, 2);
    assert_eq!(next.query.query, "电源");
    assert_eq!(next.query.sort, "like");
    feed.complete(next, Ok(page(&["next"], 2, false)));
    assert_eq!(ids(&feed), ["fresh", "next"]);
}

#[test]
fn exhausted_feed_refresh_failure_keeps_cards_and_supports_explicit_retry() {
    let mut feed = FeedController::default();
    let first = feed.search("", "recommend");
    feed.complete(first, Ok(page(&["old"], 1, false)));
    let refresh = feed.refresh().unwrap();
    feed.complete(refresh, Err(AppError::new("offline", "刷新离线")));
    assert_eq!(ids(&feed), ["old"]);
    assert_eq!(feed.snapshot().page, 1);
    assert_eq!(feed.snapshot().total, 50);
    assert!(!feed.snapshot().has_more);
    assert!(!feed.snapshot().refreshing);
    assert!(feed.snapshot().error.is_none());
    assert_eq!(
        feed.snapshot().refresh_error.as_ref().unwrap().message,
        "刷新离线"
    );
    let retry = feed.refresh().unwrap();
    assert!(feed.snapshot().refresh_error.is_none());
    feed.complete(retry, Ok(page(&["fresh"], 1, false)));
    assert_eq!(ids(&feed), ["fresh"]);
    assert!(feed.snapshot().refresh_error.is_none());
}

#[test]
fn refresh_invalidates_pending_pagination_successes_and_errors() {
    for fail in [false, true] {
        let mut feed = FeedController::default();
        let first = feed.search("", "new");
        feed.complete(first, Ok(page(&["old"], 1, true)));
        let paging = feed.load_more().unwrap();
        let refresh = feed.refresh().unwrap();
        feed.complete(refresh, Ok(page(&["fresh"], 1, true)));
        let stale = if fail {
            Err(AppError::new("offline", "过期分页错误"))
        } else {
            Ok(page(&["stale"], 2, false))
        };
        assert!(!feed.complete(paging, stale));
        assert_eq!(ids(&feed), ["fresh"]);
        assert_eq!(feed.snapshot().page, 1);
        assert!(feed.snapshot().has_more);
        assert!(feed.snapshot().error.is_none());
        assert!(feed.snapshot().refresh_error.is_none());
    }
}

#[test]
fn new_search_invalidates_an_older_refresh_success_or_error() {
    for fail in [false, true] {
        let mut feed = FeedController::default();
        let first = feed.search("old", "recommend");
        feed.complete(first, Ok(page(&["old"], 1, true)));
        let refresh = feed.refresh().unwrap();
        let current = feed.search("new", "like");
        feed.complete(current, Ok(page(&["new"], 1, true)));
        let stale = if fail {
            Err(AppError::new("offline", "过期刷新错误"))
        } else {
            Ok(page(&["stale"], 1, false))
        };
        assert!(!feed.complete(refresh, stale));
        assert_eq!(ids(&feed), ["new"]);
        assert!(!feed.snapshot().refreshing);
        assert!(feed.snapshot().error.is_none());
        assert!(feed.snapshot().refresh_error.is_none());
    }
}

#[test]
fn automatic_paging_requires_visible_populated_feed_without_errors() {
    let mut feed = FeedController::default();
    assert!(feed.load_more_if_visible(true).is_none());
    let first = feed.search("", "recommend");
    feed.complete(first, Ok(page(&["a"], 1, true)));
    assert!(feed.load_more_if_visible(false).is_none());
    assert!(!feed.snapshot().loading);
    let next = feed.load_more_if_visible(true).unwrap();
    assert_eq!(next.query.page, 2);
    feed.complete(next, Err(AppError::new("offline", "离线")));
    assert!(feed.load_more_if_visible(true).is_none());
    let retry = feed.load_more().unwrap();
    assert_eq!(retry.query.page, 2);
    feed.complete(retry, Ok(page(&["b"], 2, true)));
    let refresh = feed.refresh().unwrap();
    feed.complete(refresh, Err(AppError::new("offline", "刷新离线")));
    assert!(feed.load_more_if_visible(true).is_none());
}

#[test]
fn bom_search_matches_each_field_and_preserves_original_indices() {
    let items = vec![
        BomItem {
            name: "ESP32 模组".into(),
            designator: "U1".into(),
            footprint: "QFN48".into(),
            quantity: 2,
            part_number: "ESP32-WROOM".into(),
            manufacturer: "Espressif".into(),
            supplier: "LCSC".into(),
            supplier_part: "C2040".into(),
        },
        BomItem {
            name: "电源芯片".into(),
            designator: "U2".into(),
            footprint: "SOIC8".into(),
            quantity: 17,
            part_number: "MP1584".into(),
            manufacturer: "MPS".into(),
            supplier: "LCSC".into(),
            supplier_part: "C3456".into(),
        },
    ];
    for (query, expected) in [
        (" esp32 ", vec![0]),
        ("u2", vec![1]),
        ("soic", vec![1]),
        ("mp1584", vec![1]),
        ("espressif", vec![0]),
        ("lcsc", vec![0, 1]),
        ("c2040", vec![0]),
        ("17", vec![1]),
        ("  ", vec![0, 1]),
        ("missing", vec![]),
    ] {
        assert_eq!(search_bom(&items, query).unwrap(), expected, "{query:?}");
    }
}

#[test]
fn bom_search_retains_core_query_validation() {
    assert!(search_bom(&[], &"物".repeat(100)).unwrap().is_empty());
    for query in ["物".repeat(101), "x\ny".into(), "\t".into()] {
        assert_eq!(search_bom(&[], &query).unwrap_err().code, "INVALID_QUERY");
    }
}
