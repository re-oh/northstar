class_name HeightmapPreprocessor extends RefCounted
## Preprocesses source heightmap chunks into LOD tile caches for the SVT system.
## Source chunks: res://content/maps/{map_name}/heightmaps/{row}_{col}.exr (4096x4096, 16-bit, covers 8x8 tiles)
## Output tiles:  user://cache/maps/{map_name}/L{lod}/{x}_{y}.res (514x514, FORMAT_RH)

const _TILE_SIZE: int = 512
const _PADDED_SIZE: int = 514
const _CHUNK_METERS: float = 1024.0
const _LOD_COUNT: int = 4

## Result of preprocessing — grid dimensions per LOD
var grid_dims: Array[Vector2i] = []

## Total number of tiles generated (not skipped)
var tiles_generated: int = 0

## Total number of tiles skipped (already cached)
var tiles_skipped: int = 0

var progress: float = 0.0
var current_state: String = ""


## Clears the cache directory for the given map name.
func clear_cache(map_name: String) -> void:
	var cache_dir: String = "user://cache/maps/%s" % map_name
	if DirAccess.dir_exists_absolute(cache_dir):
		Loggie.msg("SVT: Clearing cache at %s" % cache_dir).domain("svt").info()
		_recursive_delete_dir(cache_dir)

func _recursive_delete_dir(path: String) -> void:
	var dir = DirAccess.open(path)
	if dir:
		dir.list_dir_begin()
		var fname = dir.get_next()
		while fname != "":
			if dir.current_is_dir():
				if fname != "." and fname != "..":
					_recursive_delete_dir(path + "/" + fname)
			else:
				dir.remove(fname)
			fname = dir.get_next()
		dir.list_dir_end()
		# Remove the now-empty directory
		DirAccess.remove_absolute(path)

## Main entry point. Preprocesses all LODs for the given map.
## Returns OK on success, or an error code.
func preprocess(map_name: String) -> Error:
	var source_dir: String = "res://content/maps/%s/heightmaps" % map_name
	var cache_dir: String = "user://cache/maps/%s" % map_name

	# --- Discover source chunks (Now 4096x4096) ---
	var dir := DirAccess.open(source_dir)
	if dir == null:
		Loggie.msg("SVT: Cannot open source dir: %s" % source_dir).domain("svt").error()
		return ERR_FILE_NOT_FOUND

	var max_meta_row: int = -1
	var max_meta_col: int = -1
	var metachunk_files: Dictionary = {}  # "row_col" -> full path (where row/col are 4k indices)

	dir.list_dir_begin()
	var file_name: String = dir.get_next()
	while file_name != "":
		# Check for .exr
		if not dir.current_is_dir() and file_name.ends_with(".exr"):
			var base: String = file_name.get_basename()
			var parts: PackedStringArray = base.split("_")
			if parts.size() == 2:
				var row: int = parts[0].to_int()
				var col: int = parts[1].to_int()
				max_meta_row = maxi(max_meta_row, row)
				max_meta_col = maxi(max_meta_col, col)
				metachunk_files["%d_%d" % [row, col]] = source_dir + "/" + file_name
		file_name = dir.get_next()
	dir.list_dir_end()

	if max_meta_row < 0 or max_meta_col < 0:
		Loggie.msg("SVT: No valid 4k source chunk files found in %s" % source_dir).domain("svt").error()
		return ERR_FILE_NOT_FOUND

	# Grid is now 8x bigger in tile coordinates than metachunk coordinates
	# Each metachunk = 4096 / 512 = 8 tiles width/height
	var available_grid_rows: int = (max_meta_row + 1) * 8
	var available_grid_cols: int = (max_meta_col + 1) * 8
	
	Loggie.msg("SVT: Found %dx%d 4k source chunks -> covering %dx%d 512px tiles" % [max_meta_row + 1, max_meta_col + 1, available_grid_rows, available_grid_cols]).domain("svt").info()

	# --- Process each LOD ---
	self.grid_dims.clear()
	self.tiles_generated = 0
	self.tiles_skipped = 0
	self._cached_meta_image = null
	self._cached_meta_key = ""

	for lod in range(_LOD_COUNT):
		var divisor: int = 1 << lod  # 1, 2, 4
		var lod_rows: int = ceili(float(available_grid_rows) / divisor)
		var lod_cols: int = ceili(float(available_grid_cols) / divisor)
		self.grid_dims.append(Vector2i(lod_cols, lod_rows))

		var lod_dir: String = "%s/L%d" % [cache_dir, lod]
		self._ensure_dir(lod_dir)

		Loggie.msg("SVT: Processing LOD %d — %dx%d tiles" % [lod, lod_cols, lod_rows]).domain("svt").info()

		var total_tiles_lod = lod_rows * lod_cols
		var processed_in_lod = 0

		for ty in range(lod_rows):
			for tx in range(lod_cols):
				processed_in_lod += 1
				
				# Rough progress calculation: simplified to be linear per LOD for now, 
				# or we can do global progress if we pre-calculate total tiles.
				# Let's do a simple per-LOD progress for the status text, 
				# and use (lod / _LOD_COUNT) + (processed / total) / _LOD_COUNT for global.
				var lod_progress = float(processed_in_lod) / float(total_tiles_lod)
				self.progress = (float(lod) + lod_progress) / float(_LOD_COUNT)
				self.current_state = "LOD %d: %d/%d" % [lod, processed_in_lod, total_tiles_lod]
				
				var tile_path: String = "%s/%d_%d.res" % [lod_dir, tx, ty]

				# Skip if already cached
				if FileAccess.file_exists(tile_path):
					self.tiles_skipped += 1
					continue

				var tile_img: Image = self._build_lod_tile(lod, tx, ty, divisor, available_grid_rows, available_grid_cols, metachunk_files, source_dir)
				if tile_img == null:
					Loggie.msg("SVT: Failed to build tile L%d/%d_%d" % [lod, tx, ty]).domain("svt").error()
					continue

				var err: Error = ResourceSaver.save(tile_img, tile_path)
				if err != OK:
					Loggie.msg("SVT: Failed to save tile %s: %s" % [tile_path, error_string(err)]).domain("svt").error()
					continue

				self.tiles_generated += 1

	# --- Write manifest ---
	var manifest := {
		"map_name": map_name,
		"tile_size": _TILE_SIZE,
		"padded_size": _PADDED_SIZE,
		"lod_count": _LOD_COUNT,
		"lods": []
	}
	for lod in range(_LOD_COUNT):
		manifest["lods"].append({
			"level": lod,
			"cols": self.grid_dims[lod].x,
			"rows": self.grid_dims[lod].y,
		})

	var manifest_path: String = "%s/tile_manifest.json" % cache_dir
	var manifest_file := FileAccess.open(manifest_path, FileAccess.WRITE)
	if manifest_file:
		manifest_file.store_string(JSON.stringify(manifest, "\t"))
		manifest_file.close()

	Loggie.msg("SVT: Preprocessing complete — %d generated, %d skipped" % [self.tiles_generated, self.tiles_skipped]).domain("svt").info()
	return OK


