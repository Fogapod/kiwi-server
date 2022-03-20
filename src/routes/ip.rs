use actix_web::{
    get,
    web::{self, Data},
    HttpRequest, HttpResponse, Responder,
};
use image::{
    codecs::gif::{GifDecoder, GifEncoder},
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
#[allow(clippy::too_many_arguments)]
fn draw_text_with_border<'a, C>(
    canvas: &mut C,
    x: u32,
    y: u32,
    scale: Scale,
    font: &'a Font<'a>,
    text: &str,
    color: C::Pixel,
    outline_color: C::Pixel,
    outline_width: u8,
) where
    C: GenericImage<Pixel = Rgba<u8>>,
{
    let mut background: DynamicImage = DynamicImage::new_luma8(canvas.width(), canvas.height());

    imageproc::drawing::draw_text_mut(
        &mut background,
        color,
        x as i32,
        y as i32,
        scale,
        font,
        text,
    );

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

    imageproc::drawing::draw_text_mut(canvas, color, x as i32, y as i32, scale, font, text);
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
    let files = fs::read_dir(path).expect("read memes directory");

    files
        .choose(&mut rand::thread_rng())
        .expect("memes directory is empty")
        .expect("get next image in directory")
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
    // FIXME: allows elative paths!!!
    im: Option<PathBuf>,
}

#[get("")]
async fn get_ip(
    req: HttpRequest,
    font: Data<Font<'_>>,
    cfg: Data<Config>,
    // FIXME: allows elative paths!!!
    query: web::Query<IPQuery>,
) -> impl Responder {
    let conn_info = req.connection_info();
    let text = conn_info.realip_remote_addr().unwrap_or("anon");

    // FIXME: allows elative paths!!!
    let image_path = query
        .im
        .clone()
        .and_then(|path| {
            let full_path = cfg.memes_path.join(path);
            if full_path.exists() {
                Some(full_path)
            } else {
                None
            }
        })
        .unwrap_or_else(|| get_random_file(&cfg.memes_path));

    let (image_format, opened_image) = open_image(&image_path);

    let mut bytes = vec![];

    match image_format {
        ImageFormat::Gif => {
            let decoder = GifDecoder::new(opened_image.into_inner()).expect("reading gif image");

            let (dim_x, dim_y) = decoder.dimensions();

            let scale = Scale::uniform(dim_x as f32 / text.len() as f32 * 2.5);

            let rendered_text_size = text_size(scale, &font, text);

            let frames = decoder.into_frames();
            let mut frames = frames.collect_frames().expect("decoding gif");
            for frame in frames.iter_mut() {
                draw_text_with_border(
                    frame.buffer_mut(),
                    dim_x / 2 - min(rendered_text_size.0, dim_x) / 2,
                    min(
                        ((dim_y as f32 * 0.85) - rendered_text_size.1 as f32 * 0.5) as u32,
                        dim_y - min(rendered_text_size.1, dim_y),
                    ),
                    scale,
                    &font,
                    text,
                    Rgba([255u8, 255u8, 255u8, 255u8]),
                    Rgba([0u8, 0u8, 0u8, 255u8]),
                    2,
                );
            }
            let mut encoder = GifEncoder::new(&mut bytes);
            encoder
                .encode_frames(frames.into_iter())
                .expect("encoding gif");
        }
        _ => {
            let mut image = opened_image.decode().expect("reading image");
            let (dim_x, dim_y) = image.dimensions();

            let scale = Scale::uniform(dim_x as f32 / text.len() as f32 * 2.5);

            let rendered_text_size = text_size(scale, &font, text);

            draw_text_with_border(
                &mut image,
                dim_x / 2 - min(rendered_text_size.0, dim_x) / 2,
                min(
                    ((dim_y as f32 * 0.85) - rendered_text_size.1 as f32 * 0.5) as u32,
                    dim_y - min(rendered_text_size.1, dim_y),
                ),
                scale,
                &font,
                text,
                Rgba([255u8, 255u8, 255u8, 255u8]),
                Rgba([0u8, 0u8, 0u8, 255u8]),
                2,
            );
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
    let font_bytes = fs::read(env::var("FONT_FILE").expect("FONT_FILE not set"))
        .expect("font file does not exist");

    cfg.app_data(Data::new(
        Font::try_from_vec(font_bytes).expect("load font"),
    ))
    .app_data(Data::new(Config {
        memes_path: PathBuf::from(env::var("MEMES_PATH").expect("MEMES_PATH not set")),
    }))
    .service(get_ip);
}
