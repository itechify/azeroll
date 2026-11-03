// Azeroll's HUD widgets, one UiMaterial with a `kind` switch:
//   0  the size dial: a layered rosette with the goal ring around it,
//   1  the clock: a pie that drains as the time runs out,
//   2  a model bubble: a render-target image in a round, translucent glass,
//   3  a guide arrow: a soft chevron pointing up, turned by the UI node,
//   4  Fizzwick's porthole: the commissioner's portrait in a brass-rimmed round window.
// Colours are written as gamma bytes (the player-UI lane composites in gamma, `ui_gamma.wgsl` decodes
// once), and the bubble's texture is the booth target, which already holds gamma.
#import bevy_ui::ui_vertex_output::UiVertexOutput

// x kind, y seconds, z alpha, w pulse (a brief flash on growth; kind 2: the rim flash)
@group(1) @binding(0) var<uniform> kt: vec4<f32>;
// kind 0 (and 10 + stars lit + the newest star's pop 0..0.99, the dial after a size goal):
//   x goal fill 0..1, y celebrate 0..1, z ring tick 1..0 (a growth beat), w bloom 1..0 (a tier)
// kind 1: x elapsed fraction 0..1, y urgency 0..1, z free roll (1), w finish key held 0..1
// kind 2: x rim (0 glass, 1 gold), yzw the flash colour (strength is kt.w)
// kind 3: xyz the arrow's colour
@group(1) @binding(1) var<uniform> p: vec4<f32>;
@group(1) @binding(2) var tex: texture_2d<f32>;
@group(1) @binding(3) var tex_sampler: sampler;

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
const INK = vec3<f32>(0.08, 0.05, 0.03);

// Coverage of `d < 0` with one pixel of anti-aliasing.
fn cover(d: f32, w: f32) -> f32 {
    return 1.0 - smoothstep(-w, w, d);
}

// Over-composite `src` (rgb, a) onto `dst`.
fn over(dst: vec4<f32>, src: vec3<f32>, a: f32) -> vec4<f32> {
    let oa = a + dst.a * (1.0 - a);
    let rgb = select((src * a + dst.rgb * dst.a * (1.0 - a)) / oa, vec3<f32>(0.0), oa < 1e-5);
    return vec4<f32>(rgb, oa);
}

// A scalloped disc: radius `r0` wobbling by `amp` with `n` petals.
fn petal(r: f32, th: f32, r0: f32, amp: f32, n: f32, phase: f32) -> f32 {
    return r - (r0 + amp * cos(n * th + phase));
}

