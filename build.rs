use std::fs;
use std::path::Path;

use tiny_skia::{Pixmap, Transform};
use usvg::{Options, Tree};

const ICON_SIZE: u32 = 64;
const ICON_SIZES: [u32; 5] = [16, 32, 64, 96, 128];

fn main() {
    // Render the icon to a bitmap and store the raw bytes so that they can be included when the binary is compiled
    let input = Path::new("assets/icon.svg");
    let output_bytes = Path::new("target/icon.bytes");
    let output_ico = Path::new("target\\icon.ico");

    let tree;
    let size;
    #[cfg(any(feature = "gui", windows))]
    {
        let svg = fs::read_to_string(input).expect("Could not read svg");
        let mut opts = Options::default();
        opts.fontdb_mut().load_system_fonts();
        tree = Tree::from_str(&svg, &opts).expect("Could not parse svg");
        size = tree.size().width().max(tree.size().height());
    }

    #[cfg(feature = "gui")]
    {
        let scale = (ICON_SIZE as f32) / size;
        let mut pixmap = Pixmap::new(ICON_SIZE, ICON_SIZE).unwrap();
        resvg::render(
            &tree,
            Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        fs::write(output_bytes, pixmap.data()).expect("Could not write image dump");
    }

    #[cfg(windows)]
    {
        // Create a ico file and embed it with resources in the Windows executable
        let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
        for icon_size in ICON_SIZES {
            let scale = (icon_size as f32) / size;
            let mut pixmap = Pixmap::new(icon_size, icon_size).unwrap();
            resvg::render(
                &tree,
                Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            let img = ico::IconImage::from_rgba_data(icon_size, icon_size, pixmap.data().to_vec());
            icon.add_entry(ico::IconDirEntry::encode(&img).expect("Could not encode ico"));
        }
        {
            icon.write(fs::File::create(output_ico).expect("Could not create icon file"))
                .expect("Could not write icon file");
        }
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&output_ico.to_string_lossy());
        res.set_language(0x0809);
        res.compile().expect("Could not compile resources");
    }
}
