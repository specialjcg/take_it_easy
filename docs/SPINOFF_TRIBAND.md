# TriBand - Spin-off du moteur Take It Easy

**Statut** : Proposition
**Date** : 2026-03-03
**Objectif** : Creer un jeu original base sur le moteur existant pour eviter les problemes de propriete intellectuelle avec Take It Easy (marque Burley Games / Ravensburger).

---

## 1. Pourquoi un spin-off

### Probleme juridique

| Element de Take It Easy | Protection | Risque |
|--------------------------|-----------|--------|
| Nom "Take It Easy" | Marque deposee (Burley Games) | Eleve |
| Artwork / design des tuiles | Copyright | Moyen |
| App digitale | Licence Ravensburger Digital | Eleve |
| Grille hexagonale 19 cases | Non protegeable (geometrie) | Aucun |
| Scoring par lignes | Non protegeable (mecanique) | Aucun |
| Tirage aleatoire simultane | Non protegeable (mecanique) | Aucun |

Un spin-off avec nom, valeurs et regles de scoring differents neutralise tout risque tout en reutilisant 95% du code (architecture GT, expectimax, frontend Elm, auth, infra).

### Avantage narratif

"J'ai concu mon propre jeu de plateau + une IA qui le bat" > "J'ai clone un jeu existant".

---

## 2. Regles de TriBand

### Ce qui ne change pas (vs Take It Easy)

- Grille hexagonale 19 cases, layout 3-4-5-4-3
- 3 bandes par tuile, une par direction (horizontale, diag-gauche, diag-droite)
- 15 lignes de scoring (5 par direction)
- Tirage aleatoire, 19 tours, placement simultane
- Pas de rotation des tuiles

### Ce qui change

#### 2.1 Systeme de tuiles

**Take It Easy** : 3 valeurs x 3 valeurs x 3 valeurs = 27 tuiles fixes
- Direction 0 : {1, 5, 9}
- Direction 1 : {2, 6, 7}
- Direction 2 : {3, 4, 8}

**TriBand** : 5 valeurs x 4 valeurs x 3 valeurs = 60 tuiles dans le pool, 19 tirees par partie
- Direction 0 (horizontale) : {1, 3, 5, 7, 9}
- Direction 1 (diag-gauche) : {2, 4, 6, 8}
- Direction 2 (diag-droite) : {1, 5, 9}

Consequences :
- Pool de 60 tuiles (vs 27 fixes) → chaque partie est differente
- On tire 19 tuiles sur 60 → le joueur ne sait pas quelles tuiles restent
- Plus de diversite de valeurs → plus de choix strategiques
- La direction 0 a le plus de valeurs (5) → les lignes horizontales sont les plus dures a completer

#### 2.2 Scoring graduel (changement majeur)

**Take It Easy** : tout-ou-rien. Si une tuile casse la ligne → 0 points.

**TriBand** : scoring en 3 paliers par ligne.

```
Pour chaque ligne de longueur L :
  - Compter le nombre M de tuiles portant la valeur majoritaire V
  - Si M == L (ligne complete)  : score += V × L × 2    (bonus x2)
  - Si M >= ceil(L/2) (majorite): score += V × M
  - Sinon                       : score += 0
```

Exemples sur une ligne de 5 cases (direction 0) :

| Valeurs placees     | V | M | Palier    | Score |
|---------------------|---|---|-----------|-------|
| [7, 7, 7, 7, 7]    | 7 | 5 | Complete  | 7×5×2 = **70** |
| [7, 7, 7, 3, 1]    | 7 | 3 | Majorite  | 7×3 = **21** |
| [7, 7, 3, 3, 1]    | 7 | 2 | Aucun     | **0** |
| [9, 9, 9, 9, _]    | 9 | 4 | Majorite* | 9×4 = **36** |

(*) 4 sur 5, la case vide est ignoree. Complete si la 5e est aussi un 9.

Comparaison avec Take It Easy :
- TIE : [7, 7, 7, 3, 1] → **0 pts** (tout-ou-rien)
- TriBand : [7, 7, 7, 3, 1] → **21 pts** (majorite)
- TIE : [7, 7, 7, 7, 7] → **35 pts** (7×5)
- TriBand : [7, 7, 7, 7, 7] → **70 pts** (7×5×2, bonus complete)

