use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, SetAttribute, Attribute},
    terminal::{self, ClearType, disable_raw_mode, enable_raw_mode, size},
    cursor::{self, MoveTo},
};
use std::io::{self, Write, stdout};
use std::io::Result;
use std::env;
use std::fs;


#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
}

struct Editor {
    mode: Mode,
    cursor_pos: (u16, u16),
    offset: (u16, u16),
    buffer: Vec<Vec<char>>,
    theme: Theme,
    show_fringe: bool,
    show_line_numbers: bool,
}

impl Editor {
    fn new() -> Editor {
        Editor { 
            mode: Mode::Normal, 
            cursor_pos: (0, 0), 
            offset: (0, 0),
            buffer: vec![vec![]], 
            theme: Theme::new(),
            show_fringe: true,
            show_line_numbers: true,
        }
    }

    fn draw(&self, stdout: &mut io::Stdout) -> Result<()> {
        let (width, height) = size()?;

        // Apply the theme's overall background color first
        let background_color = hex_to_rgb(&self.theme.background_color).unwrap_or(Color::Rgb { r: 33, g: 33, b: 33 });
        execute!(
            stdout,
            SetBackgroundColor(background_color),
            terminal::Clear(ClearType::All)
        )?;

        let mut start_col = 0;

        // Draw the fringe if enabled
        if self.show_fringe {
            self.draw_fringe(stdout, height)?;
            start_col += 2; // Fringe takes 1 column
        }

        // Draw the line numbers if enabled
        if self.show_line_numbers {
            self.draw_line_numbers(stdout, height, start_col)?;
            start_col += 4; // Assuming 3 characters for line numbers + 1 space padding
        }

        // Set the text color for the main text area
        let text_color = hex_to_rgb(&self.theme.text_color).unwrap_or(Color::Rgb { r: 153, g: 149, b: 191 });
        execute!(
            stdout,
            SetForegroundColor(text_color)
        )?;

        // Drawing text area with adjusted starting column
        for (idx, line) in self.buffer.iter().enumerate() {
            if idx >= self.offset.1 as usize && idx < (self.offset.1 + height - 2) as usize {
                let line_content: String = line.iter().collect();
                execute!(
                    stdout,
                    MoveTo(start_col, (idx - self.offset.1 as usize) as u16),
                    Print(line_content)
                )?;
            }
        }

        // Draw the modeline and minibuffer
        self.draw_modeline(stdout, width, height)?;
        self.draw_minibuffer(stdout, width, height)?;

        // Reset colors to default after drawing text
        execute!(stdout, ResetColor)?;

        // Correcting cursor position and ensuring it's visible
        let cursor_pos_within_text_area = (
            self.cursor_pos.0.saturating_sub(self.offset.0) + start_col, // Adjust cursor X position by start_col
            self.cursor_pos.1.saturating_sub(self.offset.1)
        );

        if cursor_pos_within_text_area.1 < height - 2 {
            execute!(
                stdout,
                cursor::MoveTo(cursor_pos_within_text_area.0, cursor_pos_within_text_area.1),
                cursor::Show
            )?;
        }

        io::stdout().flush()?;
        Ok(())
    }
    // TODO ~ after the last line 3 options only one, none or untile the end
    fn draw_line_numbers(&self, stdout: &mut io::Stdout, height: u16, start_col: u16) -> Result<()> {
        if self.show_line_numbers {
            for y in 0..height - 2 { // Excluding modeline and minibuffer
                let line_index = (self.offset.1 as usize) + y as usize; // Calculate line index considering offset
                if line_index < self.buffer.len() { // Check if line exists
                    let absolute_line_number = line_index + 1; // Absolute line number
                    
                    // Determine the color for the line number
                    let line_number_color = if self.mode == Mode::Normal && line_index == self.cursor_pos.1 as usize {
                        hex_to_rgb(&self.theme.current_line_number_color).unwrap_or(Color::Grey)
                    } else if self.mode == Mode::Insert && line_index == self.cursor_pos.1 as usize {
                        hex_to_rgb(&self.theme.insert_cursor_color).unwrap_or(Color::Grey)
                    } else {
                        hex_to_rgb(&self.theme.line_numbers_color).unwrap_or(Color::Grey)
                    };

                    execute!(
                        stdout,
                        MoveTo(start_col, y),
                        SetForegroundColor(line_number_color),
                        Print(format!("{:>3} ", absolute_line_number)) // Right-align with 3 spaces and add 1 space padding
                    )?;
                }
            }
        }
        Ok(())
    }


    
    fn draw_fringe(&self, stdout: &mut io::Stdout, height: u16) -> Result<()> {
        if self.show_fringe {
            let fringe_color = hex_to_rgb(&self.theme.fringe_color).unwrap_or(Color::Grey);
            for y in 0..height - 2 { // Exclude modeline and minibuffer
                execute!(
                    stdout,
                    MoveTo(0, y),
                    SetForegroundColor(fringe_color),
                    Print("||") // Wider fringe
                )?;
            }
        }
        Ok(())
    }


