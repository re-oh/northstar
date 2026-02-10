class_name Map extends Resource

var _map_size: float
var _heightmaps: Array[Texture2DArray]
var _chunk_dims: int


# TODO
# 1. load metadata X
# 2. load chunks according to metadata
# 3. using a compute shader generate padding for the chunks
# 4. cache those padded chunks in user://cache/maps/{map_name}/heightmap_cache

static func build_from_folder(folder: String) -> Map:
	Loggie.info("Building map from folder: %s" % folder)
	
	var map := Map.new()
	
	var meta_flag := false
	var map_dir := DirAccess.open(folder)
	
	if map_dir:
		map_dir.list_dir_begin() # init file stream
		if map_dir.file_exists("meta.json"):
			var meta_file := FileAccess.open("meta.json", FileAccess.READ)
			var meta_data := JSON.parse_string(meta_file.get_as_text())
			if meta_data["map_size"]:
				map._map_size = meta_data["map_size"] as float
			else:
				Loggie.error("Cant find map size in meta.json")
			if meta_data["map_size"]:
				map._map_size = meta_data["map_size"] as float
		
	if heightmap_flag:
		Loggie.error("No heightmaps for folder: %s" % folder)


func load_heightmaps(folder) -> bool:
	var heightmap_dir := DirAccess.open("%s/heightmaps" % folder)
			if heightmap_dir:
				heightmap_dir.list_dir_begin()
				var heightmap_filename: String = heightmap_dir.get_next()
				var heightmap_loader_thread = Thread.new()
				heightmap_loader_thread.
