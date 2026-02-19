class_name MissionTerrainStream extends Node3D
## SVT streaming manager for terrain heightmap tiles.
## Manages a Texture2DArray VRAM cache and an indirection texture.
## Tiles are loaded from user://cache on a background thread and uploaded
## to the GPU in a time-budgeted main-thread pass.
##
## LOD Grid System:
##   LOD0: 5x5 tiles around camera (finest, source resolution)
##   LOD1: 9x9 tiles around camera (2x downsampled)
##   LOD2: 13x13 tiles around camera (4x downsampled)
##   LOD3: Entire map (8x downsampled, always loaded)

# ── Constants ──────────────────────────────────────────────────────────────────
const _PADDED_SIZE: int = 514
const _TILE_SIZE: int = 512
const _CHUNK_METERS: float = 1024.0
const _LOD_COUNT: int = 4

## Grid sizes per LOD: how many tiles to load around the camera.
## -1 means "load entire map" (LOD3).
const _LOD_GRID_SIZES: Array[int] = [5, 9, 13, -1]

## Max milliseconds per frame to spend uploading completed tiles to GPU
const _UPLOAD_BUDGET_MS: float = 4.0

## How often (in frames) to run proactive distance-based eviction
const _EVICT_SCAN_INTERVAL: int = 60

## Distance multiplier for eviction threshold (generous — avoid thrashing)
const _EVICT_DISTANCE_MULTIPLIER: float = 3.0

## Maximum new tile requests per frame (prevents flooding the queue)
const _MAX_REQUESTS_PER_FRAME: int = 32

# ── Exports ────────────────────────────────────────────────────────────────────
@export var map_name: String = "test_map"

# ── Signals ────────────────────────────────────────────────────────────────────
signal preprocessing_complete()
signal tile_loaded(lod: int, x: int, y: int, slot: int)
signal cache_full_eviction(evicted_lod: int, evicted_x: int, evicted_y: int)

# ── Physical Cache ─────────────────────────────────────────────────────────────
var _cache_slots: int = 0  # Computed dynamically at init
var _cache_texture: Texture2DArray
var _indirection_images: Array[Image] = []   # One per LOD
var _indirection_textures: Array[ImageTexture] = []  # One per LOD

# ── Bookkeeping ────────────────────────────────────────────────────────────────
## Maps "L{lod}_{x}_{y}" -> slot index
var _tile_to_slot: Dictionary = {}

## Maps slot index -> { "lod": int, "x": int, "y": int, "frame": int }
var _slot_to_tile: Dictionary = {}

## Stack of free slot indices
var _free_slots: Array[int] = []

## Grid dimensions per LOD [Vector2i(cols, rows)]
var _grid_dims: Array[Vector2i] = []

## Cache dir for this map
var _cache_dir: String = ""

## Set of keys currently in the pending or completed queue (for O(1) duplicate check)
var _in_flight_keys: Dictionary = {}

# ── Per-frame tracking ─────────────────────────────────────────────────────────
## Last tile coordinate the camera was on, per LOD (to avoid redundant requests)
var _last_camera_tile: Array[Vector2i] = []

## New tile requests made this frame
var _requests_this_frame: int = 0

# ── Threading ──────────────────────────────────────────────────────────────────
var _load_thread: Thread
var _load_mutex: Mutex
var _load_semaphore: Semaphore
var _pending_loads: Array = []      # { lod, x, y, slot, path }
var _completed_loads: Array = []    # { lod, x, y, slot, image }
var _thread_running: bool = false

# ── Stats (exposed for debug UI) ──────────────────────────────────────────────
var stats_loads_this_frame: int = 0
var stats_evictions_this_frame: int = 0
var stats_total_loaded: int = 0
var stats_total_evictions: int = 0
var stats_pending_count: int = 0
var stats_slots_used: int = 0

# ── Frame counter ─────────────────────────────────────────────────────────────
var _frame_count: int = 0
var _lod3_loaded: bool = false

var _preprocessor: HeightmapPreprocessor
var _preprocess_thread: Thread
var _is_preprocessing: bool = false
var _preprocessing_done: bool = false


## Compute the exact number of cache slots needed for the current map.
## = sum of tiles loaded per LOD (grid_size² or full lod dims if -1)
static func compute_required_slots(grid_dims: Array[Vector2i]) -> int:
	var total: int = 0
	for lod in range(mini(_LOD_COUNT, grid_dims.size())):
		var dims: Vector2i = grid_dims[lod]
		var grid_size: int = _LOD_GRID_SIZES[lod] if lod < _LOD_GRID_SIZES.size() else 5
		if grid_size < 0:
			# Full map coverage
			total += dims.x * dims.y
		else:
			total += grid_size * grid_size
	return total


