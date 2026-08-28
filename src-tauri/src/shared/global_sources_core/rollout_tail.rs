use super::source_envelope::SourceFileIdentity;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RolloutCheckpoint {
    pub generation: String,
    pub committed_byte_offset: u64,
    pub record_ordinal: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RolloutTailState {
    checkpoint: RolloutCheckpoint,
    read_byte_offset: u64,
    pending_tail: Vec<u8>,
}

impl RolloutTailState {
    pub(crate) fn new(generation: impl Into<String>) -> Self {
        Self::from_checkpoint(RolloutCheckpoint {
            generation: generation.into(),
            committed_byte_offset: 0,
            record_ordinal: 0,
        })
    }

    pub(crate) fn from_checkpoint(checkpoint: RolloutCheckpoint) -> Self {
        Self {
            read_byte_offset: checkpoint.committed_byte_offset,
            checkpoint,
            pending_tail: Vec::new(),
        }
    }

    pub(crate) fn checkpoint(&self) -> &RolloutCheckpoint {
        &self.checkpoint
    }

    pub(crate) fn pending_tail(&self) -> &[u8] {
        &self.pending_tail
    }

    fn reset(&mut self, generation: String) {
        self.checkpoint = RolloutCheckpoint {
            generation,
            committed_byte_offset: 0,
            record_ordinal: 0,
        };
        self.read_byte_offset = 0;
        self.pending_tail.clear();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompleteJsonlRecord {
    pub text: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub record_ordinal: u64,
    pub line_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RolloutDelta {
    pub records: Vec<CompleteJsonlRecord>,
    pub did_reset: bool,
}

pub(crate) fn read_rollout_delta(
    path: &Path,
    source_file: &mut SourceFileIdentity,
    state: &mut RolloutTailState,
    observed_timestamp_ms: i64,
) -> std::io::Result<RolloutDelta> {
    let file_length = std::fs::metadata(path)?.len();
    let mut did_reset = false;

    if state.checkpoint.generation != source_file.generation {
        state.reset(source_file.generation.clone());
        did_reset = true;
    } else if file_length < state.checkpoint.committed_byte_offset
        || file_length < state.read_byte_offset
    {
        source_file.generation =
            format!("{}:reset:{observed_timestamp_ms}", source_file.generation);
        source_file.session_meta_id = None;
        state.reset(source_file.generation.clone());
        did_reset = true;
    }

    let mut file = open_shared_read(path)?;
    file.seek(SeekFrom::Start(state.read_byte_offset))?;
    let mut appended = Vec::new();
    file.read_to_end(&mut appended)?;
    state.read_byte_offset += appended.len() as u64;
    state.pending_tail.extend_from_slice(&appended);

    let mut records = Vec::new();
    let mut consumed = 0usize;
    while let Some(relative_newline) = state.pending_tail[consumed..]
        .iter()
        .position(|byte| *byte == b'\n')
    {
        let newline = consumed + relative_newline;
        let complete_end = newline + 1;
        let mut line_end = newline;
        if line_end > consumed && state.pending_tail[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        let line = &state.pending_tail[consumed..line_end];
        let byte_start = state.checkpoint.committed_byte_offset;
        let byte_end = byte_start + (complete_end - consumed) as u64;

        if !line.is_empty() {
            let text = std::str::from_utf8(line)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
                .to_string();
            state.checkpoint.record_ordinal += 1;
            records.push(CompleteJsonlRecord {
                text,
                byte_start,
                byte_end,
                record_ordinal: state.checkpoint.record_ordinal,
                line_hash: sha256(line),
            });
        }
        state.checkpoint.committed_byte_offset = byte_end;
        consumed = complete_end;
    }

    if consumed > 0 {
        state.pending_tail.drain(..consumed);
    }
    Ok(RolloutDelta { records, did_reset })
}

fn open_shared_read(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;
        const FILE_SHARE_DELETE: u32 = 0x0000_0004;
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    options.open(path)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