// A five-point star of outer radius `r` at `c`, point up on screen (screen y runs down).
fn star_d(q: vec2<f32>, c: vec2<f32>, r: f32) -> f32 {
    let k1 = vec2<f32>(0.809016994375, -0.587785252292);
    let k2 = vec2<f32>(-k1.x, k1.y);
    var pp = (q - c) / r;
    pp = vec2<f32>(abs(pp.x), -pp.y);
    pp = pp - 2.0 * max(dot(k1, pp), 0.0) * k1;
    pp = pp - 2.0 * max(dot(k2, pp), 0.0) * k2;
    pp.x = abs(pp.x);
    pp.y = pp.y - 1.0;
    let rf = 0.45;
    let ba = rf * vec2<f32>(-k1.y, k1.x) - vec2<f32>(0.0, 1.0);
    let h = clamp(dot(pp, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pp - ba * h) * sign(pp.y * ba.x - pp.x * ba.y) * r;
}

fn dial(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let q = (uv - 0.5) * 2.0;
    let r = length(q);
    let th = atan2(q.x, -q.y); // 0 at the top, clockwise
    let t = kt.y;
    let fill = p.x;
    let cel = p.y;
    let tick = p.z;
    let bloom = p.w;
    var c = vec4<f32>(0.0);

    // The ring: an arc from lower-left over the top to lower-right.
    let a0 = -2.35;
    let a1 = 2.35;
    let ring_in = 0.885;
    let ring_out = 0.975;
    let ring_d = max(ring_in - r, r - ring_out);
    let in_arc = th >= a0 && th <= a1;
    let track = cover(ring_d, w) * select(0.0, 1.0, in_arc);
    c = over(c, INK, track * 0.62);
    // Bronze edge lines on the track.
    let edge = cover(abs(r - 0.93) - 0.047, w) - cover(abs(r - 0.93) - 0.036, w);
    c = over(c, BRONZE, edge * select(0.0, 1.0, in_arc) * 0.9);
    // The fill, gold with a travelling shine; it turns celebratory crimson-gold when won.
    let th_fill = a0 + (a1 - a0) * fill;
    let filled = in_arc && th <= th_fill;
    let band = cover(abs(r - 0.93) - 0.032, w) * select(0.0, 1.0, filled);
    let shine = 0.5 + 0.5 * sin(th * 9.0 - t * (2.5 + 6.0 * cel));
    var fcol = mix(AMBER, GOLD, 0.45 + 0.35 * shine);
    fcol = mix(fcol, CREAM, cel * 0.5 * shine);
    c = over(c, fcol, band);
    // The goal jewel at the end of the ring.
    let ga = vec2<f32>(sin(a1), -cos(a1)) * 0.93;
    let gd = length(q - ga);
    c = over(c, INK, cover(gd - 0.085, w) * 0.9);
    let jewel = select(CRIMSON, GOLD, fill >= 0.999);
    c = over(c, jewel, cover(gd - 0.062, w));
    c = over(c, CREAM, cover(gd - 0.022, w) * 0.8);
    // A growth beat: a bright pulse runs along the filled arc and the ring swells at its head.
    if tick > 0.001 && fill > 0.002 {
        let sweep = a0 + (th_fill - a0) * (1.0 - tick * tick);
        let near = pow(max(0.0, 1.0 - abs(th - sweep) * 5.0), 2.0);
        let pulse_band = cover(abs(r - 0.93) - 0.05, w) * near * select(0.0, 1.0, filled);
        c = over(c, CREAM, pulse_band * tick);
        let swell = cover(abs(r - 0.93) - 0.05 * tick, w) * select(0.0, 1.0, filled) * 0.25 * tick;
        c = over(c, CREAM, swell);
    }
    // The head of the fill: a bright bead that rides it.
    if fill > 0.002 && fill < 0.999 {
        let ha = vec2<f32>(sin(th_fill), -cos(th_fill)) * 0.93;
        let hd = length(q - ha);
        c = over(c, CREAM, cover(hd - 0.05, w) * 0.95);
    }

    // The rosette. Petal layers, outermost first, each turning slowly and the other way round.
    let spin = t * (0.15 + 0.9 * cel);
    let flash = kt.w;
    var rose = vec4<f32>(0.0);
    let base = 0.80;
    rose = over(rose, INK, cover(petal(r, th, base + 0.045, 0.045, 16.0, spin), w) * 0.55);
    rose = over(rose, CRIMSON, cover(petal(r, th, base, 0.045, 16.0, spin), w));
    rose = over(rose, GOLD, cover(petal(r, th, base - 0.065, 0.045, 16.0, PI - spin * 0.6), w));
    rose = over(rose, EMERALD, cover(petal(r, th, base - 0.13, 0.04, 12.0, spin * 0.8), w));
    rose = over(rose, CREAM, cover(petal(r, th, base - 0.19, 0.035, 12.0, PI - spin * 0.5), w));
    rose = over(rose, BLUE, cover(petal(r, th, base - 0.25, 0.03, 10.0, spin), w));
    // The centre: a deep violet disc under the number, with a pale rim and a soft inner glow.
    rose = over(rose, GOLD, cover(r - 0.545, w));
    rose = over(rose, VIOLET, cover(r - 0.52, w));
    rose = over(rose, vec3<f32>(0.42, 0.30, 0.68), cover(r - 0.52, 0.2) * smoothstep(0.1, 0.52, r) * 0.55);
    // Sparkles while celebrating.
    let spark = pow(max(0.0, sin(th * 12.0 + t * 5.0)), 6.0) * smoothstep(0.55, 0.82, r) * cel;
    rose = over(rose, CREAM, spark * 0.6 * rose.a);
    // Flash on growth.
    rose = vec4<f32>(rose.rgb + flash * 0.35 * rose.a, rose.a);
    c = over(c, rose.rgb, rose.a);
    // The stars after a size goal: gold, in a row under the number, the newest popping in.
    let stars = floor(kt.x) - 10.0;
    if stars >= 1.0 {
        let pop = fract(kt.x) / 0.99;
        for (var i = 0; i < 4; i++) {
            let fi = f32(i);
            if fi < stars {
                let cx = (fi - (stars - 1.0) * 0.5) * 0.2;
                let newest = select(0.0, 1.0, fi == stars - 1.0);
                let sz = 0.085 * (1.0 + 0.6 * pop * newest);
                let sd = star_d(q, vec2<f32>(cx, 0.33), sz);
                c = over(c, INK, cover(sd - 0.02, w) * 0.8);
                c = over(c, mix(GOLD, CREAM, 0.35 + 0.4 * pop * newest), cover(sd, w));
            }
        }
    }
    // A tier's bloom: petal rings burst outward from the centre, each a little later and a
    // different colour, thinning as they reach the ring.
    if bloom > 0.001 {
        let prog = 1.0 - bloom;
        for (var i = 0; i < 4; i++) {
            let fi = f32(i);
            let k = clamp(prog * 1.6 - fi * 0.14, 0.0, 1.0);
            if k > 0.0 && k < 1.0 {
                let rr = mix(0.5, 0.97, 1.0 - (1.0 - k) * (1.0 - k));
                var col = mix(GOLD, CREAM, 0.4);
                if i == 1 { col = EMERALD; }
                if i == 2 { col = BLUE; }
                if i == 3 { col = CRIMSON; }
                let amp = 0.07 * sin(k * PI);
                let pd = petal(r, th, rr, amp, 12.0 + fi * 2.0, fi * 0.7 + t * 2.0);
                let edge = cover(abs(pd) - 0.022 * (1.0 - k * 0.6), w);
                c = over(c, col, edge * (1.0 - k) * 0.95);
            }
        }
        // And the whole rosette glows a moment.
        c = vec4<f32>(c.rgb + bloom * bloom * 0.22 * c.a, c.a);
    }
    return c;
}

fn clock(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let q = (uv - 0.5) * 2.0;
    let r = length(q);
    let th = atan2(q.x, -q.y);
    let ang = select(th, th + TAU, th < 0.0); // 0..TAU clockwise from the top
    let elapsed = p.x * TAU;
    let urgent = p.y;
    let free = p.z > 0.5;
    let t = kt.y;
    var c = vec4<f32>(0.0);
    // A dark rim shadow and the bronze bezel.
    c = over(c, INK, cover(r - 0.97, w) * 0.5);
    c = over(c, BRONZE, cover(r - 0.93, w));
    c = over(c, GOLD, cover(r - 0.90, w) * 0.9);
    // Face: spent time is cold slate, time left is warm.
    let slate = vec3<f32>(0.20, 0.27, 0.40);
    var warm = mix(vec3<f32>(0.93, 0.66, 0.30), CRIMSON, urgent * (0.6 + 0.4 * sin(t * 9.0)));
    if free {
        warm = vec3<f32>(0.26, 0.60, 0.62);
    }
    let spent = !free && ang < elapsed;
    var face = select(warm, slate, spent);
    // A soft inner shade at the rim.
    face = face * (0.82 + 0.18 * smoothstep(0.9, 0.3, r));
    c = over(c, face, cover(r - 0.86, w));
    // Tick marks every twelfth.
    let tk = abs(fract(ang / TAU * 12.0 + 0.5) - 0.5) * (TAU * r / 12.0);
    let tick = cover(tk - 0.012, w) * smoothstep(0.62, 0.66, r) * cover(r - 0.84, w);
    c = over(c, INK, tick * 0.45);
    // The hand: from the hub to the rim at the spent edge (it sweeps in Free Roll).
    var ha = elapsed;
    if free {
        ha = t * 0.6;
    }
    let hv = vec2<f32>(sin(ha), -cos(ha));
    let along = dot(q, hv);
    let across = length(q - hv * clamp(along, 0.0, 0.8));
    let hand = cover(across - 0.045, w) * select(0.0, 1.0, along > -0.05 && along < 0.82);
    c = over(c, INK, cover(across - 0.07, w) * 0.4 * select(0.0, 1.0, along > -0.05));
    c = over(c, CREAM, hand);
    c = over(c, INK, cover(r - 0.13, w));
    c = over(c, GOLD, cover(r - 0.095, w));
    // The finish key held: a bright arc fills the bezel clockwise.
    let hold = p.w;
    if hold > 0.001 && ang / TAU < hold {
        let arc = cover(abs(r - 0.935) - 0.04, w);
        c = over(c, CREAM, arc);
    }
    return c;
}

fn bubble(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let q = (uv - 0.5) * 2.0;
    let r = length(q);
    let gold = p.x;
    var c = vec4<f32>(0.0);
    // Glass: a dark teal pool that brightens toward the rim.
    let glass = mix(vec3<f32>(0.07, 0.15, 0.26), vec3<f32>(0.16, 0.34, 0.55), smoothstep(0.3, 0.95, r));
    var rim = mix(vec3<f32>(0.55, 0.85, 1.0), GOLD, gold);
    let flash = kt.w;
    let fc = p.yzw;
    rim = mix(rim, fc, clamp(flash * 1.4, 0.0, 1.0));
    c = over(c, glass, cover(r - 0.95, w) * 0.62);
    // The flash: a coloured wash in from the rim.
    c = over(c, fc, flash * 0.5 * smoothstep(0.45, 0.95, r) * cover(r - 0.95, w));
    // The model, straight alpha, drawn inside the glass.
    let t = textureSample(tex, tex_sampler, uv);
    c = over(c, t.rgb, t.a * cover(r - 0.95, w));
    // Rim and a specular arc at the top left.
    let ring = cover(abs(r - 0.94) - 0.028, w);
    c = over(c, rim, ring * 0.9);
    c = over(c, INK, cover(abs(r - 0.975) - 0.014, w) * 0.5);
    let hl = vec2<f32>(-0.45, -0.52);
    let hd = length((q - hl) * vec2<f32>(0.85, 1.7));
    c = over(c, vec3<f32>(1.0), cover(hd - 0.2, 0.08) * 0.20 * cover(r - 0.9, w));
    return c;
}

// Fizzwick's porthole: a brass-rimmed round window on the commissioner stage's render (the upper
// part of the portrait target), over a warm workshop backdrop.
fn porthole(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let q = (uv - 0.5) * 2.0;
    let r = length(q);
    var c = vec4<f32>(0.0);
    c = over(c, INK, cover(r - 0.99, w) * 0.55);
    c = over(c, BRONZE, cover(r - 0.95, w));
    c = over(c, GOLD, cover(r - 0.89, w) * 0.9);
    c = over(c, BRONZE, cover(r - 0.86, w));
    c = over(c, mix(vec3<f32>(0.46, 0.33, 0.18), vec3<f32>(0.16, 0.11, 0.08), uv.y), cover(r - 0.83, w));
    let tuv = vec2<f32>(0.5 + (uv.x - 0.5) * 0.46, 0.27 + uv.y * 0.345);
    let t = textureSample(tex, tex_sampler, tuv);
    c = over(c, t.rgb, t.a * cover(r - 0.83, w));
    // Rivets round the bezel.
    let th = atan2(q.x, -q.y);
    let rv = abs(fract(th / TAU * 8.0 + 0.5) - 0.5) * (TAU * 0.925 / 8.0);
    c = over(c, CREAM, cover(length(vec2<f32>(rv, r - 0.925)) - 0.028, w) * 0.8);
    // A specular arc at the top left.
    let hd = length((q - vec2<f32>(-0.4, -0.5)) * vec2<f32>(0.85, 1.7));
    c = over(c, vec3<f32>(1.0), cover(hd - 0.2, 0.08) * 0.14 * cover(r - 0.8, w));
    return c;
}

// Distance to the segment a-b.
fn seg(q: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = q - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Two chevrons pointing up, the rear one trailing and sliding forward, with a dark edge so it
// reads over bright terrain. The UI node turns it.
fn arrow(uv: vec2<f32>, w: f32) -> vec4<f32> {
    let q = (uv - 0.5) * 2.0;
    var c = vec4<f32>(0.0);
    let slide = fract(kt.y * 0.9);
    for (var i = 0; i < 2; i++) {
        let fi = f32(i);
        // Chevron 0 leads; chevron 1 follows and fades as it catches up.
        let yo = select(0.06, -0.46 + 0.46 * slide, i == 1);
        let a = vec2<f32>(0.0, -0.62 - yo);
        let b = vec2<f32>(-0.58, -0.08 - yo);
        let d = vec2<f32>(0.58, -0.08 - yo);
        let dist = min(seg(q, a, b), seg(q, a, d));
        let al = select(1.0, 0.65 * (1.0 - slide), i == 1);
        c = over(c, INK, cover(dist - 0.19, w) * 0.5 * al);
        c = over(c, p.xyz, cover(dist - 0.115, w) * al);
        c = over(c, vec3<f32>(1.0), cover(dist - 0.04, w) * 0.5 * al);
    }
    return c;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // One pixel in `uv`-space-of-the-unit-disc (the widget is square).
    let w = 2.0 / max(in.size.x, 1.0);
    var c = vec4<f32>(0.0);
    let kind = i32(floor(kt.x + 0.01));
    if kind == 0 || kind >= 10 {
        c = dial(in.uv, w);
    } else if kind == 1 {
        c = clock(in.uv, w);
    } else if kind == 3 {
        c = arrow(in.uv, w);
    } else if kind == 4 {
        c = porthole(in.uv, w);
    } else {
        c = bubble(in.uv, w);
    }
    return vec4<f32>(c.rgb, c.a * kt.z);
}
