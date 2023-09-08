struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(1) color: vec3<f32>,
};

struct FragmentInput {
    @builtin(position) clip_position: vec4<f32>,
};

struct Uniforms {
    width: f32,
    height: f32,
    zoom: f32,
    center_x: f32,
    center_y: f32,
    max_iterations: i32,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

@vertex
fn vs_main(
    model: VertexInput,
) -> FragmentInput {
    var out: FragmentInput;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    let XMIN = -2.5;
    let XMAX = 1.0;
    let YMIN = -1.0;
    let YMAX = 1.0;

    let scale = vec2<f32>((XMAX - XMIN) / u.zoom / u.width, (YMAX - YMIN) / u.zoom / u.height);
    let c = in.clip_position.xy * scale + vec2<f32>(u.center_x, u.center_y);
    var z = vec2<f32>(0.0, 0.0);
    var iterations = 0;

    while iterations < u.max_iterations && dot(z, z) <= 4.0 {
        z = vec2<f32>(z.x*z.x - z.y*z.y, 2.0*z.x*z.y) + c;
        iterations = iterations + 1;
    }

    let normalized_iter = f32(iterations) / f32(u.max_iterations);
    let hue = normalized_iter * 0.5;
    let color = hsv_to_rgb(hue, 1.0, 0.7);

    // Apply a threshold based on some criterion, e.g., the normalized iteration count
    let mask = step(0.5, normalized_iter); // threshold value here is 0.5; adjust as needed

    let final_color = color * mask; // Element-wise multiplication
    return vec4<f32>(final_color, 1.0);
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let c = v * s;
    let x = c * (1.0 - abs((h * 6.0) % 2.0 - 1.0));
    let m = v - c;
    
    var color: vec3<f32>;

    if (h < 1.0/6.0) {
        color = vec3<f32>(c, x, 0.0);
    } else if (h < 2.0/6.0) {
        color = vec3<f32>(x, c, 0.0);
    } else if (h < 3.0/6.0) {
        color = vec3<f32>(0.0, c, x);
    } else if (h < 4.0/6.0) {
        color = vec3<f32>(0.0, x, c);
    } else if (h < 5.0/6.0) {
        color = vec3<f32>(x, 0.0, c);
    } else {
        color = vec3<f32>(c, 0.0, x);
    }

    return color + m;
}
