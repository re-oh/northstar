@tool
extends SceneTree

func _init() -> void:
	var cache_path = "user://cache"
	print("Clearing cache at: " + cache_path)
	var dir = DirAccess.open("user://")
	if dir:
		if dir.dir_exists("cache"):
			# Recursive delete is safer with a helper, but for now let's try OS shell if possible, 
			# or just walk it. Actually, Godot's DirAccess doesn't have recursive delete easily in 4.x without walking.
			# Simpler: just rename it to cache_old_timestamp, or try to use OS command if I can find the path.
			# But I can't rely on OS path mapping easily.
			
			# Let's try to map it to global path
			var global_path = ProjectSettings.globalize_path(cache_path)
			print("Global path: " + global_path)
			var output = []
			var exit_code = OS.execute("rm", ["-rf", global_path], output, true)
			print("rm exit code: %d" % exit_code)
			
		else:
			print("Cache dir does not exist.")
	quit()
