use oshwhub_core::{
    api::{Api, feed_request, normalize_feed, normalize_project, validate_id},
    client::{HttpClient, validate_url},
    model::FeedQuery,
};
use serde_json::{Value, json};

fn fixture(name: &str) -> Value {
    serde_json::from_str(match name {
        "feed" => include_str!("fixtures/api-feed.json"),
        "detail" => include_str!("fixtures/api-detail.json"),
        "images" => include_str!("fixtures/api-images.json"),
        _ => panic!("unknown fixture"),
    })
    .unwrap()
}

fn query() -> FeedQuery {
    FeedQuery {
        page: 2,
        page_size: 12,
        query: String::new(),
        sort: "new".to_owned(),
    }
}

#[test]
fn rejects_ids_that_could_escape_an_api_path() {
    for id in [
        "",
        "../../etc/passwd",
        "90602f2f840d4245a97479e90a31d17b?x=1",
        "xyz02f2f840d4245a97479e90a31d17b",
    ] {
        assert_eq!(validate_id(id).unwrap_err().code, "INVALID_ID");
    }
    validate_id("90602f2f840d4245a97479e90a31d17b").unwrap();
    validate_id("d4c95e111c8e5672").unwrap();
}

#[test]
fn rejects_untrusted_download_destinations_before_network_access() {
    for url in [
        "http://oshwhub.com/api/project",
        "https://oshwhub.com.evil.example/api",
        "https://sub.oshwhub.com/api",
        "https://user:secret@oshwhub.com/api",
        "https://oshwhub.com:8443/api",
        "https://127.0.0.1/api",
        "file:///etc/passwd",
    ] {
        assert_eq!(validate_url(url).unwrap_err().code, "INVALID_URL");
    }
    assert_eq!(
        validate_url("https://modules.lceda.cn/a?key=opaque")
            .unwrap()
            .host_str(),
        Some("modules.lceda.cn")
    );
}

#[test]
fn uses_encoded_get_pagination_for_browsing() {
    let (url, body) = feed_request(&query()).unwrap();
    let url = url::Url::parse(&url).unwrap();
    let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(url.path(), "/api/project");
    assert_eq!(params["page"], "2");
    assert_eq!(params["pageSize"], "12");
    assert_eq!(params["type"], "new");
    assert_eq!(params["public"], "true");
    assert!(body.is_none());
}

#[test]
fn uses_formal_post_search_with_trimmed_keywords_and_pagination() {
    let mut q = query();
    q.query = "  电源 & ESP32  ".to_owned();
    q.sort = "recommend".to_owned();
    let (url, body) = feed_request(&q).unwrap();
    assert_eq!(url, "https://oshwhub.com/api/search/data");
    assert_eq!(
        body.unwrap(),
        json!({
            "type": "project", "keywords": "电源 & ESP32", "category": "",
            "sort": "", "page": 2, "pageSize": 12
        })
    );
}

#[test]
fn rejects_invalid_pagination_sort_and_keyword_controls() {
    let mut q = query();
    q.page = 0;
    assert_eq!(feed_request(&q).unwrap_err().code, "INVALID_QUERY");
    q.page = 1;
    q.page = 10001;
    assert_eq!(feed_request(&q).unwrap_err().code, "INVALID_QUERY");
    q.page = 1;
    q.page_size = 0;
    assert_eq!(feed_request(&q).unwrap_err().code, "INVALID_QUERY");
    q.page_size = 51;
    assert_eq!(feed_request(&q).unwrap_err().code, "INVALID_QUERY");
    q.page_size = 12;
    q.sort = "arbitrary".to_owned();
    assert_eq!(feed_request(&q).unwrap_err().code, "INVALID_QUERY");
    q.sort = "new".to_owned();
    q.query = "x".repeat(101);
    assert_eq!(feed_request(&q).unwrap_err().code, "INVALID_QUERY");
    q.query = "a\0b".to_owned();
    assert_eq!(feed_request(&q).unwrap_err().code, "INVALID_QUERY");
}

#[test]
fn preserves_pagination_while_deduplicating_recommendation_ids() {
    let page = normalize_feed(&fixture("feed")).unwrap();
    assert_eq!(page.page, 1);
    assert_eq!(page.total, 4);
    assert!(page.has_more);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].name, "ESP32 开发板");
    assert_eq!(
        page.items[0].cover.as_deref(),
        Some("https://image.lceda.cn/board.png")
    );
    assert_eq!(page.items[0].likes, 44);
    assert_eq!(page.items[0].author, "立创开发板");
    assert_eq!(page.items[1].author, "bhwj");
    assert_eq!(page.items[1].cover, None);
    let serialized = serde_json::to_value(page).unwrap();
    assert!(serialized["items"][0].get("authorAvatar").is_some());
    assert!(serialized.get("hasMore").is_some());
}

