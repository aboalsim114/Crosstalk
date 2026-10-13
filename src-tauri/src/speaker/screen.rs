//! A small area of CS2's window, copied from the screen like a screenshot
//! (GDI BitBlt of the desktop, as OBS's display capture does). Crosstalk never
//! touches the game's process or memory: it only reads pixels already on the
//! user's screen. Frames stay in memory and are dropped after the OCR.

use serde::{Deserialize, Serialize};

/// Area to read, in fractions of CS2's window (0..1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for Region {
    /// CS2 lists the players talking on the left, above the chat.
    fn default() -> Self {
        Region {
            x: 0.0,
            y: 0.5,
            w: 0.25,
            h: 0.3,
        }
    }
}

/// Screen rectangle in physical pixels (right / bottom excluded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.left < o.right && o.left < self.right && self.top < o.bottom && o.top < self.bottom
    }
}

/// 32-bit BGRA pixels, top row first.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// The screen rectangle of `region` inside the game window.
pub fn region_rect(window: Rect, r: &Region) -> Option<Rect> {
    let clamp = |v: f32| v.clamp(0.0, 1.0);
    let (x0, y0) = (clamp(r.x), clamp(r.y));
    let (x1, y1) = (clamp(r.x + r.w), clamp(r.y + r.h));
    let w = window.width() as f32;
    let h = window.height() as f32;
    let rect = Rect {
        left: window.left + (x0 * w).round() as i32,
        top: window.top + (y0 * h).round() as i32,
        right: window.left + (x1 * w).round() as i32,
        bottom: window.top + (y1 * h).round() as i32,
    };
    (rect.width() >= 16 && rect.height() >= 8).then_some(rect)
}

/// Blacks out the parts of the frame covered by Crosstalk's own windows (the
/// overlay must not be read as a player name).
pub fn mask(frame: &mut Frame, at: Rect, cover: &[Rect]) {
    for c in cover.iter().filter(|c| c.intersects(&at)) {
        let x0 = (c.left.max(at.left) - at.left) as usize;
        let x1 = (c.right.min(at.right) - at.left) as usize;
        let y0 = (c.top.max(at.top) - at.top) as usize;
        let y1 = (c.bottom.min(at.bottom) - at.top) as usize;
        for y in y0..y1 {
            let row = y * frame.width as usize * 4;
            frame.bgra[row + x0 * 4..row + x1 * 4].fill(0);
        }
    }
}

/// Nothing but black: CS2 in exclusive full screen is not captured.
pub fn is_blank(frame: &Frame) -> bool {
    frame
        .bgra
        .chunks_exact(4)
        .all(|p| p[0] < 8 && p[1] < 8 && p[2] < 8)
}

/// Doubles the size (CS2's names are small; OCR reads them better larger).
pub fn upscale2(f: &Frame) -> Frame {
    let (w, h) = (f.width as usize, f.height as usize);
    let mut out = vec![0u8; w * h * 16];
    for y in 0..h * 2 {
        let src = &f.bgra[(y / 2) * w * 4..(y / 2 + 1) * w * 4];
        let dst = &mut out[y * w * 8..(y + 1) * w * 8];
        for x in 0..w {
            dst[x * 8..x * 8 + 4].copy_from_slice(&src[x * 4..x * 4 + 4]);
            dst[x * 8 + 4..x * 8 + 8].copy_from_slice(&src[x * 4..x * 4 + 4]);
        }
    }
    Frame {
        width: f.width * 2,
        height: f.height * 2,
        bgra: out,
    }
}

/// The frame as a `data:image/bmp` URL, for the preview in the settings
/// (shown to the user only, never saved).
pub fn bmp_data_url(f: &Frame) -> String {
    let size = f.bgra.len() as u32;
    let mut b = Vec::with_capacity(54 + size as usize);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(54 + size).to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(f.width as i32).to_le_bytes());
    b.extend_from_slice(&(-(f.height as i32)).to_le_bytes()); // top row first
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(&[0; 4]); // BI_RGB
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&[0; 16]);
    b.extend_from_slice(&f.bgra);
    format!("data:image/bmp;base64,{}", base64(&b))
}

fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// CS2's window on screen (client area), and whether it is in front.
#[cfg(windows)]
pub fn cs2_window() -> Option<(Rect, bool)> {
    use windows::core::w;
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetClientRect, GetForegroundWindow, IsIconic,
    };
    // SAFETY: plain Win32 queries on a window handle, valid arguments.
    unsafe {
        let hwnd = FindWindowW(None, w!("Counter-Strike 2")).ok()?;
        if IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r).ok()?;
        let mut p = POINT::default();
        if !ClientToScreen(hwnd, &mut p).as_bool() {
            return None;
        }
        let rect = Rect {
            left: p.x,
            top: p.y,
            right: p.x + r.right,
            bottom: p.y + r.bottom,
        };
        Some((rect, GetForegroundWindow() == hwnd))
    }
}

#[cfg(not(windows))]
pub fn cs2_window() -> Option<(Rect, bool)> {
    None
}

/// Copies `rect` of the screen.
#[cfg(windows)]
pub fn grab(rect: Rect) -> Result<Frame, String> {
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };
    let (w, h) = (rect.width(), rect.height());
    if w <= 0 || h <= 0 {
        return Err("zone vide".into());
    }
    let mut bgra = vec![0u8; w as usize * h as usize * 4];
    // SAFETY: GDI objects are created, used and released in this block; the
    // buffer holds exactly w × h 32-bit pixels as described by `info`.
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err("écran inaccessible".into());
        }
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        let copied = BitBlt(mem, 0, 0, w, h, Some(screen), rect.left, rect.top, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let lines = GetDIBits(
            mem,
            bmp,
            0,
            h as u32,
            Some(bgra.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        copied.map_err(|e| format!("capture impossible : {e}"))?;
        if lines != h {
            return Err("capture incomplète".into());
        }
    }
    // GDI leaves the alpha byte at 0.
    bgra.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    Ok(Frame {
        width: w as u32,
        height: h as u32,
        bgra,
    })
}

#[cfg(not(windows))]
pub fn grab(_rect: Rect) -> Result<Frame, String> {
    Err("lecture de l'écran : Windows uniquement".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, v: u8) -> Frame {
        Frame {
            width: w,
            height: h,
            bgra: vec![v; (w * h * 4) as usize],
        }
    }

    #[test]
    fn the_region_follows_the_game_window() {
        let win = Rect {
            left: 1920,
            top: 0,
            right: 3840,
            bottom: 1080,
        };
        let r = region_rect(win, &Region::default()).unwrap();
        assert_eq!(
            r,
            Rect {
                left: 1920,
                top: 540,
                right: 2400,
                bottom: 864
            }
        );
        // Out of bounds values are clamped; a tiny area is refused.
        let r = region_rect(
            win,
            &Region {
                x: 0.9,
                y: -1.0,
                w: 0.5,
                h: 1.5,
            },
        )
        .unwrap();
        assert_eq!((r.right, r.top, r.bottom), (3840, 0, 540));
        assert!(region_rect(
            win,
            &Region {
                x: 0.5,
                y: 0.5,
                w: 0.001,
                h: 0.2
            }
        )
        .is_none());
    }

    #[test]
    fn crosstalk_windows_are_blacked_out() {
        let at = Rect {
            left: 100,
            top: 100,
            right: 110,
            bottom: 105,
        };
        let mut f = frame(10, 5, 200);
        mask(
            &mut f,
            at,
            &[
                Rect {
                    left: 105,
                    top: 0,
                    right: 500,
                    bottom: 103,
                },
                Rect {
                    left: 0,
                    top: 0,
                    right: 50,
                    bottom: 50,
                },
            ],
        );
        let px = |x: usize, y: usize| f.bgra[(y * 10 + x) * 4];
        assert_eq!((px(4, 0), px(5, 0), px(9, 2), px(5, 3)), (200, 0, 0, 200));
        assert!(!is_blank(&f));
        assert!(is_blank(&frame(4, 4, 3)));
    }

    #[test]
    fn upscale_and_preview() {
        let mut f = frame(2, 1, 0);
        f.bgra[4..8].copy_from_slice(&[1, 2, 3, 255]);
        let u = upscale2(&f);
        assert_eq!((u.width, u.height), (4, 2));
        assert_eq!(&u.bgra[8..16], &[1, 2, 3, 255, 1, 2, 3, 255]);
        assert_eq!(&u.bgra[16..24], &[0; 8]);
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        let url = bmp_data_url(&f);
        assert!(url.starts_with("data:image/bmp;base64,Qk0"));
    }
}
