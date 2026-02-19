@tool
extends SceneTree

func _init() -> void:
    print("--- SVT Logic Verification ---")

    # 1. Test Indirection Encoding/Decoding (RG8)
    var test_slots = [0, 1, 10, 254, 255, 256, 500, 1023]
    var img = Image.create(1, 1, false, Image.FORMAT_RG8)
    
    print("\n[Test 1] Indirection Encoding/Decoding (RG8)")
    for slot in test_slots:
        # Encode
        # slot index is 1-based in shader logic (0=empty), but we store (slot+1)
        # However, let's test storing valid slot range 0-1023.
        # We store (slot + 1) to reserve 0 for "empty".
        var enc_val = slot + 1
        
        var r_byte = enc_val % 256
        var g_byte = enc_val / 256
        
        # Set pixel (normalized float)
        img.set_pixel(0, 0, Color(r_byte / 255.0, g_byte / 255.0, 0, 1))
        
        # Read back
        var col = img.get_pixel(0, 0)
        # Re-quantize to 0-255 range (handle float precision)
        var dec_r = floor(col.r * 255.0 + 0.5)
        var dec_g = floor(col.g * 255.0 + 0.5)
        
        var decoded_val = int(dec_r) + int(dec_g) * 256
        var decoded_slot = decoded_val - 1
        
        if decoded_slot == slot:
            print("  [Pass] Slot %4d -> Enc(%3d, %3d) -> Decoded %4d" % [slot, r_byte, g_byte, decoded_slot])
        else:
            print("  [FAIL] Slot %4d -> Enc(%3d, %3d) -> Decoded %4d" % [slot, r_byte, g_byte, decoded_slot])

    # 2. Test Texture2DArray Update
    print("\n[Test 2] Texture2DArray Update")
    var tex = Texture2DArray.new()
    var base_img = Image.create(2, 2, false, Image.FORMAT_L8)
    base_img.fill(Color(0, 0, 0, 1))
    var layers = [base_img, base_img] # 2 layers
    tex.create_from_images(layers)
    
    # Check initial
    var before = tex.get_layer_data(1)
    if before:
        print("  Layer 1 initial pixel: %.2f" % before.get_pixel(0, 0).r)
    
    # Update layer 1
    var new_img = Image.create(2, 2, false, Image.FORMAT_L8)
    new_img.fill(Color(1, 1, 1, 1)) # White
    tex.update_layer(new_img, 1)
    
    # Check after
    var after = tex.get_layer_data(1) # This pulls from server
    if after:
        print("  Layer 1 updated pixel: %.2f" % after.get_pixel(0, 0).r)
        if after.get_pixel(0, 0).r > 0.9:
            print("  [Pass] Texture update successful via update_layer()")
        else:
            print("  [FAIL] Texture update failed via update_layer()")
    else:
         print("  [FAIL] Failed to retrieve layer data")

    quit()
