// Compila la interfaz Slint incrustando imágenes y fuentes dentro del ejecutable:
// así el .exe no depende de ningún archivo externo y se puede copiar a cualquier Windows.
fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
    slint_build::compile_with_config("ui/app.slint", config).expect("no se pudo compilar la interfaz");

    // Icono y datos de versión del .exe (solo en Windows).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "d20 Calculator");
        res.set("FileDescription", "d20 Calculator");
        if let Err(e) = res.compile() {
            println!("cargo:warning=no se pudo incrustar el icono: {e}");
        }
    }
    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=assets");
}
