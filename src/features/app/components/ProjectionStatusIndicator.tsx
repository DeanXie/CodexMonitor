import type { ProjectionStatusModel } from "@app/orchestration/projectionStatusModel";

type ProjectionStatusIndicatorProps = {
  model: ProjectionStatusModel;
};

const deliveryLabels = {
  live: "Live",
  polling: "Polling",
  disconnected: "Disconnected",
} as const;

export function ProjectionStatusIndicator({
  model,
}: ProjectionStatusIndicatorProps) {
  const title = `${model.coverageDetails} · ${model.availability.label}`;
  return (
    <span
      className={`projection-status-indicator is-${model.primary}`}
      aria-label="Remote projection status"
      title={title}
    >
      <span className="projection-status-primary">{model.primaryLabel}</span>
      <span className="projection-status-availability">
        {model.availability.label}
      </span>
      {model.deliveryMode ? (
        <span className="projection-status-delivery">
          {deliveryLabels[model.deliveryMode]}
        </span>
      ) : null}
    </span>
  );
}
