class_name FreeCam extends Camera3D

# --- Settings ---
@export var move_speed: float = 100.0
@export var boost_multiplier: float = 5.0
@export var mouse_sensitivity: float = 0.3
@export var jump_distance: float = 500.0 

var _is_looking: bool = false

func _ready() -> void:
	CameraManager.register_camera("FreeCam", self)
	
	# Sanity check clips
	if self.near < 1.0: self.near = 5.0
	if self.far < 100000.0: self.far = 300000.0

func _exit_tree() -> void:
	CameraManager.unregister_camera("FreeCam")

func _input(event: InputEvent) -> void:
	# 1. Mouse Look (RMB)
	if event.is_action_pressed("fcam_look"):
		_is_looking = true
		Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
	elif event.is_action_released("fcam_look"):
		_is_looking = false
		Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
		
	if _is_looking and event is InputEventMouseMotion:
		# Yaw: Rotate around GLOBAL UP (Y) to prevent rolling
		global_rotate(Vector3.UP, deg_to_rad(-event.relative.x * mouse_sensitivity))
		
		# Pitch: Rotate around LOCAL RIGHT (X)
		rotate_object_local(Vector3.RIGHT, deg_to_rad(-event.relative.y * mouse_sensitivity))
		
		# Clamp Pitch to prevent backflips
		rotation.x = clamp(rotation.x, deg_to_rad(-90), deg_to_rad(90))
		
		# Force Z (Roll) to 0 just in case
		rotation.z = 0

	# 2. Speed Adjustment (Shift + Scroll)
	if Input.is_action_pressed("fcam_fly_faster"):
		if event.is_action_pressed("fcam_increase_speed"):
			move_speed *= 1.2
		elif event.is_action_pressed("fcam_decrease_speed"):
			move_speed *= 0.8
	
	# 3. "Jump" Teleport (Scroll without Shift)
	else: 
		if event.is_action_pressed("fcam_jump_forward"):
			global_position -= global_transform.basis.z * jump_distance
		elif event.is_action_pressed("fcam_jump_backward"):
			global_position += global_transform.basis.z * jump_distance

func _process(delta: float) -> void:
	if not current: return

	var input_dir: Vector3 = Vector3.ZERO
	# Use global basis for direction to match the new rotation logic
	var fwd = global_transform.basis.z
	var right = global_transform.basis.x
	
	if Input.is_action_pressed("fcam_move_forward"): input_dir -= fwd
	if Input.is_action_pressed("fcam_move_back"):    input_dir += fwd
	if Input.is_action_pressed("fcam_move_right"):   input_dir += right
	if Input.is_action_pressed("fcam_move_left"):    input_dir -= right
	
	input_dir = input_dir.normalized()
	
	var current_speed: float = move_speed
	if Input.is_action_pressed("fcam_fly_faster"):
		current_speed *= boost_multiplier
		
	global_position += input_dir * current_speed * delta