func _ready() -> void:
	Loggie.msg("SVT Stream: _ready() START — map_name='%s'" % self.map_name).domain("svt").channel("devtools").info()
	self._cache_dir = "user://cache/maps/%s" % self.map_name
	
	var needs_preprocessing = not DirAccess.dir_exists_absolute(self._cache_dir + "/L0")
	
	if needs_preprocessing:
		self._start_preprocessing()
	else:
		Loggie.msg("SVT Stream: Cache found, skipping preprocessing.").domain("svt").channel("devtools").info()
		self._on_preprocessing_finished_internal(true)


func _start_preprocessing() -> void:
	Loggie.msg("SVT Stream: Starting threaded preprocessing...").domain("svt").channel("devtools").info()
	self._is_preprocessing = true
	self._preprocessing_done = false
	self._preprocessor = HeightmapPreprocessor.new()
	self._preprocess_thread = Thread.new()
	self._preprocess_thread.start(self._thread_preprocess_func)


func _thread_preprocess_func() -> void:
	var err: Error = self._preprocessor.preprocess(self.map_name)
	call_deferred("_on_preprocessing_finished_internal", err == OK)


func _on_preprocessing_finished_internal(success: bool) -> void:
	if self._preprocess_thread and self._preprocess_thread.is_started():
		self._preprocess_thread.wait_to_finish()
	
	self._is_preprocessing = false
	self._preprocessing_done = true
	
	if not success:
		Loggie.msg("SVT Stream: Preprocessing FAILED").domain("svt").channel("devtools").error()
		return

	Loggie.msg("SVT Stream: Preprocessing DONE — initializing cache").domain("svt").channel("devtools").info()
	
	if self._preprocessor:
		self._grid_dims = self._preprocessor.grid_dims
	else:
		self._load_manifest()

	# Compute dynamic cache size: exact tiles needed + no waste
	self._cache_slots = compute_required_slots(self._grid_dims)
	Loggie.msg("SVT Stream: Dynamic cache size = %d slots (exact fit for grid sizes)" % self._cache_slots).domain("svt").channel("devtools").info()

	Loggie.msg("SVT Stream: Initializing physical cache (%d slots, %dpx tiles)..." % [self._cache_slots, _PADDED_SIZE]).domain("svt").channel("devtools").info()
	self._init_physical_cache()
	Loggie.msg("SVT Stream: Initializing indirection textures...").domain("svt").channel("devtools").info()
	self._init_indirection_textures()
	self._init_free_slots()
	
	# Init per-LOD camera tile tracking
	self._last_camera_tile.clear()
	for i in range(_LOD_COUNT):
		self._last_camera_tile.append(Vector2i(-9999, -9999))
	
	Loggie.msg("SVT Stream: Free slots initialized — count=%d" % self._free_slots.size()).domain("svt").channel("devtools").info()

	# Start background loader thread
	self._load_mutex = Mutex.new()
	self._load_semaphore = Semaphore.new()
	self._thread_running = true
	self._load_thread = Thread.new()
	self._load_thread.start(self._loader_thread_func)
	Loggie.msg("SVT Stream: Background loader thread started").domain("svt").channel("devtools").info()

	Loggie.msg("SVT Stream: Initialization COMPLETE — emitting preprocessing_complete signal").domain("svt").channel("devtools").info()
	self.preprocessing_complete.emit()


func _load_manifest() -> void:
	var manifest_path: String = self._cache_dir + "/tile_manifest.json"
	if FileAccess.file_exists(manifest_path):
		var file = FileAccess.open(manifest_path, FileAccess.READ)
		var json = JSON.parse_string(file.get_as_text())
		if json and json.has("lods"):
			self._grid_dims.clear()
			for lod_data in json["lods"]:
				self._grid_dims.append(Vector2i(lod_data["cols"], lod_data["rows"]))
			Loggie.msg("SVT Stream: Loaded manifest dims: %s" % str(self._grid_dims)).domain("svt").channel("devtools").info()
	else:
		Loggie.msg("SVT Stream: ERROR — Manifest not found at %s" % manifest_path).domain("svt").channel("devtools").error()


