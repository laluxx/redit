use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, SetAttribute, Attribute},
    terminal::{self, ClearType, disable_raw_mode, enable_raw_mode, size},
    cursor::{self, MoveTo},
};

use std::io::{self, stdout, Stdout, Write};
use std::io::Result;
use std::env;
use std::fs;
use std::path::Path;

use std::fs::DirEntry;
use std::path::PathBuf;
use chrono::{DateTime, Local};
use std::collections::HashMap;

// TODO Syntax highlighting
// extern crate tree_sitter;
// extern crate tree_sitter_rust;


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

    fn new(current_path: PathBuf, focus: Option<&str>) -> io::Result<Self> {
        let entries = Dired::list_directory_contents(&current_path)?;
        let mut cursor_pos = if entries.len() == 0 { 0 } else { 2 }; // Skip '.' and '..'

        // If a focus is specified, attempt to find it in the list and set the cursor position
        if let Some(dir_name) = focus {
            for (i, entry) in entries.iter().enumerate() {
                if entry.file_name().to_str() == Some(dir_name) {
                    cursor_pos = i as u16 + 2; // Adjust for '.' and '..' being at positions 0 and 1
                    break;
                }
            }
        }

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

    pub fn create_directory(&mut self, dir_name: &str) -> io::Result<()> {
        let new_dir_path = self.current_path.join(dir_name);
        fs::create_dir(&new_dir_path)?;
        Ok(())
    }

    pub fn delete_entry(&mut self) -> io::Result<()> {
        // Ensure its not '.' or '..'
        if self.cursor_pos > 1 && (self.cursor_pos as usize - 2) < self.entries.len() {
            let entry_to_delete = &self.entries[self.cursor_pos as usize - 2];
            let path_to_delete = entry_to_delete.path();

            if path_to_delete.is_dir() {
                // Recursive
                fs::remove_dir_all(&path_to_delete)?;
            } else {
                fs::remove_file(&path_to_delete)?;
            }

            self.refresh_directory_contents()?;

            // Check if the deleted entry was the last one
            if self.cursor_pos as usize - 2 >= self.entries.len() {
                self.cursor_pos -= 1;
            }
        }

        Ok(())
    }

    pub fn rename_entry(&mut self, new_name: &str) -> io::Result<()> {
        if self.cursor_pos > 1 && (self.cursor_pos as usize - 2) < self.entries.len() { // Skipping '.' and '..'
            let entry_to_rename = &self.entries[self.cursor_pos as usize - 2];
            let original_path = entry_to_rename.path();
            let new_path = original_path.parent().unwrap().join(new_name);
            fs::rename(&original_path, &new_path)?;
            self.refresh_directory_contents()?;
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid selection for rename."));
        }

        Ok(())
    }

    // TODO color file extentions if color_dired is true, fix background
    // TODO scrolling, it overlap the modeline..
    pub fn draw_dired(&mut self, stdout: &mut Stdout, height: u16, theme: &Theme) -> io::Result<()> {
        let display_path = self.current_path.display().to_string();
        let trimmed_path = display_path.trim_end_matches('/');
        execute!(
            stdout,
            MoveTo(3, 0),
            SetForegroundColor(theme.dired_path_color),
            SetBackgroundColor(theme.background_color),
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
                    SetBackgroundColor(theme.background_color),
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
                    execute!(
                        stdout,
                        SetForegroundColor(color),
                        SetBackgroundColor(theme.background_color),
                        Print(ch)
                    )?;
                }
            } else {
                execute!(
                    stdout,
                    SetForegroundColor(theme.text_color),
                    SetBackgroundColor(theme.background_color),
                    Print(permissions),
                    ResetColor
                )?;
            }

            execute!(
                stdout,
                ResetColor,
                SetBackgroundColor(theme.background_color),
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
    themes: HashMap<String, Theme>,
    current_theme_name: String,
    show_fringe: bool,
    show_line_numbers: bool,
    insert_line_cursor: bool,
    minibuffer_active: bool,
    minibuffer_height: u16,
    minibuffer_content: String,
    minibuffer_prefix: String,
    current_file_path: PathBuf,
    should_open_file: bool,
    fzy: Option<Fzy>,
}

impl Editor {
    fn new() -> Editor {
        let mut themes = HashMap::new();
        themes.insert("nature".to_string(), Theme::nature());
        themes.insert("everforest".to_string(), Theme::everforest_medium());
        let initial_theme_name = "nature".to_string();
        let current_path = env::current_dir().unwrap();

        Editor { 
            mode: Mode::Normal, 
            cursor_pos: (0, 0), 
            offset: (0, 0),
            buffer: vec![vec![]],
            themes,
            current_theme_name: initial_theme_name,
            show_fringe: true,
            show_line_numbers: true,
            insert_line_cursor: false,
            dired: None,
            minibuffer_active: false,
            minibuffer_height: 1,
            minibuffer_content: String::new(),
            minibuffer_prefix: String::new(),
            current_file_path: current_path.clone(),
            should_open_file: false,
            // fzy: Fzy::new(current_path),
            fzy: Some(Fzy::new(current_path)),
        }
    }

    fn current_theme(&self) -> &Theme {
        self.themes.get(&self.current_theme_name).expect("Current theme not found")
    }

    fn switch_theme(&mut self, theme_name: &str) {
        if self.themes.contains_key(theme_name) {
            self.current_theme_name = theme_name.to_string();
        } else {
            // TODO implement message() theme doesn't exist 
        }
    }

    pub fn buffer_save(&self) -> Result<()> {
        let content: String = self.buffer.iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<String>>()
            .join("\n");

        fs::write(&self.current_file_path, content)
            .expect("Failed to save file");
        
        println!("File saved successfully.");
        Ok(())
    }

    fn enter(&mut self) {
        let tail = self.buffer[self.cursor_pos.1 as usize].split_off(self.cursor_pos.0 as usize);
        self.buffer.insert(self.cursor_pos.1 as usize + 1, tail);
        self.cursor_pos.1 += 1;
        self.cursor_pos.0 = 0;
    }

    fn dired_jump(&mut self) {
        // Clone the path to avoid borrowing issues
        let current_file_path_clone = self.current_file_path.clone();
        let (path_to_open, focus) = if current_file_path_clone.is_file() {
            (current_file_path_clone.parent().unwrap_or_else(|| Path::new("/")).to_path_buf(),
             current_file_path_clone.file_name().and_then(|n| n.to_str()))
        } else {
            (current_file_path_clone, None)
        };
        
        // Now it's safe to call `self.open` since `self.current_file_path` is not borrowed anymore
        if let Err(e) = self.open(&path_to_open, focus) {
            // Handle the error, maybe show a message to the user
            eprintln!("Error opening directory: {}", e);
        }
    }
    
    fn draw(&mut self, stdout: &mut Stdout) -> Result<()> {
        let (width, height) = terminal::size()?;
        let background_color = self.current_theme().background_color;

        execute!(
            stdout,
            SetBackgroundColor(background_color),
            terminal::Clear(ClearType::All)
        )?;

        // Always draw modeline and minibuffer
        self.draw_modeline(stdout, width, height)?;
        self.draw_minibuffer(stdout, width, height)?;


        if let Some(mut fzy) = self.fzy.take() { // Temporarily take `fzy` out of `self`
            let theme = self.current_theme(); // Now it's safe to borrow `self` immutably
            if fzy.active {
                fzy.draw(stdout, theme)?;
            }
            self.fzy.replace(fzy); // Put `fzy` back into `self`
        }

        if self.mode == Mode::Dired {
            if let Some(mut dired) = self.dired.take() { // Temporarily take `dired` out of `self`
                let theme = self.current_theme(); // Now it's safe to borrow `self` immutably
                dired.draw_dired(stdout, height, theme)?;
                self.dired.replace(dired); // Put `dired` back into `self`
            }
        }

        // Reset the background color for fringe and line numbers
        execute!(stdout, SetBackgroundColor(background_color))?;

        // Draw text area for non-Dired modes
        if self.mode != Mode::Dired {
            let mut start_col = 0;
            if self.show_fringe {
                self.draw_fringe(stdout, height)?;
                start_col += 2;
            }
            if self.show_line_numbers {
                self.draw_line_numbers(stdout, height, start_col)?;
            }
            self.draw_text(stdout)?;
        }

        let cursor_pos = if self.minibuffer_active {
            let minibuffer_cursor_pos_x = 2 + self.minibuffer_prefix.len() as u16 + self.minibuffer_content.len() as u16;
            let minibuffer_cursor_pos_y = height - self.minibuffer_height;
            (minibuffer_cursor_pos_x, minibuffer_cursor_pos_y)
        } else if self.fzy.as_ref().map_or(false, |fzy| fzy.active) { // Check if fzy is Some and active
            // Access fzy.input safely via as_ref() and map_or
            let minibuffer_cursor_pos_x = 18 + self.fzy.as_ref().map_or(0, |fzy| fzy.input.len()) as u16;
            let minibuffer_cursor_pos_y = height - self.minibuffer_height;
            (minibuffer_cursor_pos_x, minibuffer_cursor_pos_y)
        } else if self.mode == Mode::Dired {
            self.dired.as_ref().map_or((0, 0), |dired| {
                let cursor_line = (dired.cursor_pos + 2).min(height - self.minibuffer_height - 1);
                (dired.entry_first_char_column, cursor_line)
            })
        } else {
            let mut start_col = 0;
            if self.show_fringe {
                start_col += 2; // Account for fringe
            }
            if self.show_line_numbers {
                start_col += 4; // Account for line number
            }
            let cursor_x = self.cursor_pos.0.saturating_sub(self.offset.0) + start_col;
            // Ensure cursor Y-position doesn't go into the minibuffer area or below.
            let cursor_y = (self.cursor_pos.1.saturating_sub(self.offset.1)).min(height - self.minibuffer_height - 2);
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
        let (width, height) = size()?; // TODO Horizzonatl scroll
        let text_color = self.current_theme().text_color;
        let mut start_col = 0;

        if self.show_fringe {
            start_col += 2; // Fringe width
        }

        if self.show_line_numbers {
            start_col += 4; // Space for line numbers
        }

        execute!(stdout, SetForegroundColor(text_color))?; // Set the text color
        let bottom_exclude = self.minibuffer_height + 1; // Calculate area to exclude

        for (idx, line) in self.buffer.iter().enumerate() {
            if idx >= self.offset.1 as usize && idx < (self.offset.1 + height - bottom_exclude) as usize {
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
            let bottom_exclude = self.minibuffer_height + 1;

            for y in 0..height - bottom_exclude {
                let line_index = (self.offset.1 as usize) + y as usize;
                if line_index < self.buffer.len() {
                    let absolute_line_number = line_index + 1;
                    
                    let line_number_color = if self.mode == Mode::Normal && line_index == self.cursor_pos.1 as usize {
                        self.current_theme().current_line_number_color
                    } else if self.mode == Mode::Insert && line_index == self.cursor_pos.1 as usize {
                        self.current_theme().insert_cursor_color
                    } else {
                        self.current_theme().line_numbers_color
                    };

                    execute!(
                        stdout,
                        MoveTo(start_col, y),
                        SetForegroundColor(line_number_color),
                        Print(format!("{:>3} ", absolute_line_number))
                    )?;
                }
            }
        }
        Ok(())
    }

    fn draw_fringe(&self, stdout: &mut io::Stdout, height: u16) -> Result<()> {
        if self.show_fringe {
            let fringe_color = self.current_theme().fringe_color;
            let bottom_exclude = self.minibuffer_height + 1;

            for y in 0..height - bottom_exclude { 
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

        let modeline_y = height - self.minibuffer_height - 1;

        // Determine what to display based on the current mode.
        let display_str = match self.mode {
            Mode::Dired => {
                if let Some(dired) = &self.dired {
                    format!("󰉋 {}", dired.current_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("")).to_str().unwrap())
                } else {
                    "󰉋 Unknown".to_string()
                }
            },
            // In other modes, display just the file name from `current_file_path`.
            _ => self.current_file_path.file_name().map_or("Untitled".to_string(), |os_str| os_str.to_str().unwrap_or("Untitled").to_string()),
        };

        let (mode_str, mode_bg_color, mode_text_color) = match self.mode {
            Mode::Normal => ("NORMAL", self.current_theme().normal_cursor_color, Color::Black),
            Mode::Insert => ("INSERT", self.current_theme().insert_cursor_color, Color::Black),
            Mode::Dired => ("DIRED", self.current_theme().dired_mode_color, Color::Black),
        };

        let file_bg_color = self.current_theme().modeline_lighter_color;
        let file_text_color = self.current_theme().text_color;
        let modeline_bg_color = self.current_theme().modeline_color;

        execute!(stdout, SetBackgroundColor(mode_bg_color), MoveTo(0, modeline_y), Print(" "))?;
        execute!(stdout, SetForegroundColor(mode_text_color), SetAttribute(Attribute::Bold), Print(format!(" {} ", mode_str)), SetAttribute(Attribute::Reset))?;
        execute!(stdout, SetBackgroundColor(file_bg_color), SetForegroundColor(mode_bg_color), Print(sep_r))?;
        execute!(stdout, SetBackgroundColor(file_bg_color), Print(" "))?;
        execute!(stdout, SetForegroundColor(file_text_color), Print(format!(" {} ", display_str)))?;
        execute!(stdout, SetBackgroundColor(modeline_bg_color), SetForegroundColor(file_bg_color), Print(sep_r))?;

        let pos_str = format!("{}:{}", self.cursor_pos.1 + 1, self.cursor_pos.0 + 1);
        let pos_str_length = pos_str.len() as u16 + 2;

        let fill_length_before_pos_str = if self.mode == Mode::Dired {
            width - (4 + mode_str.len() as u16 + display_str.len() as u16 + pos_str_length)
        } else {
            width - (4 + mode_str.len() as u16 + display_str.len() as u16 + pos_str_length + 3)
        };

        execute!(stdout, SetBackgroundColor(modeline_bg_color), Print(" ".repeat(fill_length_before_pos_str as usize)))?;
        execute!(stdout, SetBackgroundColor(modeline_bg_color), SetForegroundColor(self.current_theme().normal_cursor_color), Print(sep_l))?;
        execute!(stdout, SetBackgroundColor(self.current_theme().normal_cursor_color), SetForegroundColor(Color::Black), Print(format!("{} ", pos_str)))?;
        execute!(stdout, ResetColor)?;

        Ok(())
    }

    fn draw_minibuffer(&self, stdout: &mut io::Stdout, width: u16, height: u16) -> Result<()> {
        let minibuffer_bg = self.current_theme().minibuffer_color;
        let content_fg = self.current_theme().text_color;
        let prefix_fg = self.current_theme().normal_cursor_color;

        let minibuffer_start_y = height - self.minibuffer_height;

        // Fill the minibuffer background
        for y_offset in 0..self.minibuffer_height {
            execute!(
                stdout,
                MoveTo(0, minibuffer_start_y + y_offset),
                SetBackgroundColor(minibuffer_bg),
                Print(" ".repeat(width as usize))
            )?;
        }

        // Draw minibuffer prefix and content
        execute!(
            stdout,
            MoveTo(0, minibuffer_start_y),
            SetForegroundColor(prefix_fg),
            Print(format!(" {}", self.minibuffer_prefix)),
            SetForegroundColor(content_fg),
            Print(format!(" {}", self.minibuffer_content))
        )?;

        Ok(())
    }

    pub fn open(&mut self, path: &PathBuf, focus: Option<&str>) -> Result<()> {
        self.current_file_path = path.clone();

        if path.is_dir() {
            self.dired = Some(Dired::new(path.clone(), focus)?);
            self.mode = Mode::Dired;
        } else {
            let contents = fs::read_to_string(path)
                .unwrap_or_else(|_| "".to_string());
            self.buffer = contents.lines()
                .map(|line| line.chars().collect())
                .collect();

            if self.buffer.is_empty() {
                self.buffer.push(Vec::new());
            }
            self.mode = Mode::Normal;
        }

        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let mut stdout = stdout();
        enable_raw_mode()?;
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

        loop {
            let fzy_active = self.fzy.as_ref().map_or(false, |fzy| fzy.active);
            self.current_theme().apply_cursor_color(self.cursor_pos, &self.buffer, &self.mode, self.minibuffer_active, fzy_active);
            self.draw(&mut stdout)?;
            
            if let Event::Key(key) = event::read()? {
                let mut event_handled = false;

                if self.fzy.as_ref().map_or(false, |fzy| fzy.active) {
                    if let Some(mut fzy) = self.fzy.take() { // Temporarily take `fzy` out
                        event_handled = fzy.handle_input(key, self);
                        self.fzy.replace(fzy); // Put `fzy` back
                        if event_handled {
                            self.minibuffer_height = 1;
                        }
                    }
                }

                if self.minibuffer_active && !event_handled {
                    match key.code {
                        KeyCode::Char(c) => self.minibuffer_content.push(c),
                        KeyCode::Backspace => { self.minibuffer_content.pop(); },
                        KeyCode::Esc => {
                            self.minibuffer_active = false;
                            self.minibuffer_prefix.clear();
                            self.minibuffer_content.clear();
                        },
                        KeyCode::Enter => {
                            let minibuffer_content = std::mem::take(&mut self.minibuffer_content);
                            if self.minibuffer_prefix == "Switch theme:" {
                                self.switch_theme(&minibuffer_content);
                            } else if self.mode == Mode::Dired {
                                if self.minibuffer_prefix == "Create directory:" {
                                    if let Some(dired) = &mut self.dired {
                                        dired.create_directory(&minibuffer_content)?;
                                        dired.refresh_directory_contents()?;
                                    }
                                } else if self.minibuffer_prefix.starts_with("Delete ") && self.minibuffer_prefix.ends_with(" [y/n]:") {
                                    if minibuffer_content == "y" {
                                        if let Some(dired) = &mut self.dired {
                                            dired.delete_entry()?;
                                        }
                                    }
                                } else if self.minibuffer_prefix == "Rename:" {
                                    if let Some(dired) = &mut self.dired {
                                        dired.rename_entry(&minibuffer_content)?;
                                    }
                                } else {
                                    let file_path = self.dired.as_ref().unwrap().current_path.join(&minibuffer_content);
                                    if std::fs::File::create(&file_path).is_ok() {
                                        if let Some(dired) = &mut self.dired {
                                            dired.refresh_directory_contents()?;
                                            if self.should_open_file {
                                                self.open(&file_path, None)?;
                                            }
                                        }
                                    }
                                    self.should_open_file = false;
                                }
                            }
                            self.minibuffer_active = false;
                            self.minibuffer_prefix.clear();
                            event_handled = true;
                        },
                        _ => {}
                    }
                }

                if !event_handled && !self.fzy.as_ref().map_or(false, |fzy| fzy.active) && !self.minibuffer_active {
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
        let block = "\x1b[2 q";
        let line = "\x1b[6 q";

        let shape = match self.mode {
            Mode::Normal | Mode::Dired => block,
            Mode::Insert => if self.insert_line_cursor { line } else { block },
        };

        print!("{}", shape);
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
                if let Some(dired) = &mut self.dired {
                    let current_dir_name = dired.current_path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(""); // Get the current directory name as a &str

                    let parent_path = dired.current_path.parent()
                        .unwrap_or_else(|| Path::new("/"))
                        .to_path_buf();

                    // Update Dired with the parent path, highlighting the directory we came from
                    *dired = Dired::new(parent_path, Some(current_dir_name))?;
                }
            },

            KeyCode::Char('l') | KeyCode::Enter => {
                if let Some(dired) = &mut self.dired {
                    if dired.cursor_pos == 0 {
                        // Do nothing for '.'
                    } else if dired.cursor_pos == 1 {
                        // Handle '..' the same as 'h', navigate to the parent directory
                        let parent_path = dired.current_path.parent().unwrap_or_else(|| Path::new("/")).to_path_buf();
                        *dired = Dired::new(parent_path, None)?;
                    } else {
                        let selected_entry = &dired.entries[dired.cursor_pos as usize - 2]; // Adjusting for '.' and '..'
                        let path = selected_entry.path();
                        if path.is_dir() {
                            *dired = Dired::new(path.to_path_buf(), None)?;
                        } else if path.is_file() {
                            self.open(&path, None)?;
                        }
                    }
                }
            },
             
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.minibuffer_active = true;
                self.minibuffer_prefix = if matches!(key.code, KeyCode::Char('T')) {
                    "Touch and open:".to_string()
                } else {
                    "Touch:".to_string()
                };
                self.minibuffer_content = "".to_string();
                self.should_open_file = matches!(key.code, KeyCode::Char('T'));
            },

            KeyCode::Char('d') => {
                self.minibuffer_active = true;
                self.minibuffer_prefix = "Create directory:".to_string();
                self.minibuffer_content = "".to_string();
            },

            KeyCode::Char('D') => {
                if let Some(dired) = &self.dired {
                    if dired.cursor_pos > 1 && (dired.cursor_pos as usize - 2) < dired.entries.len() {
                        let entry_to_delete = &dired.entries[dired.cursor_pos as usize - 2];
                        let entry_name = entry_to_delete.file_name().to_string_lossy().into_owned();

                        self.minibuffer_prefix = format!("Delete {} [y/n]:", entry_name);
                        self.minibuffer_active = true;
                        self.minibuffer_content = "".to_string();
                    }
                }
            },

            KeyCode::Char('r') => {
                if let Some(dired) = &self.dired {
                    // Ensure the cursor is on a valid entry (not '.' or '..')
                    if dired.cursor_pos > 1 && (dired.cursor_pos as usize - 2) < dired.entries.len() {
                        let entry_to_rename = &dired.entries[dired.cursor_pos as usize - 2];
                        let entry_name = entry_to_rename.file_name().to_string_lossy().into_owned();

                        // Activate the minibuffer for renaming, pre-filling it with the entry's name
                        self.minibuffer_active = true;
                        self.minibuffer_prefix = "Rename:".to_string();
                        self.minibuffer_content = entry_name;
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
        match key {
            KeyEvent {
                code: KeyCode::Char('t') | KeyCode::Char('T'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // Activates the minibuffer for theme switching with Ctrl+T
                self.minibuffer_active = true;
                self.minibuffer_prefix = "Switch theme:".to_string();
                self.minibuffer_content = "".to_string();
            },

            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.enter();
            }

            KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                ..
            } => match code {
                KeyCode::Char('f') => {
                    if let Some(fzy) = &mut self.fzy {
                        // fzy.current_path = self.current_file_path.parent().unwrap().to_path_buf(); // TODO
                        fzy.active = true;
                        fzy.input.clear();
                        fzy.update_items();
                        self.minibuffer_height = fzy.calculate_minibuffer_height(fzy.max_visible_lines) as u16;
                    }
                },

                KeyCode::Char('s') => {
                    self.buffer_save()?;
                },
                KeyCode::Char('i') => {
                    self.mode = Mode::Insert;
                    self.set_cursor_shape();
                },

                KeyCode::Char('d') => {
                    self.dired_jump();
                },

                KeyCode::Char('j') => {
                    if self.cursor_pos.1 < self.buffer.len() as u16 - 1 {
                        self.cursor_pos.1 += 1;
                        let next_line_len = self.buffer[self.cursor_pos.1 as usize].len() as u16;
                        if self.cursor_pos.0 > next_line_len {
                            self.cursor_pos.0 = next_line_len;
                        }

                        let (_, height) = size()?;
                        let text_area_height = height - self.minibuffer_height - 1; // -1 for modeline

                        // Adjust offset if cursor moves beyond the last line of the text area
                        if self.cursor_pos.1 >= self.offset.1 + text_area_height {
                            self.offset.1 += 1;
                        }
                    }
                },
                KeyCode::Char('k') => {
                    if self.cursor_pos.1 > 0 {
                        self.cursor_pos.1 -= 1;

                        let prev_line_len = self.buffer[self.cursor_pos.1 as usize].len() as u16;
                        if self.cursor_pos.0 > prev_line_len {
                            self.cursor_pos.0 = prev_line_len;
                        }

                        if self.cursor_pos.1 < self.offset.1 {
                            self.offset.1 = self.offset.1.saturating_sub(1);
                        }
                    }
                },
                KeyCode::Char('h') => {
                    if self.cursor_pos.0 > 0 {
                        self.cursor_pos.0 -= 1;
                    }
                },
                KeyCode::Char('l') => {
                    if self.cursor_pos.0 < self.buffer[self.cursor_pos.1 as usize].len() as u16 {
                        self.cursor_pos.0 += 1;
                    }
                },
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    execute!(stdout(), terminal::LeaveAlternateScreen, cursor::Show)?;
                    std::process::exit(0);
                },
                _ => {}
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
                self.enter();
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
    fn nature() -> Self {
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

    fn everforest_medium() -> Self {
        Theme {
            background_color: hex_to_rgb("#2D353B").unwrap(),
            text_color: hex_to_rgb("#D3C6AA").unwrap(),
            normal_cursor_color: hex_to_rgb("#A7C080").unwrap(), 
            insert_cursor_color: hex_to_rgb("#E67E80").unwrap(), 
            fringe_color: hex_to_rgb("#2D353B").unwrap(), 
            line_numbers_color: hex_to_rgb("#3D484D").unwrap(), 
            current_line_number_color: hex_to_rgb("#A7C080").unwrap(), 
            modeline_color: hex_to_rgb("#3D484D").unwrap(), 
            modeline_lighter_color: hex_to_rgb("#475258").unwrap(), 
            minibuffer_color: hex_to_rgb("#232A2E").unwrap(), 
            dired_mode_color: hex_to_rgb("#D699B6").unwrap(), 
            dired_timestamp_color: hex_to_rgb("#D699B6").unwrap(), 
            dired_path_color: hex_to_rgb("#A7C080").unwrap(), 
            dired_size_color: hex_to_rgb("#3D484D").unwrap(), 
            dired_dir_color: hex_to_rgb("#A7C080").unwrap(), 
            comment_color: hex_to_rgb("#3D484D").unwrap(), 
            warning_color: hex_to_rgb("#DBBC7F").unwrap(), 
            error_color: hex_to_rgb("#E67E80").unwrap(), 
            ok_color: hex_to_rgb("#A7C080").unwrap(), 
        }
    }

    fn apply_cursor_color(
        &self,
        cursor_pos: (u16, u16),
        buffer: &Vec<Vec<char>>,
        mode: &Mode,
        minibuffer_active: bool,
        fzy_active: bool,)
    {
        let is_over_text = if let Mode::Normal = mode {
            buffer.get(cursor_pos.1 as usize)
                .and_then(|line| line.get(cursor_pos.0 as usize))
                .map(|&c| c != ' ') // Check if the character is not a space
                .unwrap_or(false)
        } else {
            false
        };

        let color = if is_over_text && !minibuffer_active && !fzy_active {
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

    if args.len() > 1 {
        // let file_path = &args[1];
        let file_path = PathBuf::from(&args[1]);
        editor.open(&file_path, None)?;
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


struct Fzy {
    active: bool,
    items: Vec<String>,
    input: String,
    selection_index: usize,
    max_visible_lines: usize,
    current_path: PathBuf,
    initial_input_line_y: Option<u16>,
    initial_items_start_y: Option<u16>,
    initial_positioning_done: bool,
}

impl Fzy {
    fn new(current_path: PathBuf) -> Self {
        Fzy {
            active: false,
            items: Vec::new(),
            input: String::new(),
            selection_index: 0,
            max_visible_lines: 11,
            current_path,
            initial_input_line_y: None,
            initial_items_start_y: None,
            initial_positioning_done: false,
        }
    }

    fn update_items(&mut self) {
        let mut entries = vec![".".to_string(), "..".to_string()];
        let dir_entries = std::fs::read_dir(&self.current_path)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let file_name = path.file_name()?.to_str()?.to_owned();

                // Skip '.' and '..' as they are already added.
                if file_name == "." || file_name == ".." {
                    return None;
                }

                // Add file if it matches the input filter.
                if self.input.is_empty() || file_name.contains(&self.input) {
                    Some(file_name)
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        entries.extend(dir_entries);

        if !self.input.is_empty() {
            entries.retain(|entry| entry.contains(&self.input));
        }

        self.items = entries;
        self.selection_index = 0; // Reset selection index on each update
    }

    pub fn calculate_minibuffer_height(&self, max_height: usize) -> usize {
        let total_items = self.items.len();
        let visible_items = total_items.min(self.max_visible_lines) + 1; // +1 for the input line
        visible_items.min(max_height + 1) // Ensure it does not exceed max_height
    }

    // TODO prefix, path, scroll, end scroll
    fn draw(&mut self, stdout: &mut Stdout, theme: &Theme) -> io::Result<()> {
        let (width, height) = terminal::size()?;

        if !self.initial_positioning_done {
            let items_to_display = self.items.len().min(self.max_visible_lines);
            self.initial_input_line_y = Some(height.saturating_sub(items_to_display as u16 + 1));
            self.initial_items_start_y = Some(self.initial_input_line_y.unwrap().saturating_add(1));
            self.initial_positioning_done = true;
        }

        let input_line_y = self.initial_input_line_y.unwrap_or(height.saturating_sub(self.max_visible_lines as u16 + 1));
        let items_start_y = self.initial_items_start_y.unwrap_or(input_line_y.saturating_add(1));

        execute!(
            stdout,
            MoveTo(1, input_line_y),
            SetForegroundColor(theme.normal_cursor_color),
            Print(format!(" {:}/{:<2} ", self.selection_index + 1, self.items.len())),
            Print("Find file: "),
            SetForegroundColor(theme.text_color),
            Print(&self.input)
        )?;

        for (index, item) in self.items.iter().enumerate() {
            let y_pos = items_start_y + index as u16;
            if y_pos >= height { break; }

            let is_dir = item == "." || item == ".." || self.current_path.join(item).is_dir();
            let formatted_item = if is_dir { format!("{}/", item) } else { item.clone() };

            let (icon, icon_color) = match item.as_str() {
                ".." => ("󱚁", theme.text_color),
                "." => ("󰉋", theme.text_color),
                ".git" => ("", theme.text_color),
                _ if item.ends_with(".rs") => ("", hex_to_rgb("#DEA584").unwrap()),
                _ if item.ends_with(".lock") => ("󰌾", theme.text_color), 
                _ if item.ends_with(".toml") => ("", theme.text_color),
                _ if item.ends_with(".json") => ("", hex_to_rgb("#CBCB41").unwrap()),
                _ => ("󰈚", theme.text_color), // Default icon for files
            };

            if index == self.selection_index {
                execute!(
                    stdout,
                    MoveTo(0, y_pos),
                    SetBackgroundColor(theme.normal_cursor_color),
                    Print(" ".repeat(width as usize)),
                )?;
            }

            execute!(
                stdout,
                MoveTo(1, y_pos),
                SetForegroundColor(icon_color),
                Print(format!("{} ", icon)),
            )?;

            let item_color = if is_dir { theme.dired_dir_color } else { theme.text_color };
            execute!(
                stdout,
                SetForegroundColor(item_color),
                Print(format!(" {}", formatted_item)),
                SetBackgroundColor(theme.minibuffer_color)
            )?;
        }

        execute!(stdout, MoveTo(0, height - 1))?;
        Ok(())
    }
    
    pub fn handle_input(&mut self, key: KeyEvent, editor: &mut Editor) -> bool {
        let mut state_changed = false;

        match key {
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.selection_index < self.items.len() - 1 { 
                    self.selection_index += 1;
                }
            },

            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.selection_index > 0 {
                    self.selection_index -= 1;
                }
            },

            KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                ..
            } => match code {
                KeyCode::Char(c) => {
                    self.input.push(c);
                    self.update_items();
                },
                KeyCode::Backspace => {
                    self.input.pop();
                    self.update_items();
                },
                KeyCode::Up => {
                    if self.selection_index > 0 {
                        self.selection_index -= 1;
                    }
                },
                KeyCode::Down => {
                    if self.selection_index < self.items.len() - 1 {
                        self.selection_index += 1;
                    }
                },
                KeyCode::Esc => {
                    self.active = false;
                    self.input.clear();
                    self.items.clear();
                    state_changed = true; // Indicate that the fuzzy finder was deactivated
                },
                KeyCode::Enter => {
                    if let Some(item) = self.items.get(self.selection_index) {
                        let full_path = self.current_path.join(item);
                        editor.open(&full_path, None);
                        self.active = false;
                        self.input.clear();
                        self.items.clear();
                        state_changed = true;
                    }
                }
                _ => {}
            },
            _ => {}
        }

        state_changed // Return whether the fuzzy finder's state has changed
    }
}

