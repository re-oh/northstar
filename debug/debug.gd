extends Node

var _major_version: int = 0
var _minor_version: int = 3
var _patch_version: int = 0

var version: String:
	get:
		var ver := [self._major_version, self._minor_version, self._patch_version]
		return "%d.%d.%d-dev" % ver if OS.has_feature("debug") else "%d.%d.%d" % ver

# General State
var _wireframe_state: Array = [false]
var _terrain_debug_state: Array = [false]

# Loggie domains
var _loggie_domains: Dictionary[String, Array] = {
	"terrain": [true],
	"map_loading": [true],
	"general": [true],
}

var _devtools_channel: DevToolsChannel
var _log_search_text: String = ""
var _log_auto_scroll: Array = [true]
var _log_level_filter: int = 0

func _ready() -> void:
	self._devtools_channel = DevToolsChannel.new()
	Loggie.add_channel(self._devtools_channel)
	
	# Enable configured domains
	for domain in self._loggie_domains:
		Loggie.set_domain_enabled(domain, self._loggie_domains[domain][0])

func _process(_delta: float) -> void:
	ImGui.Begin("Developer Tools")
	if ImGui.BeginTabBar("DebugTabs"):
		self._info_tab()
		self._general_tab()
		self._camera_tab()
		self._terrain_tab()
		self._logging_tab()
		ImGui.EndTabBar()	
	ImGui.End()


func _info_tab() -> void:
	if ImGui.BeginTabItem("Info"):
		ImGui.Text("Version: %s" % self.version)
		ImGui.EndTabItem()

func _general_tab() -> void:
	if ImGui.BeginTabItem("General"):
		ImGui.Text("Render Settings")
		ImGui.Separator()
		
		if ImGui.Checkbox("Wireframe Mode", self._wireframe_state):
			var vp := self.get_viewport()
			if self._wireframe_state[0]:
				vp.debug_draw = Viewport.DEBUG_DRAW_WIREFRAME
			else:
				vp.debug_draw = Viewport.DEBUG_DRAW_DISABLED
		
		ImGui.EndTabItem()

func _camera_tab() -> void:
	if ImGui.BeginTabItem("Camera"):
		ImGui.Text("Active Camera Control")
		ImGui.Separator()
		
		var cam_names: Array = CameraManager.get_camera_names()
		var current_cam: String = CameraManager.get_current_camera_name()
		
		# Create a Combo Box (Dropdown)
		if ImGui.BeginCombo("Select Camera", current_cam):
			for cam_name in cam_names:
				var is_selected: bool = (cam_name == current_cam)
				if ImGui.SelectableEx(cam_name, is_selected):
					CameraManager.set_active_camera(cam_name)
				
				if is_selected:
					ImGui.SetItemDefaultFocus()
			ImGui.EndCombo()
		
		ImGui.Dummy(Vector2(0, 10)) # Spacer
		ImGui.TextColored(Color(0.5, 0.5, 0.5, 1), "Stats:")
		
		var active_cam_node := self.get_viewport().get_camera_3d()
		if active_cam_node:
			ImGui.Text("Pos: %v" % active_cam_node.global_position)
			ImGui.Text("Near: %.2f | Far: %.1f" % [active_cam_node.near, active_cam_node.far])
			
			# Check for FreeCam specific stats
			if active_cam_node is FreeCam:
				ImGui.Text("Fly Speed: %.1f" % active_cam_node.move_speed)
	
		ImGui.EndTabItem()

func _terrain_tab() -> void:
	if ImGui.BeginTabItem("Terrain"):
		ImGui.Text("Clipmap Control")
		ImGui.Separator()
		
		# Updated: Uses RenderingServer directly (Global Shader Param)
		if ImGui.Checkbox("Show LOD Colors (HSV)", self._terrain_debug_state):
			RenderingServer.global_shader_parameter_set("terrain_do_debug_color", self._terrain_debug_state[0])
		
		# We can access constants statically via the class_name 'MissionTerrain'
		ImGui.Dummy(Vector2(0, 5))
		ImGui.Text("Block Size: %d" % MissionTerrain._BLOCK_SIZE)
		ImGui.Text("Levels: %d" % MissionTerrain._LEVELS)

		ImGui.EndTabItem()

