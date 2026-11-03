// The hub's and the results screen's art, one UiMaterial with a `kind` switch (kt.x):
//   0  the scene: the night, the planet Azeroth turning (a texture of its two continents), named stars,
//   1  a plaque: riveted brass frame round dark parchment (a card, a tag, a button) or a speech bubble,
//   2  the scale ring: the ball you start as, inside the ring you must reach,
//   3  the candle: the time limit, burning,
//   4  the verdict medal: a rosette, its rays and its stars (and the comet),
//   5  the commissioner's frame: an arched brass window onto his render target,
//   6  the inset: the Elwynn map, a pin for each level.
// Colours are gamma bytes, as hud.wgsl's are (the player-UI lane composites in gamma).
#import bevy_ui::ui_vertex_output::UiVertexOutput

// kt: x kind, y seconds, z alpha, w kind-specific (scene: level count)
// p:  kind-specific (see each function)
// lv: scene: three vec4 per level (A, B, C); lv[0].x of the other kinds is the node's height in UI
//     units, so a border is the same thickness at any window size.
struct Data {
    kt: vec4<f32>,
    p: vec4<f32>,
    lv: array<vec4<f32>, 24>,
};
@group(1) @binding(0) var<uniform> d: Data;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var tex_sampler: sampler;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;

const CRIMSON = vec3<f32>(0.78, 0.20, 0.16);
const GOLD = vec3<f32>(0.98, 0.80, 0.24);
const AMBER = vec3<f32>(0.93, 0.62, 0.20);
const BLUE = vec3<f32>(0.20, 0.42, 0.80);
const EMERALD = vec3<f32>(0.22, 0.70, 0.42);
const CREAM = vec3<f32>(0.97, 0.92, 0.74);
const VIOLET = vec3<f32>(0.27, 0.19, 0.48);
const BRONZE = vec3<f32>(0.58, 0.42, 0.20);
const BRASS = vec3<f32>(0.80, 0.62, 0.26);
const DARKBRASS = vec3<f32>(0.34, 0.24, 0.11);
const INK = vec3<f32>(0.08, 0.05, 0.03);
const NIGHT = vec3<f32>(0.035, 0.035, 0.105);

fn cover(dist: f32, w: f32) -> f32 {
    return 1.0 - smoothstep(-w, w, dist);
}

fn over(dst: vec4<f32>, src: vec3<f32>, a: f32) -> vec4<f32> {
    let oa = a + dst.a * (1.0 - a);
    let rgb = select((src * a + dst.rgb * dst.a * (1.0 - a)) / oa, vec3<f32>(0.0), oa < 1e-5);
    return vec4<f32>(rgb, oa);
}

