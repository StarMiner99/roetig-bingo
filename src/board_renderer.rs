use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use image::{Rgb, RgbImage};
use rusttype::{Font, Scale, point};

const FONT_CANDIDATES: &[&str] = &[
    "Arial", "Helvetica", "DejaVuSans", "LiberationSans", "SegoeUI", "Segoe UI", "NotoSans-Regular", "NotoSans", "Cantarell-Regular"
];

fn find_system_font_data() -> Option<Vec<u8>> {
    // Allow explicit override for debugging or custom font selection
    if let Ok(path) = std::env::var("BINGO_FONT_PATH") {
        if let Ok(bytes) = fs::read(&path) { return Some(bytes); }
    }

    let mut search_dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        search_dirs.extend([
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/Library/Fonts"),
        ]);
        if let Some(home) = dirs_next::home_dir() { search_dirs.push(home.join("Library/Fonts")); }
    } else if cfg!(target_os = "windows") {
        if let Some(win) = std::env::var_os("WINDIR") { search_dirs.push(PathBuf::from(win).join("Fonts")); }
        search_dirs.push(PathBuf::from("C:/Windows/Fonts"));
    } else { // Linux / BSD
        search_dirs.extend([
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ]);
        if let Some(home) = dirs_next::home_dir() { search_dirs.push(home.join(".fonts")); }
        if let Some(home) = dirs_next::home_dir() { search_dirs.push(home.join(".local/share/fonts")); }
    }

    // Collect font files recursively to catch fonts in subdirectories (e.g., NotoSans subsets)
    let mut font_files: Vec<PathBuf> = Vec::new();
    for dir in search_dirs {
        if !dir.exists() { continue; }
        for entry in walkdir::WalkDir::new(&dir).follow_links(true).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() { continue; }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_l = ext.to_ascii_lowercase();
                if matches!(ext_l.as_str(), "ttf" | "otf") { font_files.push(path.to_path_buf()); }
            }
        }
    }

    if font_files.is_empty() { return None; }

    // Fast path: try candidate names first
    for &cand in FONT_CANDIDATES {
        if let Some(p) = font_files.iter().find(|p| p.file_stem().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case(cand)).unwrap_or(false)) {
            if let Ok(data) = fs::read(p) { return Some(data); }
        }
    }

    // Scoring: choose font with largest ASCII coverage (printable 32..=126)
    let mut best: Option<(usize, &Path)> = None;
    for path in &font_files {
        if let Ok(bytes) = fs::read(path) {
            if let Some(font) = Font::try_from_vec(bytes.clone()) {
                let mut score = 0usize;
                for ch in 32u8..=126u8 { // printable ASCII
                    let c = ch as char;
                    if font.glyph(c).id().0 != 0 { score += 1; }
                }
                if best.map(|(s, _)| score > s).unwrap_or(true) {
                    best = Some((score, path));
                }
            }
        }
    }
    if let Some((_, p)) = best { if let Ok(bytes) = fs::read(p) { return Some(bytes); } }

    None
}

struct TextPainter {
    font: Font<'static>,
    scale: Scale,
    line_height: f32,
}

impl TextPainter {
    fn new(font_data: Vec<u8>, px: f32) -> Result<Self, Box<dyn Error>> {
        let font = Font::try_from_vec(font_data).ok_or("Invalid font data")?;
        let scale = Scale::uniform(px);
        // Approximate line height using v_metrics
        let v = font.v_metrics(scale);
        let line_height = (v.ascent - v.descent + v.line_gap).ceil();
        Ok(Self { font, scale, line_height })
    }

    fn word_width(&self, word: &str) -> f32 {
        let v: Vec<_> = self.font.layout(word, self.scale, point(0.0, 0.0)).collect();
        if let Some(last) = v.last() {
            last.position().x + last.unpositioned().h_metrics().advance_width
        } else { 0.0 }
    }

