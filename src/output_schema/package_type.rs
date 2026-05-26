// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct OutputPackageType(String);

impl OutputPackageType {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OutputPackageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<crate::models::PackageType> for OutputPackageType {
    fn from(value: crate::models::PackageType) -> Self {
        Self(value.as_str().to_owned())
    }
}

impl From<&crate::models::PackageType> for OutputPackageType {
    fn from(value: &crate::models::PackageType) -> Self {
        Self(value.as_str().to_owned())
    }
}

impl TryFrom<&OutputPackageType> for crate::models::PackageType {
    type Error = String;
    fn try_from(value: &OutputPackageType) -> Result<Self, Self::Error> {
        Self::from_str(value.as_str())
    }
}

impl AsRef<str> for OutputPackageType {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_via_model() {
        let model = crate::models::PackageType::Cargo;
        let output = OutputPackageType::from(model);
        assert_eq!(output.as_str(), "cargo");
        let back = crate::models::PackageType::try_from(&output).unwrap();
        assert_eq!(back, model);
    }

    #[test]
    fn serializes_as_plain_string() {
        let output = OutputPackageType::from(crate::models::PackageType::Cargo);
        let json = serde_json::to_string(&output).unwrap();
        assert_eq!(json, "\"cargo\"");
    }

    #[test]
    fn deserializes_from_plain_string() {
        let output: OutputPackageType = serde_json::from_str("\"cargo\"").unwrap();
        assert_eq!(output.as_str(), "cargo");
    }

    #[test]
    fn kebab_case_variant_roundtrips() {
        let model = crate::models::PackageType::JbossService;
        let output = OutputPackageType::from(model);
        assert_eq!(output.as_str(), "jboss-service");
        let back = crate::models::PackageType::try_from(&output).unwrap();
        assert_eq!(back, model);
    }
}
