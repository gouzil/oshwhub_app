use oshwhub_core::{
    api::{validate_comment, validate_id, validate_login_code},
    model::{Comment, CommentsPage, ProjectStatus, User},
};
use serde_json::json;

#[test]
fn validates_login_codes_at_the_boundary() {
    for code in ["abc-123", "A_b.c~9"] {
        validate_login_code(code).unwrap();
    }
    for code in ["", "abc def", "abc\n123", "abc/123", &"a".repeat(513)] {
        assert_eq!(validate_login_code(code).unwrap_err().code, "INVALID_CODE");
    }
}

#[test]
fn validates_comment_content_before_network_access() {
    validate_comment("可以复现，谢谢！").unwrap();
    for content in ["", "   ", "有控制\n字符", &"x".repeat(2001)] {
        assert_eq!(
            validate_comment(content).unwrap_err().code,
            "INVALID_COMMENT"
        );
    }
    validate_id("90602f2f840d4245a97479e90a31d17b").unwrap();
    assert_eq!(
        validate_id("../../etc/passwd").unwrap_err().code,
        "INVALID_ID"
    );
}

#[test]
fn account_models_use_the_bridge_camel_case_contract() {
    let user = User {
        id: "user-1".into(),
        username: "tester".into(),
        nickname: "测试用户".into(),
        avatar: Some("https://image.lceda.cn/avatar.png".into()),
    };
    let comment = Comment {
        uuid: "comment-1".into(),
        content: "你好".into(),
        creator: user.clone(),
        created_at: "2026-10-24 12:00:00".into(),
        like: false,
        replies: vec![],
    };
    let value = json!({
        "user": user,
        "status": ProjectStatus { album_collect: false, like: false, collect: true },
        "comments": CommentsPage { items: vec![comment], page: 1, total: 1, has_more: false },
    });
    assert!(value["status"].get("albumCollect").is_some());
    assert!(value["comments"].get("hasMore").is_some());
    assert!(
        value["comments"]["items"][0]["creator"]
            .get("avatar")
            .is_some()
    );
}
