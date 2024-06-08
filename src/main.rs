use crossterm::{
    event::{self, poll, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, SetAttribute, Attribute},
    terminal::{self, ClearType, disable_raw_mode, enable_raw_mode, size},
    cursor::{self, MoveTo},
};

use std::{io::{self, stdout, Stdout, Write}, vec};
use std::io::Result;
use std::env;
use std::fs;
use std::path::Path;

use std::fs::DirEntry;
use std::path::PathBuf;
use chrono::{DateTime, Local};
use std::collections::HashMap;

use std::time::Duration;

use std::process::Command;

// TODO  color minibuffer prefix
// TODO  fzy find in M-x 
// TODO  wdired


// TODO Syntax highlighting
// extern crate tree_sitter;
// extern crate tree_sitter_rust;

#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
    Dired,
    Visual,
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

    // TODO color file extentions if color_dired is true
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
                    // theme.normal_cursor_color
                    theme.dired_dir_color
                } else {
                    theme.dired_dir_color
                }
            } else {
                theme.text_color
            };

            execute!(stdout, MoveTo(5, line_number))?;

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
                SetForegroundColor(if self.color_dired { theme.dired_timestamp_color } else { theme.text_color }), Print(format!("{:14}", modified.format("%b %d %H:%M"))),
                SetForegroundColor(entry_color), Print(format!("{}", entry_name)),
                ResetColor
            )?;

            line_number += 1;
        }

        Ok(())
    }
}





use mlua::{Lua, Result as LuaResult, Function};

// #[derive(Debug)]
struct Config {
    blink_cursor: bool,
    show_fringe: bool,
    show_line_numbers: bool,
    insert_line_cursor: bool,
    show_hl_line: bool,
    top_scroll_margin: u16,
    bottom_scroll_margin: u16,
    blink_limit: u8,
    themes: HashMap<String, Theme>,
    current_theme_name: String,
    indentation: usize,
    electric_pair_mode: bool,
    tree_node: char,
    current_tree_node: char,
    tree_node_separator: char,
    modeline_separator_right: char,
    modeline_separator_left: char,
    shell: String,
}

impl Config {
    fn new(lua: &Lua, lua_script_path: Option<&str>) -> LuaResult<Self> {

        let defaults = Config {
            blink_cursor: true,
            show_fringe: true,
            show_line_numbers: true,
            insert_line_cursor: false,
            show_hl_line: false,
            top_scroll_margin: 10,
            bottom_scroll_margin: 10,
            blink_limit: 10,
            themes: HashMap::new(),
            current_theme_name: "default".to_string(),
            indentation: 4,
            electric_pair_mode: true,
            tree_node: '◯',
            current_tree_node: '●',
            tree_node_separator: '—',
            modeline_separator_right: '',
            modeline_separator_left: '',
            shell: "sh".to_string(),
        };
        
        if let Some(path) = lua_script_path {
            lua.load(&std::fs::read_to_string(path)?).exec()?; // TODO print lua errors
            let globals = lua.globals();

            let lua_themes: mlua::Table = match globals.get("Themes") {
                Ok(table) => table,
                Err(_) => lua.create_table().expect("Failed to create a new Lua table"),
            };


            let mut themes = HashMap::new();
            themes.insert("wal".to_string(), Theme::wal());
            let initial_theme_name: String = globals.get("Theme").unwrap_or("wal".to_string());



            for pair in lua_themes.pairs::<String, mlua::Table>() {
                let (name, theme_table) = pair?;
                let theme = Theme {
                    background_color: hex_to_rgb(&theme_table.get::<_, String>("background_color")?).unwrap(),
                    text_color: hex_to_rgb(&theme_table.get::<_, String>("text_color")?).unwrap(),
                    normal_cursor_color: hex_to_rgb(&theme_table.get::<_, String>("normal_cursor_color")?).unwrap(),
                    insert_cursor_color: hex_to_rgb(&theme_table.get::<_, String>("insert_cursor_color")?).unwrap(),
                    fringe_color: hex_to_rgb(&theme_table.get::<_, String>("fringe_color")?).unwrap(),
                    line_numbers_color: hex_to_rgb(&theme_table.get::<_, String>("line_numbers_color")?).unwrap(),
                    current_line_number_color: hex_to_rgb(&theme_table.get::<_, String>("current_line_number_color")?).unwrap(),
                    modeline_color: hex_to_rgb(&theme_table.get::<_, String>("modeline_color")?).unwrap(),
                    modeline_lighter_color: hex_to_rgb(&theme_table.get::<_, String>("modeline_lighter_color")?).unwrap(),
                    minibuffer_color: hex_to_rgb(&theme_table.get::<_, String>("minibuffer_color")?).unwrap(),
                    dired_mode_color: hex_to_rgb(&theme_table.get::<_, String>("dired_mode_color")?).unwrap(),
                    dired_timestamp_color: hex_to_rgb(&theme_table.get::<_, String>("dired_timestamp_color")?).unwrap(),
                    dired_path_color: hex_to_rgb(&theme_table.get::<_, String>("dired_path_color")?).unwrap(),
                    dired_size_color: hex_to_rgb(&theme_table.get::<_, String>("dired_size_color")?).unwrap(),
                    dired_dir_color: hex_to_rgb(&theme_table.get::<_, String>("dired_dir_color")?).unwrap(),
                    comment_color: hex_to_rgb(&theme_table.get::<_, String>("comment_color")?).unwrap(),
                    warning_color: hex_to_rgb(&theme_table.get::<_, String>("warning_color")?).unwrap(),
                    error_color: hex_to_rgb(&theme_table.get::<_, String>("error_color")?).unwrap(),
                    ok_color: hex_to_rgb(&theme_table.get::<_, String>("ok_color")?).unwrap(),
                    search_bg_color: hex_to_rgb(&theme_table.get::<_, String>("search_bg_color")?).unwrap(),
                    visual_mode_color: hex_to_rgb(&theme_table.get::<_, String>("visual_mode_color")?).unwrap(),
                    selection_color: hex_to_rgb(&theme_table.get::<_, String>("selection_color")?).unwrap(),
                    hl_line_color: hex_to_rgb(&theme_table.get::<_, String>("hl_line_color")?).unwrap(),
                    use_color: hex_to_rgb(&theme_table.get::<_, String>("use_color")?).unwrap(),
                    string_color: hex_to_rgb(&theme_table.get::<_, String>("string_color")?).unwrap(),
                };
                themes.insert(name, theme);
            }


            let tree_node: char = globals.get::<_, String>("Tree_node").unwrap_or(defaults.tree_node.to_string()).chars().next().unwrap_or(defaults.tree_node);
            let current_tree_node: char = globals.get::<_, String>("Current_tree_node").unwrap_or(defaults.current_tree_node.to_string()).chars().next().unwrap_or(defaults.current_tree_node);
            let tree_node_separator: char = globals.get::<_, String>("Tree_node_separator").unwrap_or(defaults.tree_node_separator.to_string()).chars().next().unwrap_or(defaults.tree_node_separator);

            let modeline_separator_right: char = globals.get::<_, String>("Modeline_separator_right").unwrap_or(defaults.modeline_separator_right.to_string()).chars().next().unwrap_or(defaults.modeline_separator_right);
            let modeline_separator_left: char = globals.get::<_, String>("Modeline_separator_left").unwrap_or(defaults.modeline_separator_left.to_string()).chars().next().unwrap_or(defaults.modeline_separator_left);
            // let shell:String = globals.get::<_, String>("Shell").unwrap_or(defaults.shell.to_string()).chars().next().unwrap_or(defaults.shell);
	    let shell: String = globals.get::<_, String>("Shell").unwrap_or(defaults.shell.to_string());
	    
            Ok(Config {
                blink_cursor: globals.get("Blink_cursor").unwrap_or(defaults.blink_cursor),
                show_fringe: globals.get("Show_fringe").unwrap_or(defaults.show_fringe),
                show_line_numbers: globals.get("Show_line_numbers").unwrap_or(defaults.show_line_numbers),
                insert_line_cursor: globals.get("Insert_line_cursor").unwrap_or(defaults.insert_line_cursor),
                show_hl_line: globals.get("Show_hl_line").unwrap_or(defaults.show_hl_line),
                top_scroll_margin: globals.get("Top_scroll_margin").unwrap_or(defaults.top_scroll_margin),
                bottom_scroll_margin: globals.get("Bottom_scroll_margin").unwrap_or(defaults.bottom_scroll_margin),
                blink_limit: globals.get("Blink_limit").unwrap_or(defaults.blink_limit),
                themes, // Use the loaded themes
                current_theme_name: initial_theme_name,
                indentation: globals.get("Indentation").unwrap_or(defaults.indentation),
                electric_pair_mode: globals.get("Electric_pair_mode").unwrap_or(defaults.electric_pair_mode),
                tree_node,
                current_tree_node,
                tree_node_separator,
                modeline_separator_right,
                modeline_separator_left,
		shell,
            })
        } else {
            Ok(defaults)
        }
    }
}


struct UndoState {
    buffer: Vec<Vec<char>>,
    cursor_pos: (u16, u16),
}


struct Keychords {
    ctrl_x_pressed: bool,
}

impl Keychords {
    fn new() -> Keychords {
        Keychords {
            ctrl_x_pressed: false,
        }
    }

    fn reset(&mut self) {
        self.ctrl_x_pressed = false;
    }
}


#[derive(Debug)]
struct Highlight {
    start: usize,
    end: usize,
    color: Color,
}

struct SyntaxHighlighter {
    parser: tree_sitter::Parser,
    tree: Option<tree_sitter::Tree>,
    highlights: Vec<Highlight>,
}

impl SyntaxHighlighter {
    fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_rust::language()).expect("Error loading Rust grammar");
        SyntaxHighlighter {
            parser,
            tree: None,
            highlights:  Vec::new()
        }
    }
    
    pub fn parse(&mut self, buffer: &[Vec<char>]) {
        let source_code: String = buffer.iter()
            .map(|line| line.iter().collect::<String>() + "\n")
            .collect();

        self.tree = self.parser.parse(&source_code, None);
    }

    fn get_color_for_node_kind(&self, kind: &str, theme: &Theme) -> Option<Color> {
        match kind {
            "let_condition" => Some(theme.dired_path_color),
            "string_literal" => Some(theme.string_color),
            "use_declaration" => Some(theme.use_color),
            _ => None,
        }
    }



    pub fn update_syntax_highlights(&mut self, theme: &Theme) {
        let mut highlights = std::mem::take(&mut self.highlights); // Temporarily take highlights out

        if let Some(tree) = &self.tree {
            let root_node = tree.root_node();
            self.collect_highlights(&root_node, &mut highlights, theme); // Work with the taken highlights
        }

        self.highlights = highlights; // Put it back
    }


    fn collect_highlights(&self, node: &tree_sitter::Node, highlights: &mut Vec<Highlight>, theme: &Theme) {
        // Check if the node kind should be highlighted
        if let Some(color) = self.get_color_for_node_kind(node.kind(), theme) {
            highlights.push(Highlight {
                start: node.start_byte(),
                end: node.end_byte(),
                color,
            });
        }

        // Recursively visit children
        let child_count = node.child_count();
        for i in 0..child_count {
            if let Some(child) = node.child(i) {
                self.collect_highlights(&child, highlights, theme);
            }
        }
    }

    pub fn highlight_line(&self, line_num: usize, buffer: &[Vec<char>], theme: &Theme) -> Vec<Highlight> {
        let mut highlights = Vec::new();
        let line_start_byte = self.line_to_byte_index(line_num, buffer);
        let line_end_byte = if line_num + 1 < buffer.len() {
            self.line_to_byte_index(line_num + 1, buffer)
        } else {
            buffer.iter().map(|line| line.len() + 1).sum::<usize>() - 1 // Adjusting for the last line
        };

        if let Some(tree) = &self.tree {
            let root_node = tree.root_node();
            self.traverse_node_for_highlights(root_node, line_start_byte, line_end_byte, &mut highlights, theme);
        }
        highlights
    }

    fn line_to_byte_index(&self, line_num: usize, buffer: &[Vec<char>]) -> usize {
        buffer.iter().take(line_num).map(|line| line.len() + 1).sum()
    }

    fn traverse_node_for_highlights(
        &self, node: tree_sitter::Node,
        line_start_byte: usize,
        line_end_byte: usize,
        highlights: &mut Vec<Highlight>,
        theme: &Theme,
    ) {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        
        if start_byte < line_end_byte && end_byte > line_start_byte {
            if let Some(color) = self.get_color_for_node_kind(node.kind(), theme) {
                let start_pos = std::cmp::max(start_byte, line_start_byte) - line_start_byte;
                let end_pos = std::cmp::min(end_byte, line_end_byte) - line_start_byte;
                
                highlights.push(Highlight {
                    start: start_pos,
                    end: end_pos,
                    color,
                });
            }
            
            let child_count = node.child_count();
            for i in 0..child_count {
                if let Some(child) = node.child(i) {
                    self.traverse_node_for_highlights(child, line_start_byte, line_end_byte, highlights, theme);
                }
            }
        }
    }
}