## Public API to forcefully clear cache and re-run preprocessing
func force_regenerate_cache() -> void:
	if self._is_preprocessing:
		Loggie.msg("SVT Stream: Already preprocessing, ignoring force regen request").domain("svt").channel("devtools").warn()
		return

	if self._thread_running:
		self._exit_tree()

	var pp := HeightmapPreprocessor.new()
	pp.clear_cache(self.map_name)
	
	self._start_preprocessing()


func _exit_tree() -> void:
	Loggie.msg("SVT Stream: _exit_tree() — shutting down loader thread").domain("svt").channel("devtools").info()
	self._thread_running = false
	if self._load_semaphore:
		self._load_semaphore.post()
	if self._load_thread and self._load_thread.is_started():
		self._load_thread.wait_to_finish()
	Loggie.msg("SVT Stream: Loader thread stopped").domain("svt").channel("devtools").info()


func _process(_delta: float) -> void:
	self._frame_count += 1
	
	# ImGui Progress Window
	if self._is_preprocessing and self._preprocessor:
		if ImGui.Begin("SVT Generation"):
			ImGui.Text("Generating Terrain Cache...")
			ImGui.ProgressBar(self._preprocessor.progress, Vector2(0, 0), "%.1f%%" % (self._preprocessor.progress * 100))
			ImGui.Text(self._preprocessor.current_state)
		ImGui.End()
		return

	self.stats_loads_this_frame = 0
	self.stats_evictions_this_frame = 0
	self._requests_this_frame = 0

	# Upload completed tiles (time-budgeted)
	self._process_completed_uploads()

	# Grid-based tile loading around camera (coarse-to-fine order)
	self._load_tiles_around_camera()

	# Proactive distance-based eviction (infrequent, generous threshold)
	if self._frame_count % _EVICT_SCAN_INTERVAL == 0:
		self._evict_distant_tiles()

	# Update stats
	self._load_mutex.lock()
	self.stats_pending_count = self._pending_loads.size()
	self._load_mutex.unlock()
	self.stats_slots_used = self._cache_slots - self._free_slots.size()

	# Per-frame summary log (every 120 frames to avoid spam)
	if self._frame_count % 120 == 0:
		Loggie.msg("SVT Stream: [Frame %d] slots=%d/%d free=%d pending=%d loaded=%d evicted=%d" % [
			self._frame_count,
			self.stats_slots_used, self._cache_slots,
			self._free_slots.size(),
			self.stats_pending_count,
			self.stats_total_loaded,
			self.stats_total_evictions,
		]).domain("svt").channel("devtools").info()


# ── Public API ─────────────────────────────────────────────────────────────────

## Request a tile be in the cache. Returns the slot index, or -1 if pending/failed.
func request_tile(lod: int, x: int, y: int) -> int:
	var key: String = "L%d_%d_%d" % [lod, x, y]

	# Already loaded?
	if self._tile_to_slot.has(key):
		var existing_slot: int = self._tile_to_slot[key]
		self._slot_to_tile[existing_slot]["frame"] = Engine.get_process_frames()
		return existing_slot

	# Already in-flight (pending or completed but not yet uploaded)?
	if self._in_flight_keys.has(key):
		return -1

	# Per-frame request limit
	if self._requests_this_frame >= _MAX_REQUESTS_PER_FRAME:
		return -1

	# Check bounds
	if lod < 0 or lod >= self._grid_dims.size():
		return -1
	var dims: Vector2i = self._grid_dims[lod]
	if x < 0 or x >= dims.x or y < 0 or y >= dims.y:
		return -1

	# Get a free slot
	if self._free_slots.is_empty():
		# Cache is exactly sized — shouldn't need eviction, but safety fallback
		var evicted_slot: int = self._evict_lru()
		if evicted_slot < 0:
			return -1
		self._free_slots.append(evicted_slot)

	var slot: int = self._free_slots.pop_back()

	# Build file path
	var path: String = "%s/L%d/%d_%d.res" % [self._cache_dir, lod, x, y]

	# Queue for background loading
	self._load_mutex.lock()
	self._pending_loads.append({
		"lod": lod,
		"x": x,
		"y": y,
		"slot": slot,
		"path": path,
	})
	self._in_flight_keys[key] = true
	self._load_mutex.unlock()
	self._load_semaphore.post()

	self._requests_this_frame += 1
	return -1


## Get the physical cache Texture2DArray.
func get_cache_texture() -> Texture2DArray:
	return self._cache_texture


