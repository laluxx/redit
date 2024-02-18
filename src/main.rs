use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, SetAttribute, Attribute},
    terminal::{self, ClearType, disable_raw_mode, enable_raw_mode, size},
    cursor::{self, MoveTo},
};

use std::io::{self, Stdout, Write, stdout};
use std::io::Result;
use std::env;
use std::fs;


#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
    Dired,
}

use std::fs::DirEntry;
use std::path::PathBuf;

use chrono::{DateTime, Local};


pub struct Dired {
    current_path: PathBuf,
    entries: Vec<DirEntry>,
    cursor_pos: u16,
    entry_first_char_column: u16,
}

impl Dired {

    fn new(current_path: PathBuf) -> io::Result<Self> {
        let entries = Dired::list_directory_contents(&current_path)?;
        let cursor_pos = if entries.len() > 2 { 2 } else { 0 }; // Skip '.' and '..'
        Ok(Dired {
            current_path,
            entries,
            cursor_pos,
            entry_first_char_column: 0,
        })
    }


    // Lists the contents of the specified directory
    fn list_directory_contents(path: &PathBuf) -> io::Result<Vec<DirEntry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            entries.push(entry);
        }
        Ok(entries)
    }
    
    pub fn draw_dired(&mut self, stdout: &mut Stdout, height: u16) -> io::Result<()> {
        let display_path = self.current_path.display().to_string();
        let trimmed_path = display_path.trim_end_matches('/');
        execute!(
            stdout,
            MoveTo(6, 0),
            Print(format!("{}:", trimmed_path))
        )?;

        let mut line_number = 2u16;

        let mut entries: Vec<String> = vec![".".into(), "..".into()];
        entries.extend(self.entries.iter().map(|e| e.file_name().to_str().unwrap_or("").to_string()));

        // Determine the maximum length of file size
        let max_size_length = entries.iter()
            .map(|entry_name| {
                let path = self.current_path.join(entry_name);
                fs::metadata(&path).map(|m| m.len().to_string().len()).unwrap_or(0)
            })
            .max()
            .unwrap_or(0);

        self.entry_first_char_column = 37 + max_size_length as u16;

        for entry_name in &entries {
            if line_number >= height - 1 { break; }

            let path = self.current_path.join(entry_name);
            let metadata = fs::metadata(&path)?;
            let file_type = if metadata.is_dir() { "d" } else { "-" };
            let permissions = "rwxr-xr-x";
            let size = metadata.len();
            let modified: DateTime<Local> = DateTime::from(metadata.modified()?);
            let owner = "l l";

            // Use the max size length to adjust padding dynamically
            let size_str = format!("{:1$}", size, max_size_length);
            let file_info = format!("{:3}{}{} {:<3} {} {:14} {}", "", file_type, permissions, owner, size_str, modified.format("%b %d %H:%M"), entry_name);

            execute!(
                stdout,
                MoveTo(3, line_number),
                Print(file_info)
            )?;

            line_number += 1;
        }

        Ok(())
    }
}

