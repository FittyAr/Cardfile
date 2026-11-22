//! Build script: embed the icon and version information into the Windows executable.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/windows/notechest.ico")
            .set("ProductName", "NoteChest")
            .set("FileDescription", "NoteChest")
            .set("CompanyName", "FittyAr")
            .set(
                "LegalCopyright",
                "Copyright (c) 2026 FittyAr. MIT OR Apache-2.0.",
            )
            .set("OriginalFilename", "notechest.exe");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
    println!("cargo:rerun-if-changed=../../packaging/windows/notechest.ico");
}
