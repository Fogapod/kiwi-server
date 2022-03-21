use actix_web::{
    get,
    web::{self, Data},
    HttpRequest, HttpResponse, Responder,
};
use image::{
    codecs::gif::{self, GifDecoder, GifEncoder},
    io::Reader as ImageReader,
    AnimationDecoder, DynamicImage, GenericImage, ImageDecoder, ImageFormat, Rgba,
};
use serde::Deserialize;

use imageproc::drawing::Canvas;
use rand::seq::IteratorRandom;
use rusttype::{point, Font, PositionedGlyph, Rect, Scale};
use std::{
    cmp::{max, min},
    env, fs,
    io::Cursor,
    path::{Path, PathBuf},
};

// a modified version of:
// https://github.com/silvia-odwyer/gdl/blob/421c8df718ad32f66275d178edec56ec653caff9/crate/src/text.rs#L23
fn make_text_overlay<'a>(
    width: u32,
    height: u32,
    font: &'a Font<'a>,
    text: &str,
) -> impl GenericImage<Pixel = Rgba<u8>> {
    let color = Rgba([255, 255, 255, 255]);
    let outline_color = Rgba([0, 0, 0, 255]);
    // TODOL dynamic
    let outline_width: u32 = 3;

    // wtf is this insane math? TODO: cleanup
    let scale = Scale::uniform(width as f32 / text.len() as f32 * 2.5);
    let (rendered_width, rendered_height) = text_size(scale, font, text);

    // wtf is this insane math? TODO: cleanup
    let pos_x = width / 2 - min(rendered_width, width) / 2;
    let pos_y = min(
        ((height as f32 * 0.85) - rendered_height as f32 * 0.5) as u32,
        height - min(rendered_height, height),
    );

    let mut canvas = DynamicImage::new_rgba8(width, height);

    let mut text_image: DynamicImage = DynamicImage::new_luma8(width, height);
    let mut text_image_outline: DynamicImage = DynamicImage::new_luma8(width, height);

    imageproc::drawing::draw_text_mut(
        &mut text_image,
        color,
        pos_x as i32,
        pos_y as i32,
        scale,
        font,
        text,
    );
    imageproc::drawing::draw_text_mut(
        &mut text_image_outline,
        color,
        pos_x as i32,
        pos_y as i32,
        scale,
        font,
        text,
    );

    let mut text_image_outline = text_image_outline.to_luma8();

    imageproc::morphology::dilate_mut(
        &mut text_image_outline,
        imageproc::distance_transform::Norm::LInf,
        outline_width as u8,
    );

    // FIXME: these can probably overflow on outline_width
    for x in pos_x - outline_width..width {
        for y in pos_y - outline_width..height {
            let pixval = 255 - text_image_outline.get_pixel(x, y).0[0];
            if pixval != 255 {
                canvas.put_pixel(x, y, outline_color);
            }
        }
    }
    for x in pos_x - outline_width..width {
        for y in pos_y - outline_width..height {
            let pixval = 255 - text_image.get_pixel(x, y).0[0];
            if pixval != 255 {
                canvas.put_pixel(x, y, color);
            }
        }
    }

    canvas
}

fn paste_image_mut<S, D>(src: &S, dst: &mut D)
where
    S: GenericImage<Pixel = Rgba<u8>>,
    D: GenericImage<Pixel = Rgba<u8>>,
{
    for x in 0..dst.width() {
        for y in 0..dst.height() {
            let pixel = src.get_pixel(x, y);
            if pixel.0[3] != 0 {
                dst.put_pixel(x, y, pixel);
            }
        }
    }
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
fn text_size(scale: Scale, font: &Font, text: &str) -> (u32, u32) {
    let (x, y) = layout_glyphs(scale, font, text, |_, _| {});

    (x as u32, y as u32)
}

fn get_random_file(path: &Path) -> PathBuf {
    let files = fs::read_dir(path).expect("reading memes folder");

    files
        .choose(&mut rand::thread_rng())
        .expect("memes folder is empty")
        .expect("geting next image in directory")
        .path()
}

fn open_image(path: &Path) -> (ImageFormat, ImageReader<Cursor<Vec<u8>>>) {
    let random_image = fs::read(path).expect("read image");

    let image_reader = ImageReader::new(Cursor::new(random_image))
        .with_guessed_format()
        .expect("guessing image format");

    (
        image_reader.format().expect("format not detected"),
        image_reader,
    )
}

// a reversed version of ImageFormat.from_extension
fn image_format_to_mime(format: &ImageFormat) -> &'static str {
    match format {
        ImageFormat::Avif => "image/avif",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Png => "image/png",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        ImageFormat::Tiff => "image/tiff",
        ImageFormat::Tga => "image/x-targa",
        ImageFormat::Dds => "image/vnd-ms.dds",
        ImageFormat::Bmp => "image/bmp",
        ImageFormat::Ico => "image/x-icon",
        ImageFormat::Hdr => "image/vnd.radiance",
        ImageFormat::OpenExr => "image/x-exr",
        ImageFormat::Pnm => "image/x-portable-bitmap",
        _ => "application/octet-stream",
    }
}

#[derive(Debug, Deserialize)]
struct IPQuery {
    image: Option<String>,
}

#[get("")]
async fn get_ip(
    req: HttpRequest,
    font: Data<Font<'_>>,
    cfg: Data<Config>,
    query: web::Query<IPQuery>,
) -> impl Responder {
    let conn_info = req.connection_info();
    let text = conn_info.realip_remote_addr().unwrap_or("anon");

    let image_path = query
        .image
        .clone()
        .and_then(|path| {
            // hope this is safe.. i did not find any other meaningful way
            if path.contains("..") {
                None
            } else {
                let full_path = cfg.memes_path.join(&path);
                if full_path.is_file() {
                    Some(full_path)
                } else {
                    None
                }
            }
        })
        .unwrap_or_else(|| get_random_file(&cfg.memes_path));

    let (image_format, opened_image) = open_image(&image_path);

    let mut bytes = vec![];

    match image_format {
        ImageFormat::Gif => {
            let decoder = GifDecoder::new(opened_image.into_inner()).expect("reading gif image");
            let mut encoder = GifEncoder::new(&mut bytes);
            encoder
                .set_repeat(gif::Repeat::Infinite)
                .expect("setting gif repeat");

            let (dim_x, dim_y) = decoder.dimensions();

            let overlay = make_text_overlay(dim_x, dim_y, &font, text);

            for mut frame in decoder.into_frames().map(|f| f.expect("decoding frame")) {
                paste_image_mut(&overlay, frame.buffer_mut());
                encoder.encode_frame(frame).expect("encoding frame");
            }
        }
        _ => {
            let mut image = opened_image.decode().expect("reading image");
            let (dim_x, dim_y) = image.dimensions();

            paste_image_mut(&make_text_overlay(dim_x, dim_y, &font, text), &mut image);

            image
                .write_to(&mut Cursor::new(&mut bytes), image_format)
                .expect("encoding image");
        }
    }

    HttpResponse::Ok()
        .content_type(image_format_to_mime(&image_format))
        .body(bytes)
}

struct Config {
    memes_path: PathBuf,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    let font_bytes =
        fs::read(env::var("FONT_FILE").expect("FONT_FILE not set")).expect("font file missing");

    cfg.app_data(Data::new(
        Font::try_from_vec(font_bytes).expect("loading font"),
    ))
    .app_data(Data::new(Config {
        memes_path: PathBuf::from(env::var("MEMES_PATH").expect("MEMES_PATH not set")),
    }))
    .service(get_ip);
}
