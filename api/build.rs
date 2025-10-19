fn main() {
    cc::Build::new()
        .file("ipset_shim.c")
        .compile("ipset_shim");
    println!("cargo:rustc-link-lib=ipset");
}