    fn draw_wrapped(&self, img: &mut RgbImage, text: &str, left: u32, top: u32, max_w: u32, max_h: u32, color: Rgb<u8>) {
        let mut pen_y = 0.0f32;
        let v_metrics = self.font.v_metrics(self.scale);
        let ascent = v_metrics.ascent;
        let max_wf = max_w as f32;
        let max_hf = max_h as f32;
        let mut line = String::new();
        let mut line_width = 0.0f32;

        let words: Vec<&str> = text.split_whitespace().collect();
        for (i, w) in words.iter().enumerate() {
            let w_width = self.word_width(w);
            let extra = if line.is_empty() { 0.0 } else { self.word_width(" ") };
            if !line.is_empty() && line_width + extra + w_width > max_wf {
                // render current line
                if pen_y + self.line_height > max_hf { break; }
                self.draw_line(img, &line, left, top, pen_y + ascent, color);
                pen_y += self.line_height;
                line.clear();
                line_width = 0.0;
            }
            if !line.is_empty() { line.push(' '); line_width += extra; }
            line.push_str(w);
            line_width += w_width;
            if i == words.len()-1 {
                if pen_y + self.line_height > max_hf { break; }
                self.draw_line(img, &line, left, top, pen_y + ascent, color);
            }
        }
    }

    fn draw_line(&self, img: &mut RgbImage, text: &str, left: u32, top: u32, baseline_y: f32, color: Rgb<u8>) {
        for glyph in self.font.layout(text, self.scale, point(0.0, baseline_y)) {
            if let Some(bb) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    if v < 0.05 { return; }
                    let gx = left as i32 + x as i32 + bb.min.x;
                    let gy = top as i32 + y as i32 + bb.min.y;
                    if gx >= 0 && gy >= 0 && (gx as u32) < img.width() && (gy as u32) < img.height() {
                        let dst = img.get_pixel_mut(gx as u32, gy as u32);
                        let a = v;
                        for i in 0..3 { dst[i] = ((dst[i] as f32)*(1.0 - a) + (color[i] as f32)*a) as u8; }
                    }
                });
            }
        }
    }
}

pub fn render_board_to_png(elements: &[String], board_size: u32, path: &str) -> Result<(), Box<dyn Error>> {
    assert_eq!((board_size * board_size) as usize, elements.len(), "elements length must match board_size^2");

    // Layout constants
    let cell_px = 128u32; // cell size
    let padding = 20u32; // outer padding
    let grid_w = board_size * cell_px;
    let grid_h = board_size * cell_px;
    let img_w = grid_w + padding * 2;
    let img_h = grid_h + padding * 2;

    let bg = Rgb([245, 245, 245]);
    let line = Rgb([30, 30, 30]);
    let txt = Rgb([20, 20, 20]);

    let mut img = RgbImage::from_pixel(img_w, img_h, bg);

    // Draw grid lines
    for i in 0..=board_size {
        let y = padding + i * cell_px;
        if y < img_h { for x in padding..(padding+grid_w) { img.put_pixel(x, y, line); } }
        let x = padding + i * cell_px;
        if x < img_w { for y in padding..(padding+grid_h) { img.put_pixel(x, y, line); } }
    }

    // Load system font data
    let font_data = find_system_font_data().ok_or("No system font found for rendering")?;
    let painter = TextPainter::new(font_data, 18.0)?; // reduced font size per user request

    // Fill cells with text
    for row in 0..board_size {
        for col in 0..board_size {
            let idx = (row * board_size + col) as usize;
            let content = &elements[idx];
            let x0 = padding + col * cell_px + 10; // inner margin
            let y0 = padding + row * cell_px + 10;
            let max_w = cell_px - 20;
            let max_h = cell_px - 20;
            painter.draw_wrapped(&mut img, content, x0, y0, max_w, max_h, txt);
        }
    }

    let mut file = File::create(path)?;
    img.write_to(&mut file, image::ImageFormat::Png)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_render_board_to_png() {
        let elems: Vec<String> = (0..25).map(|i| format!("Item {i}" )).collect();
        render_board_to_png(&elems, 5, "test_board.png").expect("render");
        assert!(std::path::Path::new("test_board.png").exists());
        std::fs::remove_file("test_board.png").ok();
    }
}