#### 2.3 Score theorique

Take It Easy max = 9×5 + 9×4×2 + 9×3×2 + 7×5 + 7×4×2 + 7×3×2 + 8×5 + 8×4×2 + 8×3×2 ... (complexe, ~307 pts theorique, ~153 pts moyen IA)

TriBand max (tout complete, valeurs max) :
- 5 lignes direction 0 : 9×5×2 + 9×4×2×2 + 9×3×2×2 = 90+144+108 = 342
- 5 lignes direction 1 : 8×5×2 + 8×4×2×2 + 8×3×2×2 = 80+128+96 = 304
- 5 lignes direction 2 : 9×5×2 + 9×4×2×2 + 9×3×2×2 = 90+144+108 = 342

Max theorique = **988 pts** (impossible en pratique car les 3 directions sont couplees).
Score IA attendu : ~250-350 pts (a determiner par benchmark).

---

## 3. Impact sur le code

### 3.1 Fichiers a modifier

| Fichier | Modification | Effort |
|---------|-------------|--------|
| `src/game/create_deck.rs` | Pool de 60 tuiles, tirage de 19 | Faible |
| `src/game/deck.rs` | Ajouter `fn draw_subset(n: usize) -> Deck` | Faible |
| `src/scoring/scoring.rs` | Scoring graduel (3 paliers) | Moyen |
| `src/neural/tensor_conversion.rs` | Adapter channels 8-16 (bag counts pour 5+4+3 valeurs) | Moyen |
| `frontend-elm/src/Main.elm` | Renommer, adapter affichage tuiles | Faible |
| `frontend-elm/public/index.html` | Titre, meta, branding | Faible |
| Tests | Adapter tests scoring + deck | Moyen |

### 3.2 Fichiers qui ne changent PAS

