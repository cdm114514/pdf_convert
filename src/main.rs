// ========== Part 0: Dependencies ==========
use pdfium_render::prelude::*;
use pdf_writer::{Pdf, Ref, Rect, Content, Name}; 
use pdf_writer::Finish;
use anyhow::Result;
use clap::Parser;

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
    pub text: String,
    pub x: f32,
    pub baseline: f32,
    pub font: String,
    pub size: f32,
}

fn group_lines(mut glyphs: Vec<Glyph>) -> Vec<Line> {
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
        let mut text = String::new();
        let mut prev_right = gs[0].x;
        for g in &gs {
            if g.x - prev_right > g.size * 0.3 {
                text.push(' ');
            }
            text.push(g.ch);
            prev_right = g.x + g.w;
        }
        Line {
            text,
            x: gs[0].x,
            baseline: gs[0].y + gs[0].size * 0.22,
            font: gs[0].font.clone(),
            size: gs[0].size,
        }
    }).collect()
}

// ========== Part 2: Extract lines ==========
pub fn extract_lines(path: &str) -> Result<Vec<Vec<Line>>> {
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
            let font = ch.font_name();
            let w = bbox.width().value as f32;
            glyphs.push(Glyph {
                ch: c.unwrap_or('?'),
                x: bbox.left().value as f32,
                y: bbox.bottom().value as f32,
                w,
                size: size.value as f32,
                font,
            });
        }
        let lines = group_lines(glyphs);
        pages_out.push(lines);
    }
    Ok(pages_out)
}

// ========== Part 3: Write PDF ==========
pub fn render_like_typst(pages: Vec<Vec<Line>>, out: &str) -> Result<()> {
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let pages_id = Ref::new(2);
    let mut next_id = 10;
    let font_id = Ref::new(9999);
    let pages_count = pages.len();

    pdf.catalog(catalog).pages(pages_id);
    pdf.type1_font(font_id).base_font(Name(b"Times-Roman"));

    let mut page_refs = Vec::new();
    for lines in pages.into_iter() {
        let page = Ref::new(next_id);
        let contents = Ref::new(next_id + 1);
        next_id += 2;
        page_refs.push(page);

        let mut c = Content::new();
        for line in lines {
            c.begin_text();
            c.set_font(Name(b"F1"), line.size);
            c.set_text_matrix([1.0, 0.0, 0.0, 1.0, line.x, line.baseline]);
            c.show(pdf_writer::Str(line.text.as_bytes()));
            c.end_text();
        }
        pdf.stream(contents, &c.finish());

        pdf.page(page)
            .parent(pages_id)
            .media_box(Rect::new(0.0, 0.0, 595.0, 842.0))
            .contents(contents)
            .resources()
                .fonts()
                    .pair(Name(b"F1"), font_id)
                    .finish()
                .finish();
    }
    pdf.pages(pages_id)
        .count(pages_count as i32)
        .kids(page_refs);
    std::fs::write(out, pdf.finish())?;
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
    let pages = extract_lines(&opt.input)?;
    for (p, lines) in pages.iter().enumerate() {
        for line in lines {
            println!("page {:>2}  {:3.0} {:3.0}  size {:>4.1}  '{}'", p + 1, line.x, line.baseline, line.size, line.text);
        }
    }
    render_like_typst(pages, &opt.output)?;
    println!("✅ Done: {}", opt.output);
    Ok(())
}