    fn draw_modeline(&self, stdout: &mut io::Stdout, width: u16, height: u16) -> Result<()> {
        let sep = ""; // Separator symbol
        let file = "main.rs"; // Hardcoded file name for demonstration

        // Determine mode string, mode background color, and text color
        let (mode_str, mode_bg_color, mode_text_color) = match self.mode {
            Mode::Normal => (
                "NORMAL", 
                self.theme.normal_cursor_color.clone(), 
                Color::Black,
            ),
            Mode::Insert => (
                "INSERT", 
                self.theme.insert_cursor_color.clone(), 
                Color::Black,
            ),
        };

        let mode_bg_color = hex_to_rgb(&mode_bg_color).unwrap();
        let file_bg_color = hex_to_rgb(&self.theme.modeline_lighter_color).unwrap(); // Background color for the file section
        let file_text_color = hex_to_rgb(&self.theme.text_color).unwrap(); // Text color for the file name
        let modeline_bg_color = hex_to_rgb(&self.theme.modeline_color).unwrap(); // Background color for the modeline

        // Mode section
        execute!(stdout, SetBackgroundColor(mode_bg_color), MoveTo(0, height - 2), Print(" "))?;
        execute!(stdout, SetForegroundColor(mode_text_color), SetAttribute(Attribute::Bold), Print(format!(" {} ", mode_str.to_uppercase())), SetAttribute(Attribute::Reset))?;

        // First separator - now using the background color of line numbers
        execute!(stdout, SetBackgroundColor(file_bg_color), SetForegroundColor(mode_bg_color), Print(sep))?;

        // File name section
        execute!(stdout, SetBackgroundColor(file_bg_color), Print(" "))?;
        execute!(stdout, SetForegroundColor(file_text_color), Print(format!(" {} ", file)))?;

        // Second separator - now using the background color of the modeline
        execute!(stdout, SetBackgroundColor(modeline_bg_color), SetForegroundColor(file_bg_color), Print(sep))?;

        // Fill the rest of the modeline with the modeline background color
        let fill_length = width.saturating_sub(4 + mode_str.len() as u16 + file.len() as u16 + 4);
        execute!(stdout, SetBackgroundColor(modeline_bg_color), Print(" ".repeat(fill_length as usize)))?;

        // Reset styles to default
        execute!(stdout, ResetColor)?;
        Ok(())
    }

 

    fn draw_minibuffer(&self, stdout: &mut io::Stdout, width: u16, height: u16) -> Result<()> {
        let minibuffer_bg = hex_to_rgb(&self.theme.minibuffer_color).unwrap_or(Color::Grey);
        execute!(
            stdout,
            SetBackgroundColor(minibuffer_bg),
            SetForegroundColor(Color::Yellow),
            MoveTo(0, height - 1),
            Print(" ".repeat(width as usize)), // Fill minibuffer background
            MoveTo(0, height - 1),
            Print("Minibuffer content here") // Placeholder for actual content
        )?;
        Ok(())
    }

    fn open(&mut self, path: &str) -> Result<()> {
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|_| "".to_string());

        self.buffer = contents.lines()
            .map(|line| line.chars().collect())
            .collect();

        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let mut stdout = stdout();
        enable_raw_mode()?;
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

