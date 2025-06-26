// ========== Part 0: Dependencies ==========
use pdfium_render::prelude::*;
use krilla::{
    Document, 
    text::{Font, KrillaGlyph, GlyphId}, 
    page::PageSettings, 
    geom::Point, 
    paint::Fill, 
    color::rgb, 
    num::NormalizedF32, 
    surface::Surface
};
use anyhow::Result;
use clap::Parser;
use std::ops::Range;
use rustybuzz::{Face, UnicodeBuffer};

// ========== Part 1: Define Glyph, Line and clustering functions ==========
#[derive(Clone)]
pub struct Glyph {
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub size: f32,
    pub font: String,
}

pub struct Line {
    pub glyphs: Vec<Glyph>,
    pub y: f32,
    pub font: String,
    pub size: f32,
}

fn group_lines(mut glyphs: Vec<Glyph>, _font: &Font) -> Vec<Line> {
    glyphs.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap());
    let mut lines: Vec<Vec<Glyph>> = Vec::new();
    for g in glyphs {
        if let Some(bucket) = lines.iter_mut().find(|bucket| (bucket[0].y - g.y).abs() < g.size * 0.4) {
            bucket.push(g);
        } else {
            lines.push(vec![g]);
        }
    }
    lines.into_iter().map(|mut gs| {
        gs.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        let font = gs[0].font.clone();
        let size = gs[0].size;
        let y = gs[0].y + gs[0].size * 0.22;
        let glyphs = gs.into_iter().filter(|g| !g.ch.is_control()).collect();
        Line { glyphs, y, font, size }
    }).collect()
}

// ========== Part 2: Extract lines ==========
pub fn extract_lines(path: &str, font: &Font) -> Result<Vec<Vec<Line>>> {
    let pdfium = Pdfium::default();
    let doc = pdfium.load_pdf_from_file(path, None)?;
    let mut pages_out = Vec::new();
    for page_index in 0..doc.pages().len() {
        let page = doc.pages().get(page_index)?;
        let tp = page.text()?;
        let mut glyphs = Vec::new();
        for ch in tp.chars().iter() {
            let c = ch.unicode_char();
            let bbox = ch.loose_bounds()?;
            let size = ch.scaled_font_size();
            let font_name = ch.font_name();
            let w = bbox.width().value as f32;
            glyphs.push(Glyph {
                ch: c.unwrap_or('?'),
                x: bbox.left().value as f32,
                y: bbox.bottom().value as f32,
                w,
                size: size.value as f32,
                font: font_name,
            });
        }
        let lines = group_lines(glyphs, font);
        pages_out.push(lines);
    }
    Ok(pages_out)
}

// ========== Part 3: Write PDF using krilla with Typst-like style ==========

fn load_font_and_bytes() -> (Font, Vec<u8>) {
    let font_paths = [
        "NewCM10-Regular.otf",
        "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
        "/System/Library/Fonts/Times.ttc",
        "C:/Windows/Fonts/times.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
    ];
    let mut font_data = None;
    let mut font_bytes = None;
    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            font_data = Some(data.clone());
            font_bytes = Some(data);
            println!("✅ Using font: {}", path);
            break;
        }
    }
    let font = if let Some(data) = &font_data {
        Font::new(data.clone().into(), 0).unwrap()
    } else {
        println!("⚠️  No fonts found, using fallback");
        Font::new(vec![].into(), 0).unwrap()
    };
    (font, font_bytes.unwrap_or_else(|| vec![]))
}

fn shape_line_with_rustybuzz(font_bytes: &[u8], line: &Line) -> (String, Vec<KrillaGlyph>) {
    let face = Face::from_slice(font_bytes, 0).unwrap();
    let upem = face.units_per_em() as f32;
    let mut buffer = UnicodeBuffer::new();
    let text: String = line.glyphs.iter().map(|g| g.ch).collect();
    buffer.push_str(&text);
    let output = rustybuzz::shape(&face, &[], buffer);
    let mut kglyphs = Vec::new();
    let mut cluster_to_range = Vec::new();
    for (i, _) in text.char_indices() {
        cluster_to_range.push(i);
    }
    cluster_to_range.push(text.len());
    for (info, pos) in output.glyph_infos().iter().zip(output.glyph_positions()) {
        let gid = GlyphId::new(info.glyph_id);
        let adv = pos.x_advance as f32 / upem;
        let dx  = pos.x_offset  as f32 / upem;
        let start = cluster_to_range.get(info.cluster as usize).copied().unwrap_or(0);
        let end = cluster_to_range.get(info.cluster as usize + 1).copied().unwrap_or(text.len());
        kglyphs.push(KrillaGlyph::new(
            gid, adv, dx, 0.0, 0.0, start..end, None,
        ));
    }
    (text, kglyphs)
}

fn draw_one_line<'a>(
    surface: &mut Surface<'a>,
    font: &Font,
    font_bytes: &[u8],
    line: &Line,
) {
    let (plain, kglyphs) = shape_line_with_rustybuzz(font_bytes, line);
    let start_x = line.glyphs[0].x;
    let baseline_y = 841.89 - line.y;
    surface.draw_glyphs(
        Point::from_xy(start_x, baseline_y),
        &kglyphs,
        font.clone(),
        &plain,
        line.size,
        false,
    );
}

pub fn render_like_typst(pages: Vec<Vec<Line>>, out: &str) -> Result<()> {
    let (font, font_bytes) = load_font_and_bytes();
    let mut document = Document::new();
    for (_page_num, lines) in pages.into_iter().enumerate() {
        let mut page = document.start_page_with(PageSettings::new(595.28, 841.89));
        let mut surface = page.surface();
        surface.set_fill(Some(Fill {
            paint: rgb::Color::new(0, 0, 0).into(),
            opacity: NormalizedF32::ONE,
            rule: Default::default(),
        }));
        for line in lines {
            draw_one_line(&mut surface, &font, &font_bytes, &line);
        }
        surface.finish();
        page.finish();
    }
    let pdf = document.finish().map_err(|e| anyhow::anyhow!("PDF generation failed: {:?}", e))?;
    std::fs::write(out, &pdf)?;
    Ok(())
}

// ========== Part 4: Command line entry ==========
#[derive(Parser)]
struct Opt {
    input: String,
    output: String,
}

fn main() -> Result<()> {
    let opt = Opt::parse();
    let (font, _font_bytes) = load_font_and_bytes();
    let pages = extract_lines(&opt.input, &font)?;
    // Print extracted text for debugging
    for (p, lines) in pages.iter().enumerate() {
        for line in lines {
            println!("page {:>2}  {:3.0} {:3.0}  size {:>4.1}  '{}'", 
                     p + 1, line.glyphs[0].x, line.glyphs[0].y, line.glyphs[0].size, line.glyphs[0].ch);
        }
    }
    render_like_typst(pages, &opt.output)?;
    println!("✅ Done: {}", opt.output);
    Ok(())
}