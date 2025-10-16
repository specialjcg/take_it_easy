use crate::game::tile::Tile;

pub fn generate_tile_image_names(tiles: &[Tile]) -> Vec<String> {
    tiles
        .iter()
        .map(|tile| format!("../image/{}{}{}.png", tile.0, tile.1, tile.2))
        .collect()
}
