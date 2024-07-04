-- config.lua
Blink_cursor = true
Show_fringe = false
Show_line_numbers = true
Insert_line_cursor = false
Show_hl_line = false
Top_scroll_margin = 10
Bottom_scroll_margin = 10
Blink_limit = 10
Indentation = 4
Electric_pair_mode = true
Tree_node = "◯" -- □
Current_tree_node = "●" -- ■
Tree_node_separator = "—"
Modeline_separator_right = ""
Modeline_separator_left = "◖" --
Shell = "zsh"
Rainbow_mode = true
Scroll_bar_mode = true
Compile_command = "cargo build"
Max_minibuffer_height = 30
Emacs_scrolling = true

-- TODO message in lua
-- TODO error in lua and rust

function c()
   Normal_cursor_color = "#10B1FE"
   Insert_cursor_color = "#9F7EFE"
end

function a()
   for i = 1, 20 do  -- This loop will iterate 20 times
      Normal_cursor_color = "#10B1FE"
      Normal_cursor_color = "#FFFFFF"
      Normal_cursor_color = "#151515"
   end
end


Theme = "dark" -- WAL
-- Here you can define your hown themes
-- TODO if a theme field is missing it crash
Themes = {
   dark = {
      background_color = "#18181B",
      text_color = "#E4E4E8",
      normal_cursor_color = "#E4E4E8",
      insert_cursor_color = "#80BBB5",
      fringe_color = "#18181B",
      line_numbers_color = "#545c5e",
      current_line_number_color = "#919a9c",
      modeline_color = "#131316",
      modeline_lighter_color = "#222225",
      minibuffer_color = "#18181B",
      dired_mode_color = "#968CC7",
      dired_timestamp_color = "#9d81ba",
      dired_path_color = "#80bcb6",
      dired_size_color = "#cd5c60",
      dired_dir_color = "#4d9391",
      comment_color = "#545c5e",
      warning_color = "#dbac66",
      error_color = "#cd5c60",
      ok_color = "#6fb593",
      search_bg_color = "#303035",
      visual_mode_color = "#CD9575",
      selection_color = "#2E403B",
      hl_line_color = "#222225",
      use_color = "#4d9391",
      string_color = "#6fb593",
   },
   ocean = {
      background_color = "#1A1A25",
      text_color = "#E6E6E8",
      normal_cursor_color = "#E6E6E8",
      insert_cursor_color = "#738FD7",
      fringe_color = "#1A1A25",
      line_numbers_color = "#545C5E",
      current_line_number_color = "#738FD7",
      modeline_color = "#252534",
      modeline_lighter_color = "#2F2F43",
      minibuffer_color = "#1A1A25",
      dired_mode_color = "#738FD7",
      dired_timestamp_color = "#9587DD",
      dired_path_color = "#4CA6E8",
      dired_size_color = "#EED891",
      dired_dir_color = "#738FD7",
      comment_color = "#545C5E",
      warning_color = "#DBAC66",
      error_color = "#E84C58",
      ok_color = "#65E6A7",
      search_bg_color = "#32324A",
      visual_mode_color = "#D24B83",
      selection_color = "#2E403B",
      hl_line_color = "#252534",
      use_color = "#4d9391",
      string_color = "#7CF083",
   },
   nature = {
      background_color = "#090909",
      text_color = "#9995BF",
      normal_cursor_color = "#658B5F",
      insert_cursor_color = "#514B8E",
      fringe_color = "#090909",
      line_numbers_color = "#171717",
      current_line_number_color = "#C0ACD1",
      modeline_color = "#060606",
      modeline_lighter_color = "#171717",
      minibuffer_color = "#070707",
      dired_mode_color = "#565663",
      dired_timestamp_color = "#514B8E",
      dired_path_color = "#658B5F",
      dired_size_color = "#48534A",
      dired_dir_color = "#514B8E",
      comment_color = "#867892",
      warning_color = "#565663",
      error_color = "#444E46",
      ok_color = "#4C6750",
      search_bg_color = "#3B5238",
      visual_mode_color = "#3B5238",
      selection_color = "#262626",
      hl_line_color = "#070707",
      use_color = "#514B8E",
      string_color = "#9ECE6A",
   },
   tokyonight = {
      background_color = "#1A1B26",
      text_color = "#C0CAF5",
      normal_cursor_color = "#7AA2F7",
      insert_cursor_color = "#9ECE6A",
      fringe_color = "#1A1B26",
      line_numbers_color = "#3B4261",
      current_line_number_color = "#737AA2",
      modeline_color = "#292E42",
      modeline_lighter_color = "#3B4261",
      minibuffer_color = "#1A1B26",
      dired_mode_color = "#E0AF68",
      dired_timestamp_color = "#F7768E",
      dired_path_color = "#7AA2F7",
      dired_size_color = "#BB9AF7",
      dired_dir_color = "#7AA2F7",
      comment_color = "#565F89",
      warning_color = "#E0AF68",
      error_color = "#F7768E",
      ok_color = "#9ECE6A",
      search_bg_color = "#3D59A1",
      visual_mode_color = "#BB9AF7",
      selection_color = "#283457",
      hl_line_color = "#292E42",
      use_color = "#7DCFFF",
      string_color = "#9ECE6A",
   },
   doom_one = {
      background_color = "#282C34",
      text_color = "#BBC2CF",
      normal_cursor_color = "#51AFEF",
      insert_cursor_color = "#A9A1E1",
      fringe_color = "#282C34",
      line_numbers_color = "#3F444A",
      current_line_number_color = "#BBC2CF",
      modeline_color = "#1D2026",
      modeline_lighter_color = "#252931",
      minibuffer_color = "#21242B",
      dired_mode_color = "#C678DD",
      dired_timestamp_color = "#46D9FC",
      dired_path_color = "#51AFEF",
      dired_size_color = "#DA8548",
      dired_dir_color = "#C678DD",
      comment_color = "#5B6268",
      warning_color = "#ECBE7B",
      error_color = "#FF6C6B",
      ok_color = "#98BE65",
      search_bg_color = "#387AA7",
      visual_mode_color = "#C678DD",
      selection_color = "#42444A",
      hl_line_color = "#21242B",
      use_color = "#51AFEF",
      string_color = "#9ECE6A",
   },
}