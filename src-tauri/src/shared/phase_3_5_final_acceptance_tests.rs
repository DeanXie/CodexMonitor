use serde_json::Value;
use std::path::PathBuf;

const FIXTURE_DIRECTORY: &str = "phase-3-5-final-acceptance";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn load_fixture(name: &str) -> Value {
    let path = repository_root()
        .join("docs")
        .join("fixtures")
        .join(FIXTURE_DIRECTORY)
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
        panic!("read final acceptance fixture {}: {error}", path.display())
    });
    serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!("parse final acceptance fixture {}: {error}", path.display())
    })
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("string array")
        .iter()
        .map(|entry| entry.as_str().expect("string entry").to_string())
        .collect()
}

fn authority() -> Value {
    let fixture = load_fixture("authority-contract.json");
    assert_eq!(fixture["schemaVersion"], 1);
    fixture
}

#[test]
fn final_identity_hierarchy_is_independent() {
    assert_eq!(
        strings(&authority()["identityHierarchy"]),
        [
            "RemoteHostIdentity",
            "DaemonProcessGeneration",
            "RemoteTransportGeneration",
            "WorkspaceSessionGeneration",
            "AppServerConnectionGeneration",
            "CodexThreadKey",
            "CodexTurnKey",
        ]
    );
}

#[test]
fn final_authority_invariants_are_frozen() {
    let invariants = &authority()["authorityInvariants"];
    assert_eq!(invariants["observedEqualsRequested"], false);
    assert_eq!(invariants["requestedEqualsEstimated"], false);
    assert_eq!(invariants["noEvidenceMeansUnknown"], true);
    assert_eq!(invariants["canonicalThreadEqualsUiProjection"], false);
    assert_eq!(
        invariants["canonicalThreadEqualsTransportAvailability"],
        false
    );
    assert_eq!(invariants["transportGenerationEqualsClientIdentity"], false);
    assert_eq!(
        invariants["workspaceSessionEqualsAppServerConnection"],
        false
    );
    assert_eq!(invariants["sameHostIdentityMeansSameSession"], false);
    assert_eq!(invariants["recoveryEqualsMutationReplay"], false);
}

#[test]
fn final_mutation_retry_and_replay_counts_are_zero() {
    for operation in [
        "resumeThread",
        "approvalDecision",
        "threadDelete",
        "upstreamUnsubscribe",
    ] {
        assert_eq!(authority()["mutationPolicy"][operation]["retry"], 0);
        assert_eq!(authority()["mutationPolicy"][operation]["replay"], 0);
    }
    assert_eq!(authority()["mutationPolicy"]["forceTakeover"], false);
}

#[test]
fn final_writer_subscription_runtime_authorities_remain_separate() {
    assert_eq!(
        strings(&authority()["observationAuthorities"]),
        [
            "WriterAdmissionObservation",
            "ThreadSubscriptionObservation",
            "ThreadRuntimeAvailabilityObservation",
        ]
    );
    assert_eq!(
        strings(&authority()["writerStates"]),
        [
            "NOT_OBSERVED",
            "ADMISSION_PENDING",
            "ADMITTED_FOR_SESSION",
            "BLOCKED_BY_ACTIVE_WRITER",
            "ADMISSION_OUTCOME_UNKNOWN",
            "SESSION_ENDED_RELEASE_UNOBSERVED",
        ]
    );
}

#[test]
fn final_approval_and_delete_contracts_are_exact_and_non_inferential() {
    let approval = &authority()["approval"];
    assert_eq!(approval["exactCurrentIdentityRequired"], true);
    assert_eq!(approval["staleOrUnavailableActionable"], false);
    assert_eq!(approval["decisionDispatchedMeansAccepted"], false);
    let delete = &authority()["delete"];
    assert_eq!(delete["exactCodexThreadKeyRequired"], true);
    assert_eq!(delete["confirmedCreatesTombstone"], true);
    assert_eq!(delete["unknownCreatesTombstone"], false);
    assert_eq!(delete["projectionAbsenceConfirmsDelete"], false);
}

#[test]
fn final_projection_gap_and_ui_contracts_are_honest() {
    assert_eq!(
        strings(&authority()["projection"]["coverages"]),
        [
            "workspace_catalog",
            "thread_catalog",
            "thread_detail",
            "observation_snapshot",
        ]
    );
    assert_eq!(authority()["projection"]["eventPromotesToCurrent"], false);
    assert_eq!(authority()["eventGap"]["mutatesSharedAuthority"], false);
    assert_eq!(authority()["eventGap"]["otherClientBecomesStale"], false);
    assert_eq!(authority()["eventGap"]["streamCompletenessProven"], false);
    assert_eq!(
        strings(&authority()["ui"]["states"]),
        ["Current", "Hydrating", "Stale", "Unavailable", "Unknown"]
    );
    assert_eq!(authority()["ui"]["staleMeansAbsent"], false);
    assert_eq!(authority()["ui"]["unavailableMeansAbsent"], false);
    assert_eq!(authority()["ui"]["unknownMeansIdle"], false);
    assert_eq!(authority()["ui"]["disconnectedMeansDeleted"], false);
}

#[test]
fn final_fixture_families_resolve_inside_repository() {
    let manifest = load_fixture("fixture-family-manifest.json");
    let families = manifest["families"].as_array().expect("fixture families");
    assert!(families.len() >= 20);
    for family in families {
        let path = repository_root().join(family["path"].as_str().expect("fixture path"));
        assert!(path.starts_with(repository_root()));
        assert!(path.exists(), "missing fixture family {}", path.display());
    }
}

#[test]
fn final_remaining_not_proven_items_are_explicit() {
    assert_eq!(
        strings(&authority()["remainingNotProven"]),
        [
            "duplicate_or_late_approval_response_exact_upstream_behavior",
            "concurrent_duplicate_delete_exact_upstream_ordering_or_winner",
            "daemon_broadcast_event_stream_completeness",
        ]
    );
}
