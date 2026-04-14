fn main() {
    // Tell cargo to compile our C library.
    // The cc crate handles compiler flags, include paths, and object linking
    // so you don't have to write a Makefile.
    cc::Build::new()
        .file("c_libs/ccsds_framer.c")
        .compile("ccsds_framer");

    // Tell cargo to rerun this build script if any C files change.
    // Without these lines, cargo won't know to recompile when you edit the C code.
    println!("cargo:rerun-if-changed=c_libs/ccsds_framer.c");
    println!("cargo:rerun-if-changed=c_libs/ccsds_framer.h");
}
