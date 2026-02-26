use colored::*;
use serde::Serialize;
use souk_core::error::{Severity, ValidationResult};

/// Output mode for the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Quiet,
}

/// Accumulated JSON result entry.
#[derive(Debug, Serialize, Clone)]
pub struct JsonResultEntry {
    #[serde(rename = "type")]
    pub result_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Accumulated JSON output.
#[derive(Debug, Serialize)]
pub struct JsonOutput {
    pub results: Vec<JsonResultEntry>,
}

/// Reporter handles all output formatting.
pub struct Reporter {
    mode: OutputMode,
    json_results: Vec<JsonResultEntry>,
}

impl Reporter {
    pub fn new(mode: OutputMode) -> Self {
        Self {
            mode,
            json_results: Vec::new(),
        }
    }

    /// Returns the current output mode.
    pub fn mode(&self) -> OutputMode {
        self.mode
    }

    pub fn error(&mut self, message: &str) {
        match self.mode {
            OutputMode::Human => {
                eprintln!("{} {}", "ERROR:".red(), message);
            }
            OutputMode::Json => {
                self.json_results.push(JsonResultEntry {
                    result_type: "error".to_string(),
                    message: message.to_string(),
                    details: None,
                });
            }
            OutputMode::Quiet => {
                eprintln!("{} {}", "ERROR:".red(), message);
            }
        }
    }

    pub fn warning(&mut self, message: &str) {
        match self.mode {
            OutputMode::Human => {
                eprintln!("{} {}", "WARNING:".yellow(), message);
            }
            OutputMode::Json => {
                self.json_results.push(JsonResultEntry {
                    result_type: "warning".to_string(),
                    message: message.to_string(),
                    details: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    pub fn success(&mut self, message: &str) {
        match self.mode {
            OutputMode::Human => {
                println!("{} {}", "✓".green(), message);
            }
            OutputMode::Json => {
                self.json_results.push(JsonResultEntry {
                    result_type: "success".to_string(),
                    message: message.to_string(),
                    details: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    pub fn success_with_details(&mut self, message: &str, details: &str) {
        match self.mode {
            OutputMode::Human => {
                println!("{} {}", "✓".green(), message);
            }
            OutputMode::Json => {
                self.json_results.push(JsonResultEntry {
                    result_type: "success".to_string(),
                    message: message.to_string(),
                    details: Some(details.to_string()),
                });
            }
            OutputMode::Quiet => {}
        }
    }

    pub fn info(&mut self, message: &str) {
        match self.mode {
            OutputMode::Human => {
                println!("{} {}", "INFO:".blue(), message);
            }
            OutputMode::Json => {
                self.json_results.push(JsonResultEntry {
                    result_type: "info".to_string(),
                    message: message.to_string(),
                    details: None,
                });
            }
            OutputMode::Quiet => {}
        }
    }

    pub fn section(&mut self, title: &str) {
        if self.mode == OutputMode::Human {
            println!("{}", format!("=== {title} ===").cyan());
        }
    }

    pub fn report_validation(&mut self, result: &ValidationResult) {
        for diagnostic in &result.diagnostics {
            let mut msg = diagnostic.message.clone();
            if let Some(path) = &diagnostic.path {
                msg = format!("{msg} ({path})", path = path.display());
            }
            match diagnostic.severity {
                Severity::Error => self.error(&msg),
                Severity::Warning => self.warning(&msg),
            }
        }
    }

    pub fn finish(&self) {
        if self.mode == OutputMode::Json {
            let output = JsonOutput {
                results: self.json_results.clone(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&output) {
                println!("{json}");
            }
        }
    }
}
