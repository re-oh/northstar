/**
 * SVTCacheManager implementation — synced with GDScript version.
 *
 * Key features:
 * - Dynamic cache sizing (exact fit for grid sizes, no wasted VRAM)
 * - Thread pool (4 threads) for parallel tile I/O
 * - Coarse-to-fine loading (LOD3 → LOD0)
 * - Per-frame request throttle (32/frame max)
 * - Only re-requests when camera moves to a new tile
 * - Never evicts tiles loaded in the current frame
 * - LOD3 eviction protection
 * - SIMD-accelerated format conversion / downsampling
 */

#include "svt_cache_manager.h"

#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/classes/engine.hpp>
#include <godot_cpp/classes/time.hpp>
#include <godot_cpp/classes/viewport.hpp>
#include <godot_cpp/classes/json.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>

#include <algorithm>
#include <cstring>
#include <limits>

using namespace godot;

namespace terrain {

// ── Constructor / Destructor ────────────────────────────────────────────────

SVTCacheManager::SVTCacheManager() {
    for (int i = 0; i < LOD_COUNT; i++) {
        last_camera_tile_[i] = Vector2i(-9999, -9999);
    }
}

SVTCacheManager::~SVTCacheManager() {
    stop_loader_threads();
}

// ── Godot Binding ───────────────────────────────────────────────────────────

void SVTCacheManager::_bind_methods() {
    ClassDB::bind_method(D_METHOD("set_map_name", "name"), &SVTCacheManager::set_map_name);
    ClassDB::bind_method(D_METHOD("get_map_name"), &SVTCacheManager::get_map_name);
    ADD_PROPERTY(PropertyInfo(Variant::STRING, "map_name"), "set_map_name", "get_map_name");

    ClassDB::bind_method(D_METHOD("request_tile", "lod", "x", "y"), &SVTCacheManager::request_tile);
    ClassDB::bind_method(D_METHOD("get_cache_texture"), &SVTCacheManager::get_cache_texture);
    ClassDB::bind_method(D_METHOD("get_indirection_texture", "lod"), &SVTCacheManager::get_indirection_texture);
    ClassDB::bind_method(D_METHOD("get_grid_dims", "lod"), &SVTCacheManager::get_grid_dims);
    ClassDB::bind_method(D_METHOD("force_regenerate_cache"), &SVTCacheManager::force_regenerate_cache);

    ClassDB::bind_method(D_METHOD("get_slots_used"), &SVTCacheManager::get_slots_used);
    ClassDB::bind_method(D_METHOD("get_pending_count"), &SVTCacheManager::get_pending_count);
    ClassDB::bind_method(D_METHOD("get_total_loaded"), &SVTCacheManager::get_total_loaded);
    ClassDB::bind_method(D_METHOD("get_total_evictions"), &SVTCacheManager::get_total_evictions);
    ClassDB::bind_method(D_METHOD("get_cache_slots"), &SVTCacheManager::get_cache_slots);

    ADD_SIGNAL(MethodInfo("preprocessing_complete"));
    ADD_SIGNAL(MethodInfo("tile_loaded",
        PropertyInfo(Variant::INT, "lod"),
        PropertyInfo(Variant::INT, "x"),
        PropertyInfo(Variant::INT, "y"),
        PropertyInfo(Variant::INT, "slot")));
}

// ── Property Accessors ──────────────────────────────────────────────────────

void SVTCacheManager::set_map_name(const String& name) {
    map_name_ = name;
}

String SVTCacheManager::get_map_name() const {
    return map_name_;
}

// ── Static Helpers ──────────────────────────────────────────────────────────

int SVTCacheManager::compute_required_slots(const Vector2i grid_dims[], int count) {
    int total = 0;
    for (int lod = 0; lod < count && lod < LOD_COUNT; lod++) {
        int grid_size = LOD_GRID_SIZES[lod];
        if (grid_size < 0) {
            // Full map coverage
            total += grid_dims[lod].x * grid_dims[lod].y;
        } else {
            total += grid_size * grid_size;
        }
    }
    return total;
}

// ── Lifecycle ───────────────────────────────────────────────────────────────

void SVTCacheManager::_ready() {
    cache_dir_ = String("user://cache/maps/") + map_name_;

    bool needs_preprocessing = !DirAccess::dir_exists_absolute(cache_dir_ + "/L0");

    if (needs_preprocessing) {
        UtilityFunctions::print("[SVT Native] Cache not found at ", cache_dir_, "/L0, preprocessing must be done first.");
        return;
    }

    // Load manifest
    load_manifest();

    // Compute dynamic cache size
    cache_slots_ = compute_required_slots(grid_dims_, LOD_COUNT);
    UtilityFunctions::print("[SVT Native] Dynamic cache size = ", cache_slots_,
        " slots (exact fit for grid sizes)");

    // Initialize
    init_physical_cache();
    init_indirection_textures();
    init_free_slots();

    // Reset camera tile tracking
    for (int i = 0; i < LOD_COUNT; i++) {
        last_camera_tile_[i] = Vector2i(-9999, -9999);
    }

    start_loader_threads();

    UtilityFunctions::print("[SVT Native] Initialized — ",
        cache_slots_, " slots, ", LOADER_THREADS, " loader threads");

    emit_signal("preprocessing_complete");
}

void SVTCacheManager::_process(double delta) {
    frame_count_++;
    requests_this_frame_ = 0;

    process_completed_uploads();

    // Coarse-to-fine loading
    load_tiles_around_camera();

    // Periodic eviction (safety only — cache is sized to fit)
    if (frame_count_ % 60 == 0) {
        evict_distant_tiles();
    }
}

void SVTCacheManager::_exit_tree() {
    stop_loader_threads();
}

// ── Initialization ──────────────────────────────────────────────────────────

void SVTCacheManager::load_manifest() {
    String manifest_path = cache_dir_ + "/tile_manifest.json";

    if (!FileAccess::file_exists(manifest_path)) {
        UtilityFunctions::printerr("[SVT Native] Manifest not found: ", manifest_path);
        return;
    }

    Ref<FileAccess> file = FileAccess::open(manifest_path, FileAccess::READ);
    String text = file->get_as_text();

    Ref<JSON> json;
    json.instantiate();
    Error err = json->parse(text);
    if (err != OK) {
        UtilityFunctions::printerr("[SVT Native] JSON parse error in manifest");
        return;
    }

    Dictionary data = json->get_data();
    if (!data.has("lods")) return;

    Array lods = data["lods"];
    for (int i = 0; i < lods.size() && i < LOD_COUNT; i++) {
        Dictionary lod_data = lods[i];
        grid_dims_[i] = Vector2i(
            static_cast<int>(lod_data["cols"]),
            static_cast<int>(lod_data["rows"])
        );
    }

    UtilityFunctions::print("[SVT Native] Manifest loaded — LOD dims: ",
        grid_dims_[0], ", ", grid_dims_[1], ", ", grid_dims_[2], ", ", grid_dims_[3]);
}

void SVTCacheManager::init_physical_cache() {
    TypedArray<Image> images;
    images.resize(cache_slots_);

    for (int i = 0; i < cache_slots_; i++) {
        Ref<Image> img = Image::create(PADDED_SIZE, PADDED_SIZE, false, Image::FORMAT_RH);
        img->fill(Color(0, 0, 0, 1));
        images[i] = img;
    }

    cache_texture_.instantiate();
    cache_texture_->create_from_images(images);
}

void SVTCacheManager::init_indirection_textures() {
    for (int lod = 0; lod < LOD_COUNT; lod++) {
        Vector2i dims = grid_dims_[lod];
        int w = (dims.x > 0) ? dims.x : 1;
        int h = (dims.y > 0) ? dims.y : 1;

        indirection_images_[lod] = Image::create(w, h, false, Image::FORMAT_RG8);
        indirection_images_[lod]->fill(Color(0, 0, 0, 1));
        indirection_textures_[lod] = ImageTexture::create_from_image(indirection_images_[lod]);
    }
}

void SVTCacheManager::init_free_slots() {
    slot_info_.resize(cache_slots_);
    slot_occupied_.resize(cache_slots_, false);
    free_slots_.clear();
    free_slots_.reserve(cache_slots_);
    for (int i = cache_slots_ - 1; i >= 0; i--) {
        free_slots_.push_back(i);
    }
}

// ── Thread Pool ─────────────────────────────────────────────────────────────

void SVTCacheManager::start_loader_threads() {
    threads_running_.store(true, std::memory_order_release);
    for (int i = 0; i < LOADER_THREADS; i++) {
        loader_threads_[i] = std::thread(&SVTCacheManager::loader_thread_func, this);
    }
}

void SVTCacheManager::stop_loader_threads() {
    threads_running_.store(false, std::memory_order_release);
    queue_cv_.notify_all();

    for (int i = 0; i < LOADER_THREADS; i++) {
        if (loader_threads_[i].joinable()) {
            loader_threads_[i].join();
        }
    }
}

void SVTCacheManager::loader_thread_func() {
    while (threads_running_.load(std::memory_order_acquire)) {
        LoadJob job;

        {
            std::unique_lock<std::mutex> lock(queue_mutex_);
            queue_cv_.wait(lock, [this] {
                return !pending_queue_.empty() ||
                       !threads_running_.load(std::memory_order_acquire);
            });

            if (!threads_running_.load(std::memory_order_acquire)) break;
            if (pending_queue_.empty()) continue;

            job = pending_queue_.front();
            pending_queue_.pop_front();
        }

        Ref<Image> image;
        Ref<Resource> resource = ResourceLoader::get_singleton()->load(
            job.path, "Image", ResourceLoader::CACHE_MODE_IGNORE);

        if (resource.is_valid()) {
            image = resource;
        }

        if (image.is_null()) {
            std::lock_guard<std::mutex> lock(queue_mutex_);
            free_slots_.push_back(job.slot);
            in_flight_.erase(job.key);
            continue;
        }

        if (image->get_format() != Image::FORMAT_RH) {
            image->convert(Image::FORMAT_RH);
        }

        {
            std::lock_guard<std::mutex> lock(completed_mutex_);
            completed_queue_.push_back({job.key, job.slot, image});
        }
    }
}

// ── Slot Management ─────────────────────────────────────────────────────────

int SVTCacheManager::evict_lru() {
    uint64_t oldest_frame = std::numeric_limits<uint64_t>::max();
    int oldest_slot = -1;
    uint64_t current_frame = Engine::get_singleton()->get_process_frames();

    for (int i = 0; i < cache_slots_; i++) {
        if (!slot_occupied_[i]) continue;
        if (slot_info_[i].key.lod == 3) continue;  // Never evict LOD3
        if (slot_info_[i].last_frame == current_frame) continue;  // Never evict current frame
        if (slot_info_[i].last_frame < oldest_frame) {
            oldest_frame = slot_info_[i].last_frame;
            oldest_slot = i;
        }
    }

    if (oldest_slot < 0) return -1;

    const auto& evicted = slot_info_[oldest_slot];
    int lod = evicted.key.lod;

    if (lod < LOD_COUNT && indirection_images_[lod].is_valid()) {
        indirection_images_[lod]->set_pixel(evicted.key.x, evicted.key.y, Color(0, 0, 0, 1));
        indirection_textures_[lod]->update(indirection_images_[lod]);
    }

    tile_to_slot_.erase(evicted.key);
    slot_occupied_[oldest_slot] = false;
    total_evictions_.fetch_add(1, std::memory_order_relaxed);

    return oldest_slot;
}

void SVTCacheManager::evict_distant_tiles() {
    Viewport* vp = get_viewport();
    if (!vp) return;
    Camera3D* cam = vp->get_camera_3d();
    if (!cam) return;

    Vector3 cam_pos = cam->get_global_position();
    std::vector<int> to_evict;

    for (int i = 0; i < cache_slots_; i++) {
        if (!slot_occupied_[i]) continue;
        if (slot_info_[i].key.lod == 3) continue;

        int lod = slot_info_[i].key.lod;
        float lod_scale = static_cast<float>(1 << lod);
        float tile_world_size = CHUNK_METERS * lod_scale;

        Vector2i dims = grid_dims_[lod];
        float half_world_x = dims.x * tile_world_size * 0.5f;
        float half_world_z = dims.y * tile_world_size * 0.5f;

        float tile_cx = (slot_info_[i].key.x + 0.5f) * tile_world_size - half_world_x;
        float tile_cz = (slot_info_[i].key.y + 0.5f) * tile_world_size - half_world_z;

        float dx = tile_cx - cam_pos.x;
        float dz = tile_cz - cam_pos.z;
        float dist_sq = dx * dx + dz * dz;

        int grid_size = LOD_GRID_SIZES[lod];
        float threshold = grid_size * 3.0f * tile_world_size;  // 3x multiplier
        float threshold_sq = threshold * threshold;

        if (dist_sq > threshold_sq) {
            to_evict.push_back(i);
        }
    }

    for (int slot : to_evict) {
        const auto& info = slot_info_[slot];

        if (info.key.lod < LOD_COUNT && indirection_images_[info.key.lod].is_valid()) {
            indirection_images_[info.key.lod]->set_pixel(info.key.x, info.key.y, Color(0, 0, 0, 1));
            indirection_textures_[info.key.lod]->update(indirection_images_[info.key.lod]);
        }

        tile_to_slot_.erase(info.key);
        slot_occupied_[slot] = false;
        free_slots_.push_back(slot);
        total_evictions_.fetch_add(1, std::memory_order_relaxed);
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

int SVTCacheManager::request_tile(int lod, int x, int y) {
    TileKey key{lod, x, y};

    auto it = tile_to_slot_.find(key);
    if (it != tile_to_slot_.end()) {
        int slot = it->second;
        slot_info_[slot].last_frame = Engine::get_singleton()->get_process_frames();
        return slot;
    }

    {
        std::lock_guard<std::mutex> lock(queue_mutex_);
        if (in_flight_.count(key)) return -1;
    }

    // Per-frame throttle
    if (requests_this_frame_ >= MAX_REQUESTS_PER_FRAME) return -1;

    if (lod < 0 || lod >= LOD_COUNT) return -1;
    Vector2i dims = grid_dims_[lod];
    if (x < 0 || x >= dims.x || y < 0 || y >= dims.y) return -1;

    // Get free slot
    if (free_slots_.empty()) {
        int evicted = evict_lru();
        if (evicted < 0) return -1;
        free_slots_.push_back(evicted);
    }

    int slot = free_slots_.back();
    free_slots_.pop_back();

    String path = cache_dir_ + "/L" + String::num_int64(lod)
                  + "/" + String::num_int64(x) + "_" + String::num_int64(y) + ".res";

    {
        std::lock_guard<std::mutex> lock(queue_mutex_);
        pending_queue_.push_back({key, slot, path});
        in_flight_.insert(key);
    }
    queue_cv_.notify_one();

    requests_this_frame_++;
    return -1;
}

Ref<Texture2DArray> SVTCacheManager::get_cache_texture() const {
    return cache_texture_;
}

Ref<ImageTexture> SVTCacheManager::get_indirection_texture(int lod) const {
    if (lod >= 0 && lod < LOD_COUNT) return indirection_textures_[lod];
    return Ref<ImageTexture>();
}

Vector2i SVTCacheManager::get_grid_dims(int lod) const {
    if (lod >= 0 && lod < LOD_COUNT) return grid_dims_[lod];
    return Vector2i(0, 0);
}

void SVTCacheManager::force_regenerate_cache() {
    UtilityFunctions::print("[SVT Native] force_regenerate_cache() — delegate to GDScript preprocessor");
}

int SVTCacheManager::get_slots_used() const {
    return cache_slots_ - static_cast<int>(free_slots_.size());
}

int SVTCacheManager::get_pending_count() const {
    return static_cast<int>(pending_queue_.size());
}

int SVTCacheManager::get_total_loaded() const {
    return total_loaded_.load(std::memory_order_relaxed);
}

int SVTCacheManager::get_total_evictions() const {
    return total_evictions_.load(std::memory_order_relaxed);
}

int SVTCacheManager::get_cache_slots() const {
    return cache_slots_;
}

// ── Upload Processing ───────────────────────────────────────────────────────

void SVTCacheManager::process_completed_uploads() {
    std::deque<CompletedJob> batch;
    {
        std::lock_guard<std::mutex> lock(completed_mutex_);
        batch.swap(completed_queue_);
    }

    for (auto& job : batch) {
        {
            std::lock_guard<std::mutex> lock(queue_mutex_);
            in_flight_.erase(job.key);
        }

        if (cache_texture_.is_valid() && job.slot < cache_texture_->get_layers()) {
            cache_texture_->update_layer(job.image, job.slot);
        }

        tile_to_slot_[job.key] = job.slot;
        slot_info_[job.slot] = {job.key, Engine::get_singleton()->get_process_frames()};
        slot_occupied_[job.slot] = true;

        int lod = job.key.lod;
        if (lod < LOD_COUNT && indirection_images_[lod].is_valid()) {
            int val = job.slot + 1;
            float r = static_cast<float>(val % 256) / 255.0f;
            float g = static_cast<float>(val / 256) / 255.0f;
            indirection_images_[lod]->set_pixel(job.key.x, job.key.y, Color(r, g, 0, 1));
            indirection_textures_[lod]->update(indirection_images_[lod]);
        }

        total_loaded_.fetch_add(1, std::memory_order_relaxed);
        emit_signal("tile_loaded", job.key.lod, job.key.x, job.key.y, job.slot);
    }
}

// ── Grid-Based Loading (coarse-to-fine) ─────────────────────────────────────

void SVTCacheManager::load_tiles_around_camera() {
    Viewport* vp = get_viewport();
    if (!vp) return;
    Camera3D* cam = vp->get_camera_3d();
    if (!cam) return;

    Vector3 cam_pos = cam->get_global_position();

    // Coarse-to-fine: LOD3 → LOD2 → LOD1 → LOD0
    for (int lod = LOD_COUNT - 1; lod >= 0; lod--) {
        Vector2i dims = grid_dims_[lod];
        if (dims.x <= 0 || dims.y <= 0) continue;

        int grid_size = LOD_GRID_SIZES[lod];
        int divisor = 1 << lod;
        float tile_world_size = CHUNK_METERS * static_cast<float>(divisor);

        if (grid_size < 0) {
            if (!lod3_loaded_) {
                load_entire_lod(lod);
                lod3_loaded_ = true;
            }
            continue;
        }

        float total_world_x = dims.x * tile_world_size;
        float total_world_z = dims.y * tile_world_size;

        int center_tx = static_cast<int>(std::floor(
            (cam_pos.x + total_world_x * 0.5f) / tile_world_size));
        int center_ty = static_cast<int>(std::floor(
            (cam_pos.z + total_world_z * 0.5f) / tile_world_size));

        center_tx = std::clamp(center_tx, 0, dims.x - 1);
        center_ty = std::clamp(center_ty, 0, dims.y - 1);

        // Only re-request if camera moved to a different tile
        Vector2i cam_tile(center_tx, center_ty);
        if (cam_tile == last_camera_tile_[lod]) continue;
        last_camera_tile_[lod] = cam_tile;

        int half = grid_size / 2;
        for (int dy = -half; dy <= half; dy++) {
            for (int dx = -half; dx <= half; dx++) {
                int tx = center_tx + dx;
                int ty = center_ty + dy;
                if (tx >= 0 && tx < dims.x && ty >= 0 && ty < dims.y) {
                    request_tile(lod, tx, ty);
                }
            }
        }
    }
}

void SVTCacheManager::load_entire_lod(int lod) {
    Vector2i dims = grid_dims_[lod];
    UtilityFunctions::print("[SVT Native] Loading entire LOD ", lod,
        " — ", dims.x, "x", dims.y, " = ", dims.x * dims.y, " tiles");

    for (int ty = 0; ty < dims.y; ty++) {
        for (int tx = 0; tx < dims.x; tx++) {
            request_tile(lod, tx, ty);
        }
    }
}

// ── SIMD Helpers ────────────────────────────────────────────────────────────

void SVTCacheManager::convert_rf_to_rh_simd(
    const float* src, uint16_t* dst, int count)
{
#ifdef __F16C__
    int i = 0;
    const int simd_width = 8;
    for (; i + simd_width <= count; i += simd_width) {
        __m256 floats = _mm256_loadu_ps(src + i);
        __m128i halfs = _mm256_cvtps_ph(floats, _MM_FROUND_TO_NEAREST_INT);
        _mm_storeu_si128(reinterpret_cast<__m128i*>(dst + i), halfs);
    }
    for (; i < count; i++) {
        __m128 f = _mm_set_ss(src[i]);
        __m128i h = _mm_cvtps_ph(f, _MM_FROUND_TO_NEAREST_INT);
        dst[i] = static_cast<uint16_t>(_mm_extract_epi16(h, 0));
    }
#else
    for (int i = 0; i < count; i++) {
        uint32_t f = *reinterpret_cast<const uint32_t*>(&src[i]);
        uint16_t sign = (f >> 16) & 0x8000;
        int exp = ((f >> 23) & 0xFF) - 127 + 15;
        uint16_t frac = (f >> 13) & 0x03FF;

        if (exp <= 0) {
            dst[i] = sign;
        } else if (exp >= 31) {
            dst[i] = sign | 0x7C00;
        } else {
            dst[i] = sign | (static_cast<uint16_t>(exp) << 10) | frac;
        }
    }
#endif
}

void SVTCacheManager::downsample_2x_simd(
    const uint16_t* src, uint16_t* dst,
    int src_width, int src_height)
{
    int dst_width = src_width / 2;
    int dst_height = src_height / 2;

#ifdef __F16C__
    for (int dy = 0; dy < dst_height; dy++) {
        int sy = dy * 2;
        const uint16_t* row0 = src + sy * src_width;
        const uint16_t* row1 = src + (sy + 1) * src_width;
        uint16_t* out = dst + dy * dst_width;

        int dx = 0;
        for (; dx + 8 <= dst_width; dx += 8) {
            int sx = dx * 2;

            __m128i h16_0 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(row0 + sx));
            __m128i h16_1 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(row0 + sx + 8));
            __m256 f_row0_a = _mm256_cvtph_ps(h16_0);
            __m256 f_row0_b = _mm256_cvtph_ps(h16_1);

            h16_0 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(row1 + sx));
            h16_1 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(row1 + sx + 8));
            __m256 f_row1_a = _mm256_cvtph_ps(h16_0);
            __m256 f_row1_b = _mm256_cvtph_ps(h16_1);

            __m256 pair_r0 = _mm256_hadd_ps(f_row0_a, f_row0_b);
            __m256 pair_r1 = _mm256_hadd_ps(f_row1_a, f_row1_b);
            __m256 total = _mm256_add_ps(pair_r0, pair_r1);
            __m256 avg = _mm256_mul_ps(total, _mm256_set1_ps(0.25f));

            __m256d avg_d = _mm256_permute4x64_pd(_mm256_castps_pd(avg), 0xD8);
            avg = _mm256_castpd_ps(avg_d);
            __m128i result = _mm256_cvtps_ph(avg, _MM_FROUND_TO_NEAREST_INT);
            _mm_storeu_si128(reinterpret_cast<__m128i*>(out + dx), result);
        }

