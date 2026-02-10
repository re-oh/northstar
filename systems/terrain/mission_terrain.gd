class_name MissionTerrain extends Node3D

const _BLOCK_SIZE: int = 64
const _LEVELS: int = 16 
# 6x6 Grid provides the safety margin we need
const _GRID_SIZE: int = 6 

@export var terrain_material: Material

var _block_mesh: PlaneMesh
var _block_instances: Array[Array] = []

func _ready() -> void:
	Loggie.msg("Test log").domain("general").channel("devtools").notice()
	self._generate_geometry_instances()
	if self.terrain_material:
		# Ensure debug color is off by default
		RenderingServer.global_shader_parameter_set("terrain_do_debug_color", false)

func _exit_tree() -> void:
	for level_list in self._block_instances:
		for rid in level_list:
			RenderingServer.free_rid(rid)
	self._block_instances.clear()

func _process(_delta: float) -> void:
	var cam: Camera3D = self.get_viewport().get_camera_3d()
	if cam:
		self._update_clipmap_transforms(cam.global_position)

func _generate_geometry_instances() -> void:
	self._block_mesh = PlaneMesh.new()
	self._block_mesh.size = Vector2(1, 1)
	self._block_mesh.subdivide_width = _BLOCK_SIZE - 1
	self._block_mesh.subdivide_depth = _BLOCK_SIZE - 1

	var scenario: RID = self.get_world_3d().scenario
	var mat_rid: RID = self.terrain_material.get_rid() if self.terrain_material else RID()

	for level in range(_LEVELS):
		var level_blocks: Array[RID] = []
		
		# 6x6 Grid
		for i in range(_GRID_SIZE * _GRID_SIZE):
			var instance: RID = RenderingServer.instance_create()
			RenderingServer.instance_set_base(instance, self._block_mesh.get_rid())
			RenderingServer.instance_set_scenario(instance, scenario)
			
			# Start invisible
			RenderingServer.instance_set_visible(instance, false)
			
			# FIX: Reasonable AABB for Frustum Culling
			# We assume max mountain height is 5000m. 
			# We extend X/Z only by block size, but Y by height range.
			var aabb_size = _BLOCK_SIZE * (1 << level)
			var bounds = AABB(Vector3(-aabb_size, -5000, -aabb_size), Vector3(aabb_size*2, 10000, aabb_size*2))
			RenderingServer.instance_set_custom_aabb(instance, bounds)
			
			if mat_rid.is_valid():
				RenderingServer.instance_geometry_set_material_override(instance, mat_rid)
			
			# Set LOD Level
			RenderingServer.instance_geometry_set_shader_parameter(instance, "lod_level", level)
			
			level_blocks.append(instance)
		
		self._block_instances.append(level_blocks)

func _update_clipmap_transforms(origin_position: Vector3) -> void:
	var centers: Array[Vector3] = []
	centers.resize(_LEVELS)
	
	# 1. Pre-calculate Centers
	for level in range(_LEVELS):
		var scale_factor: int = 1 << level 
		var block_world_size: float = self._BLOCK_SIZE * scale_factor
		var snap_step: float = 1.0 * block_world_size 
		
		centers[level] = Vector3(
			floorf(origin_position.x / snap_step) * snap_step,
			0.0,
			floorf(origin_position.z / snap_step) * snap_step
		)

	# 2. Update Instances
	for level in range(_LEVELS):
		var scale_factor: int = 1 << level 
		var block_world_size: float = self._BLOCK_SIZE * scale_factor
		var center: Vector3 = centers[level]
		
		# --- HOLE LOGIC ---
		# We calculate the precise world-space bounds of the PREVIOUS level.
		# This is the "Hole" that Level N must cut out.
		var hole_rect := Rect2(0,0,0,0) # (x, y, width, height)
		
		if level > 0:
			var prev_scale = 1 << (level - 1)
			var prev_block_size = self._BLOCK_SIZE * prev_scale
			var prev_center = centers[level - 1]
			
			# The previous level is a 6x6 grid.
			# But to avoid any gaps, we cut a hole slightly SMALLER than the full previous level.
			# We cut a hole exactly the size of the "Inner 2x2" area of the current level?
			# No, we cut a hole matching the total coverage of Level N-1.
			
			# Grid Size 6. Center is in middle.
			# Radius = 3 blocks.
			var prev_radius = (_GRID_SIZE / 2.0) * prev_block_size
			
			# Create the rect (MinX, MinZ, SizeX, SizeZ)
			hole_rect = Rect2(
				prev_center.x - prev_radius, 
				prev_center.z - prev_radius, 
				prev_radius * 2.0, 
				prev_radius * 2.0
			)
			
			# Shrink the hole slightly (e.g., 10cm) to ensure overlap (prevent gaps due to float errors)
			var shrink = 0.5 
			hole_rect.position += Vector2(shrink, shrink)
			hole_rect.size -= Vector2(shrink * 2.0, shrink * 2.0)
		else:
			# Level 0 has no hole (size 0)
			hole_rect = Rect2(0,0,0,0)

		# Convert Rect2 to Vector4 (MinX, MinZ, MaxX, MaxZ) for Shader
		var hole_vec4 = Vector4(
			hole_rect.position.x, 
			hole_rect.position.y, 
			hole_rect.position.x + hole_rect.size.x, 
			hole_rect.position.y + hole_rect.size.y
		)

		var block_idx: int = 0
		var half_grid: int = _GRID_SIZE / 2
		
		for x in range(-half_grid, half_grid):
			for z in range(-half_grid, half_grid):
				var rid = self._block_instances[level][block_idx]
				
				# 1. Update Transform
				var grid_offset = Vector3(x + 0.5, 0.0, z + 0.5)
				var pos = center + (grid_offset * block_world_size)
				
				var xform = Transform3D().scaled(Vector3.ONE * block_world_size)
				xform.origin = pos
				RenderingServer.instance_set_transform(rid, xform)
				
				# 2. Update Shader Param for Hole Cutting
				# Only needed if level > 0, but safe to set always
				RenderingServer.instance_geometry_set_shader_parameter(rid, "hole_rect", hole_vec4)
				
				# 3. Always Visible (Shader handles the cutting)
				RenderingServer.instance_set_visible(rid, true)
				
				block_idx += 1
