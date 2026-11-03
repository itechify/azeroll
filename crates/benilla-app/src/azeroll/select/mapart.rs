//! Feeds `maps.rs` from the game's data: once the archives are open, reads the world-map tiles
//! through the patch chain, composes the globe's texture and the explored Elwynn map, uploads
//! them, and works out where Elwynn is on the globe and where each level's pin is on the map.
//! Without the data (a bare checkout, a failed read) [`MapArt`] stays empty and the hub draws a
//! plain painted sea and a plain parchment inset.

use benilla_assets::{LockRecover, WorldAssets};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::maps::{self, Overlay, Rect, Rgba};
use crate::azeroll::campaign::Campaign;

/// Elwynn Forest's `WorldMapArea` art folder, and the `AreaTable` id of its overlays' rows is
/// looked up by name.
const ELWYNN: &str = "Elwynn";
/// Goldshire, the globe's aim: the Lion's Pride Inn.
const AIM_WOW: (f32, f32) = (-9467.0, 36.0);

/// The composed maps, once there are any.
#[derive(Resource, Default)]
pub(super) struct MapArt {
    /// Equirectangular texture of the planet.
    pub globe: Option<Handle<Image>>,
    /// The explored Elwynn map, exactly the 1002 x 668 of art.
    pub elwynn: Option<Handle<Image>>,
    /// Longitude and latitude (radians) of Goldshire on the globe.
    pub aim: Vec2,
    /// Each level's pin on the Elwynn map, as a share of the art.
    pub pins: Vec<Vec2>,
    /// The loader has run (successfully or not).
    pub tried: bool,
}

impl MapArt {
    pub fn ready(&self) -> bool {
        self.globe.is_some() && self.elwynn.is_some()
    }
}

fn image(img: &Rgba, wrap_u: bool) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: img.w,
            height: img.h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        img.px.clone(),
        // Raw bytes: the UI lane composites in gamma, so the shader wants the gamma values as
        // they are in the art.
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: if wrap_u {
            ImageAddressMode::Repeat
        } else {
            ImageAddressMode::ClampToEdge
        },
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// Where each pin falls on a map sheet covering `rect`.
pub(super) fn pin_uvs(rect: &Rect, pins: &[[f32; 2]]) -> Vec<Vec2> {
    pins.iter()
        .map(|p| {
            let (x, y) = rect.uv(p[0], p[1]);
            Vec2::new(x, y)
        })
        .collect()
}

/// Read the tiles and compose the maps, once, when the archives are open.
pub(super) fn load(
    mut art: ResMut<MapArt>,
    assets: Option<Res<WorldAssets>>,
    campaign: Res<Campaign>,
    mut images: ResMut<Assets<Image>>,
) {
    if art.tried {
        return;
    }
    let Some(assets) = assets else { return };
    art.tried = true;
    let started = bevy::platform::time::Instant::now();
    let mut chain = assets.chain.lock_recover();
    let wma = match benilla_formats::load_world_map_area_catalog(&mut chain) {
        Ok(w) => w,
        Err(e) => {
            warn!("azeroll hub: WorldMapArea unavailable, no map art: {e:#}");
            return;
        }
    };
    let overlays = benilla_formats::load_world_map_overlay_catalog(&mut chain);
    let elwynn_row = wma.iter().find(|(_, a)| a.name == ELWYNN);
    let Some((elwynn_id, elwynn_area)) = elwynn_row else {
        warn!("azeroll hub: no Elwynn map row, no map art");
        return;
    };
    let mut read = |zone: &str, name: &str| {
        let bytes = chain
            .read_file(&format!("Interface\\WorldMap\\{zone}\\{name}.blp"))
            .ok()?;
        let (w, h, px) = benilla_formats::blp_to_rgba(&bytes).ok()?;
        Some(Rgba { w, h, px })
    };
    let specs: Vec<Overlay> = overlays
        .as_ref()
        .map(|o| {
            o.for_area(elwynn_id)
                .iter()
                .map(|o| Overlay {
                    name: o.texture_name.clone(),
                    w: o.texture_width,
                    h: o.texture_height,
                    x: o.offset_x,
                    y: o.offset_y,
                })
                .collect()
        })
        .unwrap_or_default();
    let elwynn = maps::zone_explored(&mut read, ELWYNN, &specs);
    let eastern = maps::zone_base(&mut read, "Azeroth");
    let kalimdor = maps::zone_base(&mut read, "Kalimdor");
    let (atlas, layout) = maps::globe_atlas(&eastern, &kalimdor);

    // Where Goldshire is on the Eastern Kingdoms sheet, hence on the globe.
    if let Some((_, cont)) = wma.continent(0) {
        let rect = Rect {
            left: cont.loc_left,
            right: cont.loc_right,
            top: cont.loc_top,
            bottom: cont.loc_bottom,
        };
        let (lon, lat) = layout.eastern(rect.uv(AIM_WOW.0, AIM_WOW.1));
        art.aim = Vec2::new(lon, lat);
    }
    let rect = Rect {
        left: elwynn_area.loc_left,
        right: elwynn_area.loc_right,
        top: elwynn_area.loc_top,
        bottom: elwynn_area.loc_bottom,
    };
    let pins: Vec<[f32; 2]> = campaign.levels.iter().map(|l| l.pin).collect();
    art.pins = pin_uvs(&rect, &pins);
    // The inset shows exactly the art, not the sheet's padding.
    let mut crop = Rgba::new(maps::MAP_W, maps::MAP_H);
    crop.over(0, 0, &elwynn);
    art.globe = Some(images.add(image(&atlas, true)));
    art.elwynn = Some(images.add(image(&crop, false)));
    info!(
        "azeroll hub: map art composed in {:?}: aim lon {:.1} lat {:.1}, pins {:?}",
        started.elapsed(),
        art.aim.x.to_degrees(),
        art.aim.y.to_degrees(),
        art.pins
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_land_on_the_sheet_in_the_order_the_levels_are() {
        let rect = Rect {
            left: 1535.4166,
            right: -1935.4166,
            top: -7939.583,
            bottom: -10254.166,
        };
        let pins = pin_uvs(
            &rect,
            &[[-9467.0, 36.0], [-9620.0, 210.0], [-8520.0, 880.0]],
        );
        assert_eq!(pins.len(), 3);
        for p in &pins {
            assert!(
                (0.0..1.0).contains(&p.x) && (0.0..1.0).contains(&p.y),
                "{p:?}"
            );
        }
        // The inn, then the fair a little south-west of it, then Stormwind far north-west.
        assert!(pins[1].x < pins[0].x && pins[1].y > pins[0].y);
        assert!(pins[2].x < pins[1].x && pins[2].y < pins[0].y);
        // Far enough apart that the pins do not sit on each other at the inset's zoom.
        let d01 = (pins[0] - pins[1]).length();
        assert!(d01 > 0.04, "{d01}");
    }
}