## Get the indirection ImageTexture for a specific LOD.
func get_indirection_texture(lod: int) -> ImageTexture:
	if lod >= 0 and lod < self._indirection_textures.size():
		return self._indirection_textures[lod]
	return null


## Get grid dimensions for a specific LOD.
func get_grid_dims(lod: int) -> Vector2i:
	if lod >= 0 and lod < self._grid_dims.size():
		return self._grid_dims[lod]
	return Vector2i.ZERO


# ── Initialization ─────────────────────────────────────────────────────────────

func _init_physical_cache() -> void:
	var images: Array[Image] = []
	for i in range(self._cache_slots):
		var img := Image.create(_PADDED_SIZE, _PADDED_SIZE, false, Image.FORMAT_RH)
		img.fill(Color(0, 0, 0, 1))
		images.append(img)

	self._cache_texture = Texture2DArray.new()
	self._cache_texture.create_from_images(images)
	Loggie.msg("SVT Stream: Physical cache Texture2DArray created — %d layers, %dx%d, FORMAT_RH" % [self._cache_slots, _PADDED_SIZE, _PADDED_SIZE]).domain("svt").channel("devtools").info()


func _init_indirection_textures() -> void:
	self._indirection_images.clear()
	self._indirection_textures.clear()

	for lod in range(_LOD_COUNT):
		if lod >= self._grid_dims.size():
			var placeholder_img := Image.create(1, 1, false, Image.FORMAT_RG8)
			placeholder_img.fill(Color(0, 0, 0, 1))
			var placeholder_tex := ImageTexture.create_from_image(placeholder_img)
			self._indirection_images.append(placeholder_img)
			self._indirection_textures.append(placeholder_tex)
			Loggie.msg("SVT Stream: Indirection LOD %d created — 1x1 PLACEHOLDER" % lod).domain("svt").channel("devtools").info()
			continue

		var dims: Vector2i = self._grid_dims[lod]
		var img := Image.create(dims.x, dims.y, false, Image.FORMAT_RG8)
		img.fill(Color(0, 0, 0, 1))

		var tex := ImageTexture.create_from_image(img)
		self._indirection_images.append(img)
		self._indirection_textures.append(tex)
		Loggie.msg("SVT Stream: Indirection LOD %d created — %dx%d, FORMAT_RG8" % [lod, dims.x, dims.y]).domain("svt").channel("devtools").info()


func _init_free_slots() -> void:
	self._free_slots.clear()
	for i in range(self._cache_slots - 1, -1, -1):
		self._free_slots.append(i)


# ── Slot Management ───────────────────────────────────────────────────────────

## Evict the least-recently-used non-LOD3 tile and return its slot.
func _evict_lru() -> int:
	var oldest_frame: int = Engine.get_process_frames()
	var oldest_slot: int = -1
	var current_frame: int = Engine.get_process_frames()

	for slot_idx: int in self._slot_to_tile:
		var info: Dictionary = self._slot_to_tile[slot_idx]
		# Never evict LOD3 tiles
		if info["lod"] == 3:
			continue
		# Never evict tiles loaded this frame
		if info["frame"] == current_frame:
			continue
		if info["frame"] < oldest_frame:
			oldest_frame = info["frame"]
			oldest_slot = slot_idx

	if oldest_slot < 0:
		return -1

	var evicted: Dictionary = self._slot_to_tile[oldest_slot]
	var evicted_key: String = "L%d_%d_%d" % [evicted["lod"], evicted["x"], evicted["y"]]

	# Clear indirection pixel
	var lod: int = evicted["lod"]
	if lod < self._indirection_images.size():
		self._indirection_images[lod].set_pixel(evicted["x"], evicted["y"], Color(0, 0, 0, 1))
		self._indirection_textures[lod].update(self._indirection_images[lod])

	self._tile_to_slot.erase(evicted_key)
	self._slot_to_tile.erase(oldest_slot)

	self.stats_evictions_this_frame += 1
	self.stats_total_evictions += 1
	self.cache_full_eviction.emit(evicted["lod"], evicted["x"], evicted["y"])

	return oldest_slot


