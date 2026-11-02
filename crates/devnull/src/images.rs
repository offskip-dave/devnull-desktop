use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use relm4::gtk;
use relm4::gtk::gdk;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;

static CACHE: OnceLock<Mutex<HashMap<String, glib::Bytes>>> = OnceLock::new();
static INFLIGHT: OnceLock<Mutex<HashMap<String, Vec<Waiter>>>> = OnceLock::new();
static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

type Waiter = Box<dyn FnOnce(glib::Bytes) + Send>;

fn cache() -> &'static Mutex<HashMap<String, glib::Bytes>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn inflight() -> &'static Mutex<HashMap<String, Vec<Waiter>>> {
    INFLIGHT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn http() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(devnull_api::USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .pool_max_idle_per_host(8)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn ui_context() -> glib::MainContext {
    // Capture the GTK thread's context when bind runs on the UI thread so the
    // tokio fetch completion marshals back correctly (default() can miss it).
    glib::MainContext::ref_thread_default()
}

fn cached(url: &str) -> Option<glib::Bytes> {
    cache().lock().ok()?.get(url).cloned()
}

fn remember(url: String, bytes: glib::Bytes) {
    if let Ok(mut map) = cache().lock() {
        map.insert(url, bytes);
    }
}

async fn fetch(url: &str) -> Option<glib::Bytes> {
    if let Some(hit) = cached(url) {
        return Some(hit);
    }
    let response = http().get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > 10 * 1024 * 1024 {
        return None;
    }
    Some(glib::Bytes::from_owned(bytes.to_vec()))
}

/// Decode off the UI thread into PNG bytes GdkTexture can always load on Quartz.
fn decode_to_png(bytes: &glib::Bytes) -> Option<glib::Bytes> {
    let image = image::load_from_memory(bytes.as_ref()).ok()?;
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(glib::Bytes::from_owned(png))
}

pub fn absolute_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.starts_with("https://") || url.starts_with("http://") {
        Some(url.to_owned())
    } else if let Some(rest) = url.strip_prefix("//") {
        Some(format!("https://{rest}"))
    } else if url.starts_with('/') {
        Some(format!(
            "{}{url}",
            devnull_api::default_base_url().trim_end_matches('/')
        ))
    } else {
        None
    }
}

pub fn bind_bytes(picture: &gtk::Picture, data: &[u8]) {
    let bytes = glib::Bytes::from_owned(data.to_vec());
    if let Some(texture) = texture_from_bytes(&bytes) {
        picture.set_paintable(Some(&texture));
    }
}

fn texture_from_bytes(bytes: &glib::Bytes) -> Option<gdk::Texture> {
    if let Ok(texture) = gdk::Texture::from_bytes(bytes) {
        return Some(texture);
    }
    if looks_like_svg(bytes) {
        if let Some(texture) = texture_from_svg(bytes) {
            return Some(texture);
        }
    }
    let png = decode_to_png(bytes)?;
    gdk::Texture::from_bytes(&png).ok()
}

fn looks_like_svg(bytes: &glib::Bytes) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim_start();
    trimmed.starts_with("<svg") || trimmed.starts_with("<?xml")
}

fn texture_from_svg(bytes: &glib::Bytes) -> Option<gdk::Texture> {
    let loader = relm4::gtk::gdk_pixbuf::PixbufLoader::with_mime_type("image/svg+xml").ok()?;
    loader.write(bytes.as_ref()).ok()?;
    loader.close().ok()?;
    Some(gdk::Texture::for_pixbuf(&loader.pixbuf()?))
}

fn is_png(bytes: &glib::Bytes) -> bool {
    bytes
        .as_ref()
        .starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'])
}

