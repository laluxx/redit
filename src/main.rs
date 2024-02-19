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
use std::path::Path;

use std::fs::DirEntry;
use std::path::PathBuf;
use chrono::{DateTime, Local};


#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
    Dired,
}

struct Dired {
    current_path: PathBuf,
    entries: Vec<DirEntry>,
    cursor_pos: u16,
    entry_first_char_column: u16,
    color_dired: bool,
}

impl Dired {

    fn new(current_path: PathBuf) -> io::Result<Self> {
        let entries = Dired::list_directory_contents(&current_path)?;
        let cursor_pos = if entries.len() == 0 { 0 } else { 2 }; // Skip '.' and '..'
        Ok(Dired {
            current_path,
            entries,
            cursor_pos,
            entry_first_char_column: 0,
            color_dired: true,
        })
    }

    fn list_directory_contents(path: &PathBuf) -> io::Result<Vec<DirEntry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            entries.push(entry);
        }
        Ok(entries)
    }

    fn refresh_directory_contents(&mut self) -> io::Result<()> {
        self.entries = Dired::list_directory_contents(&self.current_path)?;
        Ok(())
    }


    // TODO color file extentions if color_dired is true, fix background
    pub fn draw_dired(&mut self, stdout: &mut Stdout, height: u16, theme: &Theme) -> io::Result<()> {
        // Display the path at the top with `dired_path_color`
        let display_path = self.current_path.display().to_string();
        let trimmed_path = display_path.trim_end_matches('/');
        execute!(
            stdout,
            MoveTo(3, 0),
            SetForegroundColor(theme.dired_path_color),
            Print(format!("{}:", trimmed_path)),
            ResetColor
        )?;

        let mut line_number = 2u16;

        let entries = vec![".".into(), "..".into()]
            .into_iter()
            .chain(self.entries.iter().map(|e| e.file_name().to_str().unwrap_or("").to_string()))
            .collect::<Vec<String>>();

        // Determine the maximum length of file size
        let max_size_length = entries.iter()
            .map(|entry_name| {
                let path = self.current_path.join(entry_name);
                fs::metadata(&path).map(|m| m.len().to_string().len()).unwrap_or(0)
            })
            .max()
            .unwrap_or(0);

        self.entry_first_char_column = 35 + max_size_length as u16;

        for entry_name in &entries {
            if line_number >= height - 1 { break; }

            let path = self.current_path.join(entry_name);
            let metadata = fs::metadata(&path)?;
            let is_dir = metadata.is_dir();
            let file_type_char = if is_dir { "d" } else { "-" };
            let permissions = "rwxr-xr-x";
            let size = metadata.len();
            let modified: DateTime<Local> = DateTime::from(metadata.modified()?);
            let owner = "l l";
            let size_str = format!("{:1$}", size, max_size_length);

            let entry_color = if entry_name == "." || entry_name == ".." || is_dir {
                if self.color_dired {
                    theme.normal_cursor_color
                } else {
                    theme.dired_dir_color
                }
            } else {
                theme.text_color
            };

            execute!(stdout, MoveTo(3, line_number))?;

            if self.color_dired {
                execute!(
                    stdout,
                    SetForegroundColor(entry_color),
                    Print(file_type_char),
                    ResetColor
                )?;
            } else {
                execute!(
                    stdout,
                    SetForegroundColor(theme.text_color),
                    Print(file_type_char),
                    ResetColor
                )?;
            }

            if self.color_dired {
                for ch in permissions.chars() {
                    let color = match ch {
                        'r' => theme.warning_color,
                        'w' => theme.error_color,
                        'x' => theme.ok_color,
                        '-' => theme.comment_color,
                        _ => theme.text_color, // Default color
                    };
                    execute!(stdout, SetForegroundColor(color), Print(ch))?;
                }
            } else {
                execute!(
                    stdout,
                    SetForegroundColor(theme.text_color),
                    Print(permissions),
                    ResetColor
                )?;
            }

            execute!(
                stdout,
                ResetColor,
                Print(" "),
                SetForegroundColor(theme.text_color), Print(format!("{:<3} ", owner)),
                SetForegroundColor(if self.color_dired { theme.dired_size_color } else { theme.text_color }), Print(format!("{} ", size_str)),
                SetForegroundColor(if self.color_dired { theme.dired_timestamp_color } else { theme.text_color }), Print(format!("{:14} ", modified.format("%b %d %H:%M"))),
                SetForegroundColor(entry_color), Print(format!(" {}", entry_name)),
                ResetColor
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
    insert_line_cursor: bool,
    minibuffer_active: bool,
    minibuffer_content: String,
    minibuffer_prefix: String,
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
            insert_line_cursor: false,
            dired: None,
            minibuffer_active: false,
            minibuffer_content: String::new(),
            minibuffer_prefix: String::new(),
        }
    }

    fn draw(&mut self, stdout: &mut Stdout) -> Result<()> {
        let (width, height) = terminal::size()?;
        let background_color = self.theme.background_color;

        execute!(
            stdout,
            SetBackgroundColor(background_color),
            terminal::Clear(ClearType::All)
        )?;

        // Always draw modeline and minibuffer
        self.draw_modeline(stdout, width, height)?;
        self.draw_minibuffer(stdout, width, height)?;

        if self.mode == Mode::Dired {
            if let Some(ref mut dired) = &mut self.dired {
                dired.draw_dired(stdout, height, &self.theme)?;
            }
        }

        // Reset the background color for fringe and line numbers
        execute!(stdout, SetBackgroundColor(background_color))?;

        // Draw text area for non-Dired modes
        if self.mode != Mode::Dired {
            let mut start_col = 0;
            if self.show_fringe {
                // Ensure the background color is correct for the fringe
                self.draw_fringe(stdout, height)?;
                start_col += 2;
            }
            if self.show_line_numbers {
                // Ensure the background color is correct for the line numbers
                self.draw_line_numbers(stdout, height, start_col)?;
            }
            self.draw_text(stdout)?;
        }

        // Calculate and set the cursor position
        let cursor_pos = if self.minibuffer_active {
            let minibuffer_cursor_pos_x = 2 + self.minibuffer_prefix.len() as u16 + self.minibuffer_content.len() as u16;
            (minibuffer_cursor_pos_x, height - 1) // Adjust for minibuffer's position at the bottom
        } else if self.mode == Mode::Dired {
            self.dired.as_ref().map_or((0, 0), |dired| {
                let cursor_line = dired.cursor_pos + 2; // Skip '.' and '..'
                (dired.entry_first_char_column, cursor_line)
            })
        } else {
            let mut start_col = 0;
            if self.show_fringe {
                start_col += 2; // Account for fringe column
            }
            if self.show_line_numbers {
                start_col += 4; // Account for line number columns
            }
            let cursor_x = self.cursor_pos.0.saturating_sub(self.offset.0) + start_col;
            let cursor_y = self.cursor_pos.1.saturating_sub(self.offset.1);
            (cursor_x, cursor_y)
        };

        // Position the cursor
        execute!(
            stdout,
            cursor::MoveTo(cursor_pos.0, cursor_pos.1),
            cursor::Show
        )?;

        io::stdout().flush()?;
        Ok(())
    }
    
    fn draw_text(&self, stdout: &mut io::Stdout) -> Result<()> {
        let (width, height) = size()?; // TODO horizontal scrolling
        let text_color = self.theme.text_color;
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
                        self.theme.current_line_number_color
                    } else if self.mode == Mode::Insert && line_index == self.cursor_pos.1 as usize {
                        self.theme.insert_cursor_color
                    } else {
                        self.theme.line_numbers_color
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
            let fringe_color = self.theme.fringe_color;
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

        let (mode_str, mode_bg_color, mode_text_color) = match self.mode {
            Mode::Normal => (
                "NORMAL", 
                self.theme.normal_cursor_color, 
                Color::Black,
            ),
            Mode::Insert => (
                "INSERT", 
                self.theme.insert_cursor_color, 
                Color::Black,
            ),
            Mode::Dired => (
                "DIRED", 
                self.theme.dired_mode_color, 
                Color::Black,
            ),
        };

        let mode_bg_color = mode_bg_color;
        let file_bg_color = self.theme.modeline_lighter_color;
        let file_text_color = self.theme.text_color;
        let modeline_bg_color = self.theme.modeline_color;

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
        let normal_cursor_color = self.theme.normal_cursor_color;
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
        let minibuffer_bg = self.theme.minibuffer_color;
        let content_fg = self.theme.text_color;
        let prefix_fg = self.theme.normal_cursor_color;
        execute!(
            stdout,
            SetBackgroundColor(minibuffer_bg),
            SetForegroundColor(prefix_fg),
            MoveTo(0, height - 1),
            Print(" ".repeat(width as usize)), // Fill minibuffer background
            MoveTo(0, height - 1),
            Print(format!(" {}", self.minibuffer_prefix)),
            SetForegroundColor(content_fg),
            Print(format!(" {}", self.minibuffer_content))
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
                if self.minibuffer_active {
                    match key.code {
                        KeyCode::Char(c) => {
                            self.minibuffer_content.push(c);
                        },
                        KeyCode::Backspace => {
                            self.minibuffer_content.pop();
                        },
                        KeyCode::Enter => {
                            if self.mode == Mode::Dired {
                                let file_path = self.dired.as_ref().unwrap().current_path.join(&self.minibuffer_content);
                                // std::fs::File::create(file_path)?;

                                if std::fs::File::create(file_path).is_ok() {
                                    // If file creation was successful, refresh the directory listing
                                    if let Some(dired) = &mut self.dired {
                                        dired.refresh_directory_contents()?;
                                    }
                                }

                                self.minibuffer_active = false;
                                self.minibuffer_prefix.clear();
                                self.minibuffer_content.clear();
                            }
                        },
                        _ => {}
                    }
                } else {
                    match self.mode {
                        Mode::Normal => self.handle_normal_mode(key)?,
                        Mode::Insert => self.handle_insert_mode(key)?,
                        Mode::Dired => self.handle_dired_mode(key)?,
                    }
                }
            }
        }
    }

    fn set_cursor_shape(&self) {
        let shape_code = match self.mode {
            Mode::Normal => "\x1b[2 q", // Block
            Mode::Dired => "\x1b[2 q", // Block
            Mode::Insert => {
                if self.insert_line_cursor {
                    "\x1b[6 q" // Line
                } else {
                    "\x1b[2 q" // Block
                }
            }
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
            KeyCode::Char('h') => {
                // Navigate to the parent directory
                if let Some(dired) = &mut self.dired {
                    let parent_path = dired.current_path.parent().unwrap_or_else(|| Path::new("/")).to_path_buf();
                    *dired = Dired::new(parent_path)?;
                }
            },
            KeyCode::Char('l') => {
                if let Some(dired) = &mut self.dired {
                    if dired.cursor_pos == 0 {
                        // Do nothing for '.'
                    } else if dired.cursor_pos == 1 {
                        // Handle '..' the same as 'h', navigate to the parent directory
                        let parent_path = dired.current_path.parent().unwrap_or_else(|| Path::new("/")).to_path_buf();
                        *dired = Dired::new(parent_path)?;
                    } else {
                        let selected_entry = &dired.entries[dired.cursor_pos as usize - 2]; // Adjusting for '.' and '..'
                        let path = selected_entry.path();
                        if path.is_dir() {
                            *dired = Dired::new(path.to_path_buf())?;
                        } else if path.is_file() {
                            self.open(&path.to_string_lossy())?;
                            self.mode = Mode::Normal; // Or a different mode meant for editing/viewing files
                        }
                    }
                }
            },

            // TODO ('T') should touch and open the file
            KeyCode::Char('t') => {
                self.minibuffer_active = true;
                self.minibuffer_prefix = "Touch:".to_string();
                self.minibuffer_content = "".to_string();
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
    background_color: Color,
    text_color: Color,
    normal_cursor_color: Color,
    insert_cursor_color: Color,
    fringe_color: Color,
    line_numbers_color: Color,
    current_line_number_color: Color,
    modeline_color: Color,
    modeline_lighter_color: Color,
    minibuffer_color: Color,
    dired_mode_color: Color,
    dired_timestamp_color: Color,
    dired_path_color: Color,
    dired_size_color: Color,
    dired_dir_color: Color,
    comment_color: Color,
    warning_color: Color,
    error_color: Color,
    ok_color: Color,

}

impl Theme {
    fn new() -> Self {
        Theme {
            background_color: hex_to_rgb("#090909").unwrap(),
            text_color: hex_to_rgb("#9995BF").unwrap(),
            normal_cursor_color: hex_to_rgb("#658B5F").unwrap(),
            insert_cursor_color: hex_to_rgb("#514B8E").unwrap(),
            fringe_color: hex_to_rgb("#090909").unwrap(),
            line_numbers_color: hex_to_rgb("#171717").unwrap(),
            current_line_number_color: hex_to_rgb("#C0ACD1").unwrap(),
            modeline_color: hex_to_rgb("#060606").unwrap(),
            modeline_lighter_color: hex_to_rgb("#171717").unwrap(),
            minibuffer_color: hex_to_rgb("#070707").unwrap(),
            dired_mode_color: hex_to_rgb("#565663").unwrap(),
            dired_timestamp_color: hex_to_rgb("#514B8E").unwrap(),
            dired_path_color: hex_to_rgb("#658B5F").unwrap(),
            dired_size_color: hex_to_rgb("#48534A").unwrap(),
            dired_dir_color: hex_to_rgb("#514B8E").unwrap(),
            comment_color: hex_to_rgb("#867892").unwrap(),
            warning_color: hex_to_rgb("#565663").unwrap(),
            error_color: hex_to_rgb("#444E46").unwrap(),
            ok_color: hex_to_rgb("#4C6750").unwrap(),
        }
    }
    
    fn apply_cursor_color(&self, cursor_pos: (u16, u16), buffer: &Vec<Vec<char>>, mode: &Mode) {
        let is_over_text = if let Mode::Normal = mode {
            buffer.get(cursor_pos.1 as usize)
                .and_then(|line| line.get(cursor_pos.0 as usize))
                .map(|&c| c != ' ') // Check if the character is not a space
                .unwrap_or(false)
        } else {
            false
        };

        let color = if is_over_text {
            &self.text_color
        } else {
            match mode {
                Mode::Normal => &self.normal_cursor_color,
                Mode::Insert => &self.insert_cursor_color,
                Mode::Dired => &self.normal_cursor_color,
            }
        };

        // Convert the Color::Rgb to an ANSI escape sequence
        match color {
            Color::Rgb { r, g, b } => {
                // Construct the ANSI escape code for RGB color setting
                let ansi_color = format!("\x1b]12;rgb:{:02x}/{:02x}/{:02x}\x1b\\", r, g, b);
                print!("{}", ansi_color);
            },
            _ => {} // Handle other color types
        }

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