        loop {
            self.theme.apply_cursor_color(self.cursor_pos, &self.buffer, &self.mode);
            self.draw(&mut stdout)?;

            if let Event::Key(key) = event::read()? {
                match self.mode {
                    Mode::Normal => self.handle_normal_mode(key)?,
                    Mode::Insert => self.handle_insert_mode(key)?,
                }
            }
        }
    }

    fn set_cursor_shape(&self) {
        let shape_code = match self.mode {
            Mode::Normal => "\x1b[2 q", // Block
            Mode::Insert => "\x1b[6 q", // Line
        };
        print!("{}", shape_code);
        io::stdout().flush().unwrap();
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                self.set_cursor_shape();
            },

            KeyCode::Char('j') => {
                if self.cursor_pos.1 < self.buffer.len() as u16 - 1 {
                    self.cursor_pos.1 += 1;
                    // Scroll down
                    let (_, height) = size()?;
                    // Adjust to consider the lines reserved for the minibuffer and modeline
                    let text_area_height = height - 2; 
                    if self.cursor_pos.1 >= self.offset.1 + text_area_height {
                        self.offset.1 += 1;
                    }
                }
            },
            KeyCode::Char('k') => {
                if self.cursor_pos.1 > 0 {
                    self.cursor_pos.1 -= 1;
                    // Scroll up
                    if self.cursor_pos.1 < self.offset.1 {
                        self.offset.1 = self.offset.1.saturating_sub(1);
                    }
                }
            },
            KeyCode::Char('h') => if self.cursor_pos.0 > 0 { self.cursor_pos.0 -= 1 },
            KeyCode::Char('l') => if self.cursor_pos.0 < self.buffer[self.cursor_pos.1 as usize].len() as u16 { self.cursor_pos.0 += 1 },
            KeyCode::Char('q') => {
                disable_raw_mode()?;
                execute!(stdout(), terminal::LeaveAlternateScreen, cursor::Show)?;
                std::process::exit(0);
            },
            _ => {}
        }
        Ok(())
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.set_cursor_shape();
            },
            KeyCode::Char(c) => {
                self.buffer[self.cursor_pos.1 as usize].insert(self.cursor_pos.0 as usize, c);
                self.cursor_pos.0 += 1;
            },
            KeyCode::Backspace => {
                if self.cursor_pos.0 > 0 {
                    self.buffer[self.cursor_pos.1 as usize].remove((self.cursor_pos.0 - 1) as usize);
                    self.cursor_pos.0 -= 1;
                } else if self.cursor_pos.1 > 0 {
                    // Handle removing an entire line and moving up
                    let current_line = self.buffer.remove(self.cursor_pos.1 as usize);
                    self.cursor_pos.1 -= 1;
                    self.cursor_pos.0 = self.buffer[self.cursor_pos.1 as usize].len() as u16;
                    self.buffer[self.cursor_pos.1 as usize].extend(current_line);
                }
            },
            KeyCode::Enter => {
                let tail = self.buffer[self.cursor_pos.1 as usize].split_off(self.cursor_pos.0 as usize);
                self.buffer.insert(self.cursor_pos.1 as usize + 1, tail);
                self.cursor_pos.1 += 1;
                self.cursor_pos.0 = 0;
            },
            _ => {}
        }
        Ok(())
    }
}


struct Theme {
    normal_cursor_color: String,
    insert_cursor_color: String,
    background_color: String,
    modeline_color: String,
    modeline_lighter_color: String,
    minibuffer_color: String,
    fringe_color: String,
    line_numbers_color: String,
    current_line_number_color: String,
    text_color: String,
}

impl Theme {
    fn new() -> Self {
        Theme {
            normal_cursor_color: "#658B5F".into(),
            insert_cursor_color: "#514B8E".into(),
            background_color: "#090909".into(),
            fringe_color: "#090909".into(),
            modeline_color: "#060606".into(),
            line_numbers_color: "#171717".into(),
            modeline_lighter_color: "#171717".into(),
            minibuffer_color: "#070707".into(),
            text_color: "#9995BF".into(),
            current_line_number_color: "#C0ACD1".into(),
        }
    }


    fn apply_cursor_color(&self, cursor_pos: (u16, u16), buffer: &Vec<Vec<char>>, mode: &Mode) {
        // Determine if the cursor is over a non-space character in Normal mode
        let is_over_text = if let Mode::Normal = mode {
            buffer.get(cursor_pos.1 as usize)
                .and_then(|line| line.get(cursor_pos.0 as usize))
                .map(|&c| c != ' ') // Check if the character is not a space
                .unwrap_or(false)
        } else {
            false
        };

        // Choose the color based on whether the cursor is over text
        let color = if is_over_text {
            // Use the text cursor color if over text
            "\x1b[37m" // ANSI escape code for white foreground
        } else {
            // Use the mode-specific cursor color otherwise
            match mode {
                Mode::Normal => &self.normal_cursor_color,
                Mode::Insert => &self.insert_cursor_color,
            }
        };

        // Print the ANSI escape code to set the cursor color
        print!("\x1b]12;{}\x1b\\", color);
        io::stdout().flush().unwrap();
    }
}


fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut editor = Editor::new();

    // Check if a file path is provided as an argument
    if args.len() > 1 {
        let file_path = &args[1];
        editor.open(file_path)?;
    }

    editor.run()
}


fn hex_to_rgb(hex: &str) -> std::result::Result<Color, &'static str> {
    if hex.starts_with('#') && hex.len() == 7 {
        let r = u8::from_str_radix(&hex[1..3], 16).map_err(|_| "Invalid hex format")?;
        let g = u8::from_str_radix(&hex[3..5], 16).map_err(|_| "Invalid hex format")?;
        let b = u8::from_str_radix(&hex[5..7], 16).map_err(|_| "Invalid hex format")?;
        Ok(Color::Rgb { r, g, b })
    } else {
        Err("Invalid hex format")
    }
}

