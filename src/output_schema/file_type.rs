// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputFileType {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "directory")]
    Directory,
}

impl From<crate::models::FileType> for OutputFileType {
    fn from(value: crate::models::FileType) -> Self {
        match value {
            crate::models::FileType::File => OutputFileType::File,
            crate::models::FileType::Directory => OutputFileType::Directory,
        }
    }
}

impl From<&crate::models::FileType> for OutputFileType {
    fn from(value: &crate::models::FileType) -> Self {
        match value {
            crate::models::FileType::File => OutputFileType::File,
            crate::models::FileType::Directory => OutputFileType::Directory,
        }
    }
}

impl TryFrom<OutputFileType> for crate::models::FileType {
    type Error = String;
    fn try_from(value: OutputFileType) -> Result<Self, Self::Error> {
        match value {
            OutputFileType::File => Ok(crate::models::FileType::File),
            OutputFileType::Directory => Ok(crate::models::FileType::Directory),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_serializes_to_file() {
        let json = serde_json::to_string(&OutputFileType::File).unwrap();
        assert_eq!(json, r#""file""#);
    }

    #[test]
    fn directory_serializes_to_directory() {
        let json = serde_json::to_string(&OutputFileType::Directory).unwrap();
        assert_eq!(json, r#""directory""#);
    }

    #[test]
    fn roundtrip_from_model() {
        let model = crate::models::FileType::File;
        let output = OutputFileType::from(model.clone());
        assert_eq!(output, OutputFileType::File);
        let back = crate::models::FileType::try_from(output).unwrap();
        assert_eq!(back, model);
    }
}
