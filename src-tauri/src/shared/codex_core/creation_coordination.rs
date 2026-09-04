//! Process-local Phase 3 operation coordination; not canonical Thread storage.
use super::creation_acknowledgement::acknowledge_thread_start;
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IntentId {
    pub process_epoch: String,
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnIntent {
    pub intent: IntentId,
    pub creation_intent: Option<IntentId>,
}

/// Mark immediately before the first write attempt, after preflight and locks.
/// Partial writes are possibly sent, never safe failures.
#[derive(Clone, Default)]
pub(crate) struct DispatchBoundary(Arc<AtomicBool>);
impl DispatchBoundary {
    pub(crate) fn mark_dispatched(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    fn crossed(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum State {
    IntentCreated,
    StartDispatching,
    StartInFlight,
    ThreadAcknowledged,
    CreationFailed,
    CreationOutcomeUnknown,
    FirstTurnPending,
    FirstTurnDispatching,
    FirstTurnInFlight,
    FirstTurnAccepted,
    FirstTurnOutcomeUnknown,
    FirstTurnFailed,
    FirstTurnInterrupted,
    FirstTurnCompleted,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Creation,
    Turn,
}
type EntryKey = (Kind, String);
struct Entry {
    binding: String,
    state: State,
    boundary: DispatchBoundary,
    result: Option<Result<Value, String>>,
    thread: Option<CodexThreadKey>,
    turn: Option<String>,
    failure_reason: Option<String>,
}
impl Entry {
    fn snapshot(&self) -> Value {
        let state = match (self.state, self.boundary.crossed()) {
            (State::StartDispatching, true) => State::StartInFlight,
            (State::FirstTurnDispatching, true) => State::FirstTurnInFlight,
            _ => self.state,
        };
        json!({
            "state": state,
            "threadKey": self.thread,
            "turnId": self.turn,
            "failureReason": self.failure_reason,
        })
    }
}
/// Owned by app/daemon process, not WorkspaceSession. No eviction/retry/store.
#[derive(Clone)]
pub(crate) struct CreationCoordinator {
    epoch: String,
    entries: Arc<Mutex<HashMap<EntryKey, Entry>>>,
    observed_outcomes: Arc<Mutex<HashMap<(CodexThreadKey, String), String>>>,
}
impl Default for CreationCoordinator {
    fn default() -> Self {
        Self {
            epoch: uuid::Uuid::new_v4().to_string(),
            entries: Arc::new(Mutex::new(HashMap::new())),
            observed_outcomes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
struct Lease {
    coordinator: CreationCoordinator,
    key: EntryKey,
    settled: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.settled {
            let _ = self.fail("operation canceled".into(), false, None);
        }
    }
}
impl Lease {
    fn fail(
        &mut self,
        reason: String,
        protocol_failure: bool,
        failure_reason: Option<&str>,
    ) -> Result<Value, String> {
        let mut entries = self.coordinator.entries.lock().unwrap();
        let e = entries.get_mut(&self.key).unwrap();
        e.state = match (self.key.0, e.boundary.crossed() && !protocol_failure) {
            (Kind::Creation, true) => State::CreationOutcomeUnknown,
            (Kind::Creation, false) => State::CreationFailed,
            (Kind::Turn, true) => State::FirstTurnOutcomeUnknown,
            (Kind::Turn, false) => State::FirstTurnFailed,
        };
        e.failure_reason = failure_reason.map(str::to_string);
        let error = format!("{}: {reason}", e.snapshot()["state"].as_str().unwrap());
        e.result = Some(Err(error.clone()));
        self.settled = true;
        Err(error)
    }
    fn succeed(
        &mut self,
        mut response: Value,
        thread: CodexThreadKey,
        turn: Option<String>,
    ) -> Result<Value, String> {
        let mut entries = self.coordinator.entries.lock().unwrap();
        let e = entries.get_mut(&self.key).unwrap();
        e.state = if self.key.0 == Kind::Creation {
            State::ThreadAcknowledged
        } else {
            State::FirstTurnAccepted
        };
        e.thread = Some(thread);
        e.turn = turn;
        response["result"][if self.key.0 == Kind::Creation {
            "creationCoordination"
        } else {
            "firstTurnCoordination"
        }] = e.snapshot();
        e.result = Some(Ok(response.clone()));
        self.settled = true;
        Ok(response)
    }
}
enum Claim {
    Existing(Result<Value, String>),
    Owned(Lease, DispatchBoundary),
}
impl CreationCoordinator {
    /// Preflight full canonical identity before exposing any prompt to transport.
    pub(crate) fn validate_turn_target(
        &self,
        intent: &TurnIntent,
        thread: &CodexThreadKey,
    ) -> Result<(), String> {
        self.validate(&intent.intent)?;
        let mut entries = self.entries.lock().unwrap();
        if let Some(creation) = &intent.creation_intent {
            self.validate(creation)?;
            let expected = entries
                .get(&(Kind::Creation, creation.id.clone()))
                .and_then(|entry| entry.thread.as_ref());
            if expected != Some(thread) {
                return Err("INTENT_THREAD_IDENTITY_CONFLICT".into());
            }
        }
        let entry = entries
            .get_mut(&(Kind::Turn, intent.intent.id.clone()))
            .ok_or("UNKNOWN_TURN_INTENT")?;
        if entry.state != State::FirstTurnDispatching
            || entry.boundary.crossed()
            || entry
                .thread
                .as_ref()
                .is_some_and(|existing| existing != thread)
        {
            return Err("INVALID_TURN_PREFLIGHT".into());
        }
        entry.thread = Some(thread.clone());
        Ok(())
    }
    pub(crate) fn context(&self) -> Value {
        json!({"processEpoch":self.epoch})
    }
    pub(crate) fn requires_first_turn_intent(&self, key: &CodexThreadKey) -> bool {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .any(|((kind, _), e)| *kind == Kind::Creation && e.thread.as_ref() == Some(key))
            && !entries.iter().any(|((kind, _), e)| {
                *kind == Kind::Turn && e.thread.as_ref() == Some(key) && e.turn.is_some()
            })
    }
    fn validate(&self, i: &IntentId) -> Result<(), String> {
        if i.process_epoch != self.epoch {
            return Err(
                "STALE_PROCESS_EPOCH: cross-process intent recovery is not supported".into(),
            );
        }
        if uuid::Uuid::parse_str(&i.id).is_err() {
            return Err("INVALID_INTENT_ID".into());
        }
        Ok(())
    }
    fn status(&self, kind: Kind, i: &IntentId) -> Result<Value, String> {
        self.validate(i)?;
        Ok(self.entries.lock().unwrap().get(&(kind,i.id.clone())).map(Entry::snapshot)
            .unwrap_or_else(||json!({"state":if kind==Kind::Creation {State::IntentCreated}else{State::FirstTurnPending}})))
    }
    pub(crate) fn creation_status(&self, i: &IntentId) -> Result<Value, String> {
        self.status(Kind::Creation, i)
    }
    pub(crate) fn turn_status(&self, i: &IntentId) -> Result<Value, String> {
        self.status(Kind::Turn, i)
    }
    fn claim(
        &self,
        kind: Kind,
        i: &IntentId,
        binding: String,
        creation: Option<&IntentId>,
        thread: Option<&str>,
    ) -> Result<Claim, String> {
        self.validate(i)?;
        if let Some(parent) = creation {
            self.validate(parent)?;
        }
        let key = (kind, i.id.clone());
        let mut entries = self.entries.lock().unwrap();
        if let Some(e) = entries.get(&key) {
            if e.binding != binding {
                return Err("INTENT_BINDING_CONFLICT".into());
            }
            if let Some(result) = &e.result {
                return Ok(Claim::Existing(result.clone()));
            }
            if e.state != State::FirstTurnPending {
                return Err("ALREADY_IN_FLIGHT".into());
            }
        }
        if let Some(parent) = creation {
            let parent = entries.get(&(Kind::Creation, parent.id.clone()));
            if parent.is_none_or(|e| e.state != State::ThreadAcknowledged) {
                let reason = parent
                    .and_then(|e| e.result.as_ref())
                    .and_then(|r| r.as_ref().err())
                    .cloned();
                entries.entry(key).or_insert_with(|| Entry {
                    binding,
                    state: State::FirstTurnPending,
                    boundary: DispatchBoundary::default(),
                    result: None,
                    thread: None,
                    turn: None,
                    failure_reason: None,
                });
                return Err(
                    reason.unwrap_or_else(|| "FIRST_TURN_PENDING: acknowledgement required".into())
                );
            }
            if parent
                .and_then(|e| e.thread.as_ref())
                .map(|k| k.thread_id.as_str())
                != thread
            {
                return Err("INTENT_THREAD_IDENTITY_CONFLICT".into());
            }
        }
        let boundary = DispatchBoundary::default();
        entries.insert(
            key.clone(),
            Entry {
                binding,
                state: if kind == Kind::Creation {
                    State::StartDispatching
                } else {
                    State::FirstTurnDispatching
                },
                boundary: boundary.clone(),
                result: None,
                thread: None,
                turn: None,
                failure_reason: None,
            },
        );
        Ok(Claim::Owned(
            Lease {
                coordinator: self.clone(),
                key,
                settled: false,
            },
            boundary,
        ))
    }
    pub(crate) async fn create<F, Fut>(
        &self,
        i: &IntentId,
        workspace: &str,
        operation: F,
    ) -> Result<Value, String>
    where
        F: FnOnce(DispatchBoundary) -> Fut,
        Fut: Future<Output = Result<(String, Value), String>>,
    {
        let (mut lease, boundary) =
            match self.claim(Kind::Creation, i, workspace.into(), None, None)? {
                Claim::Existing(result) => return result,
                Claim::Owned(l, b) => (l, b),
            };
        let (home, mut response) = match operation(boundary).await {
            Ok(v) => v,
            Err(e) => return lease.fail(e, false, None),
        };
        let ack = match acknowledge_thread_start(&home, None, &response) {
            Ok(v) => v,
            Err(e) => return lease.fail(e.to_string(), true, None),
        };
        response["result"]["creationAcknowledgement"] = serde_json::to_value(&ack).unwrap();
        lease.succeed(response, ack.thread_key().clone(), None)
    }
    pub(crate) async fn turn<F, Fut>(
        &self,
        i: &TurnIntent,
        workspace: &str,
        thread: &str,
        operation: F,
    ) -> Result<Value, String>
    where
        F: FnOnce(DispatchBoundary) -> Fut,
        Fut: Future<Output = Result<(CodexThreadKey, Value), String>>,
    {
        let binding = serde_json::to_string(&(workspace, thread, &i.creation_intent)).unwrap();
        let (mut lease, boundary) = match self.claim(
            Kind::Turn,
            &i.intent,
            binding,
            i.creation_intent.as_ref(),
            Some(thread),
        )? {
            Claim::Existing(result) => return result,
            Claim::Owned(l, b) => (l, b),
        };
        let (key, response) = match operation(boundary).await {
            Ok(v) => v,
            Err(e) => return lease.fail(e, false, None),
        };
        if response.get("error").is_some() {
            return lease.fail("server rejected turn/start".into(), true, Some("REJECTED"));
        }
        let expected = i.creation_intent.as_ref().and_then(|i| {
            self.entries
                .lock()
                .unwrap()
                .get(&(Kind::Creation, i.id.clone()))
                .and_then(|e| e.thread.clone())
        });
        if key.thread_id != thread || expected.as_ref().is_some_and(|expected| expected != &key) {
            return lease.fail("Thread identity conflict".into(), false, None);
        }
        let Some(turn) = response
            .pointer("/result/turn/id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
        else {
            return lease.fail(
                "invalid turn/start response; acceptance unknown".into(),
                false,
                None,
            );
        };
        let turn = turn.to_string();
        let immediate = response
            .pointer("/result/turn/status")
            .and_then(Value::as_str)
            .map(str::to_string);
        lease.succeed(response, key.clone(), Some(turn.clone()))?;
        if let Some(outcome) = immediate {
            self.observe_known_turn_outcome(&key, &turn, &outcome);
        }
        let observed = self
            .observed_outcomes
            .lock()
            .unwrap()
            .get(&(key.clone(), turn.clone()))
            .cloned();
        if let Some(outcome) = observed {
            let _ = self.observe_turn_outcome(&i.intent, &key, &turn, &outcome);
        }
        self.entries
            .lock()
            .unwrap()
            .get(&(Kind::Turn, i.intent.id.clone()))
            .unwrap()
            .result
            .clone()
            .unwrap()
    }
    pub(crate) fn observe_turn_outcome(
        &self,
        i: &IntentId,
        thread: &CodexThreadKey,
        turn: &str,
        outcome: &str,
    ) -> Result<(), String> {
        self.validate(i)?;
        let mut entries = self.entries.lock().unwrap();
        let e = entries
            .get_mut(&(Kind::Turn, i.id.clone()))
            .ok_or("UNKNOWN_TURN_INTENT")?;
        if e.thread.as_ref() != Some(thread) || e.turn.as_deref() != Some(turn) {
            return Err("TURN_EVIDENCE_IDENTITY_CONFLICT".into());
        }
        let next = match outcome {
            "completed" => State::FirstTurnCompleted,
            "failed" => State::FirstTurnFailed,
            "interrupted" => State::FirstTurnInterrupted,
            _ => return Err("UNSUPPORTED_TURN_OUTCOME".into()),
        };
        if e.state != State::FirstTurnAccepted && e.state != next {
            return Err("TURN_OUTCOME_CONFLICT".into());
        }
        e.state = next;
        let snapshot = e.snapshot();
        if let Some(Ok(response)) = &mut e.result {
            response["result"]["firstTurnCoordination"] = snapshot;
        }
        Ok(())
    }
    /// Exact already-known IDs only; no discovery/correlation of unknown intents.
    pub(crate) fn observe_known_turn_outcome(
        &self,
        thread: &CodexThreadKey,
        turn: &str,
        outcome: &str,
    ) {
        if !matches!(outcome, "completed" | "failed" | "interrupted") {
            return;
        }
        // Buffer only authoritative exact evidence for already-known Threads;
        // it cannot acknowledge an intent lacking its correlated response.
        if self
            .entries
            .lock()
            .unwrap()
            .values()
            .any(|e| e.thread.as_ref() == Some(thread))
        {
            self.observed_outcomes
                .lock()
                .unwrap()
                .entry((thread.clone(), turn.into()))
                .or_insert_with(|| outcome.into());
        }
        let ids: Vec<String> = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter(|((kind, _), e)| {
                *kind == Kind::Turn
                    && e.thread.as_ref() == Some(thread)
                    && e.turn.as_deref() == Some(turn)
            })
            .map(|((_, id), _)| id.clone())
            .collect();
        for id in ids {
            let _ = self.observe_turn_outcome(
                &IntentId {
                    process_epoch: self.epoch.clone(),
                    id,
                },
                thread,
                turn,
                outcome,
            );
        }
    }
}