#[test]
fn known_missing_default_avatars_fall_back_without_removing_custom_images() {
    for avatar in [
        "/images/avatar-default.png",
        "/images/team-avatar-default.png",
        "https://oshwhub.com/images/avatar-default.png",
        "//oshwhub.com/images/team-avatar-default.png?v=1",
    ] {
        let mut response = fixture("feed");
        response["result"]["lists"][0]["owner"]["avatar"] = json!(avatar);
        assert_eq!(
            normalize_feed(&response).unwrap().items[0].author_avatar,
            None
        );
    }
    let avatar = "https://image.lceda.cn/images/avatar-default.png";
    let mut detail = fixture("detail");
    detail["result"]["owner"]["avatar"] = json!(avatar);
    assert_eq!(
        normalize_project(&detail, Some(&fixture("images")))
            .unwrap()
            .summary
            .author_avatar
            .as_deref(),
        Some(avatar)
    );
}

#[test]
fn does_not_mark_the_last_page_as_having_more() {
    let mut response = fixture("feed");
    response["result"]["page"] = json!(2);
    assert!(!normalize_feed(&response).unwrap().has_more);
}

#[test]
fn empty_search_results_are_a_successful_last_page() {
    let response = json!({
        "success": true, "code": 0,
        "result": {"page": 1, "pageSize": 12, "total": 0, "totalPage": 0, "lists": []}
    });
    let page = normalize_feed(&response).unwrap();
    assert!(page.items.is_empty());
    assert!(!page.has_more);
}

#[test]
fn surfaces_business_errors_even_when_http_succeeded() {
    let response = json!({"success": false, "code": 104001, "result": null, "msg": ""});
    assert_eq!(normalize_feed(&response).unwrap_err().code, "NOT_FOUND");
    let response = json!({"success": false, "code": 100001, "result": null});
    assert_eq!(normalize_feed(&response).unwrap_err().code, "RATE_LIMITED");
}

#[test]
fn rejects_missing_payload_fields_and_unsafe_project_paths() {
    assert_eq!(
        normalize_feed(&json!({"success": true, "code": 0}))
            .unwrap_err()
            .code,
        "INVALID_RESPONSE"
    );
    for field in ["uuid", "name", "path"] {
        let mut response = fixture("feed");
        response["result"]["lists"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            normalize_feed(&response).unwrap_err().code,
            "INVALID_RESPONSE"
        );
    }
    let mut response = fixture("feed");
    response["result"]["lists"][0]["path"] = json!("//evil.example/project");
    assert_eq!(
        normalize_feed(&response).unwrap_err().code,
        "INVALID_RESPONSE"
    );
}

#[test]
fn preserves_bom_manufacturer_and_supplier_as_distinct_fields() {
    let mut images = fixture("images");
    images["result"]["boards"][0]["schematic"]["boms"]["boms"] = json!([
        {"Name":"10nF", "Quantity":1, "Manufacturer":"SAMSUNG(三星)",
         "Manufacturer Part":"CL05B103KB5NNNC", "Supplier":"LCSC", "Supplier Part":"C15195"},
        {"Name":"自制器件", "Quantity":0}
    ]);
    let detail = normalize_project(&fixture("detail"), Some(&images)).unwrap();
    let detail = serde_json::to_value(detail).unwrap();
    let bom = &detail["boards"][0]["bom"];
    assert_eq!(bom[0]["manufacturer"], "SAMSUNG(三星)");
    assert_eq!(bom[0]["partNumber"], "CL05B103KB5NNNC");
    assert_eq!(bom[0]["supplier"], "LCSC");
    assert_eq!(bom[0]["supplierPart"], "C15195");
    assert_eq!(bom[1]["manufacturer"], "");
    assert_eq!(bom[1]["supplier"], "");
    assert_eq!(bom[1]["quantity"], 0);
}

#[test]
fn preserves_schematic_document_ids_for_boards() {
    let detail = normalize_project(&fixture("detail"), Some(&fixture("images"))).unwrap();
    assert_eq!(
        detail.boards[0].schematic_id.as_deref(),
        Some("6e9bf6c0323c94de")
    );
    let serialized = serde_json::to_value(detail).unwrap();
    assert_eq!(serialized["boards"][0]["schematicId"], "6e9bf6c0323c94de");

    let mut images = fixture("images");
    images["result"]["boards"][0]["sch"] = json!("not-an-id");
    assert_eq!(
        normalize_project(&fixture("detail"), Some(&images))
            .unwrap_err()
            .code,
        "INVALID_RESPONSE"
    );
}

