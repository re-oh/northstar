#pragma once
/**
 * SVTCacheManager — High-performance SVT tile cache for Godot terrain.
 *
 * Manages a Texture2DArray VRAM cache + per-LOD indirection textures.
 * Owns a thread pool for async tile I/O. Cache is dynamically sized to 
 * fit exactly the tiles needed (no eviction during normal operation).
 *
 * Exposed to GDScript as a Node so it can replace MissionTerrainStream.
 */

#include <godot_cpp/classes/node3d.hpp>
#include <godot_cpp/classes/image.hpp>
#include <godot_cpp/classes/image_texture.hpp>
#include <godot_cpp/classes/texture2d_array.hpp>
#include <godot_cpp/classes/camera3d.hpp>
#include <godot_cpp/classes/rendering_server.hpp>
#include <godot_cpp/classes/file_access.hpp>
#include <godot_cpp/classes/dir_access.hpp>
#include <godot_cpp/classes/resource_loader.hpp>
#include <godot_cpp/variant/utility_functions.hpp>

#include <atomic>
#include <mutex>
#include <thread>
#include <condition_variable>
#include <vector>
#include <deque>
#include <unordered_map>
#include <unordered_set>
#include <string>
#include <cstring>
#include <cmath>

#ifdef __x86_64__
#include <immintrin.h>
#endif

namespace terrain {

// ── Compile-time constants ──────────────────────────────────────────────────

static constexpr int TILE_SIZE         = 512;
static constexpr int PADDED_SIZE       = 514;
static constexpr float CHUNK_METERS    = 1024.0f;
static constexpr int LOD_COUNT         = 4;
static constexpr int LOADER_THREADS    = 4;
static constexpr int MAX_REQUESTS_PER_FRAME = 32;

// Grid sizes: how many tiles to load around camera per LOD.
// -1 = load entire grid (LOD3).
static constexpr int LOD_GRID_SIZES[LOD_COUNT] = { 5, 9, 13, -1 };

// ── Tile key for O(1) lookups ───────────────────────────────────────────────

struct TileKey {
    int lod;
    int x;
    int y;

    bool operator==(const TileKey& o) const {
        return lod == o.lod && x == o.x && y == o.y;
    }
};

struct TileKeyHash {
    size_t operator()(const TileKey& k) const {
        return std::hash<int>()(k.lod) ^ (std::hash<int>()(k.x) << 10) ^ (std::hash<int>()(k.y) << 20);
    }
};

// ── Slot metadata ───────────────────────────────────────────────────────────

struct SlotInfo {
    TileKey key;
    uint64_t last_frame;
};

// ── Load job ────────────────────────────────────────────────────────────────

struct LoadJob {
    TileKey key;
    int slot;
    godot::String path;
};

struct CompletedJob {
    TileKey key;
    int slot;
    godot::Ref<godot::Image> image;
};

// ── SVTCacheManager class ───────────────────────────────────────────────────

class SVTCacheManager : public godot::Node3D {
    GDCLASS(SVTCacheManager, godot::Node3D)

public:
    SVTCacheManager();
    ~SVTCacheManager() override;

    // Godot lifecycle
    void _ready() override;
    void _process(double delta) override;
    void _exit_tree() override;

    // ── Public API (exposed to GDScript) ────────────────────────────────────

    void set_map_name(const godot::String& name);
    godot::String get_map_name() const;

    int request_tile(int lod, int x, int y);

    godot::Ref<godot::Texture2DArray> get_cache_texture() const;
    godot::Ref<godot::ImageTexture> get_indirection_texture(int lod) const;
    godot::Vector2i get_grid_dims(int lod) const;

    void force_regenerate_cache();

    // ── Stats ───────────────────────────────────────────────────────────────
    int get_slots_used() const;
    int get_pending_count() const;
    int get_total_loaded() const;
    int get_total_evictions() const;
    int get_cache_slots() const;

    // Static helper
    static int compute_required_slots(const godot::Vector2i grid_dims[], int count);

protected:
    static void _bind_methods();

private:
    // ── Config ──────────────────────────────────────────────────────────────
    godot::String map_name_ = "test_map";
    godot::String cache_dir_;
    int cache_slots_ = 0;  // Computed dynamically

    // ── Grid dims per LOD ───────────────────────────────────────────────────
    godot::Vector2i grid_dims_[LOD_COUNT] = {};

    // ── VRAM cache ──────────────────────────────────────────────────────────
    godot::Ref<godot::Texture2DArray> cache_texture_;
    godot::Ref<godot::Image> indirection_images_[LOD_COUNT];
    godot::Ref<godot::ImageTexture> indirection_textures_[LOD_COUNT];

    // ── Slot bookkeeping (dynamic arrays) ───────────────────────────────────
    std::unordered_map<TileKey, int, TileKeyHash> tile_to_slot_;
    std::vector<SlotInfo> slot_info_;
    std::vector<bool> slot_occupied_;
    std::vector<int> free_slots_;

    // ── Per-frame tracking ──────────────────────────────────────────────────
    godot::Vector2i last_camera_tile_[LOD_COUNT];
    int requests_this_frame_ = 0;

    // ── Thread pool ─────────────────────────────────────────────────────────
    std::thread loader_threads_[LOADER_THREADS];
    std::mutex queue_mutex_;
    std::condition_variable queue_cv_;
    std::deque<LoadJob> pending_queue_;
    std::atomic<bool> threads_running_{false};

    // ── Completed uploads ───────────────────────────────────────────────────
    std::mutex completed_mutex_;
    std::deque<CompletedJob> completed_queue_;

    // ── In-flight tracking ──────────────────────────────────────────────────
    std::unordered_set<TileKey, TileKeyHash> in_flight_;

    // ── Stats ───────────────────────────────────────────────────────────────
    std::atomic<int> total_loaded_{0};
    std::atomic<int> total_evictions_{0};
    uint64_t frame_count_ = 0;
    bool lod3_loaded_ = false;

    // ── Internal methods ────────────────────────────────────────────────────
    void init_physical_cache();
    void init_indirection_textures();
    void init_free_slots();
    void load_manifest();
    void start_loader_threads();
    void stop_loader_threads();
    void loader_thread_func();

    int evict_lru();
    void evict_distant_tiles();

    void process_completed_uploads();
    void load_tiles_around_camera();
    void load_entire_lod(int lod);

    // ── SIMD helpers ────────────────────────────────────────────────────────
    static void convert_rf_to_rh_simd(const float* src, uint16_t* dst, int count);
    static void downsample_2x_simd(const uint16_t* src, uint16_t* dst,
                                    int src_width, int src_height);
};

} // namespace terrain
