//! Embeds the Windows resources (the executable icon, `assets/nastro.rc`) into `nastro.exe`.

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/nastro.rc");
        println!("cargo:rerun-if-changed=assets/nastro.ico");
        if let Err(error) =
            embed_resource::compile("assets/nastro.rc", embed_resource::NONE).manifest_optional()
        {
            panic!("failed to embed the Windows resources: {error}");
        }
    }
}