func _logging_tab() -> void:
	if ImGui.BeginTabItem("Logging"):
		
		# --- Toolbar ---
		if ImGui.Button("Clear"):
			self._devtools_channel.clear()
		
		ImGui.SameLine()
		if ImGui.Button("Save to File"):
			self._save_logs_to_file()
			
		ImGui.SameLine()
		if ImGui.Button("Copy All"):
			var all_text: String = ""
			for tuple in self._devtools_channel.get_all_logs():
				all_text += tuple.msg.last_preprocess_result + "\n"
			DisplayServer.clipboard_set(all_text)
			
		ImGui.SameLine()
		ImGui.Checkbox("Auto-scroll", self._log_auto_scroll)
		
		ImGui.Separator()
		
		# --- Filters ---
		# 1. Search (Mocking ref with array for safety, assuming standard GDScript ImGui wrapper behavior)
		# Note: If InputText signature differs, this might need adjustment.
		# Using a simpler approach if possible: ImGui.InputText("Search", _log_search_text) -> returns string?
		# Let's try the Array ref pattern as seen in Checkbox.
		var search_ref = [self._log_search_text] 
		if ImGui.InputText("Search", search_ref, 128):
			self._log_search_text = search_ref[0]
			
		# 2. Level Filter
		# Map UI selection to Max Enum Index allowed (0=Error, 1=Warn, etc.)
		# Enum: ERROR=0, WARN=1, NOTICE=2, INFO=3, DEBUG=4
		# UI: All, Debug, Info, Notice, Warn, Error
		var levels = ["All", "Debug", "Info", "Notice", "Warn", "Error"]
		# Map UI Index to Max Allowed Type Index
		# All (0) -> 5 (Any)
		# Debug (1) -> 4
		# Info (2) -> 3
		# Notice (3) -> 2
		# Warn (4) -> 1
		# Error (5) -> 0
		
		# Allow user to change filter
		if ImGui.BeginCombo("Level", levels[self._log_level_filter]):
			for i in range(levels.size()):
				if ImGui.SelectableEx(levels[i], i == self._log_level_filter):
					self._log_level_filter = i
			ImGui.EndCombo()
			
		# 3. Domains
		if ImGui.TreeNode("Domains"):
			for domain in self._loggie_domains.keys():
				if ImGui.Checkbox(domain, self._loggie_domains[domain]):
					Loggie.set_domain_enabled(domain, self._loggie_domains[domain][0])
			ImGui.TreePop()
			
		ImGui.Separator()
		
		# --- Log Window ---
		var content_size = Vector2(0, -1) # Fill remaining height
		if ImGui.BeginChild("LogScroll", content_size, true):
			
			var max_allowed_type = 5
			if self._log_level_filter > 0:
				max_allowed_type = 5 - self._log_level_filter
			
			for tuple in self._devtools_channel.get_all_logs():
				var msg: LoggieMsg = tuple.msg
				var type: int = tuple.type
				
				if not self._loggie_domains.get(msg.domain_name, [true])[0]:
					continue
					
				if type > max_allowed_type:
					continue
					
				if self._log_search_text != "" and self._log_search_text.is_valid_filename() == false: # Just check non-empty
					if msg.last_preprocess_result.findn(self._log_search_text) == -1:
						continue
				
				var color = Color.WHITE
				match type:
					LoggieEnums.MsgType.DEBUG: color = Color.GRAY
					LoggieEnums.MsgType.INFO: color = Color.WHITE
					LoggieEnums.MsgType.NOTICE: color = Color.CYAN
					LoggieEnums.MsgType.WARN: color = Color.YELLOW
					LoggieEnums.MsgType.ERROR: color = Color.RED
				
				ImGui.TextColored(color, LoggieTools.remove_BBCode(msg.last_preprocess_result))
			
			if self._log_auto_scroll[0] and ImGui.GetScrollY() >= ImGui.GetScrollMaxY():
				ImGui.SetScrollHereY(1.0)
				
		ImGui.EndChild()
		
		ImGui.EndTabItem()

func _save_logs_to_file() -> void:
	var dir_path = "user://logs"
	if not DirAccess.dir_exists_absolute(dir_path):
		DirAccess.make_dir_absolute(dir_path)
		
	var datetime = Time.get_datetime_dict_from_system()
	var filename = "debug_log_%04d-%02d-%02d_%02d-%02d-%02d.txt" % [
		datetime.year, datetime.month, datetime.day,
		datetime.hour, datetime.minute, datetime.second
	]
	var full_path = dir_path + "/" + filename
	
	var file = FileAccess.open(full_path, FileAccess.WRITE)
	if file:
		for tuple in self._devtools_channel.get_all_logs():
			file.store_line(tuple.msg.last_preprocess_result)
		file.close()
		Loggie.info("saved logs to %s" % full_path)
	else:
		Loggie.error("failed to save logs to %s" % full_path)
