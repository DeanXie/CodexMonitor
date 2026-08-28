use super::rollout_identity::{CodexThreadKey, CodexTurnKey};
use super::source_envelope::{FreshnessEvidence, FreshnessState, SourceKind, SourceTemporalClass};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenSnapshot {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: Option<u64>,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ExternalLifecycle {
    Running,
    Waiting,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceLaneUpdate {
    pub observation_id: String,
    pub thread_key: CodexThreadKey,
    pub turn_key: Option<CodexTurnKey>,
    pub source_kind: SourceKind,
    pub temporal_class: SourceTemporalClass,
    pub source_instance_id: String,
    pub source_generation: String,
    pub source_timestamp_ms: Option<i64>,
    pub observed_timestamp_ms: i64,
    pub freshness: FreshnessEvidence,
    pub lifecycle: Option<ExternalLifecycle>,
    pub observed_model: Option<String>,
    pub token_snapshot: Option<TokenSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceRegistryBatchItem {
    pub update: SourceLaneUpdate,
    pub parent_thread_key: Option<CodexThreadKey>,
    pub agent_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FieldProvenance {
    pub source_kind: SourceKind,
    pub temporal_class: SourceTemporalClass,
    pub source_instance_id: String,
    pub source_generation: String,
    pub source_timestamp_ms: Option<i64>,
    pub observed_timestamp_ms: i64,
    pub freshness: FreshnessEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolvedValue<T> {
    pub value: T,
    pub provenance: FieldProvenance,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AuthoritativeThreadView {
    pub lifecycle: Option<ResolvedValue<ExternalLifecycle>>,
    pub observed_model: Option<ResolvedValue<String>>,
    pub token_snapshot: Option<ResolvedValue<TokenSnapshot>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalSourceSnapshot {
    pub threads: Vec<CanonicalSourceThread>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalSourceThread {
    pub key: CodexThreadKey,
    pub parent_thread_key: Option<ResolvedValue<CodexThreadKey>>,
    pub agent_path: Option<ResolvedValue<String>>,
    pub current_turn: Option<CanonicalSourceTurn>,
    pub lifecycle: Option<ResolvedValue<ExternalLifecycle>>,
    pub observed_model: Option<ResolvedValue<String>>,
    pub token_snapshot: Option<ResolvedValue<TokenSnapshot>>,
    pub authority_provenance: Option<FieldProvenance>,
    pub live_lane_count: usize,
    pub near_live_lane_count: usize,
    pub historical_lane_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalSourceTurn {
    pub key: CodexTurnKey,
    pub lifecycle: Option<ResolvedValue<ExternalLifecycle>>,
    pub started_at: Option<FieldProvenance>,
    pub completed_at: Option<FieldProvenance>,
}

#[derive(Clone, Debug, Default)]
struct SourceLane {
    lifecycle: Option<ResolvedValue<ExternalLifecycle>>,
    observed_model: Option<ResolvedValue<String>>,
    token_snapshot: Option<ResolvedValue<TokenSnapshot>>,
    latest_observed_timestamp_ms: i64,
    freshness: Option<FreshnessEvidence>,
}

#[derive(Clone, Debug, Default)]
struct CanonicalTurnEvidence {
    lifecycle: Option<ResolvedValue<ExternalLifecycle>>,
    started_at: Option<FieldProvenance>,
    completed_at: Option<FieldProvenance>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadSourceLanes {
    live: HashMap<String, SourceLane>,
    near_live: HashMap<String, SourceLane>,
    historical: HashMap<String, SourceLane>,
    parent_thread_key: Option<ResolvedValue<CodexThreadKey>>,
    agent_path: Option<ResolvedValue<String>>,
    current_turn_key: Option<ResolvedValue<CodexTurnKey>>,
    turns: HashMap<CodexTurnKey, CanonicalTurnEvidence>,
}

impl ThreadSourceLanes {
    pub(crate) fn live_count(&self) -> usize {
        self.live.len()
    }

    pub(crate) fn near_live_count(&self) -> usize {
        self.near_live.len()
    }

    pub(crate) fn historical_count(&self) -> usize {
        self.historical.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceRegistryError(String);

impl Display for SourceRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SourceRegistryError {}

#[derive(Clone, Default)]
pub(crate) struct SourceAuthorityRegistry {
    threads: HashMap<CodexThreadKey, ThreadSourceLanes>,
    observation_keys: HashMap<String, CodexThreadKey>,
    tombstoned_thread_keys: HashSet<CodexThreadKey>,
}

impl SourceAuthorityRegistry {
    pub(crate) fn ingest_batch(
        &mut self,
        items: Vec<SourceRegistryBatchItem>,
    ) -> Result<usize, SourceRegistryError> {
        let mut staged = self.clone();
        let mut applied = 0;
        for item in items {
            staged.observe_identity(&item.update, item.parent_thread_key, item.agent_path);
            if staged.ingest(item.update)? {
                applied += 1;
            }
        }
        *self = staged;
        Ok(applied)
    }

    pub(crate) fn observe_identity(
        &mut self,
        update: &SourceLaneUpdate,
        parent_thread_key: Option<CodexThreadKey>,
        agent_path: Option<String>,
    ) {
        if self.tombstoned_thread_keys.contains(&update.thread_key) {
            return;
        }
        let provenance = provenance_from_update(update);
        let thread = self.threads.entry(update.thread_key.clone()).or_default();
        update_field(
            &mut thread.parent_thread_key,
            parent_thread_key,
            &provenance,
        );
        update_field(&mut thread.agent_path, agent_path, &provenance);
    }

    pub(crate) fn ingest(&mut self, update: SourceLaneUpdate) -> Result<bool, SourceRegistryError> {
        if self.tombstoned_thread_keys.contains(&update.thread_key) {
            return Ok(false);
        }
        validate_lane(&update)?;
        let observation_key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            source_kind_key(update.source_kind),
            update.source_instance_id,
            update.source_generation,
            update.observation_id
        );
        if self.observation_keys.contains_key(&observation_key) {
            return Ok(false);
        }

        let provenance = provenance_from_update(&update);
        let lane_key = format!(
            "{}\u{1f}{}",
            update.source_instance_id, update.source_generation
        );
        let thread = self.threads.entry(update.thread_key.clone()).or_default();
        let lanes = match update.temporal_class {
            SourceTemporalClass::Live => &mut thread.live,
            SourceTemporalClass::NearLive => &mut thread.near_live,
            SourceTemporalClass::Historical => &mut thread.historical,
        };
        let lane = lanes.entry(lane_key).or_default();
        if update.observed_timestamp_ms >= lane.latest_observed_timestamp_ms {
            lane.latest_observed_timestamp_ms = update.observed_timestamp_ms;
            lane.freshness = Some(update.freshness);
        }
        update_field(&mut lane.lifecycle, update.lifecycle, &provenance);
        update_field(&mut lane.observed_model, update.observed_model, &provenance);
        update_field(&mut lane.token_snapshot, update.token_snapshot, &provenance);
        if let Some(turn_key) = update.turn_key {
            update_field(
                &mut thread.current_turn_key,
                Some(turn_key.clone()),
                &provenance,
            );
            let turn = thread.turns.entry(turn_key).or_default();
            update_field(&mut turn.lifecycle, update.lifecycle, &provenance);
            match update.lifecycle {
                Some(ExternalLifecycle::Running) => {
                    update_earliest_provenance(&mut turn.started_at, &provenance);
                }
                Some(ExternalLifecycle::Completed) => {
                    update_latest_provenance(&mut turn.completed_at, &provenance);
                }
                Some(ExternalLifecycle::Waiting) | None => {}
            }
        }
        self.observation_keys
            .insert(observation_key, update.thread_key);
        Ok(true)
    }

    pub(crate) fn retire_threads(
        &mut self,
        keys: impl IntoIterator<Item = CodexThreadKey>,
    ) -> usize {
        let mut newly_retired = 0;
        for key in keys {
            if self.tombstoned_thread_keys.insert(key.clone()) {
                newly_retired += 1;
            }
            self.threads.remove(&key);
            self.observation_keys
                .retain(|_, observation_thread_key| observation_thread_key != &key);
        }
        newly_retired
    }

    pub(crate) fn is_tombstoned(&self, key: &CodexThreadKey) -> bool {
        self.tombstoned_thread_keys.contains(key)
    }

    pub(crate) fn thread_count(&self) -> usize {
        self.threads.len()
    }

    pub(crate) fn lanes(&self, key: &CodexThreadKey) -> Option<&ThreadSourceLanes> {
        self.threads.get(key)
    }

    pub(crate) fn resolve(&self, key: &CodexThreadKey) -> Option<AuthoritativeThreadView> {
        let lanes = self.threads.get(key)?;
        let fresh_live_lifecycle = best_field(&lanes.live, |lane| lane.lifecycle.as_ref(), true);
        let lifecycle = fresh_live_lifecycle
            .or_else(|| best_field(&lanes.near_live, |lane| lane.lifecycle.as_ref(), false));

        let observed_model = best_field(&lanes.live, |lane| lane.observed_model.as_ref(), true)
            .or_else(|| best_field(&lanes.near_live, |lane| lane.observed_model.as_ref(), false))
            .or_else(|| best_field(&lanes.live, |lane| lane.observed_model.as_ref(), false))
            .or_else(|| {
                best_field(
                    &lanes.historical,
                    |lane| lane.observed_model.as_ref(),
                    false,
                )
            });

        let token_snapshot = resolve_tokens(lanes);
        Some(AuthoritativeThreadView {
            lifecycle,
            observed_model,
            token_snapshot,
        })
    }

    pub(crate) fn snapshot(&self) -> CanonicalSourceSnapshot {
        let mut threads = self
            .threads
            .iter()
            .map(|(key, lanes)| {
                let resolved = self.resolve(key).unwrap_or_default();
                let current_turn = lanes.current_turn_key.as_ref().map(|current| {
                    let evidence = lanes.turns.get(&current.value);
                    CanonicalSourceTurn {
                        key: current.value.clone(),
                        lifecycle: resolved
                            .lifecycle
                            .clone()
                            .or_else(|| evidence.and_then(|turn| turn.lifecycle.clone())),
                        started_at: evidence.and_then(|turn| turn.started_at.clone()),
                        completed_at: evidence.and_then(|turn| turn.completed_at.clone()),
                    }
                });
                let authority_provenance = authoritative_provenance(&resolved);
                CanonicalSourceThread {
                    key: key.clone(),
                    parent_thread_key: lanes.parent_thread_key.clone(),
                    agent_path: lanes.agent_path.clone(),
                    current_turn,
                    lifecycle: resolved.lifecycle,
                    observed_model: resolved.observed_model,
                    token_snapshot: resolved.token_snapshot,
                    authority_provenance,
                    live_lane_count: lanes.live_count(),
                    near_live_lane_count: lanes.near_live_count(),
                    historical_lane_count: lanes.historical_count(),
                }
            })
            .collect::<Vec<_>>();
        threads.sort_by(|left, right| {
            left.key
                .codex_home_identity
                .cmp(&right.key.codex_home_identity)
                .then_with(|| left.key.thread_id.cmp(&right.key.thread_id))
        });
        CanonicalSourceSnapshot { threads }
    }

    pub(crate) fn expire_live_lanes(
        &mut self,
        observed_timestamp_ms: i64,
        stale_after_ms: i64,
    ) -> usize {
        let mut expired = 0;
        for thread in self.threads.values_mut() {
            for lane in thread.live.values_mut() {
                let should_expire = lane
                    .freshness
                    .as_ref()
                    .is_some_and(|freshness| freshness.state == FreshnessState::Fresh)
                    && observed_timestamp_ms.saturating_sub(lane.latest_observed_timestamp_ms)
                        > stale_after_ms.max(0);
                if should_expire {
                    lane.freshness = Some(FreshnessEvidence {
                        state: FreshnessState::Stale,
                        last_complete_record_observed_at_ms: lane
                            .freshness
                            .as_ref()
                            .and_then(|freshness| freshness.last_complete_record_observed_at_ms),
                        reason: "no recent app-server observation".to_string(),
                    });
                    expired += 1;
                }
            }
        }
        expired
    }
}

fn provenance_from_update(update: &SourceLaneUpdate) -> FieldProvenance {
    FieldProvenance {
        source_kind: update.source_kind,
        temporal_class: update.temporal_class,
        source_instance_id: update.source_instance_id.clone(),
        source_generation: update.source_generation.clone(),
        source_timestamp_ms: update.source_timestamp_ms,
        observed_timestamp_ms: update.observed_timestamp_ms,
        freshness: update.freshness.clone(),
    }
}

fn update_earliest_provenance(current: &mut Option<FieldProvenance>, provenance: &FieldProvenance) {
    if current
        .as_ref()
        .map(|value| evidence_order(provenance) < evidence_order(value))
        .unwrap_or(true)
    {
        *current = Some(provenance.clone());
    }
}

fn update_latest_provenance(current: &mut Option<FieldProvenance>, provenance: &FieldProvenance) {
    if current
        .as_ref()
        .map(|value| evidence_order(provenance) >= evidence_order(value))
        .unwrap_or(true)
    {
        *current = Some(provenance.clone());
    }
}

fn authoritative_provenance(view: &AuthoritativeThreadView) -> Option<FieldProvenance> {
    let provenances = [
        view.lifecycle.as_ref().map(|value| &value.provenance),
        view.token_snapshot.as_ref().map(|value| &value.provenance),
        view.observed_model.as_ref().map(|value| &value.provenance),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    provenances
        .iter()
        .copied()
        .filter(|value| {
            value.temporal_class == SourceTemporalClass::Live
                && value.freshness.state == FreshnessState::Fresh
        })
        .max_by_key(|value| evidence_order(value))
        .or_else(|| {
            provenances
                .into_iter()
                .max_by_key(|value| evidence_order(value))
        })
        .cloned()
}

fn source_kind_key(source_kind: SourceKind) -> &'static str {
    match source_kind {
        SourceKind::MonitorAppServer => "monitor-app-server",
        SourceKind::CodexCliRollout => "codex-cli-rollout",
        SourceKind::HistoricalRolloutScan => "historical-rollout-scan",
    }
}

fn validate_lane(update: &SourceLaneUpdate) -> Result<(), SourceRegistryError> {
    let valid = matches!(
        (update.source_kind, update.temporal_class),
        (SourceKind::MonitorAppServer, SourceTemporalClass::Live)
            | (SourceKind::CodexCliRollout, SourceTemporalClass::NearLive)
            | (
                SourceKind::HistoricalRolloutScan,
                SourceTemporalClass::Historical
            )
    );
    if valid {
        Ok(())
    } else {
        Err(SourceRegistryError(format!(
            "source {:?} cannot enter {:?} lane",
            update.source_kind, update.temporal_class
        )))
    }
}

fn update_field<T: Clone>(
    current: &mut Option<ResolvedValue<T>>,
    value: Option<T>,
    provenance: &FieldProvenance,
) {
    let Some(value) = value else {
        return;
    };
    let replace = current
        .as_ref()
        .map(|current| evidence_order(provenance) >= evidence_order(&current.provenance))
        .unwrap_or(true);
    if replace {
        *current = Some(ResolvedValue {
            value,
            provenance: provenance.clone(),
        });
    }
}

fn evidence_order(provenance: &FieldProvenance) -> (i64, i64) {
    (
        provenance
            .source_timestamp_ms
            .unwrap_or(provenance.observed_timestamp_ms),
        provenance.observed_timestamp_ms,
    )
}

fn best_field<T: Clone>(
    lanes: &HashMap<String, SourceLane>,
    field: impl Fn(&SourceLane) -> Option<&ResolvedValue<T>>,
    require_fresh: bool,
) -> Option<ResolvedValue<T>> {
    lanes
        .values()
        .filter(|lane| {
            !require_fresh
                || lane
                    .freshness
                    .as_ref()
                    .is_some_and(|freshness| freshness.state == FreshnessState::Fresh)
        })
        .filter_map(|lane| {
            let mut value = field(lane)?.clone();
            if let Some(freshness) = &lane.freshness {
                value.provenance.freshness = freshness.clone();
            }
            Some(value)
        })
        .max_by_key(|value| evidence_order(&value.provenance))
}

fn resolve_tokens(lanes: &ThreadSourceLanes) -> Option<ResolvedValue<TokenSnapshot>> {
    if let Some(fresh_live) = best_field(&lanes.live, |lane| lane.token_snapshot.as_ref(), true) {
        return Some(fresh_live);
    }

    let last_live = best_field(&lanes.live, |lane| lane.token_snapshot.as_ref(), false);
    let near_live = best_field(&lanes.near_live, |lane| lane.token_snapshot.as_ref(), false);
    match (last_live, near_live) {
        (Some(live), Some(rollout)) if rollout.value.total_tokens >= live.value.total_tokens => {
            Some(rollout)
        }
        (Some(live), Some(_)) => Some(live),
        (Some(live), None) => Some(live),
        (None, Some(rollout)) => Some(rollout),
        (None, None) => best_field(
            &lanes.historical,
            |lane| lane.token_snapshot.as_ref(),
            false,
        ),
    }
}
