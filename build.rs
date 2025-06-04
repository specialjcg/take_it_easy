fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/generated")
        .compile(
            &[
                "protos/common.proto",
                "protos/session_service.proto",
                "protos/game_service.proto",
            ],
            &["protos"],
        )?;
    Ok(())
}