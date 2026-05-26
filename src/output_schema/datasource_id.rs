// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct OutputDatasourceId(String);

impl OutputDatasourceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OutputDatasourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<crate::models::DatasourceId> for OutputDatasourceId {
    fn from(value: crate::models::DatasourceId) -> Self {
        Self(value.as_str().to_owned())
    }
}

impl From<&crate::models::DatasourceId> for OutputDatasourceId {
    fn from(value: &crate::models::DatasourceId) -> Self {
        Self(value.as_str().to_owned())
    }
}

impl TryFrom<&OutputDatasourceId> for crate::models::DatasourceId {
    type Error = String;
    fn try_from(value: &OutputDatasourceId) -> Result<Self, Self::Error> {
        Self::from_str(value.as_str())
    }
}

impl AsRef<str> for OutputDatasourceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_via_model() {
        let model = crate::models::DatasourceId::NpmPackageJson;
        let output = OutputDatasourceId::from(model);
        assert_eq!(output.as_str(), "npm_package_json");
        let back = crate::models::DatasourceId::try_from(&output).unwrap();
        assert_eq!(back, model);
    }

    #[test]
    fn serializes_as_plain_string() {
        let output = OutputDatasourceId::from(crate::models::DatasourceId::CargoToml);
        let json = serde_json::to_string(&output).unwrap();
        assert_eq!(json, "\"cargo_toml\"");
    }

    #[test]
    fn deserializes_from_plain_string() {
        let output: OutputDatasourceId = serde_json::from_str("\"npm_package_json\"").unwrap();
        assert_eq!(output.as_str(), "npm_package_json");
    }
}
