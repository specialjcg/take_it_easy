module TileSvg exposing (viewTileSvg, viewEmptyHexSvg, parseTileFromPath)

{-| Module pour générer les tuiles Take It Easy en SVG
    Chaque tuile a 3 valeurs avec des bandes colorées qui se croisent
-}

import Html exposing (Html)
import Svg exposing (Svg, clipPath, defs, g, polygon, rect, svg, text, text_)
import Svg.Attributes as SA exposing (..)


{-| Représentation d'une tuile avec ses 3 valeurs
-}
type alias Tile =
    { v1 : Int -- Valeur verticale (1, 5, 9)
    , v2 : Int -- Valeur diagonale gauche (2, 6, 7)
    , v3 : Int -- Valeur diagonale droite (3, 4, 8)
    }


{-| Parse un chemin d'image comme "image/168.png" en Tile
-}
parseTileFromPath : String -> Maybe Tile
parseTileFromPath imagePath =
    let
        -- Extraire le nom de fichier (ex: "168" de "image/168.png")
        filename =
            imagePath
                |> String.replace "../" ""
                |> String.replace "image/" ""
                |> String.replace ".png" ""
    in
    case String.toList filename of
        [ c1, c2, c3 ] ->
            Maybe.map3 Tile
                (String.toInt (String.fromChar c1))
                (String.toInt (String.fromChar c2))
                (String.toInt (String.fromChar c3))

        _ ->
            Nothing


{-| Couleurs pour chaque valeur (correspondant aux tuiles originales PNG)
-}
colorForValue : Int -> String
colorForValue value =
    case value of
        1 ->
            "#a0a0a0" -- Gris

        2 ->
            "#ffb6c1" -- Rose pâle

        3 ->
            "#ff69b4" -- Rose pink

        4 ->
            "#00a0ff" -- Bleu

        5 ->
            "#00b4a0" -- Bleu vert / Teal

        6 ->
            "#ff3030" -- Rouge

        7 ->
            "#a0d800" -- Vert/Lime

        8 ->
            "#ff8c00" -- Orange

        9 ->
            "#f0d000" -- Jaune

        _ ->
            "#666666"


{-| Génère le SVG d'une tuile
-}
viewTileSvg : Tile -> Html msg
viewTileSvg tile =
    let
        -- Dimensions de l'hexagone (flat-top)
        width =
            100

        height =
            86.6

        -- Points de l'hexagone flat-top
        hexPoints =
            "25,0 75,0 100,43.3 75,86.6 25,86.6 0,43.3"

        -- Largeur des bandes
        bandWidth =
            14
    in
    svg
        [ viewBox "0 0 100 86.6"
        , SA.width "100%"
        , SA.height "100%"
        ]
        [ -- Définir le clip-path hexagonal
          defs []
            [ Svg.clipPath [ id "hexClip" ]
                [ polygon [ points hexPoints ] [] ]
            ]

        -- Fond de l'hexagone
        , polygon
            [ points hexPoints
            , fill "#1a1a2e"
            , stroke "#333"
            , strokeWidth "1"
            ]
            []

        -- Groupe avec clip-path pour les bandes
        , g [ SA.clipPath "url(#hexClip)" ]
            [ -- Bande diagonale gauche (v2) - du haut-gauche vers bas-droit
              viewDiagonalBandRight tile.v2 bandWidth

            -- Bande diagonale droite (v3) - du haut-droit vers bas-gauche
            , viewDiagonalBandLeft tile.v3 bandWidth

            -- Bande verticale (v1) - du haut vers le bas
            , viewVerticalBand tile.v1 bandWidth
            ]

        -- Numéros
        , viewNumber tile.v1 50 18 (colorForValue tile.v1)
        , viewNumber tile.v2 22 62 (colorForValue tile.v2)
        , viewNumber tile.v3 78 62 (colorForValue tile.v3)
        ]


{-| Bande verticale (valeurs 1, 5, 9)
-}
viewVerticalBand : Int -> Float -> Svg msg
viewVerticalBand value bandWidth =
    rect
        [ x (String.fromFloat (50 - bandWidth / 2))
        , y "-5"
        , SA.width (String.fromFloat bandWidth)
        , SA.height "100"
        , fill (colorForValue value)
        ]
        []


{-| Bande diagonale gauche (valeurs 2, 6, 7) - va vers bas-gauche
-}
viewDiagonalBandLeft : Int -> Float -> Svg msg
viewDiagonalBandLeft value bandWidth =
    g [ transform "rotate(-60, 50, 43.3)" ]
        [ rect
            [ x (String.fromFloat (50 - bandWidth / 2))
            , y "-20"
            , SA.width (String.fromFloat bandWidth)
            , SA.height "130"
            , fill (colorForValue value)
            ]
            []
        ]


{-| Bande diagonale droite (valeurs 3, 4, 8) - va vers bas-droit
-}
viewDiagonalBandRight : Int -> Float -> Svg msg
viewDiagonalBandRight value bandWidth =
    g [ transform "rotate(60, 50, 43.3)" ]
        [ rect
            [ x (String.fromFloat (50 - bandWidth / 2))
            , y "-20"
            , SA.width (String.fromFloat bandWidth)
            , SA.height "130"
            , fill (colorForValue value)
            ]
            []
        ]


{-| Affiche un numéro avec contour
-}
viewNumber : Int -> Float -> Float -> String -> Svg msg
viewNumber value xPos yPos bgColor =
    g []
        [ -- Contour noir
          text_
            [ x (String.fromFloat xPos)
            , y (String.fromFloat yPos)
            , textAnchor "middle"
            , dominantBaseline "middle"
            , fontSize "16"
            , fontWeight "bold"
            , fontFamily "Arial, sans-serif"
            , stroke "#000"
            , strokeWidth "3"
            , fill "#000"
            ]
            [ text (String.fromInt value) ]

        -- Texte blanc
        , text_
            [ x (String.fromFloat xPos)
            , y (String.fromFloat yPos)
            , textAnchor "middle"
            , dominantBaseline "middle"
            , fontSize "16"
            , fontWeight "bold"
            , fontFamily "Arial, sans-serif"
            , fill "#fff"
            ]
            [ text (String.fromInt value) ]
        ]


{-| SVG d'un hexagone vide (case disponible)
-}
viewEmptyHexSvg : Bool -> Int -> Html msg
viewEmptyHexSvg isAvailable index =
    let
        hexPoints =
            "25,0 75,0 100,43.3 75,86.6 25,86.6 0,43.3"

        fillColor =
            if isAvailable then
                "rgba(78, 205, 196, 0.3)"

            else
                "#1a1a2e"

        strokeColor =
            if isAvailable then
                "#4ecdc4"

            else
                "#444"
    in
    svg
        [ viewBox "0 0 100 86.6"
        , SA.width "100%"
        , SA.height "100%"
        ]
        [ polygon
            [ points hexPoints
            , fill fillColor
            , stroke strokeColor
            , strokeWidth "2"
            ]
            []
        , text_
            [ x "50"
            , y "43.3"
            , textAnchor "middle"
            , dominantBaseline "middle"
            , fontSize "14"
            , fill "rgba(255, 255, 255, 0.5)"
            ]
            [ text (String.fromInt index) ]
        ]
