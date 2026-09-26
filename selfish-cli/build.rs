//! Stamp the build's commit for `--version`, and put the tool's icon on the Windows executable.
//!
//! `assets/logo.ico` holds six sizes, so Windows picks one rather than rescaling the pixel art.
//! Linux and macOS executables have nowhere to carry an icon.

fn main() {
    oops_build::emit();

    println!("cargo:rerun-if-changed=../assets/logo.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../assets/logo.ico");
        // Fails the build rather than shipping a binary without the icon.
        resource
            .compile()
            .expect("could not embed assets/logo.ico in the executable");
    }
}
