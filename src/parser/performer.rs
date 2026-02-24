use crate::types::AnsiStyle;
use vte::Perform;

/// Implements vte::Perform, collects clean text + current ANSI style.
pub struct BotPerformer {
    pub clean_buf: String,
    pub style: AnsiStyle,
    pub lines: Vec<(String, AnsiStyle)>,
}

impl BotPerformer {
    pub fn new() -> Self {
        Self {
            clean_buf: String::new(),
            style: AnsiStyle::default(),
            lines: Vec::new(),
        }
    }

    fn flush_line(&mut self) {
        let line = std::mem::take(&mut self.clean_buf);
        self.lines.push((line, self.style.clone()));
    }
}

impl Perform for BotPerformer {
    fn print(&mut self, c: char) {
        self.clean_buf.push(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.flush_line(),
            b'\r' => { /* ignore CR */ }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        if action != 'm' {
            return; // only handle SGR
        }

        let nums: Vec<u16> = params.iter().flat_map(|sub| sub.iter().copied()).collect();

        if nums.is_empty() || nums == [0] {
            self.style = AnsiStyle::default();
            return;
        }

        let mut i = 0;
        while i < nums.len() {
            match nums[i] {
                0 => self.style = AnsiStyle::default(),
                1 => self.style.bold = true,
                2 => self.style.dim = true,
                3 => self.style.italic = true,
                4 => self.style.underline = true,
                5 => self.style.blink = true,
                22 => {
                    self.style.bold = false;
                    self.style.dim = false;
                }
                23 => self.style.italic = false,
                24 => self.style.underline = false,
                25 => self.style.blink = false,
                39 => self.style.fg = None,
                49 => self.style.bg = None,
                // Standard 16 foreground colors
                n @ 30..=37 => self.style.fg = Some(ansi_color_name(n - 30)),
                90..=97 => {} // bright colors — ignore for now
                // 256-color fg: ESC[38;5;Nm
                38 if nums.get(i + 1) == Some(&5) => {
                    if let Some(&n) = nums.get(i + 2) {
                        self.style.fg = Some(format!("ansi{n}"));
                        i += 2;
                    }
                }
                // 256-color bg: ESC[48;5;Nm
                48 if nums.get(i + 1) == Some(&5) => {
                    if let Some(&n) = nums.get(i + 2) {
                        self.style.bg = Some(format!("ansi{n}"));
                        i += 2;
                    }
                }
                // Standard background colors
                n @ 40..=47 => self.style.bg = Some(ansi_color_name(n - 40)),
                _ => {}
            }
            i += 1;
        }
    }

    // All other VTE callbacks are no-ops for content extraction
    fn hook(&mut self, _p: &vte::Params, _i: &[u8], _ig: bool, _a: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn esc_dispatch(&mut self, _i: &[u8], _ig: bool, _b: u8) {}
}

fn ansi_color_name(n: u16) -> String {
    match n {
        0 => "black",
        1 => "red",
        2 => "green",
        3 => "yellow",
        4 => "blue",
        5 => "magenta",
        6 => "cyan",
        7 => "white",
        _ => "white",
    }
    .to_string()
}