## Proactively evict tiles that are far from the camera (never evicts LOD3).
func _evict_distant_tiles() -> void:
	var cam: Camera3D = self.get_viewport().get_camera_3d()
	if not cam:
		return

	var cam_pos: Vector3 = cam.global_position
	var evicted_count: int = 0
	var slots_to_evict: Array[int] = []

	for slot_idx: int in self._slot_to_tile:
		var info: Dictionary = self._slot_to_tile[slot_idx]
		var lod: int = info["lod"]
		
		# Never evict LOD3 tiles
		if lod == 3:
			continue

		var tx: int = info["x"]
		var ty: int = info["y"]

		var lod_scale: float = pow(2.0, lod)
		var tile_world_size: float = _CHUNK_METERS * lod_scale
		var dims: Vector2i = self._grid_dims[lod] if lod < self._grid_dims.size() else Vector2i.ONE
		var half_world_x: float = dims.x * tile_world_size * 0.5
		var half_world_z: float = dims.y * tile_world_size * 0.5
		var tile_center_x: float = (tx + 0.5) * tile_world_size - half_world_x
		var tile_center_z: float = (ty + 0.5) * tile_world_size - half_world_z

		var dx: float = tile_center_x - cam_pos.x
		var dz: float = tile_center_z - cam_pos.z
		var dist_sq: float = dx * dx + dz * dz

		var grid_size: int = _LOD_GRID_SIZES[lod] if lod < _LOD_GRID_SIZES.size() else 5
		var threshold: float = grid_size * _EVICT_DISTANCE_MULTIPLIER * tile_world_size
		var threshold_sq: float = threshold * threshold

		if dist_sq > threshold_sq:
			slots_to_evict.append(slot_idx)

	for slot_idx in slots_to_evict:
		var info: Dictionary = self._slot_to_tile[slot_idx]
		var evicted_key: String = "L%d_%d_%d" % [info["lod"], info["x"], info["y"]]

		var lod: int = info["lod"]
		if lod < self._indirection_images.size():
			self._indirection_images[lod].set_pixel(info["x"], info["y"], Color(0, 0, 0, 1))
			self._indirection_textures[lod].update(self._indirection_images[lod])

		self._tile_to_slot.erase(evicted_key)
		self._slot_to_tile.erase(slot_idx)
		self._free_slots.append(slot_idx)

		evicted_count += 1
		self.stats_evictions_this_frame += 1
		self.stats_total_evictions += 1

	if evicted_count > 0:
		Loggie.msg("SVT Stream: [Frame %d] Proactive eviction — freed %d distant tiles, free_slots=%d" % [
			self._frame_count, evicted_count, self._free_slots.size()
		]).domain("svt").channel("devtools").info()


# ── Background Loader Thread ──────────────────────────────────────────────────

func _loader_thread_func() -> void:
	Loggie.msg("SVT Thread: Loader thread started").domain("svt").channel("devtools").info()
	while self._thread_running:
		self._load_semaphore.wait()

		if not self._thread_running:
			break

		# Pop one item from pending
		self._load_mutex.lock()
		if self._pending_loads.is_empty():
			self._load_mutex.unlock()
			continue
		var job: Dictionary = self._pending_loads.pop_front()
		self._load_mutex.unlock()

		var key: String = "L%d_%d_%d" % [job["lod"], job["x"], job["y"]]

		# Load the .res file from disk
		var image: Image = null
		var resource: Resource = ResourceLoader.load(job["path"], "Image", ResourceLoader.CACHE_MODE_IGNORE)
		if resource is Image:
			image = resource as Image

		if image == null:
			Loggie.msg("SVT Thread: FAILED to load '%s' from '%s'" % [key, job["path"]]).domain("svt").channel("devtools").error()
			self._load_mutex.lock()
			self._free_slots.append(job["slot"])
			self._in_flight_keys.erase(key)
			self._load_mutex.unlock()
			continue

		# Push to completed queue
		self._load_mutex.lock()
		self._completed_loads.append({
			"lod": job["lod"],
			"x": job["x"],
			"y": job["y"],
			"slot": job["slot"],
			"image": image,
		})
		self._load_mutex.unlock()

	Loggie.msg("SVT Thread: Loader thread exiting").domain("svt").channel("devtools").info()


# ── Main Thread Upload ────────────────────────────────────────────────────────