fn add(dst: vec4<f32>, src: vec3<f32>, a: f32) -> vec4<f32> {
    return vec4<f32>(dst.rgb + src * a, max(dst.a, clamp(a, 0.0, 1.0)));
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn rot(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

fn wrap_pi(a: f32) -> f32 {
    return a - TAU * floor((a + PI) / TAU);
}

// A four-point sparkle of size `s` at the origin of q, soft at the tips.
fn sparkle(q: vec2<f32>, s: f32) -> f32 {
    let n = pow(abs(q.x) + 1e-5, 0.55) + pow(abs(q.y) + 1e-5, 0.55);
    let v = 1.0 - n / pow(s, 0.55);
    return clamp(v * 1.6, 0.0, 1.0) + 0.5 * exp(-dot(q, q) / (s * s * 0.18));
}

fn seg(q: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = q - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn petal(r: f32, th: f32, r0: f32, amp: f32, n: f32, phase: f32) -> f32 {
    return r - (r0 + amp * cos(n * th + phase));
}

fn sd_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// ---------------------------------------------------------------------------------------------
// 0  The scene: the night, the planet, its stars
// ---------------------------------------------------------------------------------------------
//
// kt.w the number of levels. p: x, y the planet's centre (share of the node), z its radius (node
// heights), w 1 once the map texture is the real one. lv[3i..3i+3] per level: A (state 0 shut,
// 1 open, 2 won; -; focus; reveal), B (-; star ignition; comet; grade+1), C (star x, y in planet
// radii from the centre; -; highlighted). lv[GLOBE]: aim lon, aim lat, beacon lon, beacon lat.
// lv[GLOBE+1]: the inset's left edge x and top and bottom y (node shares), connector alpha.

const GLOBE: i32 = 18;

// The surface direction (x east, y north, z out of the screen at the aim) under a unit screen
// normal `n`, for a globe turned to face (lon0, lat0).
fn globe_dir(n: vec3<f32>, lon0: f32, lat0: f32) -> vec3<f32> {
    let c = cos(lat0);
    let s = sin(lat0);
    let v1 = vec3<f32>(n.x, n.y * c + n.z * s, -n.y * s + n.z * c);
    let cl = cos(lon0);
    let sl = sin(lon0);
    return vec3<f32>(v1.x * cl + v1.z * sl, v1.y, -v1.x * sl + v1.z * cl);
}

// The screen normal of surface point (lon, lat): the inverse of globe_dir.
fn globe_screen(lon: f32, lat: f32, lon0: f32, lat0: f32) -> vec3<f32> {
    let dv = vec3<f32>(cos(lat) * sin(lon), sin(lat), cos(lat) * cos(lon));
    let cl = cos(lon0);
    let sl = sin(lon0);
    let x1 = dv.x * cl - dv.z * sl;
    let z1 = dv.x * sl + dv.z * cl;
    let c = cos(lat0);
    let s = sin(lat0);
    return vec3<f32>(x1, dv.y * c - z1 * s, dv.y * s + z1 * c);
}

fn globe_color(dv: vec3<f32>, t: f32) -> vec3<f32> {
    let lon = atan2(dv.x, dv.z);
    let lat = asin(clamp(dv.y, -1.0, 1.0));
    let uv = vec2<f32>(lon / TAU + 0.5, 0.5 - lat / PI);
    if d.p.w < 0.5 {
        // No map art yet: a painted sea with a few guessed islands.
        let n = vnoise(vec2<f32>(lon * 3.0, lat * 4.0) + 2.0);
        return mix(vec3<f32>(0.10, 0.28, 0.5), vec3<f32>(0.85, 0.65, 0.25), smoothstep(0.62, 0.66, n));
    }
    let e = vec2<f32>(0.35 / 2048.0, 0.35 / 1024.0);
    var c = textureSampleLevel(tex, tex_sampler, uv + vec2<f32>(-e.x, -e.y), 0.0).rgb;
    c += textureSampleLevel(tex, tex_sampler, uv + vec2<f32>(e.x, -e.y), 0.0).rgb;
    c += textureSampleLevel(tex, tex_sampler, uv + vec2<f32>(-e.x, e.y), 0.0).rgb;
    c += textureSampleLevel(tex, tex_sampler, uv + vec2<f32>(e.x, e.y), 0.0).rgb;
    return c * 0.25;
}

fn scene(uv: vec2<f32>, px: vec2<f32>) -> vec4<f32> {
    let aspect = px.x / px.y;
    let t = d.kt.y;
    let n = i32(d.kt.w + 0.5);
    let cen = d.p.xy;
    let u = d.p.z;
    let w = 1.0 / (px.y * u);
    // Planet-radius units, y down, round the planet's centre.
    let q = vec2<f32>((uv.x - cen.x) * aspect, uv.y - cen.y) / u;
    let s = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);
    let lon0 = d.lv[GLOBE].x;
    let lat0 = d.lv[GLOBE].y;
    let r = length(q);

    // --- The night: a deep gradient, a blue glow round the planet, a vignette, and stars. ---
    var c = vec4<f32>(mix(NIGHT * 1.9, vec3<f32>(0.09, 0.06, 0.15), smoothstep(-0.5, 0.6, s.y)), 1.0);
    c = add(c, vec3<f32>(0.08, 0.16, 0.34), exp(-r * r * 0.55) * 0.7);
    c = add(c, vec3<f32>(0.18, 0.08, 0.22), exp(-dot(s - vec2<f32>(0.5, -0.4), s - vec2<f32>(0.5, -0.4)) * 2.5) * 0.35);
    for (var layer = 0; layer < 2; layer++) {
        let scale = select(34.0, 74.0, layer == 1);
        let g = s * scale;
        let cell = floor(g);
        let h = hash21(cell + f32(layer) * 17.0);
        if h > 0.9 {
            let at = vec2<f32>(hash21(cell + 3.1), hash21(cell + 7.7)) * 0.7 + 0.15;
            let dd = length(fract(g) - at) / scale * px.y;
            let tw = 0.55 + 0.45 * sin(t * (1.0 + 2.0 * h) + h * 40.0);
            c = add(c, mix(vec3<f32>(0.7, 0.8, 1.0), vec3<f32>(1.0, 0.9, 0.7), fract(h * 9.0)), cover(dd - 0.9 - 1.2 * f32(layer == 0), 0.8) * tw * select(0.9, 0.55, layer == 1));
        }
    }
    let vig = smoothstep(1.25, 0.35, length(s * vec2<f32>(0.9, 1.1)));
    c = vec4<f32>(c.rgb * (0.45 + 0.55 * vig), 1.0);

    // --- The planet. ---
    var beacon = vec3<f32>(0.0, 0.0, -1.0);
    {
        let nb = globe_screen(d.lv[GLOBE].z, d.lv[GLOBE].w, lon0, lat0);
        beacon = vec3<f32>(nb.x, -nb.y, nb.z);
    }
    // The atmosphere's halo, outside the disc.
    c = add(c, vec3<f32>(0.30, 0.62, 1.0), exp(-max(r - 1.0, 0.0) * 8.0) * 0.55 * select(1.0, 0.0, r < 1.0));
    if r < 1.0 + w * 2.0 {
        let z = sqrt(max(1.0 - r * r, 0.0));
        let nrm = vec3<f32>(q.x, -q.y, z);
        let dv = globe_dir(nrm, lon0, lat0);
        var col = globe_color(dv, t);
        let water = col.b > col.r * 1.02;
        let L = normalize(vec3<f32>(-0.55, 0.5, 0.68));
        let diff = max(dot(nrm, L), 0.0);
        col = col * (0.46 + 0.72 * diff);
        // A glint on the sea, and clouds drifting over everything.
        if water {
            col += vec3<f32>(1.0, 0.95, 0.8) * pow(max(dot(reflect(-L, nrm), vec3<f32>(0.0, 0.0, 1.0)), 0.0), 48.0) * 0.4;
        }
        let lon = atan2(dv.x, dv.z);
        let lat = asin(clamp(dv.y, -1.0, 1.0));
        // Smooth swirls of cloud: sums of sines with whole-number lon frequencies (no seam).
        let c1 = sin(lon * 4.0 + sin(lat * 6.0) * 1.7 + t * 0.05);
        let c2 = sin(lon * 9.0 - lat * 7.0 - t * 0.03);
        let c3 = sin(lat * 11.0 + lon * 2.0 + sin(lon * 5.0) * 1.2);
        let cl = 0.5 + 0.28 * c1 + 0.14 * c2 + 0.1 * c3;
        col = mix(col, vec3<f32>(1.0), smoothstep(0.66, 0.9, cl) * 0.32 * (0.35 + diff));
        // The atmosphere on the limb.
        col += vec3<f32>(0.32, 0.62, 1.0) * pow(1.0 - z, 2.4) * 0.75;
        c = over(c, col, cover(r - 1.0, w * 1.5));
    }
    // The beacon over Elwynn: a lens ring and a pulsing gold point, if it faces us.
    if beacon.z > 0.04 {
        let qb = beacon.xy;
        let vis = smoothstep(0.04, 0.3, beacon.z);
        let rb = length(q - qb);
        let pulse = fract(t * 0.7);
        c = add(c, vec3<f32>(1.0, 0.85, 0.35), exp(-rb * rb * 600.0) * 0.8 * vis);
        c = over(c, vec3<f32>(1.0, 0.92, 0.55), cover(rb - 0.016, w) * vis);
        c = over(c, INK, cover(abs(rb - 0.016) - 0.004, w) * 0.5 * vis);
        c = add(c, vec3<f32>(1.0, 0.85, 0.35), cover(abs(rb - (0.03 + pulse * 0.08)) - 0.004, w) * (1.0 - pulse) * vis);
        let ang = atan2(q.y - qb.y, q.x - qb.x);
        let dash = step(0.0, sin(ang * 8.0 - t * 1.4));
        c = over(c, vec3<f32>(1.0, 0.9, 0.5), cover(abs(rb - 0.13) - 0.005, w) * dash * 0.85 * vis);
        // The beam to the inset: a fan from the lens to the inset's left edge.
        let alpha = d.lv[GLOBE + 1].w;
        if alpha > 0.01 {
            let ax = vec2<f32>((d.lv[GLOBE + 1].x - cen.x) * aspect, d.lv[GLOBE + 1].y - cen.y) / u;
            let ay = vec2<f32>((d.lv[GLOBE + 1].x - cen.x) * aspect, d.lv[GLOBE + 1].z - cen.y) / u;
            // The fan leaves the lens ring's right side.
            let o = qb + vec2<f32>(0.1, 0.0);
            let e0 = (ax.x - o.x) * (q.y - o.y) - (ax.y - o.y) * (q.x - o.x);
            let e1 = (ay.x - ax.x) * (q.y - ax.y) - (ay.y - ax.y) * (q.x - ax.x);
            let e2 = (o.x - ay.x) * (q.y - ay.y) - (o.y - ay.y) * (q.x - ay.x);
            let inside = (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0);
            if inside && r > 0.0 {
                c = add(c, vec3<f32>(1.0, 0.85, 0.4), 0.06 * alpha * vis);
            }
            for (var k = 0; k < 2; k++) {
                let target_pt = select(ax, ay, k == 1);
                let dd = seg(q, o, target_pt);
                let along = length(q - o) / max(length(target_pt - o), 1e-3);
                let dashes = step(0.0, sin(along * 60.0 - t * 5.0));
                c = add(c, vec3<f32>(1.0, 0.88, 0.5), cover(dd - 0.004, w) * 0.5 * dashes * alpha * vis);
            }
        }
    }

    // --- The named stars: levels won. Each flies up from the beacon when it is lit. ---
    var prev = vec3<f32>(0.0, 0.0, -1.0);
    for (var i = 0; i < n; i++) {
        let A = d.lv[i * 3];
        let B = d.lv[i * 3 + 1];
        let C = d.lv[i * 3 + 2];
        if A.x < 1.5 {
            prev = vec3<f32>(0.0, 0.0, -1.0);
            continue;
        }
        let g = B.y;
        let origin = select(vec2<f32>(0.0, 0.0), beacon.xy, beacon.z > 0.04);
        let k = 1.0 - pow(1.0 - clamp(g, 0.0, 1.0), 3.0);
        let along = mix(origin, C.xy, k) + vec2<f32>(0.0, -0.25 * sin(k * PI));
        let size = mix(0.05, 0.1, k) * (1.0 + 0.2 * sin(t * 2.3 + f32(i)));
        let tw = 0.8 + 0.2 * sin(t * 3.0 + f32(i) * 2.0);
        let sp = sparkle(q - along, size) * tw;
        c = add(c, mix(CREAM, GOLD, 0.45), sp);
        c = add(c, vec3<f32>(0.9, 0.7, 0.25), exp(-dot(q - along, q - along) / (size * size * 5.0)) * 0.22 * k);
        if g > 0.8 && g < 1.0 {
            let rp = (g - 0.8) / 0.2;
            c = add(c, GOLD, cover(abs(length(q - C.xy) - rp * 0.3) - 0.007, w) * (1.0 - rp));
        }
        if prev.z > 0.0 && g >= 1.0 {
            let dd = seg(q, prev.xy, C.xy);
            let dash = step(0.0, sin(length(q - prev.xy) * 40.0));
            c = add(c, vec3<f32>(0.9, 0.8, 0.5), cover(dd - 0.004, w) * 0.35 * dash);
        }
        if g >= 1.0 {
            prev = vec3<f32>(C.xy, 1.0);
        } else {
            prev = vec3<f32>(0.0, 0.0, -1.0);
        }
    }

    // --- The Royal Fireworks (D33): bursts of sparks over the planet when all three parcels are
    // found. lv[GLOBE + 3].x is the seconds since the hub opened while they play, negative when off.
    let fw = d.lv[GLOBE + 3].x;
    if fw >= 0.0 {
        for (var b = 0; b < 6; b++) {
            let fb = f32(b);
            let age = fw - fb * 0.5;
            if age > 0.0 && age < 2.0 {
                let ang = hash21(vec2<f32>(fb, 5.0)) * TAU;
                let rad = 0.5 + 0.55 * hash21(vec2<f32>(fb, 9.0));
                let centre = vec2<f32>(cos(ang), sin(ang) * 0.8) * rad;
                let fade = pow(1.0 - age / 2.0, 1.5);
                let hue = mix(mix(GOLD, CRIMSON, step(0.5, hash21(vec2<f32>(fb, 2.0)))), CREAM, step(0.8, hash21(vec2<f32>(fb, 4.0))));
                let rocket = clamp(age * 5.0, 0.0, 1.0);
                // The rocket climbs from below, then bursts.
                if age < 0.2 {
                    let up = centre + vec2<f32>(0.0, (1.0 - rocket) * 0.9);
                    c = add(c, CREAM, exp(-dot(q - up, q - up) * 900.0) * 0.9);
                } else {
                    for (var k = 0; k < 20; k++) {
                        let fk = f32(k);
                        let th = fk / 20.0 * TAU + fb;
                        let sp = 0.28 + 0.12 * hash21(vec2<f32>(fk, fb));
                        let at = centre + vec2<f32>(cos(th), sin(th)) * sp * (1.0 - exp(-(age - 0.2) * 2.6)) + vec2<f32>(0.0, 0.07 * (age - 0.2) * (age - 0.2));
                        let dq = q - at;
                        c = add(c, hue, exp(-dot(dq, dq) * 5200.0) * fade);
                        c = add(c, CREAM, sparkle(dq, 0.025) * fade * 0.5);
                    }
                }
            }
        }
    }
    return c;
}

// ---------------------------------------------------------------------------------------------
// 6  The inset: the Elwynn map under a brass rim, a pin for each level
// ---------------------------------------------------------------------------------------------
//
// kt.w the number of levels. p: x, y the map point at the centre (share of the map), z zoom,
// w a pop (1 falling to 0) when the choice changes. lv[GLOBE].x the node's height in UI units;
// per level: A (state, -, focus, reveal), B (pin's offset x, y from its map point; comet, grade+1), C (pin x, y on the map; -;
// highlighted).

const PIN_UP: f32 = 15.0;
const PIN_R: f32 = 9.0;

// A five-point star of outer radius r, inner factor rf (y up).
fn sd_star5(p0: vec2<f32>, r: f32, rf: f32) -> f32 {
    let k1 = vec2<f32>(0.809016994375, -0.587785252292);
    let k2 = vec2<f32>(-k1.x, k1.y);
    var p = vec2<f32>(abs(p0.x), p0.y);
    p = p - 2.0 * max(dot(k1, p), 0.0) * k1;
    p = p - 2.0 * max(dot(k2, p), 0.0) * k2;
    p = vec2<f32>(abs(p.x), p.y - r);
    let ba = rf * vec2<f32>(-k1.y, k1.x) - vec2<f32>(0.0, 1.0);
    let h = clamp(dot(p, ba) / dot(ba, ba), 0.0, r);
    return length(p - ba * h) * sign(p.y * ba.x - p.x * ba.y);
}

// The line from a moved pin's tip back to its place on the map, and a ring on that place. B.xy is
// how far the pin was moved (UI units), so the map point is at -B.xy from the tip.
fn pin_lead(i: i32, pos: vec2<f32>, unit: f32, c: vec4<f32>) -> vec4<f32> {
    let off = d.lv[i * 3 + 1].xy;
    if length(off) < 3.0 {
        return c;
    }
    let center = (d.lv[i * 3 + 2].xy - d.p.xy) * d.p.z;
    let local = pos / unit - (center * d.lv[GLOBE + 2].xy + off);
    let w = 1.0 / unit;
    let q = -off;
    let ln = seg(local, vec2<f32>(0.0), q);
    let rg = length(local - q);
    var out = c;
    out = over(out, INK, cover(ln - 1.6, w) * 0.5);
    out = over(out, INK, cover(rg - 4.2, w) * 0.5);
    out = over(out, vec3<f32>(0.99, 0.94, 0.72), cover(ln - 0.7, w) * 0.95);
    out = over(out, vec3<f32>(0.99, 0.94, 0.72), cover(abs(rg - 2.6) - 0.8, w));
    return out;
}

fn pin_head(i: i32, pos: vec2<f32>, unit: f32, t: f32, c: vec4<f32>, selected_pass: bool) -> vec4<f32> {
    var out = c;
    let A = d.lv[i * 3];
    let B = d.lv[i * 3 + 1];
    let C = d.lv[i * 3 + 2];
    let sel = C.w > 0.5;
    if sel != selected_pass {
        return out;
    }
    let state = A.x;
    let focus = A.z;
    let reveal = A.w;
    let center = (C.xy - d.p.xy) * d.p.z;
    // The pin's tip, in UI units from the node's centre: its map point, moved off it by B.xy when
    // another pin stands too close.
    let tip = center * d.lv[GLOBE + 2].xy + B.xy;
    let local = pos / unit - tip;
    let s = 1.0 + 0.3 * focus;
    let p = local / s;
    if abs(p.x) > 40.0 || p.y > 12.0 || p.y < -60.0 {
        return out;
    }
    let w = 1.0 / (unit * s);
    var locked = state < 0.5;
    var head = vec3<f32>(0.98, 0.80, 0.24);
    var rim = vec3<f32>(1.0, 0.95, 0.7);
    if locked {
        head = vec3<f32>(0.36, 0.38, 0.50);
        rim = vec3<f32>(0.62, 0.6, 0.66);
    } else if state > 1.5 {
        head = vec3<f32>(0.30, 0.74, 0.42);
        rim = vec3<f32>(0.98, 0.85, 0.35);
    }
    let open = select(0.0, smoothstep(0.0, 0.4, reveal), reveal < 1.0 && state > 0.5);
    // The ground shadow, a pulse ring for a request that is open, the pin itself.
    out = over(out, INK, cover(length((p - vec2<f32>(0.0, 0.0)) / vec2<f32>(7.0, 2.6)) - 1.0, w) * 0.4);
    if state > 0.5 && state < 1.5 {
        let pu = fract(t * 0.8 + f32(i) * 0.3);
        out = add(out, vec3<f32>(1.0, 0.85, 0.35), cover(abs(length(p - vec2<f32>(0.0, -PIN_UP)) - (PIN_R + pu * 14.0)) - 0.8, w) * (1.0 - pu) * (0.4 + 0.6 * focus));
    }
    if focus > 0.01 {
        out = add(out, vec3<f32>(1.0, 0.85, 0.4), exp(-dot(p - vec2<f32>(0.0, -PIN_UP), p - vec2<f32>(0.0, -PIN_UP)) / 260.0) * 0.45 * focus);
    }
    // The tail, a triangle from the head to the tip.
    let ta = vec2<f32>(-6.0, -PIN_UP + 5.0);
    let tb = vec2<f32>(6.0, -PIN_UP + 5.0);
    let tc = vec2<f32>(0.0, 0.0);
    let e0 = (tb.x - ta.x) * (p.y - ta.y) - (tb.y - ta.y) * (p.x - ta.x);
    let e1 = (tc.x - tb.x) * (p.y - tb.y) - (tc.y - tb.y) * (p.x - tb.x);
    let e2 = (ta.x - tc.x) * (p.y - tc.y) - (ta.y - tc.y) * (p.x - tc.x);
    let tri = (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0);
    let hd = length(p - vec2<f32>(0.0, -PIN_UP)) - PIN_R;
    let shape = min(hd, select(1e3, -1.0, tri));
    out = over(out, INK, cover(shape - 1.4, w) * 0.7);
    out = over(out, head * (0.75 + 0.25 * smoothstep(PIN_R, -PIN_R, p.y + PIN_UP)), cover(shape, w));
    out = over(out, rim, cover(abs(hd + 1.2) - 0.9, w));
    // A won level's gold star on its pin's shoulder (it must not read as a letter beside the
    // numeral), and a spark if it beat par.
    if state > 1.5 {
        let bc = vec2<f32>(PIN_R * 0.95, -PIN_UP - PIN_R * 0.95);
        let sq = p - bc;
        let st = sd_star5(vec2<f32>(sq.x, -sq.y - 0.8), 7.6 * (1.0 + 0.05 * sin(t * 3.0)), 0.46);
        out = over(out, INK, cover(st - 1.5, w) * 0.85);
        out = over(out, vec3<f32>(1.0, 0.84, 0.18), cover(st, w));
        out = over(out, vec3<f32>(1.0, 0.97, 0.75), cover(abs(st + 1.4) - 0.5, w) * 0.6);
        if B.z > 0.5 {
            out = add(out, vec3<f32>(0.75, 0.9, 1.0), sparkle(p - (bc + vec2<f32>(11.0, -9.0 + 2.0 * sin(t * 3.0))), 6.0) * 0.9);
        }
    }
    // A padlock over a shut level's head; it hops and fades when the level opens.
    if locked || reveal < 1.0 {
        let hop = open * open * 20.0;
        let shake = select(0.0, 1.2 * sin(reveal * 70.0), reveal < 0.4 && reveal > 0.0 && state > 0.5);
        let lq = p - vec2<f32>(shake, -PIN_UP + 1.0 - hop);
        let alpha = 1.0 - smoothstep(0.4, 0.9, reveal) * select(0.0, 1.0, state > 0.5);
        let body = sd_box(lq - vec2<f32>(0.0, 1.5), vec2<f32>(4.6, 3.6), 1.0);
        let shackle = abs(length(lq - vec2<f32>(0.0, -2.2)) - 3.0) - 0.9;
        let sh = max(shackle, lq.y + 2.2);
        out = over(out, INK, cover(min(body, sh) - 0.9, w) * 0.5 * alpha);
        out = over(out, mix(BRASS, GOLD, 0.4), cover(min(body, sh), w) * alpha);
        out = over(out, INK, cover(length(lq - vec2<f32>(0.0, 1.5)) - 0.9, w) * alpha);
    }
    // The unlock burst.
    if reveal > 0.0 && reveal < 1.0 && state > 0.5 {
        let k = reveal;
        let hc = p - vec2<f32>(0.0, -PIN_UP);
        out = add(out, GOLD, cover(abs(length(hc) - (PIN_R + k * 34.0)) - 1.0, w) * (1.0 - k));
        for (var m = 0; m < 14; m++) {
            let fm = f32(m);
            let dir = vec2<f32>(cos(fm * 0.449 + 0.3), sin(fm * 0.449 + 0.3));
            let dist = (6.0 + hash21(vec2<f32>(fm, 4.0)) * 34.0) * (1.0 - pow(1.0 - k, 2.0));
            let pp = dir * (PIN_R + dist);
            out = add(out, mix(GOLD, CREAM, hash21(vec2<f32>(fm, 9.0))), exp(-dot(hc - pp, hc - pp) / 6.0) * (1.0 - k) * 1.3);
        }
    }
    return out;
}

fn inset(uv: vec2<f32>, px: vec2<f32>) -> vec4<f32> {
    let unit = px.y / max(d.lv[GLOBE].x, 1.0);
    let t = d.kt.y;
    let n = i32(d.kt.w + 0.5);
    let pos = (uv - 0.5) * px;
    let half = px * 0.5;
    let boxd = sd_box(pos, half - vec2<f32>(3.0 * unit), 9.0 * unit);
    let inside = cover(boxd, 1.0);
    let zoom = d.p.z * (1.0 + 0.04 * d.p.w * d.p.w);
    let m = clamp(d.p.xy + (uv - 0.5) / zoom, vec2<f32>(0.0), vec2<f32>(1.0));
    var col = textureSampleLevel(tex, tex_sampler, m, 0.0).rgb;
    if d.kt.w < 0.5 {
        col = vec3<f32>(0.6, 0.5, 0.3);
    }
    // A touch of vignette and glass.
    let vg = smoothstep(0.9, 0.35, length((uv - 0.5) * vec2<f32>(1.0, 1.0)));
    col = col * (0.72 + 0.34 * vg);
    var c = vec4<f32>(0.0);
    c = over(c, col, inside);
    // The leader lines of pins that were moved apart, under every pin.
    for (var i = 0; i < n; i++) {
        c = pin_lead(i, pos, unit, c);
    }
    // Pins; the chosen one last, on top of the rest.
    for (var pass_i = 0; pass_i < 2; pass_i++) {
        for (var i = 0; i < n; i++) {
            c = pin_head(i, pos, unit, t, c, pass_i == 1);
        }
    }
    // The brass rim with rivets.
    let fw = 4.0 * unit;
    let shade = clamp(0.5 + 0.9 * (-pos.x / px.x - pos.y / px.y), 0.0, 1.0);
    let brass = mix(DARKBRASS, mix(BRASS, vec3<f32>(1.0, 0.9, 0.55), 0.35), shade);
    c = over(c, brass, cover(abs(boxd + fw * 0.5) - fw * 0.5, 1.0));
    c = over(c, INK, cover(abs(boxd + fw + 1.2 * unit) - 0.7 * unit, 1.0) * 0.5);
    let ri = 9.0 * unit;
    let rp = abs(pos) - half + vec2<f32>(ri);
    let rd = length(rp) - 2.7 * unit;
    c = over(c, INK, cover(rd - 0.9 * unit, 1.0) * 0.5);
    c = over(c, mix(DARKBRASS, vec3<f32>(1.0, 0.92, 0.6), 0.6), cover(rd, 1.0));
    return c;
}

// ---------------------------------------------------------------------------------------------
// 1  Plaques
// ---------------------------------------------------------------------------------------------

// p.x style (0 card, 1 tag, 2 speech bubble, 3 button), p.y lit 0..1, p.z dim 0..1, p.w the
// bubble's tail height as a share of its height.
fn plaque(uv: vec2<f32>, px: vec2<f32>) -> vec4<f32> {
    let unit = px.y / max(d.lv[0].x, 1.0);
    let style = i32(d.p.x + 0.5);
    let lit = d.p.y;
    let dim = d.p.z;
    let t = d.kt.y;
    let pos = (uv - 0.5) * px;
    let half = px * 0.5 - vec2<f32>(0.0);
    let radius = select(10.0, 8.0, style == 1 || style == 3) * unit;
    let w = 1.0;
    var inset = 0.0;
    var tail = 0.0;
    var tail_edge = 0.0;
    if style == 2 {
        // Leave room for the tail on the left.
        inset = 14.0 * unit;
    }
    let boxd = sd_box(pos - vec2<f32>(inset * 0.5, 0.0), half - vec2<f32>(inset * 0.5, 0.0) - vec2<f32>(2.0 * unit), radius);
    var c = vec4<f32>(0.0);
    // Outer glow for a lit tag or a hovered button.
    if lit > 0.0 {
        c = add(c, vec3<f32>(1.0, 0.75, 0.25), exp(-max(boxd, 0.0) / (6.0 * unit)) * 0.5 * lit * step(0.0, boxd));
    }
    if style == 2 {
        let ty = (d.p.w - 0.5) * px.y;
        let tx = -half.x + 2.0 * unit;
        let a = vec2<f32>(tx + inset, ty - 9.0 * unit);
        let b = vec2<f32>(tx, ty);
        let cc = vec2<f32>(tx + inset, ty + 9.0 * unit);
        // The tail, a triangle: inside when the three edge tests agree.
        let e0 = (b.x - a.x) * (pos.y - a.y) - (b.y - a.y) * (pos.x - a.x);
        let e1 = (cc.x - b.x) * (pos.y - b.y) - (cc.y - b.y) * (pos.x - b.x);
        let e2 = (a.x - cc.x) * (pos.y - cc.y) - (a.y - cc.y) * (pos.x - cc.x);
        let inside = (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0);
        tail = select(0.0, 1.0, inside);
        let edge = min(seg(pos, a, b), seg(pos, b, cc));
        tail_edge = cover(edge - 1.4 * unit, w) ;
    }
    let shape = max(cover(boxd, w), tail);
    // The fill.
    var fill = vec3<f32>(0.12, 0.085, 0.055);
    let grain = vnoise(pos * 0.4 / unit) * 0.03 + vnoise(pos * 0.07 / unit) * 0.05;
    if style == 2 {
        fill = vec3<f32>(0.97, 0.91, 0.76) - grain * 1.5;
    } else if style == 1 || style == 3 {
        fill = mix(vec3<f32>(0.16, 0.10, 0.06), vec3<f32>(0.34, 0.22, 0.09), lit) + grain;
        fill = mix(fill, vec3<f32>(0.62, 0.40, 0.12), lit * 0.35 * (0.5 + 0.5 * sin(t * 3.0)) * step(2.5, f32(style) + 0.0 * lit));
    } else {
        fill = mix(vec3<f32>(0.17, 0.115, 0.07), vec3<f32>(0.09, 0.06, 0.04), uv.y) + grain;
    }
    fill = fill * (1.0 - 0.45 * dim);
    c = over(c, fill, shape);
    // Brass frame: bright on the top left, dark on the bottom right.
    let frame_w = select(2.6, 2.2, style == 1) * unit;
    let ring = cover(abs(boxd + frame_w * 0.5) - frame_w * 0.5, w);
    let shade = clamp(0.5 + 0.9 * (-pos.x / px.x - pos.y / px.y), 0.0, 1.0);
    var brass = mix(DARKBRASS, mix(BRASS, vec3<f32>(1.0, 0.9, 0.55), 0.35), shade);
    brass = mix(brass, GOLD, lit * 0.6);
    brass = brass * (1.0 - 0.5 * dim);
    c = over(c, brass, ring);
    c = over(c, INK, cover(abs(boxd + frame_w + 1.4 * unit) - 0.6 * unit, w) * 0.5 * select(1.0, 0.0, style == 2));
    if style == 2 {
        // The tail covers the bubble's border where it joins, and wears its own.
        c = over(c, fill, tail * (1.0 - tail_edge));
        c = over(c, brass, tail_edge * tail * 0.0);
        c = over(c, brass, tail_edge * step(0.5, tail + cover(abs(boxd) - 3.0 * unit, 1.0)) * 0.0);
    }
    // Rivets at the corners of a card.
    if style == 0 || style == 2 {
        let ri = 9.0 * unit;
        let rp = abs(pos - vec2<f32>(inset * 0.5, 0.0)) - (half - vec2<f32>(inset * 0.5, 0.0)) + vec2<f32>(ri);
        let rd = length(rp) - 2.7 * unit;
        c = over(c, INK, cover(rd - 0.9 * unit, w) * 0.5);
        c = over(c, mix(DARKBRASS, vec3<f32>(1.0, 0.92, 0.6), smoothstep(2.7 * unit, -2.0 * unit, rd + dot(rp, vec2<f32>(0.5, 0.7)) * 0.5)), cover(rd, w));
    }
    return c;
}

// ---------------------------------------------------------------------------------------------
// 2  The scale ring    p.x the start ball's share of the ring (0..1), p.y a pulse
// ---------------------------------------------------------------------------------------------
fn scale_ring(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let q = (uv - 0.5) * 2.0;
    let r = length(q);
    let t = d.kt.y;
    let th = atan2(q.y, q.x);
    var c = vec4<f32>(0.0);
    c = over(c, vec3<f32>(0.07, 0.06, 0.12), cover(r - 0.94, w) * 0.92);
    // The goal ring: brass with tick marks, turning slowly.
    c = over(c, INK, cover(abs(r - 0.82) - 0.13, w) * 0.5);
    c = over(c, mix(BRASS, GOLD, 0.5 + 0.5 * sin(th * 2.0 + t * 0.6)), cover(abs(r - 0.82) - 0.075, w));
    let tick = step(0.8, abs(sin(th * 12.0 + t * 0.3))) * cover(abs(r - 0.82) - 0.1, w);
    c = over(c, INK, tick * 0.55);
    // The jewel on the ring: where the goal sits.
    let ja = vec2<f32>(cos(-0.9 + 0.1 * sin(t)), sin(-0.9 + 0.1 * sin(t))) * 0.82;
    c = over(c, CRIMSON, cover(length(q - ja) - 0.12, w));
    c = over(c, CREAM, cover(length(q - ja) - 0.04, w) * 0.8);
    // The ball you start as: a small katamari with a swirl, inside.
    let br = clamp(d.p.x, 0.08, 0.6) * (1.0 + 0.04 * sin(t * 2.0));
    let v = q / br;
    let vr = length(v);
    if vr < 1.0 + w / br {
        let z = sqrt(max(1.0 - vr * vr, 0.0));
        let nrm = vec3<f32>(v.x, v.y, z);
        let diff = max(dot(nrm, normalize(vec3<f32>(-0.5, -0.7, 0.6))), 0.0);
        let sw = 0.5 + 0.5 * sin(atan2(v.y, v.x) * 3.0 + vr * 7.0 - t * 1.5);
        var col = mix(vec3<f32>(0.55, 0.28, 0.70), vec3<f32>(0.95, 0.6, 0.25), sw);
        col = col * (0.3 + 0.8 * diff) + vec3<f32>(0.5, 0.7, 1.0) * pow(1.0 - z, 3.0) * 0.25;
        c = over(c, col, cover(vr - 1.0, w / br));
    }
    // A dotted line from the ball to the ring.
    let dd = abs(q.y + 0.0) ;
    let dot_line = cover(dd - 0.012, w) * step(br * 1.15, q.x) * step(q.x, 0.74) * step(0.0, sin(q.x * 60.0));
    c = over(c, CREAM, dot_line * 0.5);
    return c;
}

// ---------------------------------------------------------------------------------------------
// 3  The candle    p.x the candle's height (0.3..1), p.y a spare
// ---------------------------------------------------------------------------------------------
fn candle(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let q = (uv - 0.5) * 2.0;
    let t = d.kt.y;
    var c = vec4<f32>(0.0);
    c = over(c, vec3<f32>(0.07, 0.06, 0.12), cover(length(q) - 0.94, w) * 0.92);
    let h = clamp(d.p.x, 0.3, 1.0);
    let base_y = 0.62;
    let top_y = base_y - 1.0 * h * 0.95;
    let half_w = 0.2;
    // A brass dish.
    c = over(c, INK, cover(sd_box(q - vec2<f32>(0.0, base_y + 0.04), vec2<f32>(0.46, 0.06), 0.05) - 0.03, w) * 0.5);
    c = over(c, mix(BRASS, DARKBRASS, smoothstep(-0.46, 0.46, q.x)), cover(sd_box(q - vec2<f32>(0.0, base_y + 0.04), vec2<f32>(0.46, 0.06), 0.05), w));
    // The wax: cream, shaded round, with a drip.
    let wax = sd_box(q - vec2<f32>(0.0, (base_y + top_y) * 0.5), vec2<f32>(half_w, (base_y - top_y) * 0.5), 0.03);
    let shade = 0.78 + 0.28 * (1.0 - smoothstep(0.0, 1.0, abs(q.x + 0.05) / half_w));
    c = over(c, CREAM * shade, cover(wax, w));
    let drip = cover(sd_box(q - vec2<f32>(0.1, top_y + 0.1), vec2<f32>(0.035, 0.1), 0.03), w);
    c = over(c, CREAM * 0.95, drip);
    // The pool of melt at the top.
    c = over(c, vec3<f32>(1.0, 0.9, 0.6), cover(length((q - vec2<f32>(0.0, top_y)) / vec2<f32>(half_w, 0.05)) - 1.0, w) * 0.9);
    // The wick and the flame, swaying.
    let sway = 0.03 * sin(t * 7.0) + 0.02 * sin(t * 13.0);
    c = over(c, INK, cover(seg(q, vec2<f32>(0.0, top_y), vec2<f32>(0.0, top_y - 0.1)) - 0.012, w));
    let fq = (q - vec2<f32>(sway * 1.5, top_y - 0.26)) / vec2<f32>(1.0, 1.0);
    let flame_r = length(vec2<f32>(fq.x * 1.0, fq.y * 0.62));
    let flame = flame_r - 0.115 * (1.0 + 0.1 * sin(t * 9.0)) + 0.2 * max(-fq.y - 0.04, 0.0) * 0.0;
    let taper = 0.18 * smoothstep(-0.02, -0.26, fq.y);
    let fd = length(vec2<f32>(fq.x * (1.0 + taper * 6.0), fq.y * 0.6)) - 0.12;
    c = add(c, vec3<f32>(1.0, 0.6, 0.15), exp(-dot(fq, fq) * 16.0) * 0.45);
    c = over(c, vec3<f32>(1.0, 0.55, 0.12), cover(fd, w * 2.0));
    c = over(c, vec3<f32>(1.0, 0.92, 0.55), cover(fd + 0.05, w * 2.0));
    let _u = flame;
    return c;
}

// ---------------------------------------------------------------------------------------------
// 4  The verdict medal    p.x grade 0..3, p.y stamp progress 0..1, p.z comet 0/1, p.w new-best flash
// ---------------------------------------------------------------------------------------------
fn medal(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let t = d.kt.y;
    let grade = i32(d.p.x + 0.5);
    let k = clamp(d.p.y, 0.0, 1.0);
    let comet = d.p.z;
    var q = (uv - 0.5) * 2.0;
    // The stamp slams in: big and quick, settling with a little bounce.
    let slam = 1.0 + 0.9 * pow(1.0 - k, 3.0) + 0.05 * sin(k * 22.0) * (1.0 - k) * select(0.0, 1.0, k < 1.0);
    q = q * slam;
    let r = length(q);
    let th = atan2(q.x, -q.y);
    var c = vec4<f32>(0.0);
    var main = CRIMSON;
    var edge = GOLD;
    var inner = vec3<f32>(0.33, 0.07, 0.07);
    if grade == 0 {
        main = BRONZE;
        edge = vec3<f32>(0.78, 0.62, 0.4);
        inner = vec3<f32>(0.22, 0.15, 0.08);
    } else if grade == 1 {
        main = EMERALD;
        edge = CREAM;
        inner = vec3<f32>(0.08, 0.24, 0.16);
    } else if grade == 2 {
        main = BLUE;
        edge = GOLD;
        inner = vec3<f32>(0.08, 0.14, 0.34);
    } else {
        main = vec3<f32>(0.62, 0.22, 0.78);
        edge = GOLD;
        inner = vec3<f32>(0.20, 0.07, 0.30);
    }
    // Rays behind, turning; more of them, brighter, for a higher grade.
    let rays_n = 8.0 + 4.0 * f32(grade);
    let ray = pow(0.5 + 0.5 * sin(th * rays_n + t * 0.35), 3.0) * smoothstep(1.35, 0.5, r) * smoothstep(0.55, 0.75, r);
    c = add(c, mix(edge, CREAM, 0.4), ray * (0.15 + 0.1 * f32(grade)) * k);
    // The rosette.
    let base = 0.72;
    c = over(c, INK, cover(petal(r, th, base + 0.05, 0.05, 20.0, t * 0.2), w) * 0.5);
    c = over(c, edge, cover(petal(r, th, base, 0.045, 20.0, t * 0.2), w));
    c = over(c, main, cover(petal(r, th, base - 0.04, 0.04, 20.0, t * 0.2), w));
    c = over(c, edge, cover(abs(r - 0.56) - 0.014, w) * 0.9);
    c = over(c, inner, cover(r - 0.54, w));
    c = over(c, vec3<f32>(1.0), cover(abs(r - 0.5) - 0.006, w) * 0.15);
    // The stars of the grade, along the bottom of the disc.
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let at = vec2<f32>((fi - 1.5) * 0.2, 0.26);
        let earned = i <= grade;
        let sd = length(q - at);
        let ang = atan2(q.y - at.y, q.x - at.x);
        let rr = 0.075 * (0.6 + 0.4 * (0.5 + 0.5 * cos(ang * 5.0 + 1.57)));
        let on = select(0.0, 1.0, earned);
        c = over(c, mix(vec3<f32>(0.2, 0.17, 0.2), GOLD, on), cover(sd - rr * 1.35, w));
        if earned {
            c = add(c, CREAM, 0.35 * pow(0.5 + 0.5 * sin(t * 4.0 + fi * 1.3), 4.0) * cover(sd - rr, w));
        }
    }
    // The centre mark: a stylised world with a ring.
    let wq = q - vec2<f32>(0.0, -0.08);
    let wr = length(wq);
    let wz = sqrt(max(1.0 - (wr / 0.24) * (wr / 0.24), 0.0));
    let wdiff = max(dot(vec3<f32>(wq / 0.24, wz), normalize(vec3<f32>(-0.5, -0.7, 0.6))), 0.0);
    c = over(c, mix(main, CREAM, 0.35) * (0.35 + 0.8 * wdiff), cover(wr - 0.24, w));
    let ringd = abs(length(vec2<f32>(wq.x, wq.y * 3.2)) - 0.38) - 0.02;
    c = over(c, edge, cover(ringd, w) * select(1.0, 0.0, wq.y > 0.0 && wr < 0.26));
    // The comet: a streak across the upper right with a bright head, if under par.
    if comet > 0.5 {
        let ck = fract(t * 0.35);
        let head = mix(vec2<f32>(1.1, -1.1), vec2<f32>(0.62, -0.62), smoothstep(0.0, 0.5, ck));
        let dirv = normalize(vec2<f32>(1.0, -1.0));
        let rel = q - head;
        let along = dot(rel, dirv);
        let across = length(rel - dirv * along);
        let tail = exp(-across * across * 400.0) * smoothstep(0.0, 0.7, along) * exp(-along * 3.0) * 1.0;
        let vis = smoothstep(0.0, 0.1, ck) * (1.0 - smoothstep(0.55, 0.8, ck));
        c = add(c, vec3<f32>(0.75, 0.9, 1.0), (tail + exp(-dot(rel, rel) * 220.0) * 1.2) * vis * k);
    }
    // A new best: a ring of pulses.
    if d.p.w > 0.01 {
        c = add(c, GOLD, cover(abs(r - (0.9 + 0.1 * fract(t))) - 0.012, w) * d.p.w * (1.0 - fract(t)));
    }
    c.a = c.a * smoothstep(0.0, 0.25, k);
    return c;
}

// ---------------------------------------------------------------------------------------------
// 5  The commissioner's frame    p.x pulse, p.y the speaking light
// ---------------------------------------------------------------------------------------------
fn portrait(uv: vec2<f32>, px: vec2<f32>) -> vec4<f32> {
    let unit = px.y / max(d.lv[0].x, 1.0);
    let t = d.kt.y;
    let pos = (uv - 0.5) * px;
    let half = px * 0.5;
    // An arched window: a rounded box with a big radius at the top only.
    let top = pos.y < -half.y * 0.2;
    let rad = select(10.0 * unit, half.x * 0.9, top);
    let boxd = sd_box(pos, half - vec2<f32>(2.0 * unit), rad);
    var c = vec4<f32>(0.0);
    let inside = cover(boxd, 1.0);
    // Behind him: the workshop wall, warm at the bottom, with two great cogs turning.
    var wall = mix(vec3<f32>(0.20, 0.13, 0.09), vec3<f32>(0.09, 0.07, 0.14), uv.y * 0.9);
    wall = wall + vec3<f32>(0.18, 0.1, 0.03) * exp(-dot(uv - vec2<f32>(0.5, 0.75), uv - vec2<f32>(0.5, 0.75)) * 5.0);
    c = over(c, wall, inside);
    for (var g = 0; g < 2; g++) {
        let gc = select(vec2<f32>(0.18, 0.26), vec2<f32>(0.86, 0.46), g == 1) * px;
        let gr = select(0.34, 0.24, g == 1) * px.x;
        let gp = pos + half - gc;
        let ga = atan2(gp.y, gp.x) + t * select(0.25, -0.34, g == 1);
        let gd = length(gp) - gr * (1.0 + 0.07 * step(0.0, sin(ga * select(14.0, 10.0, g == 1))));
        let hole = abs(length(gp) - gr * 0.62) - gr * 0.05;
        c = over(c, vec3<f32>(0.42, 0.30, 0.15), cover(max(gd, -hole), 1.0) * inside * 0.28);
        c = over(c, vec3<f32>(0.1, 0.07, 0.04), cover(length(gp) - gr * 0.12, 1.0) * inside * 0.28);
    }
    // A glow when he is talking.
    c = add(c, vec3<f32>(1.0, 0.7, 0.3), exp(-pow(length((uv - vec2<f32>(0.5, 0.45)) * vec2<f32>(1.0, 0.8)), 2.0) * 7.0) * 0.16 * d.p.y * inside);
    // Him.
    let m = textureSample(tex, tex_sampler, vec2<f32>(uv.x, uv.y));
    c = over(c, m.rgb, m.a * inside);
    // The frame.
    let fw = 4.0 * unit;
    let shade = clamp(0.5 + 0.9 * (-pos.x / px.x - pos.y / px.y), 0.0, 1.0);
    let brass = mix(DARKBRASS, mix(BRASS, vec3<f32>(1.0, 0.9, 0.55), 0.35), shade);
    c = over(c, brass, cover(abs(boxd + fw * 0.5) - fw * 0.5, 1.0));
    c = over(c, INK, cover(abs(boxd + fw + 1.2 * unit) - 0.7 * unit, 1.0) * 0.55);
    // Rivets along the arch.
    for (var i = 0; i < 5; i++) {
        let a = (f32(i) - 2.0) * 0.5;
        let rp = vec2<f32>(sin(a), -cos(a)) * (half.x - 9.0 * unit) + vec2<f32>(0.0, -half.y + half.x + 0.0);
        let on = step(rp.y, -half.y * 0.2);
        let rd = length(pos - rp) - 2.4 * unit;
        c = over(c, mix(DARKBRASS, vec3<f32>(1.0, 0.92, 0.6), 0.6), cover(rd, 1.0) * on);
    }
    return c;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let w = 2.0 / max(in.size.x, 1.0);
    let kind = i32(d.kt.x + 0.5);
    var c = vec4<f32>(0.0);
    if kind == 0 {
        c = scene(in.uv, in.size);
    } else if kind == 1 {
        c = plaque(in.uv, in.size);
    } else if kind == 2 {
        c = scale_ring(in.uv, w);
    } else if kind == 3 {
        c = candle(in.uv, w);
    } else if kind == 4 {
        c = medal(in.uv, w);
    } else if kind == 5 {
        c = portrait(in.uv, in.size);
    } else {
        c = inset(in.uv, in.size);
    }
    return vec4<f32>(c.rgb, c.a * d.kt.z);
}
