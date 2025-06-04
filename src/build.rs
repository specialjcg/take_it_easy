fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration de tonic-build
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &["protos/session_manager.protos"],
            &["protos/"],
        )?;

    // Alternative si vous voulez plus de contrôle :
    /*
    tonic_build::configure()
        .out_dir("src/generated")  // Répertoire de sortie personnalisé
        .build_server(true)
        .build_client(true)
        .compile(
            &["protos/session_manager.protos"],
            &["protos/"],
        )?;
    */

    Ok(())
}