func _process_completed_uploads() -> void:
	var start_time: float = Time.get_ticks_msec()

	while true:
		# Check budget
		var elapsed: float = Time.get_ticks_msec() - start_time
		if elapsed >= _UPLOAD_BUDGET_MS:
			break

		# Pop a completed load
		self._load_mutex.lock()
		if self._completed_loads.is_empty():
			self._load_mutex.unlock()
			break
		var job: Dictionary = self._completed_loads.pop_front()
		self._load_mutex.unlock()

		var lod: int = job["lod"]
		var x: int = job["x"]
		var y: int = job["y"]
		var slot: int = job["slot"]
		var image: Image = job["image"]
		var key: String = "L%d_%d_%d" % [lod, x, y]

		# Clear in-flight tracking
		self._load_mutex.lock()
		self._in_flight_keys.erase(key)
		self._load_mutex.unlock()

		# Ensure correct format
		if image.get_format() != Image.FORMAT_RH:
			image.convert(Image.FORMAT_RH)

		# Upload to Texture2DArray
		if self._cache_texture and self._cache_texture.get_layers() > slot:
			self._cache_texture.update_layer(image, slot)
		else:
			Loggie.msg("SVT Stream: Upload '%s' — FAILED: invalid slot %d (cache_layers=%d)" % [key, slot, self._cache_texture.get_layers() if self._cache_texture else 0]).domain("svt").channel("devtools").error()

		# Update bookkeeping
		self._tile_to_slot[key] = slot
		self._slot_to_tile[slot] = {
			"lod": lod,
			"x": x,
			"y": y,
			"frame": Engine.get_process_frames(),
		}

		# Update indirection texture pixel
		if lod < self._indirection_images.size():
			var val: int = slot + 1
			var r: float = float(val % 256) / 255.0
			@warning_ignore("integer_division")
			var g: float = float(val / 256) / 255.0

			self._indirection_images[lod].set_pixel(x, y, Color(r, g, 0, 1))
			self._indirection_textures[lod].update(self._indirection_images[lod])

		self.stats_loads_this_frame += 1
		self.stats_total_loaded += 1
		self.tile_loaded.emit(lod, x, y, slot)


# ── Grid-Based Tile Loading ───────────────────────────────────────────────────

## Load tiles around camera using the grid-based LOD system.
## Loads coarse-to-fine (LOD3 → LOD0) so fallbacks are always available.
func _load_tiles_around_camera() -> void:
	var cam: Camera3D = self.get_viewport().get_camera_3d()
	if cam == null:
		return

	var cam_pos: Vector3 = cam.global_position

	# Load coarse-to-fine: LOD3 first (full-map fallback), then LOD2, LOD1, LOD0
	for lod in range(_LOD_COUNT - 1, -1, -1):
		if lod >= self._grid_dims.size():
			continue

		var dims: Vector2i = self._grid_dims[lod]
		if dims.x <= 0 or dims.y <= 0:
			continue

		var grid_size: int = _LOD_GRID_SIZES[lod] if lod < _LOD_GRID_SIZES.size() else 5
		var divisor: int = 1 << lod
		var tile_world_size: float = _CHUNK_METERS * divisor

		if grid_size < 0:
			# LOD3: Load entire map (once)
			if not self._lod3_loaded:
				self._load_entire_lod(lod, dims)
				self._lod3_loaded = true
			continue

		# Calculate camera's tile coordinate for this LOD
		var total_world_x: float = dims.x * tile_world_size
		var total_world_z: float = dims.y * tile_world_size

		var center_tx: int = floori((cam_pos.x + total_world_x * 0.5) / tile_world_size)
		var center_ty: int = floori((cam_pos.z + total_world_z * 0.5) / tile_world_size)
		center_tx = clampi(center_tx, 0, dims.x - 1)
		center_ty = clampi(center_ty, 0, dims.y - 1)

		# Only re-request grid if camera moved to a different tile
		var cam_tile := Vector2i(center_tx, center_ty)
		if cam_tile == self._last_camera_tile[lod]:
			continue
		self._last_camera_tile[lod] = cam_tile

		# Load the NxN grid centered on the camera tile
		@warning_ignore("integer_division")
		var half_grid: int = grid_size / 2
		for dy in range(-half_grid, half_grid + 1):
			for dx in range(-half_grid, half_grid + 1):
				var tx: int = center_tx + dx
				var ty: int = center_ty + dy
				if tx >= 0 and tx < dims.x and ty >= 0 and ty < dims.y:
					self.request_tile(lod, tx, ty)


## Load all tiles for a LOD level (used for LOD3 full-map coverage).
func _load_entire_lod(lod: int, dims: Vector2i) -> void:
	Loggie.msg("SVT Stream: Loading entire LOD %d — %dx%d = %d tiles" % [lod, dims.x, dims.y, dims.x * dims.y]).domain("svt").channel("devtools").info()
	for ty in range(dims.y):
		for tx in range(dims.x):
			self.request_tile(lod, tx, ty)