struct Editor {
    syntax_highlighter: SyntaxHighlighter,
    states: Vec<UndoState>, // History of states
    current_state: usize,
    mode: Mode,
    dired: Option<Dired>,
    cursor_pos: (u16, u16),
    offset: (u16, u16),
    buffer: Vec<Vec<char>>,
    minibuffer_active: bool,
    minibuffer_height: u16,
    minibuffer_content: String,
    minibuffer_prefix: String,
    current_file_path: PathBuf,
    should_open_file: bool,
    fzy: Option<Fzy>,
    messages: Vec<String>,
    last_message_time: Option<std::time::Instant>,
    clipboard: String,
    searching: bool,
    highlight_search: bool,
    search_query: String,
    selection_start: Option<(u16, u16)>,
    selection_end: Option<(u16, u16)>,
    copied_line: bool,
    cursor_blink_state: bool,
    last_cursor_toggle: std::time::Instant,
    force_show_cursor: bool,
    blink_count: u8,
    config: Config,
    lua: Lua,
    keychords: Keychords,
}

impl Editor {
    fn new(config_path: Option<&str>) -> LuaResult<Editor> {

        let lua = Lua::new();
        let config = Config::new(&lua, config_path)?;
        let current_path = env::current_dir().expect("Failed to determine the current directory");


        let initial_undo_state = UndoState {
            buffer: vec![vec![]],
            cursor_pos: (0, 0),
        };

        let syntax_highlighter = SyntaxHighlighter::new();
    
        Ok(Editor {
            syntax_highlighter,
            states: vec![initial_undo_state],
            current_state: 0,
            mode: Mode::Normal,
            cursor_pos: (0, 0),
            offset: (0, 0),
            buffer: vec![vec![]],
            dired: None,
            minibuffer_active: false,
            minibuffer_height: 1,
            minibuffer_content: String::new(),
            minibuffer_prefix: String::new(),
            current_file_path: current_path.clone(),
            should_open_file: false,
            fzy: Some(Fzy::new(current_path)),
            messages: Vec::new(),
            last_message_time: None,
            clipboard: String::new(),
            searching: false,
            highlight_search: false,
            search_query: String::new(),
            selection_start: None,
            selection_end: None,
            copied_line: false,
            cursor_blink_state: true,
            last_cursor_toggle: std::time::Instant::now(),
            force_show_cursor: false,
            blink_count: 0,
            lua,
            config,
            keychords: Keychords::new(),
        })
    }

    pub fn debug_print_ast(&mut self) {
        if let Some(ref tree) = self.syntax_highlighter.tree {
            let tree_string = tree.root_node().to_sexp();
            self.message(&format!("Current AST: {}", tree_string));
        } else {
            self.message("No AST available.");
        }
    }
    
    
    // TODO Don't discard the branches build a tree like emacs does
    // TODO BUG cursor position, Vundo
    fn snapshot(&mut self) {
        let is_different_from_last_snapshot = if let Some(last_state) = self.states.last() {
            last_state.buffer != self.buffer || last_state.cursor_pos != self.cursor_pos
        } else {
            true
        };

        if is_different_from_last_snapshot {
            if self.current_state + 1 < self.states.len() {
                self.states.truncate(self.current_state + 1);
            }
            
            let state = UndoState {
                buffer: self.buffer.clone(),
                cursor_pos: self.cursor_pos,
            };
            
            self.states.push(state);
            self.current_state = self.states.len() - 1;
            // self.message("Snapshot took");
        } else {
            // self.message("Snapshot skipped; cursor_pos unchanged");
        }
    }

    fn undo(&mut self) {
        if self.current_state > 0 {
            self.current_state -= 1;
            let state = &self.states[self.current_state];
            self.buffer = state.buffer.clone();
            self.cursor_pos = state.cursor_pos;
            self.adjust_view_to_cursor("");
            self.message_undo_tree();
        } else {
            self.message("No more undos available.");
        }
    }

    fn redo(&mut self) {
        if self.current_state < self.states.len() - 1 {
            self.current_state += 1;
            let state = &self.states[self.current_state];
            self.buffer = state.buffer.clone();
            self.cursor_pos = state.cursor_pos;
            self.adjust_view_to_cursor("");
            self.message_undo_tree();
        } else {
            self.message("No more redos available.");
        }
    }
    
    fn message_undo_tree(&mut self) {
        let mut display = String::new();
        for i in 0..self.states.len() {
            if i == self.current_state {
                display.push(self.config.current_tree_node);  // Use configured filled node character
            } else {
                display.push(self.config.tree_node);  // Use configured empty node character
            }
            if i < self.states.len() - 1 {
                display.push(self.config.tree_node_separator);  // Use configured separator character
            }
        }
        self.message(&display);
    }

    fn eval(&mut self, code: &str) -> std::result::Result<(), String> {
        match self.lua.load(code).exec() {
            Ok(_) => {
                let globals = self.lua.globals();
                self.config.blink_cursor = globals.get("Blink_cursor").unwrap_or(self.config.blink_cursor);
                self.config.show_fringe = globals.get("Show_fringe").unwrap_or(self.config.show_fringe);
                self.config.show_line_numbers = globals.get("Show_line_numbers").unwrap_or(self.config.show_line_numbers);
                self.config.insert_line_cursor = globals.get("Insert_line_cursor").unwrap_or(self.config.insert_line_cursor);
                self.config.show_hl_line = globals.get("Show_hl_line").unwrap_or(self.config.show_hl_line);
                self.config.top_scroll_margin = globals.get("Top_scroll_margin").unwrap_or(self.config.top_scroll_margin);
                self.config.bottom_scroll_margin = globals.get("Bottom_scroll_margin").unwrap_or(self.config.bottom_scroll_margin);
                self.config.blink_limit = globals.get("Blink_limit").unwrap_or(self.config.blink_limit);
                self.config.indentation = globals.get("Indentation").unwrap_or(self.config.indentation);
                self.config.electric_pair_mode = globals.get("Electric_pair_mode").unwrap_or(self.config.electric_pair_mode);

                self.config.tree_node = globals.get::<_, String>("Tree_node")
                    .map(|s| s.chars().next().unwrap_or(self.config.tree_node))
                    .unwrap_or(self.config.tree_node);

                self.config.current_tree_node = globals.get::<_, String>("Current_tree_node")
                    .map(|s| s.chars().next().unwrap_or(self.config.current_tree_node))
                    .unwrap_or(self.config.current_tree_node);

                self.config.tree_node_separator = globals.get::<_, String>("Tree_node_separator")
                    .map(|s| s.chars().next().unwrap_or(self.config.tree_node_separator))
                    .unwrap_or(self.config.tree_node_separator);

                self.config.modeline_separator_right = globals.get::<_, String>("Modeline_separator_right")
                    .map(|s| s.chars().next().unwrap_or(self.config.modeline_separator_right))
                    .unwrap_or(self.config.modeline_separator_right);
                
                self.config.modeline_separator_left = globals.get::<_, String>("Modeline_separator_left")
                    .map(|s| s.chars().next().unwrap_or(self.config.modeline_separator_left))
                    .unwrap_or(self.config.modeline_separator_left);



                // self.config.current_theme_name = globals.get("Theme").unwrap_or(self.config.current_theme_name.clone()); // TODO BUG
                
                
                // TODO This should be extracted into a function for code organization
                // but the borrow checker don't like it so ill do it later
                let theme_field_to_lua_var = vec![
                    ("background_color", "Background_color"),
                    ("text_color", "Text_color"),
                    ("normal_cursor_color", "Normal_cursor_color"),
                    ("insert_cursor_color", "Insert_cursor_color"),
                    ("fringe_color", "Fringe_color"),
                    ("line_numbers_color", "Line_numbers_color"),
                    ("current_line_number_color", "Current_line_number_color"),
                    ("modeline_color", "Modeline_color"),
                    ("mimibuffer_color", "Minibuffer_color"),
                    ("dired_mode_color", "Dired_mode_color"),
                    ("dired_timestamp_color", "Dired_timestamp_color"),
                    ("dired_path_color", "Dired_path_color"),
                    ("dired_size_color", "Dired_size_color"),
                    ("dired_dir_color", "Dired_dir_color"),
                    ("comment_color", "Comment_color"),
                    ("warning_color", "Warning_color"),
                    ("error_color", "Error_color"),
                    ("ok_color", "Ok_color"),
                    ("search_bg_color", "Search_bg_color"),
                    ("visual_mode_color", "Visual_mode_color"),
                    ("selection_color", "Selection_color"),
                    ("hl_line_color", "Hl_line_color"),
                ];

                if let Some(theme) = self.config.themes.get_mut(&self.config.current_theme_name) {
                    for (field, lua_var) in theme_field_to_lua_var.iter() {
                        if let Ok(color_str) = globals.get::<_, String>(*lua_var) {
                            if let Ok(color) = hex_to_rgb(&color_str) {
                                match *field {
                                    "background_color" => theme.background_color = color,
                                    "text_color" => theme.text_color = color,
                                    "normal_cursor_color" => theme.normal_cursor_color = color,
                                    "insert_cursor_color" => theme.insert_cursor_color = color,
                                    "fringe_color" => theme.fringe_color = color,
                                    "line_numbers_color" => theme.line_numbers_color = color,
                                    "current_line_number_color" => theme.current_line_number_color = color,
                                    "modeline_color" => theme.modeline_color = color,
                                    "modeline_lighter_color" => theme.modeline_lighter_color = color,
                                    "minibuffer_color" => theme.minibuffer_color = color,
                                    "dired_mode_color" => theme.dired_mode_color = color,
                                    "dired_timestamp_color" => theme.dired_timestamp_color = color,
                                    "dired_path_color" => theme.dired_path_color = color,
                                    "dired_dir_color" => theme.dired_dir_color = color,
                                    "comment_color" => theme.comment_color = color,
                                    "warning_color" => theme.warning_color = color,
                                    "error_color" => theme.error_color = color,
                                    "ok_color" => theme.ok_color = color,
                                    "search_bg_color" => theme.search_bg_color = color,
                                    "visual_mode_color" => theme.visual_mode_color = color,
                                    "selection_color" => theme.selection_color = color,
                                    "hl_line_color" => theme.hl_line_color = color,
                                    _ => (),
                                }
                            }
                        }
                    }
                }

                Ok(())
            },

            Err(e) => Err(format!("Lua error: {}", e)),
        }
    }
    
