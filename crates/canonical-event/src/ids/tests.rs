use super::{OrgId, ProjectId, is_valid_slug};
use std::convert::TryFrom;

const VALID_SLUGS: [&str; 6] = [
    "acme-corp-42",                   // typical generated slug
    "blue-ocean-shop-7",              // typical generated slug
    "abcdef",                         // minimum length (6)
    "a23456789012345678901234567890", // maximum length (30), ends in a digit
    "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", // maximum length (30), ends in a letter
    "a-b-c-d",                        // hyphens between alphanumeric characters
];

const INVALID_SLUGS: [&str; 14] = [
    "abcde",                           // too short (5)
    "a234567890123456789012345678901", // too long (31)
    "1bcdef",                          // leading digit
    "-bcdef",                          // leading hyphen
    "abcde-",                          // trailing hyphen
    "Abcdef",                          // leading uppercase
    "abcdeF",                          // trailing uppercase
    "ABCDEF",                          // all uppercase
    "abc_def",                         // underscore
    "abc.def",                         // dot
    "abcdefg ",                        // trailing space
    "",                                // empty
    "日本語スラッグですよ",            // 30 bytes of non-ASCII (passes length, fails charset)
    "héllo-world",                     // non-ASCII character inside
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
fn from_str_accepts_valid_slugs_for_both_newtypes() {
    // Given/When: parse valid slugs into both slug newtypes.
    let org: OrgId = "acme-corp-42".parse().expect("valid org slug");
    let project: ProjectId = "blue-ocean-shop-7".parse().expect("valid project slug");

    // Then: the slug is preserved.
    assert_eq!(org.as_ref(), "acme-corp-42");
    assert_eq!(project.as_ref(), "blue-ocean-shop-7");
}

#[test]
fn from_str_rejects_invalid_slug_and_carries_the_input() {
    // Given/When: parse a slug with a leading digit.
    let err = "1abcdef"
        .parse::<OrgId>()
        .expect_err("leading digit is invalid");

    // Then: the error reports the rejected input and a reason.
    assert_eq!(err.input, "1abcdef");
    assert!(!err.message.is_empty());
    assert!(err.to_string().contains("1abcdef"));
}

#[test]
fn try_from_string_accepts_valid_slugs_for_both_newtypes() {
    // Given/When: convert owned strings into both slug newtypes.
    let org = OrgId::try_from(String::from("acme-corp-42")).expect("valid org slug");
    let project =
        ProjectId::try_from(String::from("blue-ocean-shop-7")).expect("valid project slug");

    // Then: the slug is preserved.
    assert_eq!(org.as_ref(), "acme-corp-42");
    assert_eq!(project.as_ref(), "blue-ocean-shop-7");
}

#[test]
fn try_from_string_rejects_invalid_slugs_for_both_newtypes() {
    for slug in INVALID_SLUGS {
        assert!(
            OrgId::try_from(String::from(slug)).is_err(),
            "expected {slug:?} to be rejected by OrgId"
        );
        assert!(
            ProjectId::try_from(slug.to_owned()).is_err(),
            "expected {slug:?} to be rejected by ProjectId"
        );
    }
}

#[test]
fn display_and_string_conversion_round_trip_the_slug() {
    // Given: a parsed org slug.
    let org: OrgId = "acme-corp-42".parse().expect("valid slug");

    // When/Then: Display and From<OrgId> for String both yield the inner slug.
    assert_eq!(org.to_string(), "acme-corp-42");
    assert_eq!(String::from(org), "acme-corp-42");
}

#[test]
fn serde_serializes_newtypes_as_bare_strings() {
    // Given: parsed slug newtypes.
    let org: OrgId = "acme-corp-42".parse().expect("valid slug");
    let project: ProjectId = "blue-ocean-shop-7".parse().expect("valid slug");

    // When/Then: they serialize as bare JSON strings, not objects.
    assert_eq!(
        serde_json::to_value(&org).expect("serializes"),
        serde_json::json!("acme-corp-42")
    );
    assert_eq!(
        serde_json::to_value(&project).expect("serializes"),
        serde_json::json!("blue-ocean-shop-7")
    );
}

#[test]
fn serde_deserializes_valid_slugs_from_bare_strings() {
    // Given/When: deserialize bare JSON strings into both newtypes.
    let org: OrgId = serde_json::from_str("\"acme-corp-42\"").expect("valid slug");
    let project: ProjectId = serde_json::from_str("\"blue-ocean-shop-7\"").expect("valid slug");

    // Then: the slug is preserved.
    assert_eq!(org.as_ref(), "acme-corp-42");
    assert_eq!(project.as_ref(), "blue-ocean-shop-7");
}

#[test]
fn serde_rejects_invalid_slugs_from_bare_strings() {
    for slug in INVALID_SLUGS {
        let json = serde_json::to_string(slug).expect("any str serializes to JSON");
        assert!(
            serde_json::from_str::<OrgId>(&json).is_err(),
            "expected {slug:?} to be rejected by OrgId deserialization"
        );
        assert!(
            serde_json::from_str::<ProjectId>(&json).is_err(),
            "expected {slug:?} to be rejected by ProjectId deserialization"
        );
    }
}

#[test]
fn serde_round_trips_valid_slugs_through_json() {
    // Given: a parsed org slug.
    let org: OrgId = "acme-corp-42".parse().expect("valid slug");

    // When: serialize then deserialize.
    let encoded = serde_json::to_string(&org).expect("serializes");
    let decoded: OrgId = serde_json::from_str(&encoded).expect("valid slug");

    // Then: the value is unchanged.
    assert_eq!(decoded, org);
}
