import { useEffect, useState, type ReactNode } from "react";
import {
  activateFreshProfile,
  getBootstrapStatus,
  isMobileRuntime,
  recoverProfileActivation,
  type BootstrapStatus,
} from "@services/tauri";

type Props = { children: ReactNode };

export function BootstrapBoundary({ children }: Props) {
  const [status, setStatus] = useState<BootstrapStatus | null>(null);
  const [mobile, setMobile] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    void isMobileRuntime()
      .then(async (isMobile) => {
        if (!active) return;
        if (isMobile) {
          setMobile(true);
          return;
        }
        setStatus(await getBootstrapStatus());
      })
      .catch((value) => {
        if (active) setError(String(value));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  if (mobile || (status?.inspection.normalLoadAllowed && !status.restartRequired)) {
    return <>{children}</>;
  }
  if (loading) {
    return <main className="bootstrap-boundary">Checking profile activation…</main>;
  }

  const activateFresh = async () => {
    setLoading(true);
    setError(null);
    try {
      setStatus(await activateFreshProfile());
    } catch (value) {
      setError(String(value));
    } finally {
      setLoading(false);
    }
  };

  const recoverActivation = async () => {
    setLoading(true);
    setError(null);
    try {
      setStatus(await recoverProfileActivation());
    } catch (value) {
      setError(String(value));
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="bootstrap-boundary" data-bootstrap-state={status?.inspection.disposition}>
      <section className="bootstrap-boundary__panel">
        <h1>CodexMonitor DeanX setup</h1>
        <p>{status?.inspection.reason ?? error ?? "Profile activation is unavailable."}</p>
        {status?.inspection.disposition === "fresh_activation_required" ? (
          <button type="button" onClick={() => void activateFresh()} disabled={loading}>
            Create fresh profile
          </button>
        ) : null}
        {status?.inspection.disposition === "recovery_required" ? (
          <button type="button" onClick={() => void recoverActivation()} disabled={loading}>
            Recover prepared activation
          </button>
        ) : null}
        {status?.restartRequired ? <p>Activation complete. Restart the application.</p> : null}
        {error ? <p role="alert">{error}</p> : null}
      </section>
    </main>
  );
}
