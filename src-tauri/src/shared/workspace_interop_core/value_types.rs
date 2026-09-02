use super::NormalizedRootLocator;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExecutionEnvironmentKey(String);

impl ExecutionEnvironmentKey {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() || normalized.contains('\0') {
            return Err("execution environment key must be non-empty".to_string());
        }
        Ok(Self(normalized.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct WorkspaceKey {
    pub execution_environment_key: ExecutionEnvironmentKey,
    pub normalized_root_locator: NormalizedRootLocator,
}

impl WorkspaceKey {
    pub(crate) fn new(
        execution_environment_key: ExecutionEnvironmentKey,
        normalized_root_locator: NormalizedRootLocator,
    ) -> Self {
        Self {
            execution_environment_key,
            normalized_root_locator,
        }
    }
}
