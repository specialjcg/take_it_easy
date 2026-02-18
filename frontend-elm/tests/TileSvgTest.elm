module TileSvgTest exposing (..)

import Expect
import Test exposing (..)
import TileSvg exposing (parseTileFromPath)


suite : Test
suite =
    describe "parseTileFromPath"
        [ test "parses image/168.png" <|
            \_ ->
                parseTileFromPath "image/168.png"
                    |> Expect.equal (Just { v1 = 1, v2 = 6, v3 = 8 })
        , test "parses image/924.png" <|
            \_ ->
                parseTileFromPath "image/924.png"
                    |> Expect.equal (Just { v1 = 9, v2 = 2, v3 = 4 })
        , test "parses ../image/573.png (legacy prefix)" <|
            \_ ->
                parseTileFromPath "../image/573.png"
                    |> Expect.equal (Just { v1 = 5, v2 = 7, v3 = 3 })
        , test "returns Nothing for empty string" <|
            \_ ->
                parseTileFromPath ""
                    |> Expect.equal Nothing
        , test "returns Nothing for invalid path (two digits)" <|
            \_ ->
                parseTileFromPath "image/12.png"
                    |> Expect.equal Nothing
        ]
