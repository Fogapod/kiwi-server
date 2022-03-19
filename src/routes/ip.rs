use actix_web::web::Data;
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use std::cmp::{max, min};
use std::env;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use image::GenericImageView;
use image::{DynamicImage, GenericImage, Rgba};
use rand::seq::IteratorRandom;
use rusttype::{point, Font, PositionedGlyph, Rect, Scale};

// a modified version of:
// https://github.com/silvia-odwyer/gdl/blob/421c8df718ad32f66275d178edec56ec653caff9/crate/src/text.rs#L23
#[allow(clippy::too_many_arguments)]
pub fn draw_text_with_border<'a>(
    canvas: &mut DynamicImage,
    x: i32,
    y: i32,
    scale: Scale,
    font: &'a Font<'a>,
    text: &str,
    color: Rgba<u8>,
    outline_color: Rgba<u8>,
    outline_width: u8,
) {
    let mut background: DynamicImage = DynamicImage::new_luma8(canvas.width(), canvas.height());

    imageproc::drawing::draw_text_mut(&mut background, color, x, y, scale, font, text);

    let mut background = background.to_luma8();

    imageproc::morphology::dilate_mut(
        &mut background,
        imageproc::distance_transform::Norm::LInf,
        outline_width,
    );

    // Add a border to the text.
    for x in 0..background.width() {
        for y in 0..background.height() {
            let pixval = 255 - background.get_pixel(x, y).0[0];
            if pixval != 255 {
                canvas.put_pixel(x, y, outline_color);
            }
        }
    }

    imageproc::drawing::draw_text_mut(canvas, color, x, y, scale, font, text);
}

// taken from https://github.com/image-rs/imageproc/pull/453
// because it is not yet released
fn layout_glyphs(
    scale: Scale,
    font: &Font,
    text: &str,
    mut f: impl FnMut(PositionedGlyph, Rect<i32>),
) -> (i32, i32) {
    let v_metrics = font.v_metrics(scale);

    let (mut w, mut h) = (0, 0);

    for g in font.layout(text, scale, point(0.0, v_metrics.ascent)) {
        if let Some(bb) = g.pixel_bounding_box() {
            w = max(w, bb.max.x);
            h = max(h, bb.max.y);
            f(g, bb);
        }
    }

    (w, h)
}

// taken from https://github.com/image-rs/imageproc/pull/453
// because it is not yet released
pub fn text_size(scale: Scale, font: &Font, text: &str) -> (i32, i32) {
    layout_glyphs(scale, font, text, |_, _| {})
}

#[get("")]
async fn get_ip(req: HttpRequest, font: Data<Font<'_>>, cfg: Data<Config>) -> impl Responder {
    let conn_info = req.connection_info();
    let text = &conn_info
        .realip_remote_addr()
        .unwrap_or("unknown remote address");

    let files = fs::read_dir(&cfg.memes_path).expect("unable to read memes directory");

    let mut rng = rand::thread_rng();

    let mut image = image::open(
        files
            .choose(&mut rng)
            .expect("memes directory is empty")
            .expect("unable to get next memes file in directory")
            .path(),
    )
    .expect("cannot open file");

    let (dim_x, dim_y) = image.dimensions();
    let (dim_x, dim_y) = (dim_x as i32, dim_y as i32);

    let scale = Scale::uniform(dim_x as f32 / text.len() as f32 * 2.5);

    let rendered_text_size = text_size(scale, &font, text);

    draw_text_with_border(
        &mut image,
        dim_x / 2 - min(rendered_text_size.0, dim_x) / 2,
        min(
            ((dim_y as f32 * 0.85) - rendered_text_size.1 as f32 * 0.5) as i32,
            dim_y - min(rendered_text_size.1, dim_y),
        ),
        scale,
        &font,
        text,
        Rgba([255u8, 255u8, 255u8, 255u8]),
        Rgba([0u8, 0u8, 0u8, 255u8]),
        2,
    );

    let mut bytes = vec![];

    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .expect("failed to encode image");

    HttpResponse::Ok().content_type("image/jpeg").body(bytes)
}

struct Config {
    memes_path: PathBuf,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    let font_bytes = fs::read(env::var("FONT_FILE").expect("FONT_FILE not set"))
        .expect("font file does not exist");

    cfg.app_data(Data::new(
        Font::try_from_vec(font_bytes).expect("failed to load font"),
    ))
    .app_data(Data::new(Config {
        memes_path: PathBuf::from(env::var("MEMES_PATH").expect("MEMES_PATH not set")),
    }))
    .service(get_ip);
}
