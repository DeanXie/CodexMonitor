import { useEffect, useState } from "react";
import type { AppSettings } from "@/types";
import { getAppBuildType, type AppBuildType } from "@services/tauri";
import { SettingsSection } from "@/features/design-system/components/settings/SettingsPrimitives";

type SettingsAboutSectionProps = {
  appSettings: AppSettings;
  onToggleAutomaticAppUpdateChecks?: () => void;
};

export function SettingsAboutSection({
  appSettings: _appSettings,
  onToggleAutomaticAppUpdateChecks: _onToggleAutomaticAppUpdateChecks,
}: SettingsAboutSectionProps) {
  const [appBuildType, setAppBuildType] = useState<AppBuildType | "unknown">("unknown");

  useEffect(() => {
    let active = true;
    const loadBuildType = async () => {
      try {
        const value = await getAppBuildType();
        if (active) setAppBuildType(value);
      } catch {
        if (active) setAppBuildType("unknown");
      }
    };
    void loadBuildType();
    return () => {
      active = false;
    };
  }, []);

  const buildDateValue = __APP_BUILD_DATE__.trim();
  const parsedBuildDate = Date.parse(buildDateValue);
  const buildDateLabel = Number.isNaN(parsedBuildDate)
    ? buildDateValue || "unknown"
    : new Date(parsedBuildDate).toLocaleString();

  return (
    <SettingsSection title="About" subtitle="App version and build metadata.">
      <div className="settings-field">
        <div className="settings-help">
          Version: <code>{__APP_VERSION__}</code>
        </div>
        <div className="settings-help">
          Build type: <code>{appBuildType}</code>
        </div>
        <div className="settings-help">
          Branch: <code>{__APP_GIT_BRANCH__ || "unknown"}</code>
        </div>
        <div className="settings-help">
          Commit: <code>{__APP_COMMIT_HASH__ || "unknown"}</code>
        </div>
        <div className="settings-help">
          Build date: <code>{buildDateLabel}</code>
        </div>
      </div>
      <div className="settings-field">
        <div className="settings-label">App Updates</div>
        <div className="settings-help">
          Automatic and manual in-app updates are disabled for this distribution.
        </div>
      </div>
    </SettingsSection>
  );
}
