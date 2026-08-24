#[allow(unused)]
use std::path::Path;
#[allow(unused)]
const ICON_SIZE: u32 = 64;
#[allow(unused)]
const ICON_SIZES: [u32; 5] = [16, 32, 64, 96, 128];

#[cfg(not(any(feature = "gui", windows)))]
fn main() {}

#[cfg(any(feature = "gui", windows))]
fn main() {
    // Render the icon to a bitmap and store the raw bytes so that they can be included when the binary is compiled
    let input = Path::new("icon.svg");
    let output_bytes = Path::new("target/icon.bytes");

    let svg = std::fs::read_to_string(input).expect("Could not read svg");
    let mut opts = usvg::Options::default();
    opts.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(&svg, &opts).expect("Could not parse svg");
    let size = tree.size().width().max(tree.size().height());

    #[cfg(feature = "gui")]
    {
        let scale = (ICON_SIZE as f32) / size;
        let mut pixmap = tiny_skia::Pixmap::new(ICON_SIZE, ICON_SIZE).unwrap();
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        std::fs::write(output_bytes, pixmap.data()).expect("Could not write image dump");
    }

    #[cfg(windows)]
    {
        // Create a ico file and embed it with resources in the Windows executable
        let output_ico = Path::new("target\\icon.ico");
        let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
        for icon_size in ICON_SIZES {
            let scale = (icon_size as f32) / size;
            let mut pixmap = tiny_skia::Pixmap::new(icon_size, icon_size).unwrap();
            resvg::render(
                &tree,
                tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            let img = ico::IconImage::from_rgba_data(icon_size, icon_size, pixmap.data().to_vec());
            icon.add_entry(ico::IconDirEntry::encode(&img).expect("Could not encode ico"));
        }
        {
            icon.write(std::fs::File::create(output_ico).expect("Could not create icon file"))
                .expect("Could not write icon file");
        }
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&output_ico.to_string_lossy());
        res.set_language(0x0809);
        res.compile().expect("Could not compile resources");
    }
}