## Build a single LOD tile at virtual coords (tx, ty).
## For LOD 0: tile = source chunk directly.
## For LOD N: composite divisor×divisor source chunks, downscale to 512, pad.
func _build_lod_tile(
	_lod: int, tx: int, ty: int,
	divisor: int, grid_rows: int, grid_cols: int,
	metachunk_files: Dictionary, _source_dir: String
) -> Image:
	if divisor == 1:
		# LOD 0 — single chunk
		return self._load_and_pad_chunk(tx, ty, metachunk_files)

	# LOD 1+ — composite divisor×divisor chunks into one image, downscale, then pad
	var composite_size: int = _TILE_SIZE * divisor
	var composite := Image.create(composite_size, composite_size, false, Image.FORMAT_RH)
	composite.fill(Color(0, 0, 0, 1))

	var base_row: int = ty * divisor
	var base_col: int = tx * divisor

	for dy in range(divisor):
		for dx in range(divisor):
			var src_row: int = base_row + dy
			var src_col: int = base_col + dx
			if src_row >= grid_rows or src_col >= grid_cols:
				continue

			var chunk: Image = self._load_slice_from_metachunk(src_row, src_col, metachunk_files)
			if chunk == null:
				continue

			# Ensure FORMAT_RH for blitting (already done in loader, but safe to keep check)
			if chunk.get_format() != Image.FORMAT_RH:
				chunk.convert(Image.FORMAT_RH)

			var dst_x: int = dx * _TILE_SIZE
			var dst_y: int = dy * _TILE_SIZE
			composite.blit_rect(chunk, Rect2i(0, 0, _TILE_SIZE, _TILE_SIZE), Vector2i(dst_x, dst_y))

	# Downscale to tile size
	composite.resize(_TILE_SIZE, _TILE_SIZE, Image.INTERPOLATE_BILINEAR)

	# Add padding
	return self._add_padding(composite)


## Load a single source chunk, convert to FORMAT_RH, and add padding (for LOD 0).
func _load_and_pad_chunk(
	tx: int, ty: int,
	metachunk_files: Dictionary
) -> Image:
	var chunk: Image = self._load_slice_from_metachunk(ty, tx, metachunk_files)
	if chunk == null:
		# Create a black tile for missing chunks
		chunk = Image.create(_TILE_SIZE, _TILE_SIZE, false, Image.FORMAT_RH)
		chunk.fill(Color(0, 0, 0, 1))

	if chunk.get_format() != Image.FORMAT_RH:
		chunk.convert(Image.FORMAT_RH)

	# Ensure correct size (in case source is slightly different)
	if chunk.get_width() != _TILE_SIZE or chunk.get_height() != _TILE_SIZE:
		chunk.resize(_TILE_SIZE, _TILE_SIZE, Image.INTERPOLATE_BILINEAR)

	return self._add_padding(chunk)


