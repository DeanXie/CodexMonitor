import type { UpdateState } from "../hooks/useUpdater";

type UpdateToastProps = {
  state: UpdateState;
  onUpdate: () => void;
  onDismiss: () => void;
};

/** The custom distribution has no in-app update notification surface. */
export function UpdateToast(_props: UpdateToastProps) {
  return null;
}
