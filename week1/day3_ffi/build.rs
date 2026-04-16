fn main() {
    // Indica a cargo que compile nuestra biblioteca C.
    // El crate cc maneja los indicadores del compilador, rutas de inclusión y enlace de objetos
    // para que no tengas que escribir un Makefile.
    cc::Build::new()
        .file("c_libs/ccsds_framer.c")
        .compile("ccsds_framer");

    // Indica a cargo que vuelva a ejecutar este script de construcción si algún archivo C cambia.
    // Sin estas líneas, cargo no sabrá que debe recompilar cuando edites el código C.
    println!("cargo:rerun-if-changed=c_libs/ccsds_framer.c");
    println!("cargo:rerun-if-changed=c_libs/ccsds_framer.h");
}
