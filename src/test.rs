use rand::Rng;
use crate::plateau::Plateau;

pub fn create_shuffle_deck() -> Deck {
    let tiles = vec![
        new_tiles(1, 2, 3),
        new_tiles(1, 6, 8),
        new_tiles(1, 7, 3),
        new_tiles(1, 6, 3),
        new_tiles(1, 2, 8),
        new_tiles(1, 2, 4),
        new_tiles(1, 7, 4),
        new_tiles(1, 6, 4),
        new_tiles(1, 7, 8),
        new_tiles(5, 2, 3),
        new_tiles(5, 6, 8),
        new_tiles(5, 7, 3),
        new_tiles(5, 6, 3),
        new_tiles(5, 2, 8),
        new_tiles(5, 2, 4),
        new_tiles(5, 7, 4),
        new_tiles(5, 6, 4),
        new_tiles(5, 7, 8),
        new_tiles(9, 2, 3),
        new_tiles(9, 6, 8),
        new_tiles(9, 7, 3),
        new_tiles(9, 6, 3),
        new_tiles(9, 2, 8),
        new_tiles(9, 2, 4),
        new_tiles(9, 7, 4),
        new_tiles(9, 6, 4),
        new_tiles(9, 7, 8),
    ];

   Deck { tiles}
}


#[derive(Debug, Clone, PartialEq, Copy)]
pub(crate) struct Tile(pub i32, pub i32, pub i32);


fn new_tiles(x: i32, y: i32, z: i32) -> Tile {
    Tile(x, y, z)
}
#[derive(Debug, Clone, PartialEq)]
pub struct Deck{
    pub(crate) tiles: Vec<Tile>,
}


pub(crate) fn create_plateau_empty() -> Plateau {
    Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    }
}
pub(crate) fn remove_tile_from_deck(deck: &Deck, tile_to_remove: &Tile) -> Deck {
    // Filtre toutes les tuiles sauf celle à retirer
    let new_tiles: Vec<Tile> = deck
        .tiles
        .iter()
        .filter(|&tile| tile != tile_to_remove) // Conserve uniquement les tuiles différentes
        .cloned() // Copie chaque tuile dans le nouveau vecteur
        .collect();

    Deck { tiles: new_tiles } // Crée un nouveau deck
}
pub fn choisir_et_placer(deck: &mut Deck, plateau: &mut Plateau) {
    let mut rng = rand::thread_rng();

    // Répéter jusqu'à ce que le plateau soit plein
    while plateau.tiles.contains(&Tile(0, 0, 0)) {
        // Choisir une tuile aléatoirement dans le deck
        let deck_len = deck.tiles.len();
        if deck_len == 0 {
            break; // Plus de tuiles disponibles
        }
        let tile_index = rng.gen_range(0..deck_len);
        let tuile = deck.tiles.remove(tile_index); // Retirer la tuile du deck

        // Choisir une position aléatoire dans le plateau
        let mut position;
        loop {
            position = rng.gen_range(0..plateau.tiles.len());
            if plateau.tiles[position] == Tile(0, 0, 0) {
                break; // Trouver une case vide
            }
        }

        // Placer la tuile dans le plateau
        plateau.tiles[position] = tuile;
    }
}
fn placer_tile(plateau: &mut Plateau, tuile: Tile, position: usize) -> bool {
    if plateau.tiles[position] != Tile(0, 0, 0) {
        return false; // Case déjà occupée
    }
    plateau.tiles[position] = tuile;
    true
}

#[cfg(test)]
pub(crate) mod tests {
    use rand::Rng;
    use crate::plateau::Plateau;
    use crate::test::{choisir_et_placer, create_plateau_empty, create_shuffle_deck, Deck, placer_tile, remove_tile_from_deck,  Tile};



    #[test]
    fn test_placement_tuile_valide_take_it_easy() {
        let mut plateau:Plateau=create_plateau_empty();
        let deckSfuffle:Deck= create_shuffle_deck();
        let tuile = deckSfuffle.tiles[5].clone();
        assert!(placer_tile(&mut plateau, tuile.clone(), 1));
        assert_eq!(plateau.tiles[1], tuile);
    }
    #[test]
    fn test_placement_tuile_not_valide_take_it_easy() {
        let mut plateau:Plateau=create_plateau_empty();
        let deckSfuffle:Deck= create_shuffle_deck();
        let tile = deckSfuffle.tiles[5].clone();
        assert!(placer_tile(&mut plateau, tile.clone(), 1));
        assert_eq!(plateau.tiles[1], tile);
        let tile = deckSfuffle.tiles[5].clone();
        assert!(!placer_tile(&mut plateau, tile.clone(), 1));
    }
    #[test]
    fn test_choir_aleatorytile() {
        // Crée un deck
        let deck_shuffle: Deck = create_shuffle_deck();

        // Génère un index aléatoire
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..deck_shuffle.tiles.len());

