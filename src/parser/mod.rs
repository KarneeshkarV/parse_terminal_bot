pub mod performer;
pub mod semantic;

use vte::Parser;
use performer::BotPerformer;
use crate::types::AnsiStyle;

/// Parse a raw terminal line (may contain ANSI escapes) into clean text + style.
pub fn parse_ansi(raw: &str) -> (String, AnsiStyle) {
    let mut parser   = Parser::new();
    let mut perf     = BotPerformer::new();

    for byte in raw.bytes() {
        parser.advance(&mut perf, byte);
    }
    // Flush any trailing content without a final newline
    if !perf.clean_buf.is_empty() {
        let line = std::mem::take(&mut perf.clean_buf);
        perf.lines.push((line, perf.style.clone()));
    }

    // Return the last line (single raw input line → single output line)
    perf.lines.pop().unwrap_or_default()
}
