// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutputCopyright {
    pub copyright: String,
    pub start_line: u64,
    pub end_line: u64,
}

impl From<&crate::models::Copyright> for OutputCopyright {
    fn from(value: &crate::models::Copyright) -> Self {
        Self::from_with_compat_mode(value, crate::cli::CompatibilityMode::Native)
    }
}

impl OutputCopyright {
    pub fn from_with_compat_mode(
        value: &crate::models::Copyright,
        mode: crate::cli::CompatibilityMode,
    ) -> Self {
        Self {
            copyright: match mode {
                crate::cli::CompatibilityMode::Native => value.copyright.clone(),
                crate::cli::CompatibilityMode::Scancode => value.normalized_text().to_string(),
            },
            start_line: value.start_line.get() as u64,
            end_line: value.end_line.get() as u64,
        }
    }
}

impl TryFrom<&OutputCopyright> for crate::models::Copyright {
    type Error = String;
    fn try_from(value: &OutputCopyright) -> Result<Self, Self::Error> {
        use crate::models::LineNumber;
        let start_line = LineNumber::new(value.start_line as usize)
            .ok_or_else(|| format!("invalid start_line: {}", value.start_line))?;
        let end_line = LineNumber::new(value.end_line as usize)
            .ok_or_else(|| format!("invalid end_line: {}", value.end_line))?;
        Ok(Self {
            copyright: value.copyright.clone(),
            normalized_copyright: crate::copyright::refine_copyright(&value.copyright),
            start_line,
            end_line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LineNumber;

    #[test]
    fn output_copyright_uses_selected_rendering_mode() {
        let copyright = crate::models::Copyright {
            copyright: "Copyright 2024 Example Corp. All rights reserved.".to_string(),
            normalized_copyright: Some("Copyright 2024 Example Corp.".to_string()),
            start_line: LineNumber::ONE,
            end_line: LineNumber::ONE,
        };

        let raw = OutputCopyright::from_with_compat_mode(
            &copyright,
            crate::cli::CompatibilityMode::Native,
        );
        let compat = OutputCopyright::from_with_compat_mode(
            &copyright,
            crate::cli::CompatibilityMode::Scancode,
        );

        assert_eq!(
            raw.copyright,
            "Copyright 2024 Example Corp. All rights reserved."
        );
        assert_eq!(compat.copyright, "Copyright 2024 Example Corp.");
    }

    #[test]
    fn output_copyright_try_from_reconstructs_normalized_text() {
        let output = OutputCopyright {
            copyright: "Copyright 2024 Example Corp. All rights reserved.".to_string(),
            start_line: 1,
            end_line: 1,
        };

        let converted = crate::models::Copyright::try_from(&output).expect("conversion");

        assert_eq!(converted.copyright, output.copyright);
        assert_eq!(
            converted.normalized_copyright.as_deref(),
            Some("Copyright 2024 Example Corp.")
        );
    }
}
