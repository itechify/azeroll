//! The hub's maps, composed on the CPU from the game's own world-map art (D24): an equirectangular
//! texture for the globe (the Eastern Kingdoms and Kalimdor, cut out of their parchment sheets and
//! set in a painted ocean) and the fully explored Elwynn Forest map for the zoomed inset. Pure
//! pixel work on [`Rgba`] with no engine types, so it is unit-tested with synthetic tiles and
//! dumped to PNG for a look (`AZEROLL_DUMP_MAPS=<dir>`, see the tests); the loader that feeds it
//! decoded tiles lives in `orrery.rs`.

/// A map sheet is 4 x 3 tiles of 256 px, of which the top left 1002 x 668 is art.
pub(super) const MAP_W: u32 = 1002;
pub(super) const MAP_H: u32 = 668;
const TILE: u32 = 256;

/// The globe texture's size: one texel per 0.176 degrees.
pub(super) const ATLAS_W: u32 = 2048;
pub(super) const ATLAS_H: u32 = 1024;

/// Where the continents' cut-outs sit in the globe texture, and how large (texels per map pixel).
const SCALE: f32 = 0.78;
/// The part of a continent sheet that holds land: left, top, width, height (the sheet's sea,
/// ship, sea monster and compass are outside it).
const CROP: (u32, u32, u32, u32) = (285, 34, 440, 600);
/// The cut-out's top left in the atlas, for Kalimdor and the Eastern Kingdoms.
const KALIMDOR_AT: (f32, f32) = (790.0, 250.0);
const EASTERN_AT: (f32, f32) = (1180.0, 250.0);
/// The cut-out fades into the sea over this many sheet pixels at its edges.
const FEATHER: f32 = 48.0;

/// A straight-alpha 8-bit RGBA image.
#[derive(Clone, Debug)]
pub(super) struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Rgba {
    /// Fully transparent.
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w * h * 4) as usize],
        }
    }

    #[cfg(test)]
    pub fn filled(w: u32, h: u32, c: [u8; 4]) -> Self {
        let mut s = Self::new(w, h);
        for p in s.px.chunks_exact_mut(4) {
            p.copy_from_slice(&c);
        }
        s
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y.min(self.h - 1) * self.w + x.min(self.w - 1)) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    pub fn set(&mut self, x: u32, y: u32, c: [u8; 4]) {
        let i = ((y * self.w + x) * 4) as usize;
        self.px[i..i + 4].copy_from_slice(&c);
    }

    /// Composite `src` over this image with its top left at (`x`, `y`) (which may be negative or
    /// run off the edge), the destination left opaque where it was.
    pub fn over(&mut self, x: i64, y: i64, src: &Rgba) {
        for sy in 0..src.h {
            let dy = y + i64::from(sy);
            if dy < 0 || dy >= i64::from(self.h) {
                continue;
            }
            for sx in 0..src.w {
                let dx = x + i64::from(sx);
                if dx < 0 || dx >= i64::from(self.w) {
                    continue;
                }
                let s = src.get(sx, sy);
                let a = f32::from(s[3]) / 255.0;
                if a <= 0.0 {
                    continue;
                }
                let d = self.get(dx as u32, dy as u32);
                let da = f32::from(d[3]) / 255.0;
                let oa = a + da * (1.0 - a);
                let mut out = [0u8; 4];
                for c in 0..3 {
                    let v = (f32::from(s[c]) * a + f32::from(d[c]) * da * (1.0 - a)) / oa.max(1e-6);
                    out[c] = v.round().clamp(0.0, 255.0) as u8;
                }
                out[3] = (oa * 255.0).round() as u8;
                self.set(dx as u32, dy as u32, out);
            }
        }
    }
}

/// How a zone's tiles are named and read: `read("Elwynn3")` is the decoded
/// `Interface\WorldMap\<zone>\Elwynn3.blp`, or `None` when it cannot be had.
pub(super) type TileReader<'a> = dyn FnMut(&str, &str) -> Option<Rgba> + 'a;

/// A zone's own sheet: its twelve tiles, 4 across.
pub(super) fn zone_base(read: &mut TileReader, zone: &str) -> Rgba {
    let mut sheet = Rgba::new(MAP_W, MAP_H);
    for i in 1..=12u32 {
        if let Some(tile) = read(zone, &format!("{zone}{i}")) {
            sheet.over(
                i64::from(((i - 1) % 4) * TILE),
                i64::from(((i - 1) / 4) * TILE),
                &tile,
            );
        }
    }
    sheet
}

