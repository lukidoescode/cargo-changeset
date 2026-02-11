use std::path::PathBuf;

use changeset_operations::verification::VerificationResult;

use super::OutputFormatter;

pub(crate) struct PlainTextFormatter;

impl PlainTextFormatter {
    fn format_affected_packages(output: &mut String, result: &VerificationResult) {
        output.push_str("Changed packages:\n");
        for pkg in &result.affected_packages {
            let status = if result.covered_packages.contains(&pkg.name) {
                "✓"
            } else {
                "✗"
            };
            output.push_str(&format!("  {status} {}\n", pkg.name));
        }
    }

    fn format_file_list(output: &mut String, title: &str, files: &[PathBuf]) {
        if !files.is_empty() {
            output.push_str(&format!("\n{title}:\n"));
            for file in files {
                output.push_str(&format!("  {}\n", file.display()));
            }
        }
    }

    fn format_covered_packages(output: &mut String, result: &VerificationResult) {
        if !result.covered_packages.is_empty() {
            output.push_str("\nChangesets cover:\n");
            for name in &result.covered_packages {
                output.push_str(&format!("  {name}\n"));
            }
        }
    }

    fn format_common_sections(output: &mut String, result: &VerificationResult) {
        Self::format_affected_packages(output, result);
        Self::format_file_list(output, "Project-level files", &result.project_files);
        Self::format_file_list(output, "Ignored files", &result.ignored_files);
        Self::format_covered_packages(output, result);
    }
}

impl OutputFormatter for PlainTextFormatter {
    fn format_success(&self, result: &VerificationResult) -> String {
        let mut output = String::new();
        Self::format_common_sections(&mut output, result);
        output.push_str("\nAll changed packages have changeset coverage\n");
        output
    }

    fn format_failure(&self, result: &VerificationResult) -> String {
        let mut output = String::new();
        Self::format_common_sections(&mut output, result);

        if !result.uncovered_packages.is_empty() {
            output.push_str("Packages without changeset coverage:\n");
            for pkg in &result.uncovered_packages {
                output.push_str(&format!("  {}\n", pkg.name));
            }
        }

        output
    }
}
