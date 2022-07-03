use std::{fs, path::Path};

use tiny_skia::{Pixmap, Transform};
use usvg::{FitTo, Options, Tree};

const ICON_SIZE: u32 = 64;
const ICON_SIZES: [u32; 2] = [16, 64];

fn main() {
    // Render the icon to a bitmap and store the raw bytes so that they can be included when the binary is compiled
    let input = Path::new("assets/icon.svg");
    let output = Path::new("target/icon.dump");
    let svg = fs::read_to_string(input).expect("Could not read svg");
    let mut opts = Options::default();
    opts.fontdb.load_system_fonts();
    let tree = Tree::from_str(&svg, &opts.to_ref()).expect("Could not parse svg");
    let mut pixmap = Pixmap::new(ICON_SIZE, ICON_SIZE).unwrap();
    resvg::render(
        &tree,
        FitTo::Size(ICON_SIZE, ICON_SIZE),
        Transform::identity(),
        pixmap.as_mut(),
    )
    .unwrap();
    fs::write(output, pixmap.data()).expect("Could not write image dump");

    #[cfg(windows)]
    {
        // Create a ico file and embed it with resources in the Windows executable
        let output = Path::new("target/icon.ico");
        let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
        for size in ICON_SIZES {
            let mut pixmap = Pixmap::new(size, size).unwrap();
            resvg::render(
                &tree,
                FitTo::Size(size, size),
                Transform::identity(),
                pixmap.as_mut(),
            )
            .unwrap();
            let img = ico::IconImage::from_rgba_data(size, size, pixmap.data().to_vec());
            icon.add_entry(ico::IconDirEntry::encode(&img).expect("Could not encode ico"));
        }
        {
            icon.write(fs::File::create(output).expect("Could not create icon file"))
                .expect("Could not write icon file");
        }
        // TODO The `winres` library needs to be updated, in order to do something with Rust 1.61+
        winres::WindowsResource::new()
            .set_icon(&output.to_string_lossy())
            .compile()
            .expect("Could not compile resources");
    }
}