- `src/models/graph_transformer.rs` — architecture identique
- `src/strategy/expectimax.rs` — fonctionne tel quel (value net evalue n'importe quel etat)
- `src/bin/train_value_net.rs` — pipeline identique
- `src/bin/distill_expectimax.rs` — pipeline identique
- `src/services/` — game_manager, session_manager inchanges
- `src/auth/` — tout le systeme auth inchange
- `src/servers/` — gRPC + HTTP inchanges
- Proto files — messages identiques (tile = 3 ints, plateau = 19 positions)
- Infra (deploy.sh, systemd, Cloudflare) — rien ne change

### 3.3 Details d'implementation

#### create_deck.rs

```rust
// Ancien (Take It Easy) : 3×3×3 = 27 tuiles fixes
// Nouveau (TriBand) : 5×4×3 = 60 tuiles dans le pool

pub fn create_full_pool() -> Vec<Tile> {
    let dir0 = [1, 3, 5, 7, 9];
    let dir1 = [2, 4, 6, 8];
    let dir2 = [1, 5, 9];
    let mut tiles = Vec::with_capacity(60);
    for &v0 in &dir0 {
        for &v1 in &dir1 {
            for &v2 in &dir2 {
                tiles.push(Tile(v0, v1, v2));
            }
        }
    }
    tiles
}

pub fn create_deck() -> Deck {
    let mut pool = create_full_pool();
    pool.shuffle(&mut thread_rng());
    Deck { tiles: pool[..19].to_vec() }
}
```

#### scoring.rs

```rust
pub fn result(plateau: &Plateau) -> i32 {
    let patterns: Vec<ScoringPattern> = vec![
        // Direction 0 (horizontale) - 5 lignes
        (&[0, 1, 2][..],       Box::new(|t: &Tile| t.0)),
        (&[3, 4, 5, 6][..],    Box::new(|t: &Tile| t.0)),
        (&[7, 8, 9, 10, 11][..], Box::new(|t: &Tile| t.0)),
        (&[12, 13, 14, 15][..], Box::new(|t: &Tile| t.0)),
        (&[16, 17, 18][..],    Box::new(|t: &Tile| t.0)),
        // Direction 1 (diag-gauche) - 5 lignes
        (&[0, 3, 7][..],       Box::new(|t: &Tile| t.1)),
        (&[1, 4, 8, 12][..],   Box::new(|t: &Tile| t.1)),
        (&[2, 5, 9, 13, 16][..], Box::new(|t: &Tile| t.1)),
        (&[6, 10, 14, 17][..], Box::new(|t: &Tile| t.1)),
        (&[11, 15, 18][..],    Box::new(|t: &Tile| t.1)),
        // Direction 2 (diag-droite) - 5 lignes
        (&[7, 12, 16][..],     Box::new(|t: &Tile| t.2)),
        (&[3, 8, 13, 17][..],  Box::new(|t: &Tile| t.2)),
        (&[0, 4, 9, 14, 18][..], Box::new(|t: &Tile| t.2)),
        (&[1, 5, 10, 15][..],  Box::new(|t: &Tile| t.2)),
        (&[2, 6, 11][..],      Box::new(|t: &Tile| t.2)),
    ];

    let mut total = 0;

    for (indices, selector) in &patterns {
        let values: Vec<i32> = indices.iter()
            .map(|&i| selector(&plateau.tiles[i]))
            .filter(|&v| v != 0) // ignorer cases vides
            .collect();

        if values.is_empty() { continue; }

        // Trouver la valeur majoritaire
        let mut counts = std::collections::HashMap::new();
        for &v in &values {
            *counts.entry(v).or_insert(0usize) += 1;
        }
        let (&majority_val, &majority_count) = counts.iter()
            .max_by_key(|(_, c)| *c)
            .unwrap();

        let line_len = indices.len();

        if majority_count == line_len {
            // Ligne complete : bonus x2
            total += majority_val * line_len as i32 * 2;
        } else if majority_count >= (line_len + 1) / 2 {
            // Majorite stricte : score partiel
            total += majority_val * majority_count as i32;
        }
        // Sinon : 0
    }

    total
}
```

#### tensor_conversion.rs (channels 8-16)

```rust
// Ancien : 9 channels (3 valeurs × 3 directions)
// Nouveau : 12 channels (5 + 4 + 3 valeurs)

// Direction 0 : compter les tuiles restantes avec v0 ∈ {1,3,5,7,9}
// Ch 8:  count(v0==1) / 12
// Ch 9:  count(v0==3) / 12
// Ch 10: count(v0==5) / 12
// Ch 11: count(v0==7) / 12
// Ch 12: count(v0==9) / 12

// Direction 1 : compter les tuiles restantes avec v1 ∈ {2,4,6,8}
// Ch 13: count(v1==2) / 15
// Ch 14: count(v1==4) / 15
// Ch 15: count(v1==6) / 15
// Ch 16: count(v1==8) / 15

// Direction 2 : compter les tuiles restantes avec v2 ∈ {1,5,9}
// Ch 17: count(v2==1) / 20
// Ch 18: count(v2==5) / 20
// Ch 19: count(v2==9) / 20

// Total : 50 channels (vs 47)
// → Adapter la projection lineaire du GT : input_dim = 50
```

---

## 4. Impact sur l'IA

### 4.1 Pourquoi l'IA devrait etre meilleure

Le scoring graduel change fondamentalement la dynamique :

1. **Moins de variance** : dans TIE, une seule tuile mal placee = 0 pts sur la ligne. Dans TriBand, les lignes "presque completees" rapportent quand meme. Le value net aura un MAE plus bas (actuellement ~17 pts).

2. **Value net plus precise** → **Expectimax plus efficace** : si V(state) est plus precis, la recherche 1-ply et 2-ply prend de meilleures decisions.

3. **Moins de parties catastrophiques** : le score minimum sera plus eleve (actuellement 66 pts au pire en 2-ply, attendu ~100+ avec scoring graduel).

4. **Plus de diversite d'entrainement** : 60 tuiles dans le pool (vs 27 fixes), chaque partie a un sous-ensemble different → le GT voit plus de situations variees, risque d'overfitting reduit.

### 4.2 Risques

1. **Plus de valeurs = espace plus grand** : 5 valeurs en direction 0 (vs 3) → plus de decisions a considerer. Peut necessiter plus de donnees d'entrainement (100k → 200k ?).

2. **Scoring graduel plus lisse** : le GT pourrait atteindre un bon score plus facilement → le gain marginal de l'expectimax pourrait etre plus faible.

3. **Features a re-calibrer** : les 30 line features (potentiel + compatibilite) doivent etre adaptees au scoring graduel (notion de "majorite" vs "tout-ou-rien").

### 4.3 Pipeline de re-entrainement

```
1. Modifier create_deck.rs + scoring.rs + tensor_conversion.rs
2. Generer des donnees MCTS (50k parties) pour le supervised initial
3. Entrainer le GT policy sur les donnees MCTS
4. Benchmark GT Direct (score attendu ~200-250 pts ?)
5. Generer 100k parties GT self-play pour le value net
6. Entrainer le value net (MSE, 120 epochs)
7. Benchmark 1-ply expectimax (gain attendu +5-10 pts)
8. Benchmark 2-ply expectimax
9. Distillation de politique
10. Deployer
```

Temps estime : 2-3 jours sur RTX 3090 (Vast.ai).

---

## 5. Impact sur le frontend

### 5.1 Branding

- Titre : "TriBand"
- Sous-titre : "Un jeu de strategie hexagonal avec IA"
- URL : `triband.mooo.com` ou garder `takeitasy.mooo.com` avec redirect
- Palette : a definir (suggestion : tons bleu/violet pour differencier de TIE orange/jaune)

### 5.2 Affichage des tuiles

Les tuiles SVG actuelles affichent 3 bandes colorees avec des nombres. Le changement est :
- Plus de valeurs possibles (1-9 au lieu de sous-ensembles restreints)
- Meme rendu SVG, juste des nombres differents
- Optionnel : coder les valeurs par couleur pour la lisibilite (1=bleu fonce ... 9=rouge)

### 5.3 Affichage du score

Nouveau : afficher le score par ligne avec le palier atteint :
- Ligne complete (x2) : surligner en or
- Majorite : surligner en argent
- Aucun : grise

---

## 6. Differences juridiques vs Take It Easy

| Critere | Take It Easy | TriBand |
|---------|-------------|---------|
| Nom | Take It Easy (TM) | TriBand (original) |
| Pool de tuiles | 27 fixes ({1,5,9}×{2,6,7}×{3,4,8}) | 60, on tire 19 ({1,3,5,7,9}×{2,4,6,8}×{1,5,9}) |
| Tirage | Deck fixe de 27, on joue les 27 | Pool de 60, on tire 19 aleatoirement |
| Scoring | Tout-ou-rien (0 si incomplet) | Graduel (complete x2 / majorite / 0) |
| Nombre de parties identiques possibles | 27! ordres de tirage | C(60,19) × 19! ≈ 10^30 parties uniques |

Ces 4 differences (nom, pool, tirage, scoring) rendent TriBand un jeu fondamentalement different, tout en conservant l'essence strategique (placement hexagonal, lignes, 3 directions).

---

## 7. Alternatives envisagees

### Option A : "HexForge" (minimal)
- Memes 27 tuiles, juste redistribuer les valeurs + ajouter un bonus central
- Trop proche de TIE, risque juridique residuel

### Option C : "ChromaHex" (ambitieux)
- Scoring par gamme (ordre croissant/decroissant) en plus de l'unisson
- Feature engineering complexe (encoder la proximite des valeurs)
- Risque : le GT pourrait avoir du mal avec la notion d'ordre

### Pourquoi B (TriBand) est le meilleur compromis
1. Pool variable = jeu fondamentalement different (chaque partie a des tuiles differentes)
2. Scoring graduel = meilleur game design (moins frustrant, plus strategique)
3. Impact code modere (3-4 fichiers a modifier, architecture inchangee)
4. Meilleure IA attendue (moins de variance, value net plus precise)
5. Narratif fort pour le post : "j'ai concu un jeu + son IA"
