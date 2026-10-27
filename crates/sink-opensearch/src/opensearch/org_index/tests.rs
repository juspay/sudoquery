use std::path::PathBuf;

use super::*;

/// A CAC file where merchant-1 has a dedicated index and workspace eu of
/// merchant-2 has another.
const CAC: &str = r#"
[default-configs]
"opensearch.index" = { value = "events-{org_id}", schema = { type = "string" } }

[dimensions]
org_id = { position = 1, schema = { type = "string" } }
workspace_id = { position = 2, schema = { type = "string" } }

[[overrides]]
_context_ = { org_id = "merchant-1" }
"opensearch.index" = "events-merchant-1-dedicated"

[[overrides]]
_context_ = { org_id = "merchant-2", workspace_id = "eu" }
"opensearch.index" = "events-merchant-2-eu"

[[overrides]]
_context_ = { org_id = "broken" }
"opensearch.index" = "Not A Valid Index"
"#;

async fn indexes() -> (OrgIndexes, PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "sink-opensearch-cac-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&path, CAC).unwrap();
    (OrgIndexes::new(Cac::load(&path).await.unwrap(), None), path)
}

async fn index_name(indexes: &OrgIndexes, org_id: &str, workspace_id: Option<&str>) -> String {
    let template = indexes.index_for(org_id, workspace_id).await.unwrap();
    template
        .render(org_id, workspace_id.unwrap_or_default())
        .unwrap()
        .into_owned()
}

#[tokio::test]
async fn orgs_without_an_override_use_the_default() {
    let (indexes, path) = indexes().await;

    assert_eq!(
        index_name(&indexes, "merchant-9", None).await,
        "events-merchant-9"
    );
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn org_overrides_apply() {
    let (indexes, path) = indexes().await;

    assert_eq!(
        index_name(&indexes, "merchant-1", None).await,
        "events-merchant-1-dedicated"
    );
    assert_eq!(
        index_name(&indexes, "merchant-1", Some("eu")).await,
        "events-merchant-1-dedicated"
    );
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn workspace_overrides_apply() {
    let (indexes, path) = indexes().await;

    assert_eq!(
        index_name(&indexes, "merchant-2", Some("eu")).await,
        "events-merchant-2-eu"
    );
    assert_eq!(
        index_name(&indexes, "merchant-2", Some("us")).await,
        "events-merchant-2"
    );
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn a_pinned_index_ignores_org_overrides() {
    let (indexes, path) = indexes().await;
    let indexes = OrgIndexes {
        pinned: Some(IndexTemplate::parse("events-env-{org_id}").unwrap()),
        ..indexes
    };

    assert_eq!(
        index_name(&indexes, "merchant-1", None).await,
        "events-env-merchant-1"
    );
    assert_eq!(
        index_name(&indexes, "merchant-2", Some("eu")).await,
        "events-env-merchant-2"
    );
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn an_invalid_override_is_an_error_for_that_org_only() {
    let (indexes, path) = indexes().await;

    let error = indexes.index_for("broken", None).await.unwrap_err();

    assert!(error.contains("not a valid index name"), "{error}");
    assert_eq!(
        index_name(&indexes, "merchant-9", None).await,
        "events-merchant-9"
    );
    std::fs::remove_file(path).unwrap();
}