struct Editor {
    mode: Mode,
    dired: Option<Dired>,
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
            dired: None,
        }
    }

    fn draw(&mut self, stdout: &mut Stdout) -> Result<()> {
        let (width, height) = terminal::size()?;
        let background_color = hex_to_rgb(&self.theme.background_color).unwrap();
        execute!(
            stdout,
            SetBackgroundColor(background_color),
            terminal::Clear(ClearType::All)
        )?;

        // In Dired mode, only draw the directory listing
        if self.mode == Mode::Dired {

            self.draw_modeline(stdout, width, height)?;
            self.draw_minibuffer(stdout, width, height)?;
            
            execute!(stdout, ResetColor)?;

            execute!(
                stdout,
                SetBackgroundColor(background_color),
                // terminal::Clear(ClearType::All)
            )?;
            
            if let Some(ref mut dired) = &mut self.dired {
                dired.draw_dired(stdout, height)?;

                let cursor_line = dired.cursor_pos + 2; // Skip '.' and '..'
                execute!(
                    stdout,
                    cursor::MoveTo(dired.entry_first_char_column, cursor_line), // Move cursor to the first character of the entry
                    // cursor::Show
                )?;

            }

        } else {
            let mut start_col = 0;

            // Fringe
            if self.show_fringe {
                self.draw_fringe(stdout, height)?;
                start_col += 2; // Fringe takes 1 column
            }

            // Line numbers
            if self.show_line_numbers {
                self.draw_line_numbers(stdout, height, start_col)?;
                start_col += 4; // Assuming 3 characters for line numbers + 1 space padding
            }

            self.draw_text(stdout)?;
            self.draw_modeline(stdout, width, height)?;
            self.draw_minibuffer(stdout, width, height)?;
            execute!(stdout, ResetColor)?;

            // Cursor
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
        }

        io::stdout().flush()?;
        Ok(())
    }

    fn draw_text(&self, stdout: &mut io::Stdout) -> Result<()> {
        let (width, height) = size()?; // TODO horizontal scrolling
        let text_color = hex_to_rgb(&self.theme.text_color).unwrap();
        let mut start_col = 0;

        if self.show_fringe {
            start_col += 2;
        }

        if self.show_line_numbers {
            start_col += 4;
        }

        execute!(
            stdout,
            SetForegroundColor(text_color)
        )?;

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

        Ok(())
    }


    // TODO ~ after the last line 3 options only one, none or untile the end
    fn draw_line_numbers(&self, stdout: &mut io::Stdout, height: u16, start_col: u16) -> Result<()> {
        if self.show_line_numbers {
            for y in 0..height - 2 { // Excluding modeline and minibuffer
                let line_index = (self.offset.1 as usize) + y as usize; // Calculate line index considering offset
                if line_index < self.buffer.len() { // Check if line exists
                    let absolute_line_number = line_index + 1;
                    
                    // Determine the color for the line number
                    let line_number_color = if self.mode == Mode::Normal && line_index == self.cursor_pos.1 as usize {
                        hex_to_rgb(&self.theme.current_line_number_color).unwrap()
                    } else if self.mode == Mode::Insert && line_index == self.cursor_pos.1 as usize {
                        hex_to_rgb(&self.theme.insert_cursor_color).unwrap()
                    } else {
                        hex_to_rgb(&self.theme.line_numbers_color).unwrap()
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
        let sep_r = "";
        let sep_l = "";
        let file = "main.rs"; // TODO hardcoded

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
            Mode::Dired => (
                "DIRED", 
                self.theme.dired_mode_color.clone(), 
                Color::Black,
            ),
        };

        let mode_bg_color = hex_to_rgb(&mode_bg_color).unwrap();
        let file_bg_color = hex_to_rgb(&self.theme.modeline_lighter_color).unwrap();
        let file_text_color = hex_to_rgb(&self.theme.text_color).unwrap();
        let modeline_bg_color = hex_to_rgb(&self.theme.modeline_color).unwrap();

        // Mode section
        execute!(stdout, SetBackgroundColor(mode_bg_color), MoveTo(0, height - 2), Print(" "))?;
        execute!(stdout, SetForegroundColor(mode_text_color), SetAttribute(Attribute::Bold), Print(format!(" {} ", mode_str.to_uppercase())), SetAttribute(Attribute::Reset))?;

        // First separator
        execute!(stdout, SetBackgroundColor(file_bg_color), SetForegroundColor(mode_bg_color), Print(sep_r))?;

        // File name section
        execute!(stdout, SetBackgroundColor(file_bg_color), Print(" "))?;
        execute!(stdout, SetForegroundColor(file_text_color), Print(format!(" {} ", file)))?;

        // Second separator
        execute!(stdout, SetBackgroundColor(modeline_bg_color), SetForegroundColor(file_bg_color), Print(sep_r))?;

        // Determine the position string, adjusting for 0-based index
        let pos_str = format!("{}:{}", self.cursor_pos.1 + 1, self.cursor_pos.0 + 1);

        // Calculate remaining space after drawing the existing sections
        let pos_str_length = pos_str.len() as u16 + 2;
        let fill_length_before_pos_str = width - (4 + mode_str.len() as u16 + file.len() as u16 + pos_str_length + 3);

        // Fill the space between file section and position section with the modeline background color
        execute!(stdout, SetBackgroundColor(modeline_bg_color), Print(" ".repeat(fill_length_before_pos_str as usize)))?;

        // Separator before position section, with modeline color as text color and normal cursor color as background
        let normal_cursor_color = hex_to_rgb(&self.theme.normal_cursor_color).unwrap();
        execute!(stdout, SetBackgroundColor(modeline_bg_color), SetForegroundColor(normal_cursor_color), Print(sep_l))?;

        // Adding a small padding from the right of the screen
        let right_padding = 11; // Change this value to add more padding if needed
        let padding_spaces = " ".repeat(right_padding as usize);

        // Position section with normal cursor color as background and black text
        execute!(stdout, SetBackgroundColor(normal_cursor_color), SetForegroundColor(Color::Black), Print(format!("{}{} ", pos_str, padding_spaces)))?;

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
                    Mode::Dired => self.handle_dired_mode(key)?,
                }
            }
        }
    }

    fn set_cursor_shape(&self) {
        let shape_code = match self.mode {
            Mode::Normal => "\x1b[2 q", // Block
            Mode::Dired => "\x1b[2 q", // Block
            Mode::Insert => "\x1b[6 q", // Line
        };
        print!("{}", shape_code);
        io::stdout().flush().unwrap();
    }

    fn handle_dired_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') => {
                if let Some(dired) = &mut self.dired {
                    let max_index = dired.entries.len() as u16 + 1;
                    if dired.cursor_pos < max_index {
                        dired.cursor_pos += 1;
                    }
                }
            },
            KeyCode::Char('k') => {
                if let Some(dired) = &mut self.dired {
                    if dired.cursor_pos > 0 {
                        dired.cursor_pos -= 1;
                    }
                }
            },
            KeyCode::Char('q') => {
                self.mode = Mode::Normal;
            },

            _ => {}
        }
        Ok(())
    }
    
    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                self.set_cursor_shape();
            },
            KeyCode::Char('d') => {
                if let Some(path) = env::current_dir().ok() {
                    self.dired = Some(Dired::new(path)?);
                    self.mode = Mode::Dired;
                }
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
    dired_mode_color: String,
}

impl Theme {
    fn new() -> Self {
        Theme {
            normal_cursor_color: "#658B5F".into(),
            insert_cursor_color: "#514B8E".into(),
            dired_mode_color: "#565663".into(),
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
            &self.text_color
            // "\x1b[37m" // ANSI escape code for white foreground
                
                
        } else {
            // Use the mode-specific cursor color otherwise
            match mode {
                Mode::Normal => &self.normal_cursor_color,
                Mode::Insert => &self.insert_cursor_color,
                Mode::Dired => &self.normal_cursor_color,
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

