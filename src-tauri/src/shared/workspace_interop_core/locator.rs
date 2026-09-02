#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RootLocatorPlatform {
    Windows,
    Posix,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NormalizedRootLocator {
    platform: RootLocatorPlatform,
    value: String,
}

impl NormalizedRootLocator {
    pub(crate) fn parse(raw: &str, platform: RootLocatorPlatform) -> Result<Self, String> {
        if raw.is_empty() || raw.contains('\0') {
            return Err("root locator is empty or contains a null byte".to_string());
        }
        let value = match platform {
            RootLocatorPlatform::Windows => normalize_windows(raw)?,
            RootLocatorPlatform::Posix => normalize_posix(raw)?,
        };
        Ok(Self { platform, value })
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) fn component_count(&self) -> usize {
        match self.platform {
            RootLocatorPlatform::Windows if self.value.starts_with("//") => self
                .value
                .trim_start_matches('/')
                .split('/')
                .count()
                .saturating_sub(2),
            RootLocatorPlatform::Windows => self.value[3..]
                .split('/')
                .filter(|component| !component.is_empty())
                .count(),
            RootLocatorPlatform::Posix => self
                .value
                .trim_start_matches('/')
                .split('/')
                .filter(|component| !component.is_empty())
                .count(),
        }
    }

    pub(crate) fn contains(&self, path: &Self) -> bool {
        if self.platform != path.platform {
            return false;
        }
        path.value == self.value
            || if self.value.ends_with('/') {
                path.value.starts_with(&self.value)
            } else {
                path.value
                    .strip_prefix(&self.value)
                    .is_some_and(|suffix| suffix.starts_with('/'))
            }
    }
}

fn normalize_windows(raw: &str) -> Result<String, String> {
    let mut path = raw.replace('\\', "/");
    let lowercase = path.to_lowercase();
    if lowercase.starts_with("//?/unc/") || lowercase.starts_with("//./unc/") {
        path = format!("//{}", &path[8..]);
    } else if lowercase.starts_with("//?/") || lowercase.starts_with("//./") {
        path = path[4..].to_string();
    }

    if path.starts_with("//") {
        return normalize_unc(&path);
    }

    let bytes = path.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || bytes[2] != b'/' {
        return Err("Windows root locator must be drive-absolute or UNC".to_string());
    }

    let drive = (bytes[0] as char).to_ascii_lowercase();
    let components = normalize_components(&path[3..], true)?;
    if components.is_empty() {
        Ok(format!("{drive}:/"))
    } else {
        Ok(format!("{drive}:/{}", components.join("/")))
    }
}

fn normalize_unc(path: &str) -> Result<String, String> {
    let raw_components = path
        .trim_start_matches('/')
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    if raw_components.len() < 2
        || matches!(raw_components[0], "." | "..")
        || matches!(raw_components[1], "." | "..")
    {
        return Err("UNC root locator must include server and share".to_string());
    }

    let server = raw_components[0].to_lowercase();
    let share = raw_components[1].to_lowercase();
    let remainder = normalize_components(&raw_components[2..].join("/"), true)?;
    let mut normalized = format!("//{server}/{share}");
    if !remainder.is_empty() {
        normalized.push('/');
        normalized.push_str(&remainder.join("/"));
    }
    Ok(normalized)
}

fn normalize_posix(raw: &str) -> Result<String, String> {
    if !raw.starts_with('/') {
        return Err("POSIX root locator must be absolute".to_string());
    }
    let components = normalize_components(raw.trim_start_matches('/'), false)?;
    if components.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", components.join("/")))
    }
}

fn normalize_components(path: &str, case_insensitive: bool) -> Result<Vec<String>, String> {
    let mut components = Vec::new();
    for component in path.split('/').filter(|component| !component.is_empty()) {
        match component {
            "." => {}
            ".." => {
                return Err(
                    "root locator with parent components requires physical path evidence"
                        .to_string(),
                );
            }
            _ if case_insensitive => components.push(component.to_lowercase()),
            _ => components.push(component.to_string()),
        }
    }
    Ok(components)
}
