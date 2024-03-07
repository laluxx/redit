-- config.lua
Blink_cursor = true
Show_fringe = true
Show_line_numbers = true
Insert_line_cursor = false
Show_hl_line = false
Top_scroll_margin = 10
Bottom_scroll_margin = 10
Blink_limit = 10

function c()
   Normal_cursor_color = "#10B1FE"
   Insert_cursor_color = "#9F7EFE"
end

Theme = "nature"
-- Here you can define your hown themes
-- TODO if a theme field is missing it crash
Themes = {
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
	},
	sonokai = {
		background_color = "#2c2e34",
		text_color = "#c0caf5",
		normal_cursor_color = "#f7768e",
		insert_cursor_color = "#9ece6a",
		fringe_color = "#2c2e34",
		line_numbers_color = "#565f89",
		current_line_number_color = "#c0caf5",
		modeline_color = "#24283b",
		modeline_lighter_color = "#414868",
		minibuffer_color = "#1a1b26",
		dired_mode_color = "#7aa2f7",
		dired_timestamp_color = "#e0af68",
		dired_path_color = "#7dcfff",
		dired_size_color = "#bb9af7",
		dired_dir_color = "#7dcfff",
		comment_color = "#565f89",
		warning_color = "#e0af68",
		error_color = "#db4b4b",
		ok_color = "#9ece6a",
		search_bg_color = "#ea9a97",
		visual_mode_color = "#73daca",
		selection_color = "#33467c",
		hl_line_color = "#292e42",
	},
}
