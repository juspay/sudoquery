use super::*;
use canonical_event::is_valid_slug;

#[test]
fn slugify_ascii_names() {
    assert_eq!(slugify("Acme Corp!"), "acme-corp");
    assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
}

#[test]
fn slugify_folds_latin_accents() {
    // deunicode folds "café" to "cafe" (the strip-non-ASCII fallback would
    // give "caf").
    assert_eq!(slugify("Café Store"), "cafe-store");
    assert_eq!(slugify("Ünïcödé Wörks"), "unicode-works");
}

#[test]
fn slugify_romanizes_cjk() {
    // deunicode romanizes 東京 to "Dong Jing".
    assert_eq!(slugify("東京 Store"), "dong-jing-store");
}

#[test]
fn slugify_transliterates_emoji() {
    // deunicode maps 🔥 to "fire", so the base stays usable.
    assert_eq!(slugify("🔥🔥"), "fire-fire");
}

#[test]
fn slugify_drops_unusable_characters() {
    assert_eq!(slugify("!!!"), "");
    assert_eq!(slugify("42"), "");
}

#[test]
fn slugify_drops_leading_digits() {
    assert_eq!(slugify("42prod"), "prod");
    assert_eq!(slugify("123-Project X"), "project-x");
}

#[test]
fn slugify_truncates_to_the_base_budget() {
    assert_eq!(slugify(&"a".repeat(60)).len(), MAX_BASE_LEN);

    // Truncation must not leave a trailing dash.
    let name = format!("{}tail", "b".repeat(MAX_BASE_LEN));
    let slug = slugify(&name);
    assert_eq!(slug.len(), MAX_BASE_LEN);
    assert!(!slug.ends_with('-'));
}

#[test]
fn generated_ids_pass_validation() {
    for name in [
        "Acme Corp",
        "東京商店",
        "🔥",
        "42",
        "a",
        "Ünïcödé Wörks",
        &"x".repeat(200),
    ] {
        let org = generate_org_id(name).to_string();
        assert!(is_valid_slug(&org), "invalid org id: {org}");

        let project = generate_project_id(name).to_string();
        assert!(is_valid_slug(&project), "invalid project id: {project}");
    }
}

#[test]
fn generated_ids_combine_base_and_suffix() {
    let org = generate_org_id("Acme Corp").to_string();
    assert!(org.starts_with("acme-corp-"));
    assert_eq!(org.len(), "acme-corp".len() + 1 + SUFFIX_LEN);

    let suffix = &org[org.len() - SUFFIX_LEN..];
    assert!(
        suffix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    );
}

#[test]
fn unusable_names_get_the_fallback_base() {
    assert!(generate_org_id("!!!").to_string().starts_with("org-"));
    assert!(generate_project_id("!!!").to_string().starts_with("proj-"));
}
