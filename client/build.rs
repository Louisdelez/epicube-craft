//! Windows : inscrit l'icone dans l'executable (bureau, barre des taches, explorateur).
fn main() {
    println!("cargo:rerun-if-changed=icon/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("icon/icon.ico");
        resource.compile().expect("icone de l'executable");
    }
}
