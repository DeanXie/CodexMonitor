type HomeActionsProps = {
  onAddWorkspace: () => void;
  onAddWorkspaceFromUrl: () => void;
  onOpenAgentMonitor?: () => void;
};

export function HomeActions({
  onAddWorkspace,
  onAddWorkspaceFromUrl,
  onOpenAgentMonitor,
}: HomeActionsProps) {
  return (
    <div className="home-actions">
      <button
        className="home-button primary home-add-workspaces-button"
        onClick={onAddWorkspace}
        data-tauri-drag-region="false"
      >
        <span className="home-icon" aria-hidden>
          +
        </span>
        Add Workspaces
      </button>
      {onOpenAgentMonitor ? (
        <button className="home-button secondary" onClick={onOpenAgentMonitor} data-tauri-drag-region="false">
          Agent Monitor
        </button>
      ) : null}
      <button
        className="home-button secondary home-add-workspace-from-url-button"
        onClick={onAddWorkspaceFromUrl}
        data-tauri-drag-region="false"
      >
        <span className="home-icon" aria-hidden>
          ⤓
        </span>
        Add Workspace from URL
      </button>
    </div>
  );
}