#[test]
fn converts_nested_html_once_without_scripts_or_unsafe_links() {
    let detail = normalize_project(&fixture("detail"), Some(&fixture("images"))).unwrap();
    let texts: Vec<_> = detail.content.iter().map(|b| b.text.as_str()).collect();
    assert_eq!(texts.iter().filter(|t| t.contains("只出现一次")).count(), 1);
    assert!(!texts.iter().any(|t| t.contains("MUST_NOT_LEAK")));
    assert!(
        detail
            .content
            .iter()
            .any(|b| b.kind == "heading" && b.text == "装配说明")
    );
    assert!(
        detail
            .content
            .iter()
            .any(|b| b.kind == "code" && b.text == "let x = 1;\n  x + 2")
    );
    assert_eq!(
        detail.content.iter().filter(|b| b.kind == "image").count(),
        1
    );
    assert!(detail.content.iter().all(|b| {
        b.url
            .as_deref()
            .is_none_or(|u| u.starts_with("https://") || u.starts_with("http://"))
    }));
    assert_eq!(detail.attachments.len(), 1);
    assert_eq!(detail.boards[0].pcb_id.as_deref(), Some("d4c95e111c8e5672"));
    assert_eq!(detail.boards[0].bom[0].quantity, 1);
    assert_eq!(detail.boards[0].bom[0].supplier_part, "C12345");
}

#[test]
fn malformed_bom_is_an_explicit_error_instead_of_an_empty_table() {
    let mut images = fixture("images");
    images["result"]["boards"][0]["schematic"]["boms"]["boms"] = json!("not json");
    assert_eq!(
        normalize_project(&fixture("detail"), Some(&images))
            .unwrap_err()
            .code,
        "INVALID_BOM"
    );
    images["result"]["boards"][0]["schematic"]["boms"]["boms"] =
        json!("[{\"Name\":\"bad\",\"Quantity\":-1}]");
    assert_eq!(
        normalize_project(&fixture("detail"), Some(&images))
            .unwrap_err()
            .code,
        "INVALID_BOM"
    );
}

#[test]
fn standard_projects_do_not_need_the_unsupported_pro_images_endpoint() {
    let mut detail = fixture("detail");
    detail["result"]["origin"] = json!("std");
    let detail = normalize_project(&detail, None).unwrap();
    assert_eq!(detail.summary.name, "ESP32 开发板");
    assert!(detail.boards.is_empty());
}

#[test]
fn includes_standalone_pcbs_and_deduplicates_board_ids() {
    let mut images = fixture("images");
    let repeated = images["result"]["boards"][0].clone();
    images["result"]["boards"]
        .as_array_mut()
        .unwrap()
        .push(repeated);
    images["result"]["out_pcbs"] = json!([{
        "uuid": "5997b37ea4c65fd9", "display_title": "独立 PCB",
        "thumb": "//image-pro.lceda.cn/standalone.webp"
    }]);
    let detail = normalize_project(&fixture("detail"), Some(&images)).unwrap();
    assert_eq!(detail.boards.len(), 2);
    assert_eq!(detail.boards[1].pcb_id.as_deref(), Some("5997b37ea4c65fd9"));
}

#[test]
fn resolves_relative_attachments_to_the_verified_cdn() {
    let mut detail = fixture("detail");
    detail["result"]["attachments"][0]["src"] = json!("/oshwhub/project/attachments/firmware.zip");
    let detail = normalize_project(&detail, Some(&fixture("images"))).unwrap();
    assert_eq!(
        detail.attachments[0].url,
        "https://image.lceda.cn/oshwhub/project/attachments/firmware.zip"
    );
}

#[test]
fn preserves_images_wrapped_in_links() {
    let mut detail = fixture("detail");
    detail["result"]["content"] = json!(
        "<p>前文<a href=\"https://example.com\"><img src=\"//image.lceda.cn/board.webp\"></a>后文</p>"
    );
    let detail = normalize_project(&detail, Some(&fixture("images"))).unwrap();
    assert_eq!(
        detail.content.iter().filter(|b| b.kind == "image").count(),
        1
    );
    assert!(detail.content.iter().any(|b| b.text == "前文"));
    assert!(detail.content.iter().any(|b| b.text == "后文"));
}

#[test]
fn rejects_excessively_deep_html_instead_of_recursing_without_a_bound() {
    let mut detail = fixture("detail");
    detail["result"]["content"] = json!(format!(
        "{}text{}",
        "<div>".repeat(160),
        "</div>".repeat(160)
    ));
    assert_eq!(
        normalize_project(&detail, Some(&fixture("images")))
            .unwrap_err()
            .code,
        "CONTENT_LIMIT"
    );
}

#[tokio::test]
#[ignore = "Public OSHWHub read-only smoke; requires internet and may be rate limited"]
async fn live_public_feed_search_and_project() {
    let api = Api::new(HttpClient::new().unwrap());
    let page = api.feed(query()).await.unwrap();
    assert_eq!(page.page, 2);
    assert!(!page.items.is_empty());
    let mut q = query();
    q.page = 1;
    q.query = "ESP32".to_owned();
    let search = api.feed(q).await.unwrap();
    assert!(!search.items.is_empty());
    let detail = api
        .project("90602f2f840d4245a97479e90a31d17b")
        .await
        .unwrap();
    assert!(!detail.boards.is_empty());
}
