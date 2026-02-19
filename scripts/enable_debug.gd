@tool
extends SceneTree

func _init() -> void:
	print("Enabling SVT debug visualization (Mode 2)...")
	RenderingServer.global_shader_parameter_set("terrain_debug_mode", 2)
	print("Done. Please switch back to the game window.")
	quit()