        // Sélectionne une tuile aléatoire
        let tuile = deck_shuffle.tiles[index].clone();

        // Vérifie que la tuile existe dans le deck
        assert!(deck_shuffle.tiles.contains(&tuile));
        println!("Tuile choisie aléatoirement : {:?}", tuile);
    }
    #[test]
    fn test_retirer_tuile_aleatoire_du_deck() {
        use rand::Rng; // Pour générer un indice aléatoire

        // Crée un deck initial
        let deck_shuffle: Deck = create_shuffle_deck();

        // Génère un indice aléatoire
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..deck_shuffle.tiles.len());

        // Récupère la tuile choisie aléatoirement
        let tuile_choisie = deck_shuffle.tiles[index].clone();

        // Supprime la tuile du deck
        let nouveau_deck = remove_tile_from_deck(&deck_shuffle, &tuile_choisie);

        // Vérifie que la nouvelle taille du deck est réduite de 1
        assert_eq!(nouveau_deck.tiles.len(), deck_shuffle.tiles.len() - 1);

        // Vérifie que la tuile choisie n'est plus présente dans le nouveau deck
        assert!(!nouveau_deck.tiles.contains(&tuile_choisie));

        println!("Tuile retirée : {:?}", tuile_choisie);
        println!("Taille du deck initial : {}", deck_shuffle.tiles.len());
        println!("Taille du nouveau deck : {}", nouveau_deck.tiles.len());
    }
    #[test]
    fn test_remplir_plateau_take_it_easy() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();

        // Remplir le plateau
        choisir_et_placer(&mut deck, &mut plateau);

        // Vérifier que le plateau est plein
        assert!(!plateau.tiles.contains(&Tile(0, 0, 0)));
        println!("Deck restant : {:?}", deck.tiles);
        println!("Plateau final : {:?}", plateau.tiles);
        // Vérifier que le deck est vide ou contient moins de tuiles
        assert!(deck.tiles.len() + plateau.tiles.len() == 27);



    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_first_3_plateau_3_1() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[1].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 2);
        let point=result(&plateau);
        assert_eq!(point,3);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_first_3_plateau_3_2() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 2);
        let point=result(&plateau);
        assert_eq!(point,15);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_2_column_plateau_4_2() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 3);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 4);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 5);
        placer_tile(&mut plateau, deck.tiles[12].clone(), 6);
        println!("{:?}", plateau.tiles);


        let point=result(&plateau);
        assert_eq!(point,20);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_column_center_plateau_5_2() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 7);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 8);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 9);
        placer_tile(&mut plateau, deck.tiles[12].clone(), 10);
        placer_tile(&mut plateau, deck.tiles[13].clone(), 11);
        println!("{:?}", plateau.tiles);


        let point=result(&plateau);
        assert_eq!(point,25);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_column_4_plateau_4_2() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 12);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 13);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 14);
        placer_tile(&mut plateau, deck.tiles[12].clone(), 15);
        println!("{:?}", plateau.tiles);


        let point=result(&plateau);
        assert_eq!(point,20);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_last_column_3_plateau_3_1() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 16);
        placer_tile(&mut plateau, deck.tiles[1].clone(),17);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 18);
        let point=result(&plateau);
        assert_eq!(point,3);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_first_diag_plateau_0_3_7() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[4].clone(),3);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 7);
        let point=result(&plateau);
        assert_eq!(point,6);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_second_diag_plateau_1_4_8_12() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[4].clone(),4);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 8);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 12);
        let point=result(&plateau);
        assert_eq!(point,8);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_third_diag_plateau_2_5_9_13_16() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 2);
        placer_tile(&mut plateau, deck.tiles[4].clone(),5);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 9);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 13);
        placer_tile(&mut plateau, deck.tiles[13].clone(), 16);
        let point=result(&plateau);
        assert_eq!(point,10);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_fourth_diag_plateau_6_10_14_17() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 6);
        placer_tile(&mut plateau, deck.tiles[4].clone(),10);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 14);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 17);

        let point=result(&plateau);
        assert_eq!(point,8);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_last_diag_plateau_11_15_18() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 11);
        placer_tile(&mut plateau, deck.tiles[4].clone(),15);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 18);


        let point=result(&plateau);
        assert_eq!(point,6);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_firdt_diag_left_plateau_7_12_16() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 7);
        placer_tile(&mut plateau, deck.tiles[2].clone(),12);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 16);


        let point=result(&plateau);
        assert_eq!(point,9);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_second_diag_left_plateau_3_8_13_17() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 3);
        placer_tile(&mut plateau, deck.tiles[2].clone(),8);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 13);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 17);


        let point=result(&plateau);
        assert_eq!(point,12);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_third_diag_left_plateau_0_4_9_14_18() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[2].clone(),4);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 9);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 14);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 18);


        let point=result(&plateau);
        assert_eq!(point,15);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_fourth_diag_left_plateau_1_5_10_15() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[2].clone(),5);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 10);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 15);


        let point=result(&plateau);
        assert_eq!(point,12);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_last_diag_left_plateau_2_6_11() {
        let mut deck = create_shuffle_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 2);
        placer_tile(&mut plateau, deck.tiles[2].clone(),6);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 11);


        let point=result(&plateau);
        assert_eq!(point,9);
    }
    pub fn result(plateau: &Plateau) -> i32 {
        let mut result =0;
        if (plateau.tiles[0].0 == plateau.tiles[1].0) && (plateau.tiles[0].0 == plateau.tiles[2].0) {
            result=result+ plateau.tiles[0].0*3;
        }
        if (plateau.tiles[3].0 == plateau.tiles[4].0)
            && (plateau.tiles[3].0 == plateau.tiles[5].0)
            &&(plateau.tiles[3].0 == plateau.tiles[6].0)
           {
            result=result+ plateau.tiles[3].0*4;
        }
        if (plateau.tiles[7].0 == plateau.tiles[8].0)
            && (plateau.tiles[7].0 == plateau.tiles[9].0)
            &&(plateau.tiles[7].0 == plateau.tiles[10].0)
            &&(plateau.tiles[7].0 == plateau.tiles[11].0)
        {
            result=result+ plateau.tiles[7].0*5;
        }
        if (plateau.tiles[12].0 == plateau.tiles[13].0)
            && (plateau.tiles[12].0 == plateau.tiles[14].0)
            &&(plateau.tiles[12].0 == plateau.tiles[15].0)
        {
            result=result+ plateau.tiles[12].0*4;
        }
        if (plateau.tiles[16].0 == plateau.tiles[17].0) && (plateau.tiles[16].0 == plateau.tiles[18].0) {
            result=result+ plateau.tiles[16].0*3;
        }
        if (plateau.tiles[0].1 == plateau.tiles[3].1) && (plateau.tiles[0].1 == plateau.tiles[7].1) {
            result=result+ plateau.tiles[0].1*3;
        }
        if (plateau.tiles[1].1 == plateau.tiles[4].1)
            && (plateau.tiles[1].1 == plateau.tiles[8].1)
            &&(plateau.tiles[1].1 == plateau.tiles[12].1)
        {
            result=result+ plateau.tiles[1].1*4;
        }
        if (plateau.tiles[2].1 == plateau.tiles[5].1)
            && (plateau.tiles[2].1 == plateau.tiles[9].1)
            &&(plateau.tiles[2].1 == plateau.tiles[13].1)
            &&(plateau.tiles[2].1 == plateau.tiles[16].1)
        {
            result=result+ plateau.tiles[2].1*5;
        }
        if (plateau.tiles[6].1 == plateau.tiles[10].1)
            && (plateau.tiles[6].1 == plateau.tiles[14].1)
            &&(plateau.tiles[6].1 == plateau.tiles[17].1)
        {
            result=result+ plateau.tiles[6].1*4;
        }
        if (plateau.tiles[11].1 == plateau.tiles[15].1) && (plateau.tiles[11].1 == plateau.tiles[18].1) {
            result=result+ plateau.tiles[11].1*3;
        }
        if (plateau.tiles[7].2 == plateau.tiles[12].2) && (plateau.tiles[7].2 == plateau.tiles[16].2) {
            result=result+ plateau.tiles[7].2*3;
        }
        if (plateau.tiles[3].2 == plateau.tiles[8].2) && (plateau.tiles[3].2 == plateau.tiles[13].2 && plateau.tiles[3].2 == plateau.tiles[17].2) {
            result=result+ plateau.tiles[3].2*4;
        }
        if (plateau.tiles[0].2 == plateau.tiles[4].2)
            && (plateau.tiles[0].2 == plateau.tiles[9].2)
            &&(plateau.tiles[0].2 == plateau.tiles[14].2)
            &&(plateau.tiles[0].2 == plateau.tiles[18].2)
        {
            result=result+ plateau.tiles[0].2*5;
        }
        if (plateau.tiles[1].2 == plateau.tiles[5].2)
            && (plateau.tiles[1].2 == plateau.tiles[10].2)
            &&(plateau.tiles[1].2 == plateau.tiles[15].2)
        {
            result=result+ plateau.tiles[1].2*4;
        }
        if (plateau.tiles[2].2 == plateau.tiles[6].2) && (plateau.tiles[2].2 == plateau.tiles[11].2) {
            result=result+ plateau.tiles[2].2*3;
        }
        result
    }

}