use super::projection_freshness::{
    ProjectionFreshnessCoverage, ProjectionFreshnessQuerySnapshot, ProjectionFreshnessStatus,
};
use serde_json::Value;
use std::path::PathBuf;

const FIXTURES: [(&str, ProjectionFreshnessStatus); 10] = [
    ("not-hydrated.json", ProjectionFreshnessStatus::NotHydrated),
    ("hydrating.json", ProjectionFreshnessStatus::Hydrating),
    (
        "current-thread-catalog.json",
        ProjectionFreshnessStatus::Current,
    ),
    (
        "current-thread-detail.json",
        ProjectionFreshnessStatus::Current,
    ),
    ("partial-coverage.json", ProjectionFreshnessStatus::Current),
    (
        "stale-after-session-generation-change.json",
        ProjectionFreshnessStatus::Stale,
    ),
    (
        "stale-after-app-server-generation-change.json",
        ProjectionFreshnessStatus::Stale,
    ),
    ("unavailable.json", ProjectionFreshnessStatus::Unavailable),
    ("unknown.json", ProjectionFreshnessStatus::Unknown),
    (
        "daemon-restart-old-historical-snapshot.json",
        ProjectionFreshnessStatus::Stale,
    ),
];

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("projection-freshness")
}

#[test]
fn projection_freshness_compatibility_fixtures_are_stable_and_sanitized() {
    for (name, expected_status) in FIXTURES {
        let path = fixture_root().join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
        let envelope: Value = serde_json::from_str(&content).expect("fixture JSON");
        let snapshot: ProjectionFreshnessQuerySnapshot =
            serde_json::from_value(envelope["snapshot"].clone()).expect("snapshot schema");
        assert_eq!(snapshot.workspace_id, "workspace-sanitized");
        assert!(snapshot
            .coverages
            .iter()
            .any(|coverage| coverage.status == expected_status));
        assert!(snapshot
            .coverages
            .iter()
            .any(|coverage| { coverage.coverage == ProjectionFreshnessCoverage::ThreadCatalog }));

        let lowered = content.to_ascii_lowercase();
        for forbidden in [
            "token",
            "secret",
            "remoteclientidentity",
            "recoverygeneration",
            "projectiongeneration",
            "clientgeneration",
            "writerowner",
            "leaseid",
        ] {
            assert!(!lowered.contains(forbidden), "{name}: {forbidden}");
        }
    }
}
