pub(crate) trait CliWriter {
    fn blank(&self);
    fn message(&self, level: MessageLevel, text: &str);
    fn heading(&self, text: &str);
    fn line(&self, text: &str);
    fn indented(&self, text: &str);
    fn detail(&self, key: &str, value: &str);
    fn list_item(&self, text: &str);
    fn warn_stderr(&self, text: &str);
    fn raw(&self, text: &str);
    fn raw_stderr(&self, text: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageLevel {
    Info,
    Success,
    Hint,
}

pub(crate) struct StdoutCliWriter;

impl CliWriter for StdoutCliWriter {
    fn blank(&self) {
        println!();
    }

    fn message(&self, _level: MessageLevel, text: &str) {
        println!("{text}");
    }

    fn heading(&self, text: &str) {
        println!("{text}");
    }

    fn line(&self, text: &str) {
        println!("{text}");
    }

    fn indented(&self, text: &str) {
        println!("  {text}");
    }

    fn detail(&self, key: &str, value: &str) {
        println!("  {key} = {value}");
    }

    fn list_item(&self, text: &str) {
        println!("  - {text}");
    }

    fn warn_stderr(&self, text: &str) {
        eprintln!("{text}");
    }

    fn raw(&self, text: &str) {
        print!("{text}");
    }

    fn raw_stderr(&self, text: &str) {
        eprint!("{text}");
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::cell::RefCell;

    use super::{CliWriter, MessageLevel};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum OutputEntry {
        Blank,
        Message { level: MessageLevel, text: String },
        Heading(String),
        Line(String),
        Indented(String),
        Detail { key: String, value: String },
        ListItem(String),
        Raw(String),
    }

    pub(crate) struct BufferCliWriter {
        stdout: RefCell<Vec<OutputEntry>>,
        stderr: RefCell<Vec<String>>,
    }

    impl BufferCliWriter {
        pub(crate) fn new() -> Self {
            Self {
                stdout: RefCell::new(Vec::new()),
                stderr: RefCell::new(Vec::new()),
            }
        }

        pub(crate) fn stdout_entries(&self) -> Vec<OutputEntry> {
            self.stdout.borrow().clone()
        }

        pub(crate) fn stderr_entries(&self) -> Vec<String> {
            self.stderr.borrow().clone()
        }

        pub(crate) fn stdout_text(&self) -> String {
            let entries = self.stdout.borrow();
            let mut output = String::new();
            for entry in entries.iter() {
                match entry {
                    OutputEntry::Blank => output.push('\n'),
                    OutputEntry::Message { text, .. } => {
                        output.push_str(text);
                        output.push('\n');
                    }
                    OutputEntry::Heading(text) | OutputEntry::Line(text) => {
                        output.push_str(text);
                        output.push('\n');
                    }
                    OutputEntry::Indented(text) => {
                        output.push_str("  ");
                        output.push_str(text);
                        output.push('\n');
                    }
                    OutputEntry::Detail { key, value } => {
                        output.push_str("  ");
                        output.push_str(key);
                        output.push_str(" = ");
                        output.push_str(value);
                        output.push('\n');
                    }
                    OutputEntry::ListItem(text) => {
                        output.push_str("  - ");
                        output.push_str(text);
                        output.push('\n');
                    }
                    OutputEntry::Raw(text) => {
                        output.push_str(text);
                    }
                }
            }
            output
        }
    }

    impl CliWriter for BufferCliWriter {
        fn blank(&self) {
            self.stdout.borrow_mut().push(OutputEntry::Blank);
        }

        fn message(&self, level: MessageLevel, text: &str) {
            self.stdout.borrow_mut().push(OutputEntry::Message {
                level,
                text: text.to_string(),
            });
        }

        fn heading(&self, text: &str) {
            self.stdout
                .borrow_mut()
                .push(OutputEntry::Heading(text.to_string()));
        }

        fn line(&self, text: &str) {
            self.stdout
                .borrow_mut()
                .push(OutputEntry::Line(text.to_string()));
        }

        fn indented(&self, text: &str) {
            self.stdout
                .borrow_mut()
                .push(OutputEntry::Indented(text.to_string()));
        }

        fn detail(&self, key: &str, value: &str) {
            self.stdout.borrow_mut().push(OutputEntry::Detail {
                key: key.to_string(),
                value: value.to_string(),
            });
        }

        fn list_item(&self, text: &str) {
            self.stdout
                .borrow_mut()
                .push(OutputEntry::ListItem(text.to_string()));
        }

        fn warn_stderr(&self, text: &str) {
            self.stderr.borrow_mut().push(text.to_string());
        }

        fn raw(&self, text: &str) {
            self.stdout
                .borrow_mut()
                .push(OutputEntry::Raw(text.to_string()));
        }

        fn raw_stderr(&self, text: &str) {
            self.stderr.borrow_mut().push(text.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{BufferCliWriter, OutputEntry};
    use super::{CliWriter, MessageLevel};

    #[test]
    fn blank_records_blank_entry() {
        let writer = BufferCliWriter::new();
        writer.blank();

        assert_eq!(writer.stdout_entries(), vec![OutputEntry::Blank]);
    }

    #[test]
    fn message_records_level_and_text() {
        let writer = BufferCliWriter::new();
        writer.message(MessageLevel::Success, "done");

        assert_eq!(
            writer.stdout_entries(),
            vec![OutputEntry::Message {
                level: MessageLevel::Success,
                text: "done".to_string()
            }]
        );
    }

    #[test]
    fn heading_records_text() {
        let writer = BufferCliWriter::new();
        writer.heading("Title");

        assert_eq!(
            writer.stdout_entries(),
            vec![OutputEntry::Heading("Title".to_string())]
        );
    }

    #[test]
    fn line_records_text() {
        let writer = BufferCliWriter::new();
        writer.line("hello");

        assert_eq!(
            writer.stdout_entries(),
            vec![OutputEntry::Line("hello".to_string())]
        );
    }

    #[test]
    fn indented_records_text() {
        let writer = BufferCliWriter::new();
        writer.indented("item");

        assert_eq!(
            writer.stdout_entries(),
            vec![OutputEntry::Indented("item".to_string())]
        );
    }

    #[test]
    fn detail_records_key_value() {
        let writer = BufferCliWriter::new();
        writer.detail("commit", "true");

        assert_eq!(
            writer.stdout_entries(),
            vec![OutputEntry::Detail {
                key: "commit".to_string(),
                value: "true".to_string()
            }]
        );
    }

    #[test]
    fn list_item_records_text() {
        let writer = BufferCliWriter::new();
        writer.list_item("first");

        assert_eq!(
            writer.stdout_entries(),
            vec![OutputEntry::ListItem("first".to_string())]
        );
    }

    #[test]
    fn warn_stderr_records_to_stderr_buffer() {
        let writer = BufferCliWriter::new();
        writer.warn_stderr("warning!");

        assert!(writer.stdout_entries().is_empty());
        assert_eq!(writer.stderr_entries(), vec!["warning!".to_string()]);
    }

    #[test]
    fn raw_stderr_records_to_stderr_buffer() {
        let writer = BufferCliWriter::new();
        writer.raw_stderr("partial error");

        assert!(writer.stdout_entries().is_empty());
        assert_eq!(writer.stderr_entries(), vec!["partial error".to_string()]);
    }

    #[test]
    fn raw_records_text_without_newline() {
        let writer = BufferCliWriter::new();
        writer.raw("partial");

        assert_eq!(
            writer.stdout_entries(),
            vec![OutputEntry::Raw("partial".to_string())]
        );
    }

    #[test]
    fn stdout_text_renders_all_entry_types() {
        let writer = BufferCliWriter::new();
        writer.heading("Header");
        writer.blank();
        writer.line("a line");
        writer.indented("indented");
        writer.detail("key", "val");
        writer.list_item("item");
        writer.message(MessageLevel::Info, "info msg");
        writer.raw("raw");

        let text = writer.stdout_text();
        assert_eq!(
            text,
            "Header\n\na line\n  indented\n  key = val\n  - item\ninfo msg\nraw"
        );
    }
}