    pub fn eval_buffer(&mut self) {
        let buffer_content = self.buffer.iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<String>>().join("\n");

        match self.eval(&buffer_content) {
            Ok(_) => {},
            Err(err_msg) => self.message(&err_msg),
        }
    }

    pub fn eval_region(&mut self) -> std::result::Result<(), String> {
        let selected_text = self.extract_selected_text();
        if !selected_text.is_empty() {
            self.eval(&selected_text)?;
        } else {
            return Err("No text selected.".to_string());
        }
        Ok(())
    }

    pub fn eval_line(&mut self) {
        let current_line_idx = self.cursor_pos.1 as usize;

        if let Some(line) = self.buffer.get(current_line_idx) {
            // Convert the current line's characters to a String
            let line_content = line.iter().collect::<String>();
            match self.eval(&line_content) {
                Ok(_) => self.message("Line executed successfully."),
                Err(err_msg) => self.message(&err_msg),
            }
        }
    }
    
    fn current_theme(&self) -> &Theme {
        self.config.themes.get(&self.config.current_theme_name).expect("Current theme not found")
    }

    fn switch_theme(&mut self, theme_name: &str) {
        if self.config.themes.contains_key(theme_name) {
            self.config.current_theme_name = theme_name.to_string();
        } else {
            self.message("Theme doesn't exist");
        }
    }
    