/// One overlay of a zone sheet (`WorldMapOverlay.dbc`): the texture's name, size and placement.
#[derive(Clone, Debug)]
pub(super) struct Overlay {
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub x: u32,
    pub y: u32,
}

/// A zone sheet with every overlay laid on it: the map as an explorer who has seen it all draws it.
pub(super) fn zone_explored(read: &mut TileReader, zone: &str, overlays: &[Overlay]) -> Rgba {
    let mut sheet = zone_base(read, zone);
    for o in overlays {
        let cols = o.w.div_ceil(TILE).max(1);
        let rows = o.h.div_ceil(TILE).max(1);
        for i in 1..=cols * rows {
            if let Some(tile) = read(zone, &format!("{}{i}", o.name)) {
                sheet.over(
                    i64::from(o.x + ((i - 1) % cols) * TILE),
                    i64::from(o.y + ((i - 1) / cols) * TILE),
                    &tile,
                );
            }
        }
    }
    sheet
}

/// A map sheet's world rectangle (`WorldMapArea.dbc`): left and right are world Y, top and bottom
/// world X.
#[derive(Clone, Copy, Debug)]
pub(super) struct Rect {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Rect {
    /// Where WoW point (`x` north, `y` west) falls on the sheet, as `(0..1, 0..1)` of the art
    /// (x to the right, y down).
    pub fn uv(&self, x: f32, y: f32) -> (f32, f32) {
        (
            (self.left - y) / (self.left - self.right),
            (self.top - x) / (self.top - self.bottom),
        )
    }
}

/// How much of each pixel of a continent sheet is land, 0..1. Land is saturated yellow, orange or
/// green with brown outlines; the sea is a pale blue-grey wash, so saturation, smoothed so the
/// sea's engraved hatching and lettering drop out, separates them.
pub(super) fn land_mask(sheet: &Rgba) -> Vec<f32> {
    let (w, h) = (sheet.w as usize, sheet.h as usize);
    let mut sat = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let p = sheet.get(x as u32, y as u32);
            let (r, g, b) = (f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
            let mx = r.max(g).max(b);
            let mn = r.min(g).min(b);
            let s = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
            // Blue-ish pixels are sea whatever their saturation.
            let warm = (r.max(g) - b) / 255.0;
            sat[y * w + x] = s.min(1.0) * (warm * 5.0).clamp(0.0, 1.0);
        }
    }
    let blurred = box_blur(&box_blur(&sat, w, h, 4), w, h, 3);
    blurred.iter().map(|&s| smoothstep(0.36, 0.50, s)).collect()
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A separable box blur of radius `r`.
fn box_blur(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; w * h];
    let n = (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let mut acc: f32 = (0..=r).map(|i| row[i.min(w - 1)]).sum::<f32>() + row[0] * r as f32;
        for x in 0..w {
            tmp[y * w + x] = acc / n;
            let add = row[(x + r + 1).min(w - 1)];
            let sub = row[x.saturating_sub(r)];
            acc += add - sub;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for x in 0..w {
        let mut acc: f32 =
            (0..=r).map(|i| tmp[i.min(h - 1) * w + x]).sum::<f32>() + tmp[x] * r as f32;
        for y in 0..h {
            out[y * w + x] = acc / n;
            let add = tmp[(y + r + 1).min(h - 1) * w + x];
            let sub = tmp[y.saturating_sub(r) * w + x];
            acc += add - sub;
        }
    }
    out
}

/// Where the globe's continents sit, to aim it: a continent sheet's `(0..1, 0..1)` point to
/// longitude and latitude in radians.
#[derive(Clone, Copy, Debug)]
pub(super) struct GlobeLayout;

impl GlobeLayout {
    /// The Eastern Kingdoms sheet's `uv` (as [`Rect::uv`] gives) as (longitude, latitude), radians.
    pub fn eastern(&self, uv: (f32, f32)) -> (f32, f32) {
        atlas_to_lonlat(
            EASTERN_AT.0 + (uv.0 * MAP_W as f32 - CROP.0 as f32) * SCALE,
            EASTERN_AT.1 + (uv.1 * MAP_H as f32 - CROP.1 as f32) * SCALE,
        )
    }
}

/// Atlas texel to longitude and latitude, radians.
pub(super) fn atlas_to_lonlat(x: f32, y: f32) -> (f32, f32) {
    (
        (x / ATLAS_W as f32 - 0.5) * std::f32::consts::TAU,
        (0.5 - y / ATLAS_H as f32) * std::f32::consts::PI,
    )
}

/// The globe's texture: a painted ocean with the two continents cut out of their sheets and set
/// in it, and where the Eastern Kingdoms sit.
pub(super) fn globe_atlas(eastern: &Rgba, kalimdor: &Rgba) -> (Rgba, GlobeLayout) {
    let mut atlas = ocean(ATLAS_W, ATLAS_H);
    for (sheet, at) in [(kalimdor, KALIMDOR_AT), (eastern, EASTERN_AT)] {
        let (cx, cy, cw, ch) = CROP;
        let (dw, dh) = ((cw as f32 * SCALE) as u32, (ch as f32 * SCALE) as u32);
        // Land, with the squarely cropped edges eased into the sea (in sheet pixels).
        let mut mask = land_mask(sheet);
        for y in 0..sheet.h {
            for x in 0..sheet.w {
                let ex = (x as f32 - cx as f32).min((cx + cw) as f32 - x as f32);
                let ey = (y as f32 - cy as f32).min((cy + ch) as f32 - y as f32);
                let edge = smoothstep(0.0, FEATHER, ex.min(ey));
                mask[(y * sheet.w + x) as usize] *= edge;
            }
        }
        // A soft sea-green shelf round the coast, under the land.
        let (sw, sh_) = (sheet.w as usize, sheet.h as usize);
        let shelf = box_blur(&box_blur(&mask, sw, sh_, 7), sw, sh_, 7);
        for dy in 0..dh {
            for dx in 0..dw {
                // The source position of this texel.
                let sx = cx as f32 + (dx as f32 + 0.5) / SCALE;
                let sy = cy as f32 + (dy as f32 + 0.5) / SCALE;
                let (ix, iy) = (sx as u32, sy as u32);
                if ix >= sheet.w || iy >= sheet.h {
                    continue;
                }
                let i = iy as usize * sheet.w as usize + ix as usize;
                let (tx, ty) = (at.0 as u32 + dx, at.1 as u32 + dy);
                if tx >= ATLAS_W || ty >= ATLAS_H {
                    continue;
                }
                let base = atlas.get(tx, ty);
                // The shelf: lighter, greener water near land.
                let sh = (shelf[i] * 1.6).clamp(0.0, 1.0) * 0.55;
                let water = mix3(base, [70, 150, 165, 255], sh);
                // The land from the sheet, averaged over the texels it covers.
                let src = avg(sheet, ix, iy);
                let out = mix3(water, src, mask[i]);
                atlas.set(tx, ty, [out[0], out[1], out[2], 255]);
            }
        }
    }
    // The cut-outs are cropped squarely: ease their north and south edges into the sea.
    (atlas, GlobeLayout)
}

fn mix3(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let mut out = [255u8; 4];
    for c in 0..3 {
        out[c] = (f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t).round() as u8;
    }
    out
}

fn avg(img: &Rgba, x: u32, y: u32) -> [u8; 4] {
    let mut acc = [0u32; 3];
    let mut n = 0;
    for dy in 0..2 {
        for dx in 0..2 {
            let p = img.get((x + dx).saturating_sub(0), y + dy);
            for c in 0..3 {
                acc[c] += u32::from(p[c]);
            }
            n += 1;
        }
    }
    [
        (acc[0] / n) as u8,
        (acc[1] / n) as u8,
        (acc[2] / n) as u8,
        255,
    ]
}

/// The sea: deep blue, brighter toward the equator and with slow swells, so even open water has
/// texture when the globe turns.
fn ocean(w: u32, h: u32) -> Rgba {
    let mut img = Rgba::new(w, h);
    for y in 0..h {
        let lat = (0.5 - (y as f32 + 0.5) / h as f32) * std::f32::consts::PI;
        for x in 0..w {
            let lon = ((x as f32 + 0.5) / w as f32 - 0.5) * std::f32::consts::TAU;
            // Swells: a few cheap sines in sphere-friendly coordinates.
            let s = 0.5
                + 0.25 * (lon * 7.0 + (lat * 5.0).sin() * 2.0).sin()
                + 0.15 * (lon * 17.0 - lat * 9.0).sin()
                + 0.10 * (lat * 23.0 + lon * 3.0).sin();
            let eq = lat.cos().powf(0.8);
            let base = [
                (22.0 + 22.0 * s * eq) as u8,
                (62.0 + 46.0 * s * eq) as u8,
                (108.0 + 50.0 * s * eq) as u8,
                255,
            ];
            img.set(x, y, base);
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(c: [u8; 4]) -> Rgba {
        Rgba::filled(256, 256, c)
    }

    #[test]
    fn a_zone_sheet_is_its_tiles_four_across() {
        let mut asked = Vec::new();
        let mut read = |_: &str, name: &str| {
            asked.push(name.to_string());
            let n: u8 = name.trim_start_matches("Z").parse().unwrap();
            Some(flat([n * 10, 0, 0, 255]))
        };
        let sheet = zone_base(&mut read, "Z");
        assert_eq!((sheet.w, sheet.h), (MAP_W, MAP_H));
        assert_eq!(asked.len(), 12);
        // Tile 1 top left, tile 4 top right, tile 5 second row first column, tile 12 bottom right.
        assert_eq!(sheet.get(10, 10)[0], 10);
        assert_eq!(sheet.get(3 * 256 + 10, 10)[0], 40);
        assert_eq!(sheet.get(10, 256 + 10)[0], 50);
        assert_eq!(sheet.get(3 * 256 + 10, 2 * 256 + 10)[0], 120);
        // The sheet is only 1002 x 668: the last column is cut.
        assert_eq!(sheet.get(MAP_W - 1, MAP_H - 1)[3], 255);
    }

    #[test]
    fn overlays_land_at_their_offsets_and_tile_row_major() {
        let mut read = |_: &str, name: &str| {
            if name == "Z1" {
                return Some(flat([0, 0, 255, 255]));
            }
            // The overlay "TOWN" is two tiles wide: TOWN1 red, TOWN2 green.
            match name {
                "TOWN1" => Some(flat([255, 0, 0, 255])),
                "TOWN2" => Some(flat([0, 255, 0, 255])),
                _ => None,
            }
        };
        let overlays = [Overlay {
            name: "TOWN".into(),
            w: 300,
            h: 100,
            x: 400,
            y: 300,
        }];
        let sheet = zone_explored(&mut read, "Z", &overlays);
        assert_eq!(
            sheet.get(100, 100)[2],
            255,
            "the base shows where nothing is laid"
        );
        assert_eq!(sheet.get(410, 310), [255, 0, 0, 255]);
        assert_eq!(sheet.get(400 + 256 + 10, 310), [0, 255, 0, 255]);
    }

    #[test]
    fn a_translucent_tile_blends_over_what_is_under_it() {
        let mut img = Rgba::filled(4, 4, [200, 0, 0, 255]);
        img.over(1, 1, &Rgba::filled(2, 2, [0, 0, 200, 128]));
        let p = img.get(1, 1);
        assert!(
            (95..=105).contains(&p[0]) && (95..=105).contains(&p[2]),
            "{p:?}"
        );
        assert_eq!(p[3], 255);
        assert_eq!(img.get(0, 0), [200, 0, 0, 255]);
        // Off the edge is ignored.
        img.over(-5, -5, &Rgba::filled(3, 3, [0, 255, 0, 255]));
        img.over(3, 3, &Rgba::filled(3, 3, [0, 255, 0, 255]));
        assert_eq!(img.get(3, 3), [0, 255, 0, 255]);
    }

    #[test]
    fn the_projection_puts_goldshire_where_the_map_shows_it() {
        // Elwynn's WorldMapArea rect.
        let r = Rect {
            left: 1535.4166,
            right: -1935.4166,
            top: -7939.583,
            bottom: -10254.166,
        };
        let (x, y) = r.uv(-9467.0, 36.0);
        assert!(
            (x - 0.432).abs() < 0.01 && (y - 0.660).abs() < 0.01,
            "{x} {y}"
        );
        // North is up and west is left: Stormwind, north and west of Goldshire, is up and left.
        let (sx, sy) = r.uv(-8520.0, 880.0);
        assert!(sx < x && sy < y);
        // The corners of the rect are the corners of the sheet.
        assert_eq!(r.uv(r.top, r.left), (0.0, 0.0));
        let (bx, by) = r.uv(r.bottom, r.right);
        assert!((bx - 1.0).abs() < 1e-5 && (by - 1.0).abs() < 1e-5);
    }

    #[test]
    fn saturated_land_is_found_and_grey_sea_is_not() {
        let mut sheet = Rgba::filled(200, 100, [150, 160, 165, 255]);
        for y in 20..80 {
            for x in 60..140 {
                sheet.set(x, y, [210, 160, 40, 255]);
            }
        }
        let m = land_mask(&sheet);
        let at = |x: usize, y: usize| m[y * 200 + x];
        assert!(at(100, 50) > 0.95, "land core {}", at(100, 50));
        assert!(at(10, 50) < 0.05 && at(190, 5) < 0.05, "sea");
        // The coast is soft, not a stair.
        assert!(at(58, 50) > 0.0 && at(58, 50) < 0.95 || at(55, 50) < 0.5);
    }

    #[test]
    fn the_globe_atlas_has_ocean_everywhere_and_the_continents_in_it() {
        // Two flat "continent sheets": warm orange land in the middle, grey sea round it.
        let sheet = |c: [u8; 4]| {
            let mut s = Rgba::filled(MAP_W, MAP_H, [150, 160, 165, 255]);
            for y in 100..560 {
                for x in 380..620 {
                    s.set(x, y, c);
                }
            }
            s
        };
        let (atlas, layout) = globe_atlas(&sheet([210, 160, 40, 255]), &sheet([60, 170, 50, 255]));
        assert_eq!((atlas.w, atlas.h), (ATLAS_W, ATLAS_H));
        // Every texel is opaque.
        assert!(atlas.px.chunks_exact(4).all(|p| p[3] == 255));
        // Somewhere is orange land (Eastern Kingdoms) and green land (Kalimdor).
        let find = |pred: &dyn Fn([u8; 4]) -> bool| {
            (0..ATLAS_H)
                .step_by(4)
                .any(|y| (0..ATLAS_W).step_by(4).any(|x| pred(atlas.get(x, y))))
        };
        assert!(
            find(&|p| p[0] > 180 && p[1] > 130 && p[2] < 90),
            "no orange land"
        );
        assert!(
            find(&|p| p[1] > 140 && p[0] < 100 && p[2] < 90),
            "no green land"
        );
        // The corner is open sea: blue wins.
        let c = atlas.get(5, 5);
        assert!(c[2] > c[0], "{c:?}");
        // Elwynn is in the Eastern Kingdoms' patch, south of the equator-ish, east of Kalimdor.
        let (lon, lat) = layout.eastern((0.4535, 0.7216));
        let (klon, _) = atlas_to_lonlat(KALIMDOR_AT.0 + 100.0, KALIMDOR_AT.1);
        assert!(lon > klon && lat.abs() < 1.0, "{lon} {lat}");
    }

    /// With `AZEROLL_DUMP_MAPS=<dir>` and the game's data (`WOW_DATA`), writes the composed
    /// Elwynn map and the globe texture there as PNGs. Not a test of anything: a way to look.
    #[test]
    fn dump_the_real_maps_for_a_look() {
        let (Some(dir), Some(data)) = (
            std::env::var_os("AZEROLL_DUMP_MAPS"),
            std::env::var_os("WOW_DATA"),
        ) else {
            return;
        };
        let mut chain = benilla_formats::Chain::open(std::path::Path::new(&data)).unwrap();
        let wmo = benilla_formats::load_world_map_overlay_catalog(&mut chain).unwrap();
        let mut read = |zone: &str, name: &str| {
            let bytes = chain
                .read_file(&format!("Interface\\WorldMap\\{zone}\\{name}.blp"))
                .ok()?;
            let (w, h, px) = benilla_formats::blp_to_rgba(&bytes).ok()?;
            Some(Rgba { w, h, px })
        };
        let overlays: Vec<Overlay> = wmo
            .for_area(30)
            .iter()
            .map(|o| Overlay {
                name: o.texture_name.clone(),
                w: o.texture_width,
                h: o.texture_height,
                x: o.offset_x,
                y: o.offset_y,
            })
            .collect();
        let elwynn = zone_explored(&mut read, "Elwynn", &overlays);
        let ek = zone_base(&mut read, "Azeroth");
        let kal = zone_base(&mut read, "Kalimdor");
        let (atlas, _) = globe_atlas(&ek, &kal);
        let dir = std::path::PathBuf::from(dir);
        for (name, img) in [("elwynn.png", &elwynn), ("globe.png", &atlas)] {
            image::save_buffer(
                dir.join(name),
                &img.px,
                img.w,
                img.h,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        let mask = land_mask(&ek);
        let m: Vec<u8> = mask.iter().map(|&v| (v * 255.0) as u8).collect();
        image::save_buffer(
            dir.join("ek-mask.png"),
            &m,
            ek.w,
            ek.h,
            image::ColorType::L8,
        )
        .unwrap();
    }
}
