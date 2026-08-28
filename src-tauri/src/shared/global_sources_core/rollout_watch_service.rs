use super::rollout_watcher::{
    FsRolloutDeltaReader, ReconcileReport, RolloutDeltaReader, RolloutTailWatcher,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) enum RolloutWatchCommand {
    IngestLive(super::source_registry::SourceLaneUpdate),
}

#[derive(Clone, Debug)]
pub(crate) enum RolloutWatchEvent {
    Reconciled(ReconcileReport),
    LiveIngested {
        update: super::source_registry::SourceLaneUpdate,
        accepted: bool,
    },
}

pub(crate) struct RolloutWatchService<R = FsRolloutDeltaReader> {
    core: RolloutTailWatcher<R>,
    _watcher: RecommendedWatcher,
    signals: tokio::sync::mpsc::UnboundedReceiver<Vec<PathBuf>>,
}

impl<R: RolloutDeltaReader + Send + 'static> RolloutWatchService<R> {
    pub(crate) fn new(core: RolloutTailWatcher<R>) -> notify::Result<Self> {
        let (sender, signals) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let paths = event.map(|event| event.paths).unwrap_or_default();
                let _ = sender.send(paths);
            })?;
        for root in core.watched_roots() {
            if root.exists() {
                watcher.watch(&root, RecursiveMode::Recursive)?;
            }
        }
        Ok(Self {
            core,
            _watcher: watcher,
            signals,
        })
    }

    pub(crate) async fn run_until<F>(
        mut self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
        mut commands: tokio::sync::mpsc::UnboundedReceiver<RolloutWatchCommand>,
        mut on_event: F,
    ) -> io::Result<()>
    where
        F: FnMut(RolloutWatchEvent, &super::source_registry::SourceAuthorityRegistry) + Send,
    {
        let mut interval = tokio::time::interval(self.core.reconciliation_interval());
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let now = chrono::Utc::now().timestamp_millis();
                    let fresh_window_ms = self.core.fresh_window_ms();
                    self.core.registry_mut().expire_live_lanes(now, fresh_window_ms);
                    let report = self.core.reconcile_now()?;
                    on_event(RolloutWatchEvent::Reconciled(report), self.core.registry());
                }
                signal = self.signals.recv() => {
                    let Some(paths) = signal else { return Ok(()); };
                    let now = chrono::Utc::now().timestamp_millis();
                    let fresh_window_ms = self.core.fresh_window_ms();
                    self.core.registry_mut().expire_live_lanes(now, fresh_window_ms);
                    self.core.record_filesystem_signal(paths, now);
                    let report = self.core.reconcile_now()?;
                    on_event(RolloutWatchEvent::Reconciled(report), self.core.registry());
                }
                command = commands.recv() => {
                    let Some(command) = command else { continue; };
                    match command {
                        RolloutWatchCommand::IngestLive(update) => {
                            let accepted = self.core.registry_mut().ingest(update.clone())
                                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
                            on_event(
                                RolloutWatchEvent::LiveIngested { update, accepted },
                                self.core.registry(),
                            );
                        }
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                }
            }
        }
    }
}