    fn quit(&self) {
        let mut stdout = stdout();
        disable_raw_mode().expect("Failed to disable raw mode");
        execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show).expect("Failed to leave alternate screen");
        std::process::exit(0);
    }

    pub fn buffer_save(&mut self) -> Result<()> {
        let content: String = self.buffer.iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<String>>()
            .join("\n");

        // Attempt to write the buffer to the file and handle the result
        match fs::write(&self.current_file_path, content) {
            Ok(_) => {
                // Display a success message with the path of the file saved
                let message = format!("Wrote {}", self.current_file_path.display());
                self.message(&message);
                Ok(())
            },
            Err(e) => {
                // Handle the error by displaying an error message
                let error_message = format!("Failed to save file: {}", e);
                self.message(&error_message);
                Err(e) // Propagate the error
            },
        }
    }

    fn enter(&mut self) {
        let tail = self.buffer[self.cursor_pos.1 as usize].split_off(self.cursor_pos.0 as usize);
        self.buffer.insert(self.cursor_pos.1 as usize + 1, tail);
        self.cursor_pos.1 += 1;
        self.cursor_pos.0 = 0;

        let (_, height) = terminal::size().unwrap();
        let text_area_height = height - self.minibuffer_height - 1; // -1 for modeline

        let effective_text_area_height = text_area_height - self.config.bottom_scroll_margin;

        if self.cursor_pos.1 >= self.offset.1 + effective_text_area_height {
            let max_offset_possible = if self.buffer.len() as u16 > text_area_height {
                self.buffer.len() as u16 - text_area_height
            } else {
                0 // No scrolling needed if the document is shorter than the viewport
            };

            if self.offset.1 < max_offset_possible {
                self.offset.1 = (self.offset.1 + 1).min(max_offset_possible); // Safely increment offset
            }
        }
    }

    fn backspace(&mut self) {
        if self.cursor_pos.0 > 0 {
            let line_index = self.cursor_pos.1 as usize;
            let char_index = (self.cursor_pos.0 - 1) as usize;

            // Check if electric pair mode is enabled and handle paired deletion
            if self.config.electric_pair_mode &&
                char_index < self.buffer[line_index].len() - 1 && // Ensure there's a character after the one to be deleted
                matches!(self.buffer[line_index][char_index], '{' | '[' | '(' | '"' | '\'') &&
                ((self.buffer[line_index][char_index] == '{' && self.buffer[line_index][char_index + 1] == '}') ||
                 (self.buffer[line_index][char_index] == '[' && self.buffer[line_index][char_index + 1] == ']') ||
                 (self.buffer[line_index][char_index] == '(' && self.buffer[line_index][char_index + 1] == ')') ||
                 (self.buffer[line_index][char_index] == '"' && self.buffer[line_index][char_index + 1] == '"') ||
                 (self.buffer[line_index][char_index] == '\'' && self.buffer[line_index][char_index + 1] == '\'')) {
                    
                    // Remove both the opening and the closing characters
                    self.buffer[line_index].remove(char_index + 1); // Remove the closing first to keep indices correct
                    self.buffer[line_index].remove(char_index); // Now remove the opening
                    self.cursor_pos.0 -= 1; // Adjust the cursor position
                } else {
                    // Normal backspace operation
                    self.buffer[line_index].remove(char_index);
                    self.cursor_pos.0 -= 1;
                }
        } else if self.cursor_pos.1 > 0 {
            // Handle removing an entire line and moving up
            self.cursor_pos.1 -= 1;
            self.cursor_pos.0 = self.buffer[self.cursor_pos.1 as usize].len() as u16;
            let current_line = self.buffer.remove(self.cursor_pos.1 as usize);
            self.buffer[self.cursor_pos.1 as usize].extend(current_line);
        }
    }



    fn delete_char(&mut self) {
        if !self.buffer[self.cursor_pos.1 as usize].is_empty() {
            if self.cursor_pos.0 < self.buffer[self.cursor_pos.1 as usize].len() as u16 {
                let removed_char = self.buffer[self.cursor_pos.1 as usize].remove(self.cursor_pos.0 as usize);
                self.clipboard = removed_char.to_string();
            }
        } else if self.buffer.len() > 1 {
            self.buffer.remove(self.cursor_pos.1 as usize);
            // An empty line is removed
            self.clipboard.clear();
        }
    }

    fn back_to_indentation(&mut self) {
        if let Some(line) = self.buffer.get(self.cursor_pos.1 as usize) {
            let first_non_blank_index = line.iter()
                .position(|&c| c != ' ' && c != '\t')
                .unwrap_or(0) as u16;

            self.cursor_pos.0 = first_non_blank_index;
        }
    }

    fn up(&mut self) {
        if self.cursor_pos.1 > 0 {
            self.cursor_pos.1 -= 1;

            // Adjust cursor position if it exceeds the length of the new line
            let prev_line_len = self.buffer[self.cursor_pos.1 as usize].len() as u16;
            if self.cursor_pos.0 > prev_line_len {
                self.cursor_pos.0 = prev_line_len;
            }

            // Top scroll margin
            if self.cursor_pos.1 < self.offset.1 + self.config.top_scroll_margin && self.offset.1 > 0 {
                self.offset.1 -= 1;
            }
        }
    }

    fn down(&mut self) {
        let (_, height) = terminal::size().unwrap();
        let text_area_height = height - self.minibuffer_height - 1; // -1 for modeline

        if self.cursor_pos.1 < self.buffer.len() as u16 - 1 {
            self.cursor_pos.1 += 1;

            // Adjust cursor position if it exceeds the length of the next line
            let next_line_len = self.buffer[self.cursor_pos.1 as usize].len() as u16;
            if self.cursor_pos.0 > next_line_len {
                self.cursor_pos.0 = next_line_len;
            }

            let effective_text_area_height = text_area_height - self.config.bottom_scroll_margin;
            // Adjust offset if cursor moves beyond the last line of the effective text area
            if self.cursor_pos.1 >= self.offset.1 + effective_text_area_height {
                // Only scroll if not at the last document line
                if self.offset.1 < self.buffer.len() as u16 - text_area_height {
                    self.offset.1 += 1;
                }
            }
        }
    }
    
    fn left(&mut self) {
        if self.cursor_pos.0 > 0 {
            self.cursor_pos.0 -= 1;
        }
    }

    fn right(&mut self) {
        if self.cursor_pos.0 < self.buffer[self.cursor_pos.1 as usize].len() as u16 {
            self.cursor_pos.0 += 1;
        }
    }



    fn indent(&mut self) {
        let cursor_row = self.cursor_pos.1 as usize;
        let mut brace_level = 0;

        // Calculate the brace level up to the current line
        for line in self.buffer.iter().take(cursor_row + 1) {
            for &c in line.iter() {
                if c == '{' {
                    brace_level += 1;
                } else if c == '}' {
                    if brace_level > 0 {
                        brace_level -= 1; // Safely decrement brace level
                    }
                }
            }
        }

        let current_line = &self.buffer[cursor_row];
        let first_non_whitespace = current_line.iter().position(|&c| !c.is_whitespace()).unwrap_or(current_line.len());
        let decrease_indent = current_line.get(first_non_whitespace).map_or(false, |&c| c == '}');
        if decrease_indent && brace_level > 0 {
            brace_level -= 1; // Decrease brace level if the line starts with a '}'
        }

        let required_indentation = brace_level * self.config.indentation;

        // Create a new line with the correct indentation followed by the rest of the line after initial whitespace
        let mut new_line = vec![' '; required_indentation];
        new_line.extend_from_slice(&current_line[first_non_whitespace..]);

        // Replace the old line with the new one
        self.buffer[cursor_row] = new_line;

        // Move the cursor to the first non-whitespace character on the line
        self.cursor_pos.0 = required_indentation as u16;
    }

    fn adjust_view_to_cursor(&mut self, adjustment: &str) {
        let (_, height) = terminal::size().unwrap();
        let text_area_height = height - self.minibuffer_height - 1;

        match adjustment {
            "center" => {
                let new_offset = self.cursor_pos.1.saturating_sub(text_area_height / 2);
                self.offset.1 = new_offset;
            },
            "enough" => {
                if self.cursor_pos.1 < self.offset.1 {
                    self.offset.1 = self.cursor_pos.1;
                } else if self.cursor_pos.1 >= (self.offset.1 + text_area_height) {
                    self.offset.1 = self.cursor_pos.1.saturating_sub(text_area_height) + 1;
                }
            },
            _ => {
                // Default behavior respecting top and bottom scroll margins
                if self.cursor_pos.1 < self.offset.1 + self.config.top_scroll_margin {
                    self.offset.1 = self.cursor_pos.1.saturating_sub(self.config.top_scroll_margin);
                } else if self.cursor_pos.1 + self.config.bottom_scroll_margin >= self.offset.1 + text_area_height {
                    self.offset.1 = self.cursor_pos.1 + self.config.bottom_scroll_margin + 1 - text_area_height;
                }
            }
        }
    }

    fn goto_line(&mut self, line_number: usize) {
        if line_number == 0 || line_number > self.buffer.len() {
            self.message(&format!("Line number {} doesn't exist.", line_number));
            return;
        }

        self.cursor_pos.1 = (line_number - 1) as u16; // Convert to 0-based index
        self.cursor_pos.0 = 0;

        self.adjust_view_to_cursor("center");
    }

    fn search_next(&mut self) {
        let mut found = false;
        let mut wrapped_around = false;
        let (orig_line, orig_col) = (self.cursor_pos.1 as usize, self.cursor_pos.0 as usize);
        let mut line_idx = orig_line;
        let mut col_idx = orig_col + 1;

        loop {
            if line_idx >= self.buffer.len() {
                // Wrap to the beginning of the document
                line_idx = 0;
                col_idx = 0;
                wrapped_around = true;
            }

            if let Some(line) = self.buffer.get(line_idx) {
                if let Some(match_idx) = line.iter().skip(col_idx).collect::<String>().find(&self.search_query) {
                    self.cursor_pos = (match_idx as u16 + col_idx as u16, line_idx as u16);
                    found = true;
                    break;
                }
            }

            // Move to the next line from the start
            line_idx += 1;
            col_idx = 0;

            // Stop if we've wrapped around to the original position
            if line_idx == orig_line && col_idx >= orig_col {
                break;
            }
        }

        if !found {
            self.message("Search query not found.");
        } else {
            self.adjust_view_to_cursor(if wrapped_around { "center" } else { "" });
        }
    }

    fn search_previous(&mut self) {
        let mut found = false;
        let mut wrapped_around = false;
        let (orig_line, orig_col) = (self.cursor_pos.1 as usize, if self.cursor_pos.0 > 0 { self.cursor_pos.0 as usize - 1 } else { usize::MAX });
        let mut line_idx = if orig_line == 0 { self.buffer.len() - 1 } else { orig_line - 1 };
        let mut col_idx = if orig_col == usize::MAX { self.buffer.get(line_idx).map_or(0, |l| l.len()) } else { orig_col };

        loop {
            if let Some(line) = self.buffer.get(line_idx) {
                let search_str: String = line.iter().take(col_idx).collect();
                if let Some(match_idx) = search_str.rfind(&self.search_query) {
                    self.cursor_pos = (match_idx as u16, line_idx as u16);
                    found = true;
                    break;
                }
            }

            if line_idx == 0 {
                // Wrap to the end of the document
                line_idx = self.buffer.len() - 1;
                col_idx = self.buffer.get(line_idx).map_or(0, |l| l.len());
                wrapped_around = true;
            } else {
                line_idx -= 1;
                col_idx = self.buffer.get(line_idx).map_or(0, |l| l.len());
            }

            // Stop if we've wrapped around to the original position
            if line_idx == orig_line && col_idx <= orig_col {
                break;
            }
        }

        if !found {
            self.message("Search query not found.");
        } else {
            self.adjust_view_to_cursor(if wrapped_around { "center" } else { "" });
        }
    }

    fn extract_selected_text(&self) -> String {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (start_line, mut start_col) = (start.1 as usize, start.0 as usize);
            let (end_line, mut end_col) = (end.1 as usize, end.0 as usize);

            // Ensure start_col is always less than or equal to end_col
            if start_line == end_line && start_col > end_col {
                std::mem::swap(&mut start_col, &mut end_col);
            }

            if start_line == end_line {
                let line = &self.buffer[start_line];
                line[start_col..=end_col].iter().collect()
            } else {
                let mut text = String::new();
                // Extract from the start line
                text.push_str(&self.buffer[start_line][start_col..].iter().collect::<String>());
                // Extract from the middle lines
                for line in &self.buffer[start_line + 1..end_line] {
                    text.push('\n');
                    text.push_str(&line.iter().collect::<String>());
                }
                // Ensure end_col is within the bounds of the last line
                end_col = end_col.min(self.buffer[end_line].len() - 1);
                // Extract from the end line
                text.push('\n');
                text.push_str(&self.buffer[end_line][..=end_col].iter().collect::<String>());
                text
            }
        } else {
            String::new()
        }
    }
    
    fn delete_selection(&mut self) {
        if let (Some(mut start), Some(mut end)) = (self.selection_start, self.selection_end) {
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }

            if (end.0 as usize) < self.buffer[end.1 as usize].len() {
                end.0 += 1;
            } else if (end.1 as usize) + 1 < self.buffer.len() {
                end.1 += 1;
                end.0 = 0;
            }

            let mut deleted_text = String::new();

            if start.1 == end.1 {
                let line = &mut self.buffer[start.1 as usize];
                if (start.0 as usize) < line.len() {
                    deleted_text = line.drain((start.0 as usize)..(end.0 as usize)).collect();
                }
            } else {
                let start_line = &mut self.buffer[start.1 as usize];
                deleted_text.extend(start_line.drain((start.0 as usize)..));

                for line_idx in ((start.1 as usize + 1)..(end.1 as usize)).rev() {
                    deleted_text.push('\n');
                    deleted_text.extend(self.buffer.remove(line_idx));
                }

                if (end.0 > 0) && ((start.1 as usize) + 1 == end.1 as usize) {
                    let end_line = &mut self.buffer[start.1 as usize + 1];
                    deleted_text.push('\n');
                    deleted_text.extend(end_line.drain(..(end.0 as usize)));
                }

                if start.1 != end.1 && !self.buffer[start.1 as usize].is_empty() && !self.buffer[start.1 as usize + 1].is_empty() {
                    let remaining = self.buffer.remove(start.1 as usize + 1);
                    self.buffer[start.1 as usize].extend(remaining);
                }
            }

            self.clipboard = deleted_text;
            self.cursor_pos = start;
            self.selection_start = None;
            self.selection_end = None;
        }
    }

    fn copy_selection(&mut self) {
        if let (Some(mut start), Some(mut end)) = (self.selection_start, self.selection_end) {
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }

            if end.0 < self.buffer[end.1 as usize].len() as u16 {
                end.0 += 1; // Include the character at the end position
            } else if end.1 < self.buffer.len() as u16 - 1 {
                // If at the end of the line, but not the last line, move to the start of the next line
                end.1 += 1;
                end.0 = 0;
            }

            let mut selected_text = String::new();

            for line_idx in start.1..=end.1 {
                let line = &self.buffer[line_idx as usize];
                
                if start.1 == end.1 {
                    // If selection is within a single line
                    selected_text.push_str(&line[start.0 as usize..end.0 as usize].iter().collect::<String>());
                } else {
                    // If selection spans multiple lines
                    if line_idx == start.1 {
                        // First line
                        selected_text.push_str(&line[start.0 as usize..].iter().collect::<String>());
                        selected_text.push('\n');
                    } else if line_idx == end.1 {
                        // Last line, adjust to not include the newline if end.0 is 0
                        if end.0 > 0 {
                            selected_text.push_str(&line[..(end.0 as usize).saturating_sub(1)].iter().collect::<String>());
                        }
                    } else {
                        // Lines in between
                        selected_text.push_str(&line.iter().collect::<String>());
                        selected_text.push('\n');
                    }
                }
            }

            self.copied_line = false;
            self.clipboard = selected_text;
            self.cursor_pos = start;
            self.selection_start = None;
            self.selection_end = None;
        }
    }


    
    fn join(&mut self) {
        if self.cursor_pos.1 as usize + 1 < self.buffer.len() {
            let next_line = self.buffer.remove(self.cursor_pos.1 as usize + 1);

            if let Some(current_line) = self.buffer.get_mut(self.cursor_pos.1 as usize) {
                // Determine if the current line ends with a non-whitespace character
                let ends_with_non_whitespace = current_line.iter().rev().find(|&&c| c != ' ').is_some();
                
                // Create a trimmed version of the next line (remove leading whitespace)
                let trimmed_next_line: Vec<char> = next_line.into_iter().skip_while(|&c| c == ' ').collect();

                // ensure a single space is added between the lines.
                if ends_with_non_whitespace && !trimmed_next_line.is_empty() {
                    current_line.push(' ');
                }

                // Extend the current line with the trimmed next line
                current_line.extend(trimmed_next_line);
            }
        }
    }


    pub fn mwim_beginning(&mut self) {
        let line = &self.buffer[self.cursor_pos.1 as usize];
        let first_non_whitespace = line.iter()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0) as u16;

        if self.cursor_pos.0 == first_non_whitespace {
            self.cursor_pos.0 = 0;
        } else {
            self.cursor_pos.0 = first_non_whitespace;
        }
        self.adjust_view_to_cursor("center");
    }

    pub fn mwim_end(&mut self) {
        let line = &self.buffer[self.cursor_pos.1 as usize];
        let last_non_whitespace = line.iter()
            .rposition(|c| !c.is_whitespace())
            .map_or(line.len(), |pos| pos + 1) as u16;

        if self.cursor_pos.0 == last_non_whitespace {
            self.cursor_pos.0 = line.len() as u16;
        } else {
            self.cursor_pos.0 = last_non_whitespace;
        }
        self.adjust_view_to_cursor("center");
    }



    
    fn open_below(&mut self) {
        if let Some(current_line) = self.buffer.get(self.cursor_pos.1 as usize) {
            let indentation = current_line.iter().take_while(|&&c| c == ' ').count();
            let new_line_indentation = vec![' '; indentation];
            let new_line_index = self.cursor_pos.1 as usize + 1;
            self.buffer.insert(new_line_index, new_line_indentation);
        } else {
            // If for some reason we're beyond the buffer, just insert a blank new line
            self.buffer.insert(self.cursor_pos.1 as usize + 1, Vec::new());
        }

        self.cursor_pos.1 += 1;
        self.cursor_pos.0 = self.buffer[self.cursor_pos.1 as usize].len() as u16; // Move cursor to the end of the indentation
        self.mode = Mode::Insert;
    }

    fn open_above(&mut self) {
        let indentation = if let Some(current_line) = self.buffer.get(self.cursor_pos.1 as usize) {
            current_line.iter().take_while(|&&c| c == ' ').count()
        } else {
            0 // Default to no indentation
        };
        let new_line_indentation = vec![' '; indentation];
        self.buffer.insert(self.cursor_pos.1 as usize, new_line_indentation);
        self.cursor_pos.0 = indentation as u16;
        self.mode = Mode::Insert;
    }

    fn kill_line(&mut self) {
        if let Some(line) = self.buffer.get_mut(self.cursor_pos.1 as usize) {
            if line.len() > self.cursor_pos.0 as usize {
                // Remove text from the cursor to the end of the line and store it in the clipboard
                let removed_text: String = line.drain(self.cursor_pos.0 as usize..).collect();
                self.clipboard = removed_text; // Replace the clipboard content
            } else {
                // If the cursor is at the end of the line or the line is empty, remove the line
                if self.cursor_pos.1 as usize != self.buffer.len() - 1 {
                    self.buffer.remove(self.cursor_pos.1 as usize);
                    // Adding an empty string to the clipboard
                    self.clipboard.clear();
                }
            }
        }
    }

    fn paste(&mut self, position: &str) {
        if self.copied_line {
            let new_line: Vec<char> = self.clipboard.chars().collect();
            let line_index = match position {
                "after" => self.cursor_pos.1 as usize + 1, // Paste after the current line
                _ => self.cursor_pos.1 as usize,           // Default to pasting before if not explicitly "after"
            };
            self.buffer.insert(line_index, new_line);
            self.cursor_pos.1 = line_index as u16;
            self.cursor_pos.0 = 0;
        } else {
            // Adjust for pasting before or after within a line.
            let line_len = self.buffer[self.cursor_pos.1 as usize].len();
            let paste_position = if position == "after" && self.cursor_pos.0 as usize == line_len {
                // When the cursor is at the end of the line.
                line_len // Use line length directly to append at the end.
            } else if position == "after" {
                self.cursor_pos.0 as usize + 1
            } else {
                self.cursor_pos.0 as usize
            };

            if let Some(line) = self.buffer.get_mut(self.cursor_pos.1 as usize) {
                if position == "after" && self.cursor_pos.0 as usize == line_len {
                    // Directly extend the line if pasting after at the line's end.
                    let clipboard_content: Vec<char> = self.clipboard.chars().collect();
                    line.extend(clipboard_content);
                } else {
                    let rest_of_line: String = line.drain(paste_position..).collect();
                    let clipboard_content: Vec<char> = self.clipboard.chars().collect();
                    line.extend(clipboard_content);
                    line.extend(rest_of_line.chars());
                }
                self.cursor_pos.0 = if position == "after" && self.cursor_pos.0 as usize == line_len {
                    (line_len + self.clipboard.chars().count()) as u16
                } else {
                    paste_position as u16 + self.clipboard.chars().count() as u16
                };
            }
        }

        // Scroll logic to ensure the cursor is visible after pasting.
        let (_, height) = terminal::size().unwrap();
        let text_area_height = height - self.minibuffer_height - 1;
        let effective_text_area_height = text_area_height - self.config.bottom_scroll_margin;
        if self.cursor_pos.1 >= self.offset.1 + effective_text_area_height {
            let max_offset_possible = if self.buffer.len() as u16 > text_area_height {
                self.buffer.len() as u16 - text_area_height
            } else {
                0
            };
            if self.offset.1 < max_offset_possible {
                self.offset.1 = (self.offset.1 + 1).min(max_offset_possible);
            }
        }
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

    fn draw_selection(&self, stdout: &mut io::Stdout) -> Result<()> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let selection_color = self.current_theme().selection_color;

            // Ensure start is before end
            let (start, end) = if start > end { (end, start) } else { (start, end) };

            // Calculate the starting column base considering fringe and line numbers
            let mut start_col_base = 0;
            if self.config.show_fringe {
                start_col_base += 2; // Account for fringe width
            }
            if self.config.show_line_numbers {
                start_col_base += 4; // Account for space for line numbers
            }

            for line_idx in start.1..=end.1 {
                let line_y = line_idx.saturating_sub(self.offset.1) as u16; // Adjust Y position based on the current offset
                if let Some(line) = self.buffer.get(line_idx as usize) {
                    let start_col = if line_idx == start.1 { start.0 as usize } else { 0 };
                    let end_col = if line_idx == end.1 { end.0 as usize + 1 } else { line.len() }; // +1 to include the end character, ensure it does not exceed line length

                    // Convert the line segment to a String
                    let line_content: String = line.iter().collect::<String>();
                    let selection_content = &line_content[start_col..end_col];

                    // Calculate the X position for the start of the selection
                    let selection_start_x = start_col_base + start_col as u16;

                    // Draw the selection background for the selected text
                    execute!(
                        stdout,
                        MoveTo(selection_start_x, line_y),
                        SetBackgroundColor(selection_color),
                        Print(selection_content),
                        ResetColor
                    )?;
                }
            }
        }

        Ok(())
    }

    fn draw_cursor(&mut self, stdout: &mut Stdout) -> Result<()> {
        let (width, height) = terminal::size()?;

        let cursor_pos = if self.minibuffer_active {
            let minibuffer_cursor_pos_x = 1 + self.minibuffer_prefix.len() as u16 + self.minibuffer_content.len() as u16;
            let minibuffer_cursor_pos_y = height - self.minibuffer_height;
            (minibuffer_cursor_pos_x, minibuffer_cursor_pos_y)
        } else if self.fzy.as_ref().map_or(false, |fzy| fzy.active) {
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
            if self.config.show_fringe {
                start_col += 2;
            }
            if self.config.show_line_numbers {
                start_col += 4;
            }
            let cursor_x = self.cursor_pos.0.saturating_sub(self.offset.0) + start_col;
            let cursor_y = (self.cursor_pos.1.saturating_sub(self.offset.1)).min(height - self.minibuffer_height - 2);
            (cursor_x, cursor_y)
        };

        if self.config.blink_cursor && (self.blink_count / 2 < self.config.blink_limit || self.force_show_cursor) {
            let now = std::time::Instant::now();
            if now.duration_since(self.last_cursor_toggle) >= Duration::from_millis(530) {
                self.cursor_blink_state = !self.cursor_blink_state;
                self.last_cursor_toggle = now;
                if !self.force_show_cursor {
                    self.blink_count += 1;
                }
            }

            if self.cursor_blink_state || self.force_show_cursor {
                execute!(stdout, cursor::MoveTo(cursor_pos.0, cursor_pos.1), cursor::Show)?;
            } else {
                execute!(stdout, cursor::Hide)?;
            }
        } else {
            // Always show the cursor if blinking is disabled or limit reached without force_show_cursor.
            execute!(stdout, cursor::MoveTo(cursor_pos.0, cursor_pos.1), cursor::Show)?;
        }

        // Reset force_show_cursor
        if self.force_show_cursor {
            self.force_show_cursor = false;
            self.blink_count = 0;
            // self.last_cursor_toggle = Instant::now();
        }

        Ok(())

    }

    fn draw(&mut self, stdout: &mut Stdout) -> Result<()> {
        let (width, height) = terminal::size()?;
        let background_color = self.current_theme().background_color;

        execute!(
            stdout,
            SetBackgroundColor(background_color),
            terminal::Clear(ClearType::All)
        )?;



        self.draw_minibuffer(stdout, width, height)?;
        
        // Draw text area for non-Dired modes
        if self.mode != Mode::Dired {
            let mut start_col = 0;
            if self.config.show_fringe {
                self.draw_fringe(stdout, height)?;
                start_col += 2;
            }
            if self.config.show_line_numbers {
                self.draw_line_numbers(stdout, height, start_col)?;
            }

            self.draw_text(stdout)?;
            self.draw_hl_line(stdout)?;
            self.draw_search_highlight(stdout)?;
            self.draw_selection(stdout)?;
        }

        self.draw_modeline(stdout, width, height)?;


        
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


        
        io::stdout().flush()?;
        Ok(())
    }

    // ORIGINAL
    fn draw_text(&self, stdout: &mut io::Stdout) -> Result<()> {
        let (width, height) = terminal::size()?;
        let text_color = self.current_theme().text_color;
        let background_color = self.current_theme().background_color;
        let mut start_col_base = 0;

        if self.config.show_fringe {
            start_col_base += 2;
        }

        if self.config.show_line_numbers {
            start_col_base += 4;
        }

        let bottom_exclude = self.minibuffer_height + 1;
        let effective_width = width.saturating_sub(start_col_base);

        for (idx, line) in self.buffer.iter().enumerate() {
            if idx >= self.offset.1 as usize && idx < (self.offset.1 + height - bottom_exclude) as usize {
                let line_y = (idx - self.offset.1 as usize) as u16;
                let line_content: String = line.iter().collect::<String>();
                let truncated_line_content = if line_content.chars().count() as u16 > effective_width {
                    // If the line exceeds the effective width, truncate it
                    line_content.chars().take(effective_width as usize).collect::<String>()
                } else {
                    line_content
                };

                execute!(
                    stdout,
                    MoveTo(start_col_base, line_y),
                    SetForegroundColor(text_color),
                    SetBackgroundColor(background_color),
                    Print(truncated_line_content)
                )?;
            }
        }

        Ok(())
    }


    // // Still flicker
    // fn draw_text(&self, stdout: &mut io::Stdout) -> Result<()> {
    //     let (width, height) = crossterm::terminal::size()?;
    //     let text_color = self.current_theme().text_color;
    //     let background_color = self.current_theme().background_color;
    //     let bottom_exclude = self.minibuffer_height + 1;
    //     let visible_lines_range = self.offset.1 as usize..(self.offset.1 + height - bottom_exclude) as usize;

    //     let mut start_col_base = 0;
    //     if self.config.show_fringe { start_col_base += 2; }
    //     if self.config.show_line_numbers { start_col_base += 4; }

    //     for line_idx in visible_lines_range {
    //         let line = match self.buffer.get(line_idx) {
    //             Some(line) => line,
    //             None => continue,
    //         };
    //         let line_y = (line_idx as u16).saturating_sub(self.offset.1);
    //         let line_highlights = self.syntax_highlighter.highlight_line(line_idx, &self.buffer, self.current_theme());

    //         // Draw each character
    //         for (char_idx, char) in line.iter().enumerate() {
    //             let char_color = line_highlights.iter()
    //                 .find(|highlight| char_idx >= highlight.start && char_idx < highlight.end)
    //                 .map_or(text_color, |highlight| highlight.color);

    //             // Calculate actual column considering the base offset and character index
    //             let current_col = start_col_base + char_idx as u16;

    //             // Ensure we do not overflow the terminal width
    //             if current_col >= width {
    //                 break;
    //             }

    //             // Draw the character with its corresponding highlight
    //             crossterm::execute!(
    //                 stdout,
    //                 crossterm::cursor::MoveTo(current_col, line_y),
    //                 crossterm::style::SetForegroundColor(char_color),
    //                 crossterm::style::Print(*char),
    //                 crossterm::style::SetBackgroundColor(background_color)
    //             )?;
    //         }

    //         // Fill the rest of the line with the background color, if needed
    //         if let Some(last_char_col) = line.iter().enumerate().last().map(|(idx, _)| start_col_base + idx as u16 + 1) {
    //             if last_char_col < width {
    //                 crossterm::execute!(
    //                     stdout,
    //                     crossterm::cursor::MoveTo(last_char_col, line_y),
    //                     crossterm::style::SetBackgroundColor(background_color),
    //                     crossterm::style::Print(" ".repeat((width - last_char_col) as usize))
    //                 )?;
    //             }
    //         }
    //     }

    //     // Reset terminal colors to defaults after drawing
    //     crossterm::execute!(
    //         stdout,
    //         crossterm::style::SetForegroundColor(text_color),
    //         crossterm::style::SetBackgroundColor(background_color),
    //         crossterm::style::ResetColor
    //     )?;

    //     Ok(())
    // }



        
    fn draw_search_highlight(&self, stdout: &mut io::Stdout) -> Result<()> {
        let (width, height) = size()?;
        let text_color = self.current_theme().text_color;
        let search_bg_color = self.current_theme().search_bg_color;
        let background_color = self.current_theme().background_color;
        let mut start_col_base = 0;

        if self.config.show_fringe {
            start_col_base += 2;
        }

        if self.config.show_line_numbers {
            start_col_base += 4;
        }

        let bottom_exclude = self.minibuffer_height + 1;
        let search_string = if self.minibuffer_active {
            &self.minibuffer_content
        } else {
            &self.search_query
        };

        if self.highlight_search && !search_string.is_empty() {
            for (idx, line) in self.buffer.iter().enumerate() {
                if idx >= self.offset.1 as usize && idx < (self.offset.1 + height - bottom_exclude) as usize {
                    let line_content: String = line.iter().collect();
                    let line_y = (idx - self.offset.1 as usize) as u16;
                    let mut current_col = start_col_base;
                    let mut last_index = 0;

                    for (start, part) in line_content.match_indices(search_string) {
                        let preceding_text = &line_content[last_index..start];
                        execute!(stdout, MoveTo(current_col, line_y), SetForegroundColor(text_color), SetBackgroundColor(background_color), Print(preceding_text))?;
                        current_col += preceding_text.len() as u16;

                        execute!(stdout, MoveTo(current_col, line_y), SetForegroundColor(text_color), SetBackgroundColor(search_bg_color), Print(part))?;
                        current_col += part.len() as u16;

                        last_index = start + part.len();
                    }

                    let trailing_text = &line_content[last_index..];
                    execute!(stdout, MoveTo(current_col, line_y), SetForegroundColor(text_color), SetBackgroundColor(background_color), Print(trailing_text))?;
                }
            }
        }

        Ok(())
    }

    fn draw_hl_line(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        if self.config.show_hl_line {
            let (width, _height) = terminal::size()?;
            let hl_color = self.current_theme().hl_line_color;

            let visible_line_index = self.cursor_pos.1 - self.offset.1;

            let mut start_col = 0;

            if self.config.show_fringe {
                start_col += 2;
            }

            if self.config.show_line_numbers {
                start_col += 4;
            }
            
            execute!(
                stdout,
                cursor::MoveTo(start_col, visible_line_index as u16),
                SetBackgroundColor(hl_color),
                Print(" ".repeat((width - start_col) as usize))
            )?;

            // Redraw the text for the highlighted line if it's within the current view
            if let Some(line) = self.buffer.get((self.cursor_pos.1) as usize) {
                let text_color = self.current_theme().text_color; // Text color
                for (i, &ch) in line.iter().enumerate() {
                    execute!(
                        stdout,
                        cursor::MoveTo(start_col + i as u16, visible_line_index as u16),
                        SetForegroundColor(text_color),
                        Print(ch)
                    )?;
                }
            }

            // Reset the background color after highlighting
            execute!(
                stdout,
                SetBackgroundColor(self.current_theme().background_color)
            )?;

            Ok(())
        } else {
            Ok(())
        }
    }
        
    // TODO ~ after the last line 3 options only one, none or untile the end
    // TODO Option for relative line numbers, add one padding when we reach 4 digits lines numbers
    fn draw_line_numbers(&self, stdout: &mut io::Stdout, height: u16, start_col: u16) -> Result<()> {
        if self.config.show_line_numbers {
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
        if self.config.show_fringe {
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
        let sep_r = self.config.modeline_separator_right;
        let sep_l = self.config.modeline_separator_left;

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
            Mode::Dired  => ("DIRED",  self.current_theme().dired_mode_color,    Color::Black),
            Mode::Visual => ("VISUAL", self.current_theme().visual_mode_color,   Color::Black),
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
        execute!(stdout, SetBackgroundColor(self.current_theme().normal_cursor_color), SetForegroundColor(Color::Black), Print(format!("{}", pos_str)))?;
        execute!(stdout, ResetColor)?;


        Ok(())
    }

    pub fn message(&mut self, msg: &str) {
        self.minibuffer_content = msg.to_string();
        self.last_message_time = Some(std::time::Instant::now());
        self.messages.push(msg.to_string());
    }


	fn draw_minibuffer(&mut self, stdout: &mut io::Stdout, width: u16, height: u16) -> Result<()> {
	    // Retrieve current theme settings.
	    let minibuffer_bg = self.current_theme().minibuffer_color;
	    let content_fg = self.current_theme().text_color;
	    let prefix_fg = self.current_theme().dired_dir_color;

	    // Split the minibuffer content into lines for separate processing.
	    let lines = self.minibuffer_content.split('\n').collect::<Vec<&str>>();
	    let num_lines = lines.len() as u16;

	    // Set the minibuffer's dynamic height based on the number of lines or disable if FZY is active.
	    if self.fzy.as_ref().map_or(true, |fzy| !fzy.active) {
		self.minibuffer_height = std::cmp::max(num_lines, 1);
	    }

	    // Calculate the starting y-coordinate for the minibuffer based on its dynamic height.
	    let minibuffer_start_y = height - self.minibuffer_height;

	    // Fill the background and draw each line of the minibuffer content.
	    for (i, line) in lines.iter().enumerate() {
		let y_position = minibuffer_start_y + i as u16;

		// Fill background for the line
		execute!(stdout, MoveTo(0, y_position), SetBackgroundColor(minibuffer_bg), Print(" ".repeat(width as usize)))?;

		// Display the prefix and the line content
		execute!(
		    stdout,
		    MoveTo(0, y_position),
		    SetForegroundColor(prefix_fg),
		    Print(&format!(" {}", self.minibuffer_prefix)), // Print prefix
		    SetForegroundColor(content_fg),
		    Print(line) // Print the actual content line
		)?;
	    }

	    Ok(())
	}



    // fn draw_minibuffer(&mut self, stdout: &mut io::Stdout, width: u16, height: u16) -> Result<()> {
    //     let minibuffer_bg = self.current_theme().minibuffer_color;
    //     let content_fg = self.current_theme().text_color;
    //     let prefix_fg = self.current_theme().dired_dir_color;

    //     // // Automatically clear the message
    //     // if let Some(last_message_time) = self.last_message_time {
    //     //     if last_message_time.elapsed() > Duration::from_millis(1) {
    //     //         self.minibuffer_content.clear();
    //     //         self.last_message_time = None;
    //     //     }
    //     // }

    //     let lines: Vec<&str> = self.minibuffer_content.split('\n').collect();
    //     let num_lines = lines.len() as u16;

    //     if self.fzy.as_ref().map_or(true, |fzy| !fzy.active) {
    //         self.minibuffer_height = std::cmp::max(num_lines, 1);
    //     }

    //     let minibuffer_start_y = height - self.minibuffer_height;

    //     // Fill the minibuffer background for each line
    //     for y_offset in 0..self.minibuffer_height {
    //         execute!(
    //             stdout,
    //             MoveTo(0, minibuffer_start_y + y_offset),
    //             SetBackgroundColor(minibuffer_bg),
    //             Print(" ".repeat(width as usize))
    //         )?;
    //     }

    //     // Display each line with the prefix and content
    //     for (i, line) in lines.iter().enumerate() {
    //         let y_position = minibuffer_start_y + i as u16;
    //         execute!(
    //             stdout,
    //             MoveTo(0, y_position),
    //             SetForegroundColor(prefix_fg),
    //             Print(format!(" {}", self.minibuffer_prefix)), // Adding space to match original format
    //             SetForegroundColor(content_fg),
    //             Print(line)
    //         )?;
    //     }
        

    //     Ok(())
    // }






    // // ORIGINAL 
    // fn draw_minibuffer(&mut self, stdout: &mut io::Stdout, width: u16, height: u16) -> Result<()> {
    //     let minibuffer_bg = self.current_theme().minibuffer_color;
    //     let content_fg = self.current_theme().text_color;
    //     let prefix_fg = self.current_theme().normal_cursor_color;

    //     let minibuffer_start_y = height - self.minibuffer_height;

    //     // Fill the minibuffer background
    //     for y_offset in 0..self.minibuffer_height {
    //         execute!(
    //             stdout,
    //             MoveTo(0, minibuffer_start_y + y_offset),
    //             SetBackgroundColor(minibuffer_bg),
    //             Print(" ".repeat(width as usize))
    //         )?;
    //     }

    //     // Automatically clear the message
    //     if let Some(last_message_time) = self.last_message_time {
    //         if last_message_time.elapsed() > Duration::from_millis(1) {
    //             // Clear only the message content if the time threshold is exceeded.
    //             self.minibuffer_content.clear();
    //             // Reset the last message time to None to prevent repeated clearing in future draws.
    //             self.last_message_time = None;
    //         }
    //     }

    //     execute!(
    //         stdout,
    //         MoveTo(0, minibuffer_start_y),
    //         SetForegroundColor(prefix_fg),
    //         Print(format!(" {}", self.minibuffer_prefix)),
    //         SetForegroundColor(content_fg),
    //         Print(format!("{}", self.minibuffer_content))
    //     )?;

    //     Ok(())
    // }


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
            self.cursor_pos = (0,0);
            self.adjust_view_to_cursor("");
            self.states.clear(); // Clears the undo history
            self.snapshot(); // Creates a new initial state for undo history
            self.current_state = 0; // Resets the current state index

            self.syntax_highlighter.parse(&self.buffer);
            let theme = self.config.themes.get(&self.config.current_theme_name).expect("Current theme not found");
            self.syntax_highlighter.update_syntax_highlights(theme);
        }

        Ok(())
    }


    fn run(&mut self) -> Result<()> {
        let mut stdout = stdout();
        enable_raw_mode()?;
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        self.draw(&mut stdout)?; // Draw the first frame
        let fzy_active = self.fzy.as_ref().map_or(false, |fzy| fzy.active);
        self.current_theme().apply_cursor_color(self.cursor_pos, &self.buffer, &self.mode, self.minibuffer_active, fzy_active);
        loop {
            self.draw_cursor(&mut stdout)?;

            if poll(Duration::from_millis(270))? {
                if let Event::Key(key) = event::read()? {
                    self.force_show_cursor = true;
                    self.blink_count = 0;
                    self.handle_keys(key)?;
                    self.last_cursor_toggle = std::time::Instant::now();
                    self.draw(&mut stdout)?;
                    self.current_theme().apply_cursor_color(self.cursor_pos, &self.buffer, &self.mode, self.minibuffer_active, fzy_active);

                    if key.modifiers.is_empty() {
                        self.keychords.reset();
                    }
                }
            }
        }
    }
       


    fn handle_keys(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
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
                    self.searching = false;
                    self.search_query.clear();
                },
                KeyCode::Enter => {
                    let minibuffer_content = std::mem::take(&mut self.minibuffer_content);
                    if self.minibuffer_prefix == "Switch theme: " {
                        self.switch_theme(&minibuffer_content);
                    } else if self.minibuffer_prefix == "Eval: "  {
                        match self.eval(&minibuffer_content) {
                            Ok(_) => self.message("Code executed successfully."),
                            Err(err) => self.message(&format!("Error executing code: {}", err)),
                        };


                    } else if self.minibuffer_prefix == "Shell command: " {
                        let output = Command::new(&self.config.shell)
                                .arg("-c")
                                .arg(&minibuffer_content)
                                .output();
                            
                            match output {
                                Ok(output) => {
                                    let stdout = String::from_utf8_lossy(&output.stdout);
                                    let stderr = String::from_utf8_lossy(&output.stderr);

                                    if !stdout.is_empty() {
                                        self.message(&format!("{}", stdout));
                                    } else if !stderr.is_empty() {
                                        self.message(&format!("Error: {}", stderr));
                                    } else {
                                        self.message("(Shell command succeeded with no output)");
                                    }
                                },
                                Err(e) => {
                                    self.message(&format!("Failed to execute command: {}", e));
                                }
                            }
                    } else if self.minibuffer_prefix == "Find file: " {
                        let file_path = PathBuf::from(&minibuffer_content);
                        self.message(&format!("Current file path: {}", file_path.display()));

                        if let Err(e) = self.open(&file_path, None) {
                            self.message(&format!("Failed to open file: {}", e));
                        }

                    } else if self.minibuffer_prefix == ":" {
                        match minibuffer_content.as_str() {
                            "w" => {
                                match self.buffer_save() {
                                    Ok(_) => self.message("File saved successfully."),
                                    Err(e) => self.message(&format!("Failed to save file: {}", e)),
                                }
                            },
                            "q" => {
                                self.quit();
                            },
                            "wq" => {
                                self.buffer_save()?;
                                self.quit();
                            },
                            _ => {
                                if let Ok(line_number) = minibuffer_content.parse::<usize>() {
                                    self.goto_line(line_number);
                                } else {
                                    self.message("Invalid command");
                                }
                            }
                        }
                    } else if self.minibuffer_prefix == "Search: " {
                        self.search_query = minibuffer_content.clone();

                        // Find the next occurrence of the search query from the cursor's current position.
                        let mut found = false;
                        for (line_idx, line) in self.buffer.iter().enumerate().skip(self.cursor_pos.1 as usize) {
                            // Determine start index for search in the current line.
                            let start_search_idx = if line_idx == self.cursor_pos.1 as usize { self.cursor_pos.0 as usize + 1 } else { 0 };
                            if let Some(match_idx) = line.iter().skip(start_search_idx).collect::<String>().find(&minibuffer_content) {
                                // Update cursor position to the start of the found match.
                                self.cursor_pos = (match_idx as u16, line_idx as u16);
                                found = true;
                                break;
                            }
                        }

                        // If no match is found after the current cursor position, optionally wrap the search to the beginning of the document.
                        if !found {
                            for (line_idx, line) in self.buffer.iter().enumerate().take(self.cursor_pos.1 as usize + 1) {
                                if let Some(match_idx) = line.iter().collect::<String>().find(&minibuffer_content) {
                                    self.cursor_pos = (match_idx as u16, line_idx as u16);
                                    break;
                                }
                            }
                        }
                        self.adjust_view_to_cursor("");
                    } else if self.mode == Mode::Dired {
                        if self.minibuffer_prefix == "Create directory: " {
                            if let Some(dired) = &mut self.dired {
                                dired.create_directory(&minibuffer_content)?;
                                dired.refresh_directory_contents()?;
                            }
                        } else if self.minibuffer_prefix.starts_with("Delete ") && self.minibuffer_prefix.ends_with(" [y/n]: ") {
                            if minibuffer_content == "y" {
                                if let Some(dired) = &mut self.dired {
                                    dired.delete_entry()?;
                                }
                            }
                        } else if self.minibuffer_prefix == "Rename: " {
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
                Mode::Normal => { self.handle_normal_mode(key)?; },
                Mode::Insert => { self.handle_insert_mode(key)?; },
                Mode::Dired  => { self.handle_dired_mode(key)?;  },
                Mode::Visual => { self.handle_visual_mode(key)?; },
            }
        }

        Ok(())
    }

        
    fn set_cursor_shape(&self) {
        let block = "\x1b[2 q";
        let line = "\x1b[6 q";

        let shape = match self.mode {
            Mode::Normal | Mode::Dired | Mode::Visual => block,
            Mode::Insert => if self.config.insert_line_cursor { line } else { block },

        };

        print!("{}", shape);
        io::stdout().flush().unwrap();
    }

    fn handle_dired_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(dired) = &mut self.dired {
                    let max_index = dired.entries.len() as u16 + 1;
                    if dired.cursor_pos < max_index {
                        dired.cursor_pos += 1;
                    }
                }
            },
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(dired) = &mut self.dired {
                    if dired.cursor_pos > 0 {
                        dired.cursor_pos -= 1;
                    }
                }
            },
            KeyCode::Char('h') | KeyCode::Left => {
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

            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
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
                self.minibuffer_prefix = "Create directory: ".to_string();
                self.minibuffer_content = "".to_string();
            },

            KeyCode::Char('D') => {
                if let Some(dired) = &self.dired {
                    if dired.cursor_pos > 1 && (dired.cursor_pos as usize - 2) < dired.entries.len() {
                        let entry_to_delete = &dired.entries[dired.cursor_pos as usize - 2];
                        let entry_name = entry_to_delete.file_name().to_string_lossy().into_owned();

                        self.minibuffer_prefix = format!("Delete {} [y/n]: ", entry_name);
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
                        self.minibuffer_prefix = "Rename: ".to_string();
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
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.keychords.ctrl_x_pressed = true;
                // self.message("C-x-"); // TODO print it only if no keys are pressed after some times
            },

            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.indent();
            },

            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.keychords.ctrl_x_pressed { 
                    if let Err(e) = self.buffer_save() {
                        self.message(&format!("Failed to save file: {}", e));
                    }
                }
            },
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.keychords.ctrl_x_pressed { 
                    self.minibuffer_active = true;
                    self.minibuffer_prefix = "Find file: ".to_string();
                    self.minibuffer_content = self.current_file_path.display().to_string();
                } else {
                    self.config.show_fringe = !self.config.show_fringe;
                }
            },
            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.redo();
            },
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.minibuffer_active = true;
                self.minibuffer_prefix = "Switch theme: ".to_string();
                self.minibuffer_content = "".to_string();
            },

            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.keychords.ctrl_x_pressed {
                    self.dired_jump();
                    self.keychords.ctrl_x_pressed = false;
                } else {
                    self.enter();
                    self.snapshot();
                }
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.backspace();
                self.snapshot();
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.config.show_line_numbers = !self.config.show_line_numbers;
            }
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // Check if there's an existing search query and display it as a message
                // if !self.search_query.is_empty() {
                //     self.message(&format!("Search query: {}", self.search_query));
                // } else {
                //     self.message("No search query.");
                // }

                // self.message(&format!("Current file path: {}", self.current_file_path.display().to_string()));
                
                let highlights_debug_str = format!("{:?}", self.syntax_highlighter.highlights);
                self.message(&highlights_debug_str);
            }
            KeyEvent {
                code: KeyCode::Char('N'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.highlight_search = true;
                self.search_previous();
            }
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                if let Some(fzy) = &mut self.fzy {
                    if !fzy.m_x_active {
                        fzy.m_x_active = true;
                        fzy.active = true;
                        fzy.input.clear();
                        fzy.update_items();
                        fzy.recalculate_positions();
                        self.minibuffer_height = fzy.calculate_minibuffer_height(fzy.max_visible_lines) as u16;
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.cursor_pos.1 = self.buffer.len() as u16 - 1; // Move to the last line
                let last_line_len = self.buffer.last().map_or(0, |line| line.len());
                self.cursor_pos.0 = last_line_len as u16; // Move to the end of the last line
                let (_, height) = size()?;
                let visible_lines = height - self.minibuffer_height - 1;
                if self.buffer.len() as u16 > visible_lines {
                    self.offset.1 = self.buffer.len() as u16 - visible_lines;
                }
            },
            KeyEvent {
                code: KeyCode::Char('O'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.snapshot();
                self.open_above();
            },
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // TODO Undo BUG
                self.kill_line();
                self.snapshot();
            },
            KeyEvent {
                code: KeyCode::Char('J'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.join();
            },
            KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.cursor_pos.0 = self.buffer[self.cursor_pos.1 as usize].len() as u16;
                self.mode = Mode::Insert;
                self.set_cursor_shape();
            },
            KeyEvent {
                code: KeyCode::Char('I'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.back_to_indentation();
                self.mode = Mode::Insert;
                self.set_cursor_shape();
            },
            KeyEvent {
                code: KeyCode::Char('m'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                self.back_to_indentation();
            },
            KeyEvent {
                code: KeyCode::Char('P'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.paste("before");
            },
            KeyEvent {
                code: KeyCode::Char(':'),
                modifiers: KeyModifiers::ALT | KeyModifiers::SHIFT,
                ..
            } => {
                self.minibuffer_active = true;
                self.minibuffer_prefix = "Eval: ".to_string();
                self.minibuffer_content = "".to_string();
            },
            
            KeyEvent {
                code: KeyCode::Char('!'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                self.minibuffer_active = true;
                self.minibuffer_prefix = "Shell command: ".to_string();
                self.minibuffer_content = "".to_string();
            },



            KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                ..
            } => match code {
                KeyCode::Backspace => {
                    self.backspace();
                    self.snapshot();
                },
                KeyCode::Char('f') => {
                    if let Some(fzy) = &mut self.fzy {
                        // fzy.current_path = self.current_file_path.parent().unwrap().to_path_buf(); // TODO
                        fzy.active = true;
                        fzy.input.clear();
                        fzy.update_items();
                        fzy.recalculate_positions();
                        self.minibuffer_height = fzy.calculate_minibuffer_height(fzy.max_visible_lines) as u16;
                    }
                },
                KeyCode::Char('/') => {
                    self.searching = true;
                    self.highlight_search = true;
                    self.minibuffer_active = true;
                    self.minibuffer_prefix = "Search: ".to_string();
                },
                KeyCode::Char('n') => {
                    self.highlight_search = true;
                    self.search_next();
                },
                KeyCode::Esc => {
                    self.highlight_search = false;
                },
                KeyCode::Char('e') => {
                    self.eval_line();
                },
                KeyCode::Char('0') => {
                    self.cursor_pos.0 = 0;
                },
                KeyCode::Char('x') => {
                    self.delete_char();
                    self.snapshot();
                },
                KeyCode::Char('y') => {
                    if let Some(line) = self.buffer.get(self.cursor_pos.1 as usize) {
                        let line_text = line.iter().collect::<String>();
                        self.clipboard = line_text;
                        self.copied_line = true;
                        self.message("Line copied to clipboard.");
                    }
                },
                KeyCode::Char('p') => {
                    self.paste("after");
                },
                KeyCode::Char('o') => {
                    self.snapshot();
                    self.open_below();
                },
                KeyCode::Char('a') => {
                    self.right();
                    self.mode = Mode::Insert;
                    self.set_cursor_shape();
                },
                KeyCode::Char('v') => {
                    // TODO clamp the cursor position
                    self.mode = Mode::Visual;
                    self.selection_start = Some(self.cursor_pos);
                    self.selection_end = Some(self.cursor_pos);
                },
                KeyCode::Char('g') => {
                    self.cursor_pos.0 = 0;
                    self.cursor_pos.1 = 0;
                    self.offset.0 = 0;
                    self.offset.1 = 0;
                },
                KeyCode::Char('i') => {
                    self.mode = Mode::Insert;
                    self.set_cursor_shape();
                    self.snapshot();
                    // self.message("--INSERT--"); 
                },
                KeyCode::Char('d') => {
                    self.dired_jump();
                },
                KeyCode::Char(':') => {
                    self.minibuffer_active = true;
                    self.minibuffer_prefix = ":".to_string();
                    self.minibuffer_content = "".to_string();
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.down();
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.up();
                },
                KeyCode::Char('h') | KeyCode::Left => {
                    self.left();
                },
                KeyCode::Char('l') | KeyCode::Right => {
                    self.right();
                },
                KeyCode::Char('u') => {
                    self.undo();
                },
                KeyCode::Char('q') => {
                    self.quit();
                },
                _ => {}
            },
            _ => {}
        }

        Ok(())
    }

    fn handle_visual_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.set_cursor_shape();
                self.selection_start = None;
                self.selection_end = None;
            },
            KeyCode::Char('v') => {
                self.mode = Mode::Normal;
                self.selection_start = None;
                self.selection_end = None;
            },
            KeyCode::Char('e') => {
                match self.eval_region() {
                    Ok(_) => {}, // Handle success case, if necessary
                    Err(err_msg) => self.message(&err_msg),
                }
            },
            KeyCode::Char('x') => {
                self.delete_selection();
                self.mode = Mode::Normal;
                self.selection_start = None;
                self.selection_end = None;
            },
            KeyCode::Char('h') | KeyCode::Left => {
                if self.cursor_pos.0 > 0 {
                    self.cursor_pos.0 -= 1;
                }
            },
            KeyCode::Char('l') | KeyCode::Right => {
                // Prevent moving into the newline character at the end of lines
                let line_len = self.buffer[self.cursor_pos.1 as usize].len() as u16;
                if self.cursor_pos.0 < line_len.saturating_sub(1) {
                    self.cursor_pos.0 += 1;
                }
            },
            KeyCode::Char('j') | KeyCode::Down => {
                if self.cursor_pos.1 < self.buffer.len() as u16 - 1 {
                    self.cursor_pos.1 += 1;
                    // Adjust for potentially shorter next line
                    self.cursor_pos.0 = self.cursor_pos.0.min(self.buffer[self.cursor_pos.1 as usize].len() as u16);
                }
            },
            KeyCode::Char('k') | KeyCode::Up => {
                if self.cursor_pos.1 > 0 {
                    self.cursor_pos.1 -= 1;
                    // Adjust for potentially shorter previous line
                    self.cursor_pos.0 = self.cursor_pos.0.min(self.buffer[self.cursor_pos.1 as usize].len() as u16);
                }
            },

            KeyCode::Char('y') => {
                // TODO Reset cursor to the original position
                self.copy_selection();
                self.mode = Mode::Normal;
                self.selection_start = None;
                self.selection_end = None;
            },
            _ => {}
        };

        // Update the selection end after movement, ensuring it's within valid text regions
        self.selection_end = Some(self.cursor_pos);

        Ok(())
    }


        fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<()> {
            match key {
                KeyEvent {
                    code: KeyCode::Tab,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.indent();
                },
                KeyEvent {
                    code: KeyCode::Char('a'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.mwim_beginning();
                },
                KeyEvent {
                    code: KeyCode::Char('e'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.mwim_end();
                },
                KeyEvent {
                    code: KeyCode::Char('v'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.paste("before");
                },
                KeyEvent {
                    code: KeyCode::Char('n'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.down();
                },
                KeyEvent {
                    code: KeyCode::Char('p'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.up();
                },
                KeyEvent {
                    code: KeyCode::Char('b'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.left();
                },
                KeyEvent {
                    code: KeyCode::Char('f'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.right();
                },
                KeyEvent {
                    code: KeyCode::Char('s'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    self.right();
                    self.searching = true;
                    self.minibuffer_active = true;
                    self.minibuffer_prefix = "Search:".to_string();
                },
                
                KeyEvent {
                    code,
                    modifiers,
                    ..
                } => match (code, modifiers) {
                    (KeyCode::Esc, KeyModifiers::NONE) => {
                        self.mode = Mode::Normal;
                        self.set_cursor_shape();
                        self.snapshot();
                    },

                    // (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                    //     // Handle uppercase letters
                    //     let uppercase = c.to_ascii_uppercase();
                    //     self.buffer[self.cursor_pos.1 as usize].insert(self.cursor_pos.0 as usize, uppercase);
                    //     self.cursor_pos.0 += 1;
                    // },
                    
                    // (KeyCode::Char(c), KeyModifiers::NONE) => {
                    //     // Handle lowercase and other characters without modifiers
                    //     self.buffer[self.cursor_pos.1 as usize].insert(self.cursor_pos.0 as usize, c);
                    //     self.cursor_pos.0 += 1;
                    // },


                    (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                        // Handle uppercase letters
                        let uppercase = c.to_ascii_uppercase();
                        self.buffer[self.cursor_pos.1 as usize].insert(self.cursor_pos.0 as usize, uppercase);
                        self.cursor_pos.0 += 1;
                    },
                    
                    (KeyCode::Char(c), KeyModifiers::NONE) => {
                        // Handle lowercase and other characters without modifiers
                        if self.config.electric_pair_mode && "([{'\"".contains(c) {
                            let closing_char = match c {
                                '(' => ')',
                                '[' => ']',
                                '{' => '}',
                                '"' => '"',
                                '\'' => '\'',
                                _ => c,
                            };
                            self.buffer[self.cursor_pos.1 as usize].insert(self.cursor_pos.0 as usize, c);
                            self.buffer[self.cursor_pos.1 as usize].insert(self.cursor_pos.0 as usize + 1, closing_char);
                            self.cursor_pos.0 += 1;  // Move cursor between the pair
                        } else {
                            self.buffer[self.cursor_pos.1 as usize].insert(self.cursor_pos.0 as usize, c);
                            self.cursor_pos.0 += 1;
                        }
                    },


                    (KeyCode::Backspace, KeyModifiers::NONE) => {
                        self.backspace();
                    },
                    
                    (KeyCode::Enter, KeyModifiers::NONE) => {
                        self.enter();
                    },
                    _ => {}
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
    search_bg_color: Color,
    visual_mode_color: Color,
    selection_color: Color,
    hl_line_color: Color,
    // Syntax highlight colors
    use_color: Color,
    string_color: Color,
}

impl Theme {

    fn wal() -> Self {
        match load_wal_colors() {
            Ok(colors) if !colors.is_empty() => {
                Theme::from_wal_colors(colors)
            },
            _ => Theme::fallback()
        }
    }

    fn from_wal_colors(colors: Vec<Color>) -> Self {
        Theme {
            background_color: colors.get(0).cloned().unwrap(),
            text_color: colors.get(7).cloned().unwrap(),
            normal_cursor_color: colors.get(12).cloned().unwrap(),
            insert_cursor_color: colors.get(13).cloned().unwrap(),
            fringe_color: colors.get(4).cloned().unwrap(),
            line_numbers_color: colors.get(8).cloned().unwrap(),
            current_line_number_color: colors.get(5).cloned().unwrap(),
            modeline_color: colors.get(6).cloned().unwrap(),
            modeline_lighter_color: colors.get(8).cloned().unwrap(),
            minibuffer_color: colors.get(0).cloned().unwrap(),
            dired_mode_color: colors.get(12).cloned().unwrap(),
            dired_timestamp_color: colors.get(11).cloned().unwrap(),
            dired_path_color: colors.get(12).cloned().unwrap(),
            dired_size_color: colors.get(14).cloned().unwrap(),
            dired_dir_color: colors.get(12).cloned().unwrap(),
            comment_color: colors.get(8).cloned().unwrap(),
            warning_color: colors.get(11).cloned().unwrap(),
            error_color: colors.get(9).cloned().unwrap(),
            ok_color: colors.get(2).cloned().unwrap(),
            search_bg_color: colors.get(8).cloned().unwrap(),
            visual_mode_color: colors.get(5).cloned().unwrap(),
            selection_color: colors.get(5).cloned().unwrap(),
            hl_line_color: colors.get(8).cloned().unwrap(),
            use_color: colors.get(12).cloned().unwrap(),
            string_color: colors.get(10).cloned().unwrap(),
        }
    }

    fn fallback() -> Self {
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
            search_bg_color: hex_to_rgb("#3B5238").unwrap(),
            visual_mode_color: hex_to_rgb("#3B5238").unwrap(),
            selection_color: hex_to_rgb("#262626").unwrap(),
            hl_line_color: hex_to_rgb("#070707").unwrap(),
            use_color: hex_to_rgb("#514B8E").unwrap(),
            string_color: hex_to_rgb("#658B5F").unwrap(),
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
                Mode::Visual => &self.normal_cursor_color,
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



use directories::BaseDirs;
    
fn get_config_path() -> Option<PathBuf> {
    if let Some(base_dirs) = BaseDirs::new() {
        let config_dir = base_dirs.home_dir().join(".config/redit/config.lua");
        if config_dir.exists() {
            return Some(config_dir);
        }
    }
    None
}

fn load_wal_colors() -> io::Result<Vec<Color>> {
    if let Some(base_dirs) = directories::BaseDirs::new() {
        let wal_colors_path = base_dirs.home_dir().join(".cache/wal/colors");
        if wal_colors_path.exists() {
            let content = fs::read_to_string(wal_colors_path)?;
            let colors = content.lines()
                .map(|line| hex_to_rgb(line.trim()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)))
                .collect::<io::Result<Vec<Color>>>();
            return colors;
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "WAL colors file not found"))
}


fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let config_path = get_config_path().map(|path| path.to_str().unwrap().to_string());
    let mut editor = Editor::new(config_path.as_deref()).expect("Failed to create editor");

    if args.len() > 1 {
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

macro_rules! register_command {
    ($commands:expr, $name:expr, $func:expr) => {
        $commands.insert(
            $name.to_string(),
            Box::new(move |editor: &mut Editor| {
                // Call the function, and wrap non-Result returning functions with Ok(())
                $func(editor);
                Ok(())
            }) as Box<dyn FnMut(&mut Editor) -> io::Result<()>>
        );
    };
}

// TODO Command filterning, Fuzzy matching highlight, Change colors on selction
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
    m_x_active: bool,
    commands: HashMap<String, Box<dyn FnMut(&mut Editor) -> io::Result<()>>>,
}

impl Fzy {
    fn new(current_path: PathBuf) -> Self {
        let mut commands: HashMap<String, Box<dyn FnMut(&mut Editor) -> io::Result<()>>> = HashMap::new();
        register_command!(commands, "save-buffer", Editor::buffer_save);
        register_command!(commands, "dired-jump", Editor::dired_jump);
        register_command!(commands, "eval-buffer", Editor::eval_buffer);
        register_command!(commands, "debug-ast", Editor::debug_print_ast);
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
            m_x_active: false,
            commands,
        }
    }

    fn update_items(&mut self) {
        // Clear existing items to prepare for new items
        self.items.clear();

        if self.m_x_active {
            self.items.extend(self.commands.keys().cloned());
        } else {
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
        }

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

            let (icon, icon_color) = if self.m_x_active {
                ("", theme.text_color)
            } else {
                match item.as_str() {
                    ".." => ("󱚁", theme.text_color),
                    "." => ("󰉋", theme.text_color),
                    ".git" => ("", theme.text_color),
                    _ if item.ends_with(".rs") => ("", hex_to_rgb("#DEA584").unwrap()),
                    _ if item.ends_with(".lua") => ("", theme.normal_cursor_color),
                    _ if item.ends_with(".org") => ("", hex_to_rgb("#77AA99").unwrap()),
                    _ if item.ends_with(".lock") => ("󰌾", theme.text_color), 
                    _ if item.ends_with(".toml") => ("", theme.text_color),
                    _ if item.ends_with(".json") => ("", hex_to_rgb("#CBCB41").unwrap()),
                    _ => ("󰈚", theme.text_color), // Default icon for files
                }
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

    fn recalculate_positions(&mut self) {
        let items_to_display = self.items.len().min(self.max_visible_lines);
        self.initial_input_line_y = Some(terminal::size().unwrap().1.saturating_sub(items_to_display as u16 + 1));
        self.initial_items_start_y = Some(self.initial_input_line_y.unwrap().saturating_add(1));
    }

    
    pub fn handle_input(&mut self, key: KeyEvent, editor: &mut Editor) -> bool {
        let mut state_changed = false;

        match key {
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.selection_index < self.items.len() - 1 { 
                    self.selection_index += 1;
                }
            },

            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.selection_index > 0 {
                    self.selection_index -= 1;
                }
            },

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
                    self.m_x_active = false;
                    self.active = false;
                    self.input.clear();
                    self.items.clear();
                    state_changed = true; // Indicate that the fuzzy finder was deactivated
                },
                KeyCode::Enter => {
                    if let Some(item) = self.items.get(self.selection_index) {
                        if self.m_x_active {
                            // Check if the selected item is a command
                            if let Some(command) = self.commands.get_mut(item) {
                                // Execute the command
                                let result = command(editor);
                                if let Err(e) = result {
                                    // TODO message handle the error
                                }
                            }
                            self.m_x_active = false; // Reset command mode
                        } else {
                            // File selection logic
                            let full_path = self.current_path.join(item);
                            if let Err(e) = editor.open(&full_path, None) {
                                // Handle potential error from opening a file
                                println!("Error opening file: {}", e);
                            }
                        }
                        // Reset Fzy state
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
