// Rebuild when translations change (rust-i18n embeds them at compile time).
fn main() {
    println!("cargo:rerun-if-changed=../../locales");
}
