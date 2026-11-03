use super::{OrgId, WorkspaceId, is_valid_slug, is_valid_workspace_id};
use std::convert::TryFrom;

const VALID_SLUGS: [&str; 7] = [
    "north_star",                     // typical slug
    "north_star_123",                 // typical slug, ends in a digit
    "abc",                            // minimum length (3)
    "a23456789012345678901234567890", // maximum length (30), ends in a digit
    "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", // maximum length (30), ends in a letter
    "a_b_c_d",                        // underscores between alphanumeric characters
    "a__b",                           // consecutive underscores
];

const INVALID_SLUGS: [&str; 15] = [
    "ab",                              // too short (2)
    "a234567890123456789012345678901", // too long (31)
    "1bcdef",                          // leading digit
    "_bcdef",                          // leading underscore
    "abcde_",                          // trailing underscore
    "Abcdef",                          // leading uppercase
    "abcdeF",                          // trailing uppercase
    "ABCDEF",                          // all uppercase
    "abc-de",                          // hyphen: it joins slugs in a workspace id
    "abc.def",                         // dot
    "abcdefg ",                        // trailing space
    "",                                // empty
    "日本語スラッグですよ",            // 30 bytes of non-ASCII (passes length, fails charset)
    "héllo_world",                     // non-ASCII character inside
    "north/star",                      // slash
];

const VALID_WORKSPACE_IDS: [&str; 3] = [
    "north_star-web_shop",
    "abc-xyz",
    "a23456789012345678901234567890-z23456789012345678901234567890",
];

const INVALID_WORKSPACE_IDS: [&str; 9] = [
    "north_star",          // no workspace slug
    "north_star-",         // empty workspace slug
    "-web_shop",           // empty org slug
    "north_star-web-shop", // two separators
    "north_star-1shop",    // workspace slug starts with a digit
    "ab-web_shop",         // org slug too short
    "north_star/web_shop", // slash
    "north_star.web_shop", // dot
    "North_star-web_shop", // uppercase
];

#[test]
fn is_valid_slug_accepts_slugs_matching_every_rule() {
    for slug in VALID_SLUGS {
        assert!(is_valid_slug(slug), "expected {slug:?} to be valid");
    }
}

#[test]
fn is_valid_slug_rejects_slugs_breaking_any_rule() {
    for slug in INVALID_SLUGS {
        assert!(!is_valid_slug(slug), "expected {slug:?} to be invalid");
    }
}

#[test]
fn is_valid_workspace_id_accepts_two_slugs_joined_by_a_hyphen() {
    for id in VALID_WORKSPACE_IDS {
        assert!(is_valid_workspace_id(id), "expected {id:?} to be valid");
    }
}

#[test]
fn is_valid_workspace_id_rejects_anything_else() {
    for id in INVALID_WORKSPACE_IDS.iter().chain(INVALID_SLUGS.iter()) {
        assert!(!is_valid_workspace_id(id), "expected {id:?} to be invalid");
    }
}

#[test]
fn a_workspace_id_joins_the_org_and_workspace_slugs() {
    let org: OrgId = "north_star".parse().expect("valid org slug");

    let workspace = WorkspaceId::new(&org, "web_shop").expect("valid workspace slug");

    assert_eq!(workspace.as_ref(), "north_star-web_shop");
    assert_eq!(workspace.org_slug(), "north_star");
    assert_eq!(workspace.slug(), "web_shop");
}

#[test]
fn a_workspace_id_needs_a_valid_workspace_slug() {
    let org: OrgId = "north_star".parse().expect("valid org slug");

    for slug in INVALID_SLUGS {
        let err = WorkspaceId::new(&org, slug).expect_err("invalid workspace slug");
        assert_eq!(err.input, slug);
    }
}

#[test]
fn from_str_accepts_valid_ids_for_both_newtypes() {
    let org: OrgId = "north_star".parse().expect("valid org slug");
    let workspace: WorkspaceId = "north_star-web_shop".parse().expect("valid workspace id");

    assert_eq!(org.as_ref(), "north_star");
    assert_eq!(workspace.as_ref(), "north_star-web_shop");
}

#[test]
fn from_str_rejects_invalid_slug_and_carries_the_input() {
    let err = "1abcdef"
        .parse::<OrgId>()
        .expect_err("leading digit is invalid");

    assert_eq!(err.input, "1abcdef");
    assert!(!err.message.is_empty());
    assert!(err.to_string().contains("1abcdef"));
}

#[test]
fn try_from_string_rejects_invalid_ids() {
    for slug in INVALID_SLUGS {
        assert!(
            OrgId::try_from(String::from(slug)).is_err(),
            "expected {slug:?} to be rejected by OrgId"
        );
    }
    for id in INVALID_WORKSPACE_IDS.iter().chain(INVALID_SLUGS.iter()) {
        assert!(
            WorkspaceId::try_from((*id).to_owned()).is_err(),
            "expected {id:?} to be rejected by WorkspaceId"
        );
    }
    // A workspace id is not an org id, nor the reverse.
    assert!(OrgId::try_from(String::from("north_star-web_shop")).is_err());
    assert!(WorkspaceId::try_from(String::from("north_star")).is_err());
}

#[test]
fn display_and_string_conversion_round_trip_the_slug() {
    let org: OrgId = "north_star".parse().expect("valid slug");

    assert_eq!(org.to_string(), "north_star");
    assert_eq!(String::from(org), "north_star");
}

#[test]
fn serde_serializes_newtypes_as_bare_strings() {
    let org: OrgId = "north_star".parse().expect("valid slug");
    let workspace: WorkspaceId = "north_star-web_shop".parse().expect("valid id");

    assert_eq!(
        serde_json::to_value(&org).expect("serializes"),
        serde_json::json!("north_star")
    );
    assert_eq!(
        serde_json::to_value(&workspace).expect("serializes"),
        serde_json::json!("north_star-web_shop")
    );
}

#[test]
fn serde_rejects_invalid_ids_from_bare_strings() {
    for slug in INVALID_SLUGS {
        let json = serde_json::to_string(slug).expect("any str serializes to JSON");
        assert!(
            serde_json::from_str::<OrgId>(&json).is_err(),
            "expected {slug:?} to be rejected by OrgId deserialization"
        );
        assert!(
            serde_json::from_str::<WorkspaceId>(&json).is_err(),
            "expected {slug:?} to be rejected by WorkspaceId deserialization"
        );
    }
}

#[test]
fn serde_round_trips_valid_ids_through_json() {
    let workspace: WorkspaceId = "north_star-web_shop".parse().expect("valid id");

    let encoded = serde_json::to_string(&workspace).expect("serializes");
    let decoded: WorkspaceId = serde_json::from_str(&encoded).expect("valid id");

    assert_eq!(decoded, workspace);
}
