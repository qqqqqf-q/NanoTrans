use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    slint_build::compile("ui/main.slint").unwrap();

    #[cfg(target_os = "macos")]
    {
        generate_tray_png("assets/tray/t.svg", &out_dir().join("tray.png"));
        generate_app_iconset(
            "assets/icons/icon.ico",
            &target_dir().join("app-icons/NanoTrans.iconset"),
        );
        println!("cargo:rerun-if-changed=assets/tray/t.svg");
        println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    }

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icons/icon.ico");
        res.compile().unwrap();
    }
}

fn out_dir() -> PathBuf {
    env::var("OUT_DIR")
        .map(PathBuf::from)
        .expect("OUT_DIR is not set")
}

fn target_dir() -> PathBuf {
    if let Ok(dir) = env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }

    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"))
        .join("target")
}

#[cfg(target_os = "macos")]
fn generate_tray_png(svg_path: &str, output_path: &Path) {
    let svg_data = fs::read(svg_path).expect("Failed to read tray svg");
    let mut options = resvg::usvg::Options::default();
    options.resources_dir = Path::new(svg_path).parent().map(Path::to_path_buf);

    let tree = resvg::usvg::Tree::from_data(&svg_data, &options)
        .expect("Failed to parse tray svg");
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(32, 32).expect("Failed to create tray pixmap");
    let size = tree.size();
    let transform = resvg::tiny_skia::Transform::from_scale(
        32.0 / size.width(),
        32.0 / size.height(),
    );
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, transform, &mut pixmap_mut);

    fs::write(output_path, pixmap.encode_png().expect("Failed to encode tray png"))
        .expect("Failed to write tray png");
}

#[cfg(target_os = "macos")]
fn generate_app_iconset(icon_path: &str, output_dir: &Path) {
    let img = image::ImageReader::open(icon_path)
        .expect("Failed to open icon")
        .with_guessed_format()
        .expect("Failed to guess icon format")
        .decode()
        .expect("Failed to decode icon");

    fs::create_dir_all(output_dir).expect("Failed to create iconset dir");

    let entries = [
        (16, "icon_16x16.png"),
        (32, "icon_16x16@2x.png"),
        (32, "icon_32x32.png"),
        (64, "icon_32x32@2x.png"),
        (128, "icon_128x128.png"),
        (256, "icon_128x128@2x.png"),
        (256, "icon_256x256.png"),
        (512, "icon_256x256@2x.png"),
        (512, "icon_512x512.png"),
        (1024, "icon_512x512@2x.png"),
    ];

    for (size, name) in entries {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let path = output_dir.join(name);
        resized
            .save(&path)
            .unwrap_or_else(|_| panic!("Failed to write icon {}", path.display()));
    }
}
