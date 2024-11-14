// shaders/pixel_sort.wgsl

struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
};

struct Params {
    img_width: u32,
    img_height: u32,
    threshold: u32,
};

@group(0) @binding(0)
var<storage, read> input: array<Pixel>;

@group(0) @binding(1)
var<storage, write> output: array<Pixel>;

@group(0) @binding(2)
var<uniform> params: Params;

// Simple insertion sort for small segments
fn insertion_sort(seg: array<Pixel, 1024>, len: u32) -> array<Pixel, 1024> {
    var sorted = seg;
    for (var i = 1u; i < len; i = i + 1u) {
        let key = sorted[i];
        var j = i;
        while (j > 0u && sorted[j - 1u].b > key.b) {
            sorted[j] = sorted[j - 1u];
            j = j - 1u;
        }
        sorted[j] = key;
    }
    return sorted;
}

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let row = global_id.x;

    if (row >= params.img_height) {
        return;
    }

    let img_width = params.img_width;
    let img_height = params.img_height;
    let threshold = params.threshold;

    var out_idx = row * img_width;

    // Temporary storage for sorted pixels
    var temp_sorted: array<Pixel, 1024>;
    var sorted_len: u32 = 0u;

    for (var col = 0u; col < img_width; col = col + 1u) {
        let idx = row * img_width + col;
        let pixel = input[idx];

        if (pixel.r > threshold) {
            // Add to segment
            temp_sorted[sorted_len] = pixel;
            sorted_len = sorted_len + 1u;
        } else {
            if (sorted_len > 0u) {
                // Sort the segment
                let sorted_segment = insertion_sort(temp_sorted, sorted_len);

                // Write sorted pixels to output
                for (var s = 0u; s < sorted_len; s = s + 1u) {
                    output[out_idx] = sorted_segment[s];
                    output[out_idx].a = 255u;
                    out_idx = out_idx + 1u;
                }

                sorted_len = 0u;
            }

            // Write the current pixel as-is
            output[out_idx] = Pixel(pixel.r, pixel.g, pixel.b, 255u);
            out_idx = out_idx + 1u;
        }
    }

    // Handle any remaining sorted segment at the end of the row
    if (sorted_len > 0u) {
        let sorted_segment = insertion_sort(temp_sorted, sorted_len);

        for (var s = 0u; s < sorted_len; s = s + 1u) {
            output[out_idx] = sorted_segment[s];
            output[out_idx].a = 255u;
            out_idx = out_idx + 1u;
        }
    }
}
