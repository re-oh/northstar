@tool
extends SceneTree

func _init() -> void:
	print("--- SVT Data Diagnostics ---")
	var map_name := "test_map"
	var cache_dir := "user://cache/maps/%s" % map_name
	
	var dir := DirAccess.open(cache_dir)
	if not dir:
		print("ERROR: Cache directory not found: %s" % cache_dir)
		quit()
		return
		
	# Check L0 tiles
	var l0_dir := cache_dir + "/L0"
	var files := Array(DirAccess.get_files_at(l0_dir))
	if files.is_empty():
		print("ERROR: No tiles found in L0 cache: %s" % l0_dir)
	else:
		print("Found %d tiles in L0. Checking L0/35_35.res..." % files.size())
		var files_to_check = ["35_35.res"]
		for i in range(files_to_check.size()):
			var fname: String = files_to_check[i]
			if not fname.ends_with(".res"): continue
			
			var path: String = l0_dir + "/" + fname
			var res := ResourceLoader.load(path)
			if res is Image:
				var img: Image = res
				var fmt := img.get_format()
				print("Tile %s: Format=%d (RH=%d), Size=%v" % [fname, fmt, Image.FORMAT_RH, img.get_size()])
				
				# Analyze pixels
				var min_h: float = 1.0
				var max_h: float = 0.0
				var avg_h: float = 0.0
				var non_zero: int = 0
				var total: int = img.get_width() * img.get_height()
				
				for y in range(0, img.get_height(), 10): # skip step for speed
					for x in range(0, img.get_width(), 10):
						var h: float = img.get_pixel(x, y).r
						if h > 0.0:
							non_zero += 1
							if h < min_h: min_h = h
							if h > max_h: max_h = h
							avg_h += h
				
				if non_zero > 0:
					avg_h /= non_zero
				
				print("  - Stats (sampled): Non-Zero=%d/%d, Min=%.4f, Max=%.4f, Avg=%.4f" % [non_zero, (total/100), min_h, max_h, avg_h])
				
				if max_h < 0.001:
					print("  ! WARNING: Tile appears flat/empty.")
			else:
				print("ERROR: Failed to load image resource: %s" % path)

	# Check source PNG directly
	print("\nChecking source PNG 35_35.png...")
	var src_path := "content/maps/%s/heightmaps/35_35.png" % map_name
	var src_img := Image.load_from_file(src_path)
	if src_img:
		print("Source PNG Format: %d, Size: %v" % [src_img.get_format(), src_img.get_size()])
		var min_h: float = 1.0
		var max_h: float = 0.0
		var avg_h: float = 0.0
		var non_zero: int = 0
		var total: int = src_img.get_width() * src_img.get_height()
		
		for y in range(0, src_img.get_height(), 10):
			for x in range(0, src_img.get_width(), 10):
				var h: float = src_img.get_pixel(x, y).r
				if h > 0.0:
					non_zero += 1
					if h < min_h: min_h = h
					if h > max_h: max_h = h
					avg_h += h
		if non_zero > 0: avg_h /= non_zero
		print("  - Stats (sampled): Non-Zero=%d/%d, Min=%.4f, Max=%.4f, Avg=%.4f" % [non_zero, (total/100), min_h, max_h, avg_h])
	else:
		print("ERROR: Failed to load source PNG: %s" % src_path)

	quit()
