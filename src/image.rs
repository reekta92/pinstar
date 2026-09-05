//! Async image decode worker + LRU protocol cache (ported from clin's
//! `image_render`; only compiled with the `images` feature).

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::Result;
use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

pub struct ImageJob {
    pub key: PathBuf,
    pub max_dim: u32,
}

pub struct DecodedImage {
    pub key: PathBuf,
    pub image: DynamicImage,
}

/// LRU-evicting cache of decoded images and their protocol renderers for one view.
pub struct ImageCache {
    map: lru::LruCache<PathBuf, ImageEntry>,
}

struct ImageEntry {
    /// Created lazily from decoded + picker; `None` while decode pending.
    proto: Option<StatefulProtocol>,
}

impl ImageCache {
    pub fn new(limit: usize) -> Self {
        Self {
            map: lru::LruCache::new(NonZeroUsize::new(limit.max(1)).expect("limit >= 1")),
        }
    }

    /// Request an image for display. If the key is absent, sends a decode job
    /// to the worker. Call `install_decoded` when the result arrives.
    pub fn request(&mut self, key: PathBuf, max_dim: u32, tx: &Sender<ImageJob>) {
        if self.map.contains(&key) {
            // Touch existing entry
            let _ = self.map.get_mut(&key);
            return;
        }
        let _ = tx.send(ImageJob {
            key: key.clone(),
            max_dim,
        });
        self.map.put(key, ImageEntry { proto: None });
    }

    /// Install a completed decode result and build the protocol renderer.
    pub fn install_decoded(&mut self, img: DecodedImage, picker: &Picker) {
        if let Some(entry) = self.map.get_mut(&img.key) {
            let proto = picker.new_resize_protocol(img.image);
            entry.proto = Some(proto);
        }
    }

    /// Get a mutable reference to the protocol for rendering, if ready.
    pub fn get_proto(&mut self, key: &PathBuf) -> Option<&mut StatefulProtocol> {
        self.map.get_mut(key).and_then(|entry| entry.proto.as_mut())
    }
}

/// Spawn the background image decode worker.
/// Returns (job sender, result receiver).
///
/// The worker loop:
/// 1. Block on `recv` for the next job.
/// 2. Open + decode + downscale the image.
/// 3. Send the result back (or an error).
/// 4. Drain any additional immediately-available jobs for throughput.
/// 5. Loop.
pub fn spawn_worker() -> (Sender<ImageJob>, Receiver<Result<DecodedImage>>) {
    let (tx, rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("pinstar-image-decode-worker".into())
        .spawn(move || {
            loop {
                let first = match rx.recv() {
                    Ok(job) => job,
                    Err(_) => {
                        // All senders dropped — shut down.
                        return;
                    }
                };

                process_job(first, &result_tx);

                // Drain any additional jobs that arrived while we were processing.
                while let Ok(job) = rx.try_recv() {
                    process_job(job, &result_tx);
                }
            }
        })
        .expect("failed to spawn image decode worker");

    (tx, result_rx)
}

fn process_job(job: ImageJob, result_tx: &Sender<Result<DecodedImage>>) {
    let result = decode_image(&job.key, job.max_dim);
    let _ = result_tx.send(result.map(|img| DecodedImage {
        key: job.key,
        image: img,
    }));
}

fn decode_image(key: &PathBuf, max_dim: u32) -> Result<DynamicImage> {
    let img = image::ImageReader::open(key)?
        .decode()
        .map_err(|e| anyhow::anyhow!("Failed to decode image {}: {e}", key.display()))?;

    if max_dim > 0 {
        let w = img.width();
        let h = img.height();
        let max_current = w.max(h);
        if max_current > max_dim {
            let ratio = max_dim as f64 / max_current as f64;
            let new_w = (w as f64 * ratio) as u32;
            let new_h = (h as f64 * ratio) as u32;
            return Ok(img.resize_exact(
                new_w.max(1),
                new_h.max(1),
                image::imageops::FilterType::Lanczos3,
            ));
        }
    }

    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eviction() {
        let mut cache = ImageCache::new(2);

        let dir = std::env::temp_dir().join("pinstar_test_img_cache");
        let _ = std::fs::create_dir_all(&dir);
        let k1 = dir.join("a.png");
        let k2 = dir.join("b.png");
        let k3 = dir.join("c.png");

        // Insert two
        let (tx, _) = std::sync::mpsc::channel();
        cache.request(k1.clone(), 100, &tx);
        cache.request(k2.clone(), 100, &tx);
        assert_eq!(cache.map.len(), 2);

        // Touch k1 so it's more recent
        cache.get_proto(&k1);

        // Insert third — should evict k2 (oldest)
        cache.request(k3.clone(), 100, &tx);
        assert_eq!(cache.map.len(), 2);
        assert!(cache.map.contains(&k1), "k1 should survive");
        assert!(!cache.map.contains(&k2), "k2 should be evicted as oldest");
        assert!(cache.map.contains(&k3), "k3 should survive");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