## Cache the last open metachunk to avoid re-opening it 64 times
var _cached_meta_image: Image = null
var _cached_meta_key: String = ""

## Load a 512x512 slice from the source 4k data corresponding to virtual tile coords (row, col)
func _load_slice_from_metachunk(row: int, col: int, metachunk_files: Dictionary) -> Image:
	# 1. Determine which 4k metachunk contains this tile
	# Each metachunk is 4096px = 8 tiles wide/high
	var meta_row: int = row / 8
	var meta_col: int = col / 8
	
	# 2. Determine offset within that metachunk (0-7) -> pixels
	var sub_row: int = row % 8
	var sub_col: int = col % 8
	var src_rect := Rect2i(sub_col * _TILE_SIZE, sub_row * _TILE_SIZE, _TILE_SIZE, _TILE_SIZE)
	
	var meta_key: String = "%d_%d" % [meta_row, meta_col]
	
	# 3. Check cache or load
	var source_img: Image = null
	
	if _cached_meta_key == meta_key and _cached_meta_image != null:
		source_img = _cached_meta_image
	else:
		# Load new metachunk
		var path: String = metachunk_files.get(meta_key, "")
		if path.is_empty():
			return null # Metachunk doesn't exist (e.g. at edges if missing)
			
		var img := Image.new()
		# Use load_from_file to bypass Godot's texture importer and keep 16-bit/32-bit
		var err: Error = img.load(path)
		if err != OK:
			Loggie.msg("SVT: Failed to load 4k chunk %s: %s" % [path, error_string(err)]).domain("svt").error()
			return null
			
		if img.get_format() != Image.FORMAT_RH:
			img.convert(Image.FORMAT_RH)
			
		# Cache it
		_cached_meta_image = img
		_cached_meta_key = meta_key
		source_img = img

	# 4. Extract region
	var slice := Image.create(_TILE_SIZE, _TILE_SIZE, false, Image.FORMAT_RH)
	slice.blit_rect(source_img, src_rect, Vector2i(0, 0))
	
	return slice


## Add 1px edge-clamped padding on all 4 sides: 512×512 → 514×514.
func _add_padding(src: Image) -> Image:
	var padded := Image.create(_PADDED_SIZE, _PADDED_SIZE, false, Image.FORMAT_RH)

	# Copy the core tile into the center (offset by 1,1)
	padded.blit_rect(src, Rect2i(0, 0, _TILE_SIZE, _TILE_SIZE), Vector2i(1, 1))

	# Top edge: copy row 0 of src into row 0 of padded (offset x by 1)
	padded.blit_rect(src, Rect2i(0, 0, _TILE_SIZE, 1), Vector2i(1, 0))

	# Bottom edge: copy last row of src into last row of padded
	padded.blit_rect(src, Rect2i(0, _TILE_SIZE - 1, _TILE_SIZE, 1), Vector2i(1, _PADDED_SIZE - 1))

	# Left edge: copy column 0 of src into column 0 of padded (offset y by 1)
	padded.blit_rect(src, Rect2i(0, 0, 1, _TILE_SIZE), Vector2i(0, 1))

	# Right edge: copy last column of src into last column of padded
	padded.blit_rect(src, Rect2i(_TILE_SIZE - 1, 0, 1, _TILE_SIZE), Vector2i(_PADDED_SIZE - 1, 1))

	# Corners: copy the 4 corner pixels
	var tl: Color = src.get_pixel(0, 0)
	var top_right: Color = src.get_pixel(_TILE_SIZE - 1, 0)
	var bl: Color = src.get_pixel(0, _TILE_SIZE - 1)
	var br: Color = src.get_pixel(_TILE_SIZE - 1, _TILE_SIZE - 1)

	padded.set_pixel(0, 0, tl)
	padded.set_pixel(_PADDED_SIZE - 1, 0, top_right)
	padded.set_pixel(0, _PADDED_SIZE - 1, bl)
	padded.set_pixel(_PADDED_SIZE - 1, _PADDED_SIZE - 1, br)

	return padded


## Ensure a directory exists, creating it recursively if needed.
func _ensure_dir(path: String) -> void:
	if not DirAccess.dir_exists_absolute(path):
		var err: Error = DirAccess.make_dir_recursive_absolute(path)
		if err != OK:
			Loggie.msg("SVT: Failed to create dir %s: %s" % [path, error_string(err)]).domain("svt").error()