/// Coalesce duplicate URL fetches; decode to PNG off-thread; paint on the UI thread.
fn apply_bytes(url: &str, done: impl FnOnce(glib::Bytes) + Send + 'static) {
    let ctx = ui_context();
    if let Some(bytes) = cached(url) {
        if is_png(&bytes) {
            done(bytes);
            return;
        }
        let url = url.to_owned();
        relm4::spawn(async move {
            let prepared =
                tokio::task::spawn_blocking(move || decode_to_png(&bytes).unwrap_or(bytes))
                    .await
                    .ok();
            let Some(prepared) = prepared else {
                return;
            };
            remember(url, prepared.clone());
            ctx.invoke(move || done(prepared));
        });
        return;
    }
    {
        let mut map = match inflight().lock() {
            Ok(map) => map,
            Err(_) => {
                return;
            }
        };
        if let Some(waiters) = map.get_mut(url) {
            waiters.push(Box::new(done));
            return;
        }
        map.insert(url.to_owned(), vec![Box::new(done)]);
    }
    let url = url.to_owned();
    relm4::spawn(async move {
        let bytes = fetch(&url).await;
        let waiters = inflight()
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&url))
            .unwrap_or_default();
        let Some(raw) = bytes else {
            return;
        };
        let prepared = tokio::task::spawn_blocking({
            let raw = raw.clone();
            move || decode_to_png(&raw).unwrap_or(raw)
        })
        .await
        .unwrap_or(raw);
        remember(url, prepared.clone());
        for waiter in waiters {
            let payload = prepared.clone();
            ctx.invoke(move || waiter(payload));
        }
    });
}

fn apply_texture(url: Option<&str>, done: impl FnOnce(gdk::Texture) + Send + 'static) {
    let Some(url) = url.and_then(absolute_url) else {
        return;
    };
    apply_bytes(&url, move |bytes| {
        if let Some(texture) = texture_from_bytes(&bytes) {
            done(texture);
        }
    });
}

pub fn fetch_into(url: &str, done: impl FnOnce(Option<glib::Bytes>) + Send + 'static) {
    let Some(url) = absolute_url(url) else {
        done(None);
        return;
    };
    apply_bytes(&url, move |bytes| done(Some(bytes)));
}

pub fn bind_picture(picture: &gtk::Picture, url: Option<&str>) {
    bind_fitted_picture(picture, url, None);
}

pub fn reapply_picture(picture: &gtk::Picture, url: &str) {
    let Some(url) = absolute_url(url) else {
        return;
    };
    if let Some(bytes) = cached(&url) {
        if let Some(texture) = texture_from_bytes(&bytes) {
            picture.set_paintable(Some(&texture));
            picture.queue_draw();
        }
    }
}

pub fn bind_fitted_picture(
    picture: &gtk::Picture,
    url: Option<&str>,
    frame: Option<&gtk::AspectFrame>,
) {
    let pic = glib::SendWeakRef::from(picture.downgrade());
    let frame = frame.map(|frame| glib::SendWeakRef::from(frame.downgrade()));
    apply_texture(url, move |texture| {
        let Some(picture) = pic.upgrade() else {
            return;
        };
        picture.set_paintable(Some(&texture));
        picture.queue_draw();
        if let Some(frame) = frame.and_then(|frame| frame.upgrade()) {
            let ratio = capped_ratio(texture.width(), texture.height());
            frame.set_ratio(ratio);
            frame.set_size_request(-1, reserved_height(texture.width(), texture.height()));
        }
    });
}

pub fn capped_ratio(width: i32, height: i32) -> f32 {
    const MAX_W: f32 = 560.0;
    const MAX_H: f32 = 512.0;
    if width <= 0 || height <= 0 {
        return 16.0 / 9.0;
    }
    let width = width as f32;
    let height = height as f32;
    if MAX_W * height / width > MAX_H {
        MAX_W / MAX_H
    } else {
        width / height
    }
}

/// Height the gallery holds before the file arrives, matching the web's 560×512 cap.
pub fn reserved_height(width: i32, height: i32) -> i32 {
    const MAX_W: f32 = 560.0;
    const MAX_H: i32 = 512;
    ((MAX_W / capped_ratio(width, height)).round() as i32).clamp(72, MAX_H)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_space_at_the_web_cap() {
        assert_eq!(reserved_height(1450, 894), 345);
        assert_eq!(reserved_height(1080, 2400), 512);
        assert_eq!(reserved_height(0, 0), 315);
    }

    #[test]
    fn decodes_lossy_webp() {
        let image = image::RgbImage::from_pixel(4, 3, image::Rgb([32, 64, 128]));
        let mut webp = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut webp),
                image::ImageFormat::WebP,
            )
            .unwrap();
        let loaded = image::load_from_memory(&webp).unwrap();
        assert_eq!(loaded.width(), 4);
        assert_eq!(loaded.height(), 3);
        assert!(texture_from_bytes(&glib::Bytes::from_owned(webp)).is_some());
    }
}
