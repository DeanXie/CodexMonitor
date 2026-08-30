use super::source_registry::TokenSnapshot;
use chrono::DateTime;
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SubAgentSpawn {
    pub parent_thread_id: String,
    pub depth: Option<u64>,
    pub agent_path: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionMetaRecord {
    pub record_timestamp_ms: i64,
    pub id: String,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub cli_version: Option<String>,
    pub source_name: Option<String>,
    pub originator: Option<String>,
    pub thread_source: Option<String>,
    pub model_provider: Option<String>,
    pub subagent_spawn: Option<SubAgentSpawn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TaskStartedRecord {
    pub record_timestamp_ms: i64,
    pub turn_id: String,
    pub started_at_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TurnContextRecord {
    pub record_timestamp_ms: i64,
    pub turn_id: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TokenCountRecord {
    pub record_timestamp_ms: i64,
    pub total: Option<TokenSnapshot>,
    pub last: Option<TokenSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TaskCompleteRecord {
    pub record_timestamp_ms: i64,
    pub turn_id: String,
    pub started_at_seconds: Option<i64>,
    pub completed_at_seconds: i64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WaitRecord {
    pub record_timestamp_ms: i64,
    pub call_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChildBoundaryMarkerRecord {
    pub record_timestamp_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParsedRolloutRecord {
    SessionMeta(SessionMetaRecord),
    TaskStarted(TaskStartedRecord),
    TurnContext(TurnContextRecord),
    TokenCount(TokenCountRecord),
    TaskComplete(TaskCompleteRecord),
    ChildBoundaryMarker(ChildBoundaryMarkerRecord),
    WaitStarted(WaitRecord),
    WaitResumed(WaitRecord),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RolloutParseError(String);

impl Display for RolloutParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RolloutParseError {}

#[derive(Clone, Debug, Default)]
pub(crate) struct RolloutRecordParser {
    pending_wait_calls: HashSet<String>,
}

impl ParsedRolloutRecord {
    pub(crate) fn record_timestamp_ms(&self) -> i64 {
        match self {
            Self::SessionMeta(value) => value.record_timestamp_ms,
            Self::TaskStarted(value) => value.record_timestamp_ms,
            Self::TurnContext(value) => value.record_timestamp_ms,
            Self::TokenCount(value) => value.record_timestamp_ms,
            Self::TaskComplete(value) => value.record_timestamp_ms,
            Self::ChildBoundaryMarker(value) => value.record_timestamp_ms,
            Self::WaitStarted(value) | Self::WaitResumed(value) => value.record_timestamp_ms,
        }
    }
}

impl RolloutRecordParser {
    pub(crate) fn parse_line(
        &mut self,
        line: &str,
    ) -> Result<Option<ParsedRolloutRecord>, RolloutParseError> {
        let value = serde_json::from_str(line)
            .map_err(|error| RolloutParseError(format!("invalid rollout JSON: {error}")))?;
        self.parse_value(&value)
    }

    pub(crate) fn parse_value(
        &mut self,
        value: &Value,
    ) -> Result<Option<ParsedRolloutRecord>, RolloutParseError> {
        let record_type = string(value, "type")?;
        let record_timestamp_ms = parse_timestamp(string(value, "timestamp")?)?;
        let payload = object(value, "payload")?;

        match record_type {
            "session_meta" => self
                .parse_session_meta(payload, record_timestamp_ms)
                .map(Some),
            "turn_context" => self
                .parse_turn_context(payload, record_timestamp_ms)
                .map(Some),
            "event_msg" => self.parse_event(payload, record_timestamp_ms),
            "response_item" => self.parse_response_item(payload, record_timestamp_ms),
            _ => Ok(None),
        }
    }

    fn parse_session_meta(
        &self,
        payload: &Value,
        record_timestamp_ms: i64,
    ) -> Result<ParsedRolloutRecord, RolloutParseError> {
        let subagent_spawn = payload
            .get("source")
            .and_then(|source| source.get("subagent"))
            .and_then(|subagent| subagent.get("thread_spawn"))
            .map(|spawn| {
                Ok(SubAgentSpawn {
                    parent_thread_id: string(spawn, "parent_thread_id")?.to_string(),
                    depth: spawn.get("depth").and_then(Value::as_u64),
                    agent_path: optional_string(spawn, "agent_path"),
                    agent_nickname: optional_string(spawn, "agent_nickname"),
                    agent_role: optional_string(spawn, "agent_role"),
                })
            })
            .transpose()?;
        Ok(ParsedRolloutRecord::SessionMeta(SessionMetaRecord {
            record_timestamp_ms,
            id: string(payload, "id")?.to_string(),
            session_id: optional_string(payload, "session_id"),
            cwd: optional_string(payload, "cwd"),
            cli_version: optional_string(payload, "cli_version"),
            source_name: payload
                .get("source")
                .and_then(Value::as_str)
                .map(str::to_string),
            originator: optional_string(payload, "originator"),
            thread_source: optional_string(payload, "thread_source"),
            model_provider: optional_string(payload, "model_provider"),
            subagent_spawn,
        }))
    }

    fn parse_turn_context(
        &self,
        payload: &Value,
        record_timestamp_ms: i64,
    ) -> Result<ParsedRolloutRecord, RolloutParseError> {
        Ok(ParsedRolloutRecord::TurnContext(TurnContextRecord {
            record_timestamp_ms,
            turn_id: string(payload, "turn_id")?.to_string(),
            model: optional_string(payload, "model"),
            effort: optional_string(payload, "effort"),
        }))
    }

    fn parse_event(
        &self,
        payload: &Value,
        record_timestamp_ms: i64,
    ) -> Result<Option<ParsedRolloutRecord>, RolloutParseError> {
        match string(payload, "type")? {
            "task_started" => Ok(Some(ParsedRolloutRecord::TaskStarted(TaskStartedRecord {
                record_timestamp_ms,
                turn_id: string(payload, "turn_id")?.to_string(),
                started_at_seconds: integer(payload, "started_at")?,
            }))),
            "task_complete" => Ok(Some(ParsedRolloutRecord::TaskComplete(
                TaskCompleteRecord {
                    record_timestamp_ms,
                    turn_id: string(payload, "turn_id")?.to_string(),
                    started_at_seconds: payload.get("started_at").and_then(Value::as_i64),
                    completed_at_seconds: integer(payload, "completed_at")?,
                    duration_ms: unsigned(payload, "duration_ms")?,
                },
            ))),
            "thread_settings_applied" => Ok(Some(ParsedRolloutRecord::ChildBoundaryMarker(
                ChildBoundaryMarkerRecord {
                    record_timestamp_ms,
                },
            ))),
            "token_count" => {
                let info = payload.get("info").filter(|value| value.is_object());
                Ok(Some(ParsedRolloutRecord::TokenCount(TokenCountRecord {
                    record_timestamp_ms,
                    total: info
                        .and_then(|value| value.get("total_token_usage"))
                        .and_then(parse_tokens),
                    last: info
                        .and_then(|value| value.get("last_token_usage"))
                        .and_then(parse_tokens),
                })))
            }
            _ => Ok(None),
        }
    }

    fn parse_response_item(
        &mut self,
        payload: &Value,
        record_timestamp_ms: i64,
    ) -> Result<Option<ParsedRolloutRecord>, RolloutParseError> {
        match string(payload, "type")? {
            "function_call"
                if payload.get("name").and_then(Value::as_str) == Some("wait_agent") =>
            {
                let call_id = string(payload, "call_id")?.to_string();
                self.pending_wait_calls.insert(call_id.clone());
                Ok(Some(ParsedRolloutRecord::WaitStarted(WaitRecord {
                    record_timestamp_ms,
                    call_id,
                })))
            }
            "function_call_output" => {
                let call_id = string(payload, "call_id")?.to_string();
                if self.pending_wait_calls.remove(&call_id) {
                    Ok(Some(ParsedRolloutRecord::WaitResumed(WaitRecord {
                        record_timestamp_ms,
                        call_id,
                    })))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}

fn parse_timestamp(value: &str) -> Result<i64, RolloutParseError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.timestamp_millis())
        .map_err(|error| RolloutParseError(format!("invalid rollout timestamp: {error}")))
}

fn parse_tokens(value: &Value) -> Option<TokenSnapshot> {
    Some(TokenSnapshot {
        input_tokens: value.get("input_tokens")?.as_u64()?,
        cached_input_tokens: value.get("cached_input_tokens")?.as_u64()?,
        cache_write_input_tokens: value
            .get("cache_write_input_tokens")
            .and_then(Value::as_u64),
        output_tokens: value.get("output_tokens")?.as_u64()?,
        reasoning_output_tokens: value.get("reasoning_output_tokens")?.as_u64()?,
        total_tokens: value.get("total_tokens")?.as_u64()?,
    })
}

fn object<'a>(value: &'a Value, key: &str) -> Result<&'a Value, RolloutParseError> {
    value
        .get(key)
        .filter(|value| value.is_object())
        .ok_or_else(|| {
            RolloutParseError(format!(
                "unsupported rollout schema: missing object field {key}"
            ))
        })
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, RolloutParseError> {
    value.get(key).and_then(Value::as_str).ok_or_else(|| {
        RolloutParseError(format!(
            "unsupported rollout schema: missing string field {key}"
        ))
    })
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn integer(value: &Value, key: &str) -> Result<i64, RolloutParseError> {
    value.get(key).and_then(Value::as_i64).ok_or_else(|| {
        RolloutParseError(format!(
            "unsupported rollout schema: missing integer field {key}"
        ))
    })
}

fn unsigned(value: &Value, key: &str) -> Result<u64, RolloutParseError> {
    value.get(key).and_then(Value::as_u64).ok_or_else(|| {
        RolloutParseError(format!(
            "unsupported rollout schema: missing unsigned field {key}"
        ))
    })
}