        for (; dx < dst_width; dx++) {
            int sx = dx * 2;
            float tl, tr, bl, br;
            __m128i h;

            h = _mm_set1_epi16(row0[sx]);
            tl = _mm_cvtss_f32(_mm_cvtph_ps(h));
            h = _mm_set1_epi16(row0[sx + 1]);
            tr = _mm_cvtss_f32(_mm_cvtph_ps(h));
            h = _mm_set1_epi16(row1[sx]);
            bl = _mm_cvtss_f32(_mm_cvtph_ps(h));
            h = _mm_set1_epi16(row1[sx + 1]);
            br = _mm_cvtss_f32(_mm_cvtph_ps(h));

            float avg_val = (tl + tr + bl + br) * 0.25f;
            __m128 f = _mm_set_ss(avg_val);
            __m128i r = _mm_cvtps_ph(f, _MM_FROUND_TO_NEAREST_INT);
            out[dx] = static_cast<uint16_t>(_mm_extract_epi16(r, 0));
        }
    }
#else
    for (int dy = 0; dy < dst_height; dy++) {
        for (int dx = 0; dx < dst_width; dx++) {
            int sx = dx * 2;
            int sy = dy * 2;
            uint32_t sum = static_cast<uint32_t>(src[sy * src_width + sx])
                         + static_cast<uint32_t>(src[sy * src_width + sx + 1])
                         + static_cast<uint32_t>(src[(sy + 1) * src_width + sx])
                         + static_cast<uint32_t>(src[(sy + 1) * src_width + sx + 1]);
            dst[dy * dst_width + dx] = static_cast<uint16_t>(sum / 4);
        }
    }
#endif
}

} // namespace terrain
