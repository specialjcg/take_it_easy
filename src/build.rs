fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration de tonic-build
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &["proto/session_manager.proto"],
            &["proto/"],
        )?;

    // Alternative si vous voulez plus de contrôle :
    /*
    tonic_build::configure()
        .out_dir("src/generated")  // Répertoire de sortie personnalisé
        .build_server(true)
        .build_client(true)
        .compile(
            &["proto/session_manager.proto"],
            &["proto/"],
        )?;
    */

    Ok(())
}