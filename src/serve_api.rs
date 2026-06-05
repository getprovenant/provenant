// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServeLivenessResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServeReadinessResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spdx_license_list_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServeVersionResponse {
    pub service: String,
    pub api_version: String,
    pub tool_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServeErrorResponse {
    pub status: String,
    pub message: String,
    pub api_version: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AsyncJobState {
    Pending,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AsyncScanAcceptedResponse {
    pub status: String,
    pub job_id: String,
    pub state: AsyncJobState,
    pub status_url: String,
    pub result_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AsyncJobStatusResponse {
    pub job_id: String,
    pub state: AsyncJobState,
    pub result_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocated_processors: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServeScanRequest {
    pub input: ServeScanInput,
    #[serde(default)]
    pub options: ServeScanOptions,
}

impl ServeScanRequest {
    pub fn decode(body: &[u8]) -> Result<Self> {
        serde_json::from_slice(body).context("request body must be valid JSON")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServeScanInput {
    Paths {
        paths: Vec<String>,
    },
    Repository {
        url: String,
        #[serde(rename = "ref")]
        reference: String,
    },
    Url {
        url: String,
    },
    Upload {
        filename: String,
        content_base64: String,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServeLicenseSource {
    #[default]
    Disabled,
    Embedded,
    Directory {
        path: String,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ServeScanOptions {
    pub collect_info: bool,
    pub detect_license: ServeLicenseSource,
    pub detect_packages: bool,
    pub detect_system_packages: bool,
    pub detect_packages_in_compiled: bool,
    pub detect_copyrights: bool,
    pub detect_emails: bool,
    pub detect_urls: bool,
    pub detect_generated: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub strip_root: bool,
    pub full_root: bool,
    pub license_text: bool,
    pub license_text_diagnostics: bool,
    pub license_diagnostics: bool,
    pub unknown_licenses: bool,
    /// Disable approximate sequence matching when license detection is enabled.
    pub no_sequence_matching: bool,
    pub license_score: u8,
    pub only_findings: bool,
    pub mark_source: bool,
    pub classify: bool,
    pub summary: bool,
    pub license_clarity_score: bool,
    pub license_references: bool,
    pub tallies: bool,
    pub tallies_key_files: bool,
    pub tallies_with_details: bool,
    pub facets: Vec<String>,
    pub tallies_by_facet: bool,
}

pub fn openapi_document() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Provenant Serve API",
            "version": API_VERSION,
            "description": "Current machine-readable contract for the implemented `provenant serve` HTTP API surface."
        },
        "paths": {
            "/livez": {
                "get": {
                    "summary": "Liveness probe",
                    "operationId": "getLivez",
                    "responses": {
                        "200": {
                            "description": "Process is alive.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeLivenessResponse"}
                                }
                            }
                        }
                    }
                }
            },
            "/readyz": {
                "get": {
                    "summary": "Readiness probe",
                    "operationId": "getReadyz",
                    "responses": {
                        "200": {
                            "description": "Service is ready to accept scan requests.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeReadinessResponse"}
                                }
                            }
                        },
                        "503": {
                            "description": "Service is still warming or startup failed.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeReadinessResponse"}
                                }
                            }
                        }
                    }
                }
            },
            "/version": {
                "get": {
                    "summary": "Version metadata",
                    "operationId": "getVersion",
                    "responses": {
                        "200": {
                            "description": "Current service and tool version metadata.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeVersionResponse"}
                                }
                            }
                        }
                    }
                }
            },
            "/v1/scans": {
                "post": {
                    "summary": "Run a synchronous scan",
                    "operationId": "postSyncScan",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/ServeScanRequest"}
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "ScanCode-compatible scan result output.",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "description": "Current ScanCode-compatible output JSON returned by the shared Provenant output schema."
                                    }
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid HTTP request or malformed JSON body.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        },
                        "415": {
                            "description": "Unsupported media type.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        },
                        "422": {
                            "description": "Request is well-formed but cannot be executed.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        }
                    }
                }
            },
            "/v1/scans:async": {
                "post": {
                    "summary": "Submit an asynchronous scan job",
                    "operationId": "postAsyncScan",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/ServeScanRequest"}
                            }
                        }
                    },
                    "responses": {
                        "202": {
                            "description": "Scan job accepted for bounded background execution.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/AsyncScanAcceptedResponse"}
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid HTTP request or malformed JSON body.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        },
                        "415": {
                            "description": "Unsupported media type.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        },
                        "422": {
                            "description": "Request is well-formed but cannot be executed.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        },
                        "503": {
                            "description": "Service has no remaining async admission capacity.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        }
                    }
                }
            },
            "/v1/jobs/{id}": {
                "get": {
                    "summary": "Inspect asynchronous job state",
                    "operationId": "getAsyncJobStatus",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Current async job state.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/AsyncJobStatusResponse"}
                                }
                            }
                        },
                        "404": {
                            "description": "Async job was not found.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        }
                    }
                }
            },
            "/v1/jobs/{id}/result": {
                "get": {
                    "summary": "Fetch completed asynchronous job result",
                    "operationId": "getAsyncJobResult",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Completed ScanCode-compatible scan result output.",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "description": "Current ScanCode-compatible output JSON returned by the shared Provenant output schema."
                                    }
                                }
                            }
                        },
                        "404": {
                            "description": "Async job was not found.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        },
                        "409": {
                            "description": "Async job has not completed yet.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        },
                        "422": {
                            "description": "Async job completed with a failure.",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ServeErrorResponse"}
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "ServeLivenessResponse": schema_json::<ServeLivenessResponse>(),
                "ServeReadinessResponse": schema_json::<ServeReadinessResponse>(),
                "ServeVersionResponse": schema_json::<ServeVersionResponse>(),
                "ServeErrorResponse": schema_json::<ServeErrorResponse>(),
                "AsyncJobState": schema_json::<AsyncJobState>(),
                "AsyncScanAcceptedResponse": schema_json::<AsyncScanAcceptedResponse>(),
                "AsyncJobStatusResponse": schema_json::<AsyncJobStatusResponse>(),
                "ServeScanRequest": schema_json::<ServeScanRequest>(),
                "ServeScanInput": schema_json::<ServeScanInput>(),
                "ServeLicenseSource": schema_json::<ServeLicenseSource>(),
                "ServeScanOptions": schema_json::<ServeScanOptions>()
            }
        }
    })
}

fn schema_json<T: JsonSchema>() -> Value {
    serde_json::to_value(schema_for!(T)).expect("schema should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_rejects_invalid_json() {
        let error =
            ServeScanRequest::decode(br#"{"input": }"#).expect_err("malformed JSON should fail");

        assert!(
            error
                .to_string()
                .contains("request body must be valid JSON")
        );
    }
}
