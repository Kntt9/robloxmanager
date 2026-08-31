fn main() {
    let mut res = winresource::WindowsResource::new();
    // build.rs runs with the crate dir (ram_ui/) as CWD, so the shared
    // assets folder is one level up.
    res.set_icon("../assets/knt-logo.ico");
    res.set("FileDescription", "KNT Manager");
    res.set("ProductName", "KNT Manager");
    res.set("LegalCopyright", "");
    if let Err(e) = res.compile() {
        eprintln!("winresource compile failed: {e}");
    }
}