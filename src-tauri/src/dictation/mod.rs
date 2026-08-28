// Dictation is intentionally disabled for this build. Keeping the stub preserves
// the IPC contract without pulling native Whisper/audio dependencies into Cargo.
#[path = "stub.rs"]
mod imp;

pub(crate) use imp::*;
