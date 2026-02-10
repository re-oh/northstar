extends Node

# Dictionary to store { "Camera Name": Camera3D_Node }
var _registered_cameras: Dictionary = {}
var _current_cam_name: String = ""

func register_camera(cam_name: String, cam_node: Camera3D) -> void:
	_registered_cameras[cam_name] = cam_node
	
	# If this is the first camera, make it active automatically
	if _registered_cameras.size() == 1:
		set_active_camera(cam_name)

func unregister_camera(cam_name: String) -> void:
	if _registered_cameras.has(cam_name):
		_registered_cameras.erase(cam_name)

func get_camera_names() -> Array:
	return _registered_cameras.keys()

func set_active_camera(cam_name: String) -> void:
	if not _registered_cameras.has(cam_name):
		Loggie.error("CameraManager: Camera '%s' not found!" % cam_name)
		return
		
	var cam_node = _registered_cameras[cam_name]
	if is_instance_valid(cam_node):
		cam_node.make_current()
		_current_cam_name = cam_name
	else:
		# Cleanup if node died unexpectedly
		_registered_cameras.erase(cam_name)

func get_current_camera_name() -> String:
	return _current_cam_name
