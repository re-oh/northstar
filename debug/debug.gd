extends Node

var _major_version: int = 0
var _minor_version: int = 3
var _patch_version: int = 0

var version: String:
	get:
		var version := [self._major_version, self._minor_version, self._patch_version]
		return "%d.%d.%d-dev" % version if OS.has_feature("debug") else "%d.%d.%d" % version

# General State
var _wireframe_state: Array = [false]
var _terrain_debug_state: Array = [false]

# Loggie domains
var _loggie_domains: Dictionary[String, bool] = {
	"terrain": true,
	"general": true,
}

func _process(_delta: float) -> void:
	ImGui.Begin("Developer Tools")
	if ImGui.BeginTabBar("DebugTabs"):
		self._info_tab()
		if ImGui.BeginTabItem("General"):
			ImGui.Text("Render Settings")
			ImGui.Separator()
			
			if ImGui.Checkbox("Wireframe Mode", _wireframe_state):
				var vp = get_viewport()
				if _wireframe_state[0]:
					vp.debug_draw = Viewport.DEBUG_DRAW_WIREFRAME
				else:
					vp.debug_draw = Viewport.DEBUG_DRAW_DISABLED
			
			ImGui.EndTabItem()

		# --- TAB 2: CAMERA ---
		if ImGui.BeginTabItem("Camera"):
			ImGui.Text("Active Camera Control")
			ImGui.Separator()
			
			var cam_names: Array = CameraManager.get_camera_names()
			var current_cam: String = CameraManager.get_current_camera_name()
			
			# Create a Combo Box (Dropdown)
			if ImGui.BeginCombo("Select Camera", current_cam):
				for name in cam_names:
					var is_selected: bool = (name == current_cam)
					if ImGui.SelectableEx(name, is_selected):
						CameraManager.set_active_camera(name)
					
					# Ensure the currently selected item is focused when opening the list
					if is_selected:
						ImGui.SetItemDefaultFocus()
				ImGui.EndCombo()
			
			ImGui.Dummy(Vector2(0, 10)) # Spacer
			ImGui.TextColored(Color(0.5, 0.5, 0.5, 1), "Stats:")
			
			var active_cam_node = get_viewport().get_camera_3d()
			if active_cam_node:
				ImGui.Text("Pos: %v" % active_cam_node.global_position)
				ImGui.Text("Near: %.2f | Far: %.1f" % [active_cam_node.near, active_cam_node.far])
				
				# Check for FreeCam specific stats
				if active_cam_node is FreeCam:
					ImGui.Text("Fly Speed: %.1f" % active_cam_node.move_speed)

			ImGui.EndTabItem()

		# --- TAB 3: TERRAIN ---
		if ImGui.BeginTabItem("Terrain"):
			ImGui.Text("Clipmap Control")
			ImGui.Separator()
			
			# Updated: Uses RenderingServer directly (Global Shader Param)
			if ImGui.Checkbox("Show LOD Colors (HSV)", _terrain_debug_state):
				RenderingServer.global_shader_parameter_set("terrain_do_debug_color", _terrain_debug_state[0])
			
			# We can access constants statically via the class_name 'MissionTerrain'
			ImGui.Dummy(Vector2(0, 5))
			ImGui.Text("Block Size: %d" % MissionTerrain._BLOCK_SIZE)
			ImGui.Text("Levels: %d" % MissionTerrain._LEVELS)

			ImGui.EndTabItem()
			
		ImGui.EndTabBar()
	
	ImGui.End()


func _info_tab() -> void:
	if ImGui.BeginTabItem("Info"):
		ImGui.Text("Version: %s" % self.version)
	ImGui.EndTabItem()
