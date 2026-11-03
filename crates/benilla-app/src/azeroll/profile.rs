//! Opt-in CPU probes; platform Instant also works on wasm. No allocation when disabled.
use bevy::{platform::time::Instant, prelude::*};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

pub(super) struct Span(Option<(&'static str, Instant)>);
pub(super) fn span(name: &'static str) -> Span {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    Span(
        ENABLED
            .get_or_init(|| crate::webenv::var("WOW_AZEROLL_PROFILE").is_some())
            .then(|| (name, Instant::now())),
    )
}
impl Drop for Span {
    fn drop(&mut self) {
        let Some((name, start)) = self.0 else { return };
        static SAMPLES: OnceLock<Mutex<BTreeMap<&'static str, (Instant, Vec<f64>)>>> =
            OnceLock::new();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut samples = SAMPLES.get_or_init(Default::default).lock().unwrap();
        let (since, values) = samples
            .entry(name)
            .or_insert_with(|| (Instant::now(), Vec::new()));
        values.push(ms);
        if since.elapsed().as_secs_f32() >= 5.0 {
            values.sort_by(f64::total_cmp);
            info!(
                "azeroll profile: {name} n={} mean_ms={:.4} p50_ms={:.4} p95_ms={:.4}",
                values.len(),
                values.iter().sum::<f64>() / values.len() as f64,
                values[values.len() / 2],
                values[(values.len() - 1) * 95 / 100]
            );
            values.clear();
            *since = Instant::now();
        }
    }
}
