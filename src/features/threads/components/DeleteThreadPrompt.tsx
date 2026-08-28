import { ModalShell } from "@/features/design-system/components/modal/ModalShell";

type DeleteThreadPromptProps = {
  title: string;
  blocked: boolean;
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onConfirm: () => void;
};

export function DeleteThreadPrompt({ title, blocked, busy, error, onCancel, onConfirm }: DeleteThreadPromptProps) {
  return (
    <ModalShell className="thread-delete-modal" onBackdropClick={busy ? undefined : onCancel} ariaLabel="Delete conversation permanently">
      <div className="ds-modal-title">Delete conversation permanently?</div>
      <div className="ds-modal-subtitle">“{title}”</div>
      <p>This permanently removes the Codex conversation and any spawned Sub-Agent descendants deleted by the server. This cannot be undone.</p>
      {blocked ? <p className="thread-delete-warning" role="status">This conversation or one of its agents is Running or Waiting. Finish it before deleting.</p> : null}
      {error ? <p className="thread-delete-error" role="alert">{error}</p> : null}
      <div className="ds-modal-actions">
        <button className="ghost ds-modal-button" type="button" onClick={onCancel} disabled={busy}>Cancel</button>
        <button className="danger ds-modal-button thread-delete-confirm" type="button" onClick={onConfirm} disabled={blocked || busy}>
          {busy ? "Deleting…" : "Delete permanently"}
        </button>
      </div>
    </ModalShell>
  );
}
