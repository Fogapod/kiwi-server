use actix_web::{
    get,
    web::{self, Data},
    HttpRequest, HttpResponse, Responder,
};
use image::{
    codecs::gif::{self, GifDecoder, GifEncoder},
    io::Reader as ImageReader,
    AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, Rgba, RgbaImage,
};
use serde::Deserialize;

use imageproc::drawing::Canvas;
use rand::seq::IteratorRandom;
use rusttype::{Font, Scale};

use std::{
    cmp::min,
    env, fs,
    io::Cursor,
    path::{Path, PathBuf},
};

// text outline algo originated in:
// https://github.com/silvia-odwyer/gdl/blob/421c8df718ad32f66275d178edec56ec653caff9/crate/src/text.rs#L23
fn make_text_overlay(width: u32, height: u32, font: &Font, text: &str) -> (RgbaImage, (i64, i64)) {
    let color = Rgba([255, 255, 255, 255]);
    let outline_color = Rgba([0, 0, 0, 255]);
    // TODOL dynamic
    let outline_width = 3;

    // wtf is this insane math? TODO: cleanup
    let scale = Scale::uniform(width as f32 / text.len() as f32 * 2.5);
    let (rendered_width, rendered_height) = imageproc::drawing::text_size(scale, font, text);
    let (rendered_width, rendered_height) = (rendered_width as u32, rendered_height as u32);

    // wtf is this insane math? TODO: cleanup
    let pos_x = width / 2 - min(rendered_width, width) / 2;
    let pos_y = min(
        ((height as f32 * 0.85) - rendered_height as f32 * 0.5) as u32,
        height - min(rendered_height, height),
    ) - outline_width;

    let canvas_width = rendered_width + outline_width * 2;
    let canvas_height = rendered_height + outline_width * 2;

    let mut canvas = RgbaImage::new(canvas_width, canvas_height);

    let mut text_image = DynamicImage::new_luma8(canvas_width, canvas_height);
    let text_image = text_image.as_mut_luma8().unwrap();

    imageproc::drawing::draw_text_mut(
        text_image,
        image::Luma([255]),
        outline_width as i32,
        outline_width as i32,
        scale,
        font,
        text,
    );

    // grow letters
    imageproc::morphology::dilate_mut(
        text_image,
        imageproc::distance_transform::Norm::LInf,
        outline_width as u8,
    );

    imageproc::drawing::draw_text_mut(
        text_image,
        image::Luma([128]),
        outline_width as i32,
        outline_width as i32,
        scale,
        font,
        text,
    );

    for x in 0..canvas_width {
        for y in 0..canvas_height {
            match text_image.get_pixel(x, y).0[0] {
                // janky but works
                200..=255 => canvas.put_pixel(x, y, outline_color),
                1..=199 => canvas.put_pixel(x, y, color),
                _ => continue,
            }
        }
    }

    (canvas, (pos_x.into(), pos_y.into()))
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

            // speed value of 10 is recommended but it semm to have no effect at the moment:
            // - https://github.com/image-rs/image/issues/1467
            // - https://github.com/image-rs/image-gif/issues/108
            // - https://github.com/image-rs/image-gif/issues/113
            let mut encoder = GifEncoder::new_with_speed(&mut bytes, 10);
            encoder
                .set_repeat(gif::Repeat::Infinite)
                .expect("setting gif repeat");

            let (dim_x, dim_y) = decoder.dimensions();

            let (overlay, (pos_x, pos_y)) = make_text_overlay(dim_x, dim_y, &font, text);

            for mut frame in decoder.into_frames().map(|f| f.expect("decoding frame")) {
                image::imageops::overlay(frame.buffer_mut(), &overlay, pos_x, pos_y);
                encoder.encode_frame(frame).expect("encoding frame");
            }
        }
        _ => {
            let mut image = opened_image.decode().expect("reading image");
            let (dim_x, dim_y) = image.dimensions();

            let (overlay, (pos_x, pos_y)) = make_text_overlay(dim_x, dim_y, &font, text);
            image::imageops::overlay(&mut image, &overlay, pos_x, pos_y);

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
