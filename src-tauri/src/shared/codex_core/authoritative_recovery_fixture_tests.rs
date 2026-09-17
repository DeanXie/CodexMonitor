use serde_json::Value;

const FIXTURES: [(&str, &str); 11] = [
    (
        "frontend-reload",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/frontend-reload.json"),
    ),
    (
        "same-daemon-reconnect",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/same-daemon-reconnect.json"),
    ),
    (
        "workspace-session-replacement",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/workspace-session-replacement.json"),
    ),
    (
        "daemon-restart",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/daemon-restart.json"),
    ),
    (
        "partial-hydration",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/partial-hydration.json"),
    ),
    (
        "selected-thread-read",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/selected-thread-read.json"),
    ),
    (
        "observation-snapshot",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/observation-snapshot.json"),
    ),
    (
        "stale-response-after-generation-change",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/stale-response-after-generation-change.json"),
    ),
    (
        "event-during-hydration",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/event-during-hydration.json"),
    ),
    (
        "hydration-read-failure",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/hydration-read-failure.json"),
    ),
    (
        "multi-client-convergence",
        include_str!("../../../tests/fixtures/phase-3-5-4c-authoritative-hydration/multi-client-convergence.json"),
    ),
];

#[test]
fn authoritative_recovery_fixtures_are_sanitized_and_complete() {
    for (scenario, raw) in FIXTURES {
        let fixture: Value = serde_json::from_str(raw).expect("valid recovery fixture JSON");
        assert_eq!(fixture["scenario"], scenario);
        let lower = raw.to_ascii_lowercase();
        for forbidden in [
            "01a08c05-7880-75a2-976c-2a5895b58723",
            "auth token",
            "rollout-2026",
            "approval content",
            "remoteclientidentity",
            "recoveryowner",
            "recoverylease",
        ] {
            assert!(
                !lower.contains(forbidden),
                "fixture {scenario} contains forbidden material: {forbidden}"
            );
        }
    }
}

#[test]
fn recovery_fixtures_never_authorize_mutation_replay() {
    for (scenario, raw) in FIXTURES {
        let lower = raw.to_ascii_lowercase();
        for forbidden in [
            "resume_thread",
            "thread/delete",
            "respond_to_server_request",
            "thread/unsubscribe",
            "force takeover",
            "automatic replay",
        ] {
            assert!(
                !lower.contains(forbidden),
                "fixture {scenario} authorizes mutation: {forbidden}"
            );
        }
    }
}
