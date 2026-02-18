module GameLogicTest exposing (..)

import Expect
import GameLogic
    exposing
        ( CmdIntent(..)
        , GameModel
        , canClickPosition
        , handleAiMoveResultPure
        , handleGameFinishedPure
        , handleMovePlayedPure
        , handlePlaceRealTilePure
        , handlePollTurnPure
        , handleSelectRealTilePure
        , handleTurnStartedPure
        , isDeadState
        )
import Test exposing (..)


{-| Default GameModel for tests — a mid-game solo state.
-}
defaultModel : GameModel
defaultModel =
    { sessionId = Just "session-123"
    , playerId = Just "player-1"
    , currentTile = Nothing
    , currentTileImage = Nothing
    , plateauTiles = List.repeat 19 ""
    , aiPlateauTiles = List.repeat 19 ""
    , availablePositions = List.range 0 18
    , myTurn = False
    , currentTurnNumber = 0
    , waitingForPlayers = []
    , isRealGameMode = False
    , showTilePicker = False
    , usedTiles = []
    , pendingAiPosition = Nothing
    , isSoloMode = True
    , loading = False
    , statusMessage = ""
    , hasSession = True
    , hasGameState = True
    , gameStateIsFinished = False
    }


suite : Test
suite =
    describe "GameLogic — Dead States"
        [ ds1_movePlayedWithoutTurnStarted
        , ds2_myTurnNoTile
        , ds3_noAvailablePositions
        , ds4_sessionNoGameState
        , ds5_gameFinishedNoPlateaus
        , ds6_tilePickerBlocked
        , ds7_aiInvalidPosition
        , ds8_initialTurnLost
        ]



-- ============================================================================
-- DS1: MovePlayed sans reponse TurnStarted (le bug trouvé)
-- ============================================================================


ds1_movePlayedWithoutTurnStarted : Test
ds1_movePlayedWithoutTurnStarted =
    describe "DS1: MovePlayed without TurnStarted response"
        [ test "after MovePlayed (not game over), myTurn=False and currentTile=Nothing" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = True
                            , currentTile = Just "168"
                            , currentTileImage = Just "image/168.png"
                        }

                    result =
                        handleMovePlayedPure model
                            { position = 5
                            , points = 10
                            , aiTiles = []
                            , aiScore = 0
                            , isGameOver = False
                            }
                in
                Expect.all
                    [ \r -> Expect.equal False r.myTurn
                    , \r -> Expect.equal Nothing r.currentTile
                    ]
                    result
        , test "after MovePlayed (not game over), cmdIntent contains SendStartTurn + SchedulePollTurn" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = True
                            , currentTile = Just "168"
                            , currentTileImage = Just "image/168.png"
                        }

                    result =
                        handleMovePlayedPure model
                            { position = 5
                            , points = 10
                            , aiTiles = []
                            , aiScore = 0
                            , isGameOver = False
                            }
                in
                case result.cmdIntent of
                    BatchCmds cmds ->
                        Expect.all
                            [ \c -> Expect.equal True (List.any isSendStartTurn c)
                            , \c -> Expect.equal True (List.any isSchedulePollTurn c)
                            ]
                            cmds

                    _ ->
                        Expect.fail "expected BatchCmds"
        , test "after MovePlayed (game over), cmdIntent is NoCmd" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = True
                            , currentTile = Just "168"
                            , currentTileImage = Just "image/168.png"
                        }

                    result =
                        handleMovePlayedPure model
                            { position = 5
                            , points = 10
                            , aiTiles = []
                            , aiScore = 0
                            , isGameOver = True
                            }
                in
                Expect.equal NoCmd result.cmdIntent
        , test "PollTurn when myTurn=False sends StartTurn" <|
            \_ ->
                let
                    model =
                        { defaultModel | myTurn = False }

                    cmd =
                        handlePollTurnPure model
                in
                Expect.equal True (isSendStartTurn cmd)
        , test "PollTurn when myTurn=True is NoCmd" <|
            \_ ->
                let
                    model =
                        { defaultModel | myTurn = True }

                    cmd =
                        handlePollTurnPure model
                in
                Expect.equal NoCmd cmd
        ]



-- ============================================================================
-- DS2: myTurn=True + currentTile=Nothing
-- ============================================================================


ds2_myTurnNoTile : Test
ds2_myTurnNoTile =
    describe "DS2: myTurn=True but currentTile=Nothing"
        [ test "TurnStarted with playerId in waiting sets currentTile" <|
            \_ ->
                let
                    model =
                        { defaultModel | playerId = Just "player-1" }

                    result =
                        handleTurnStartedPure model
                            { tile = "573"
                            , tileImage = "image/573.png"
                            , turnNumber = 3
                            , positions = [ 0, 1, 2 ]
                            , waiting = [ "player-1" ]
                            }
                in
                Expect.equal (Just "573") result.currentTile
        , test "TurnStarted with playerId NOT in waiting leaves currentTile=Nothing" <|
            \_ ->
                let
                    model =
                        { defaultModel | playerId = Just "player-1" }

                    result =
                        handleTurnStartedPure model
                            { tile = "573"
                            , tileImage = "image/573.png"
                            , turnNumber = 3
                            , positions = [ 0, 1, 2 ]
                            , waiting = [ "player-2" ]
                            }
                in
                Expect.equal Nothing result.currentTile
        , test "canClickPosition with myTurn=True, currentTile=Nothing is False" <|
            \_ ->
                let
                    model =
                        { defaultModel | myTurn = True, currentTile = Nothing }
                in
                Expect.equal False (canClickPosition model)
        , test "canClickPosition with myTurn=True, currentTile=Just is True" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = True
                            , currentTile = Just "168"
                        }
                in
                Expect.equal True (canClickPosition model)
        ]



-- ============================================================================
-- DS3: availablePositions=[] + currentTile=Just
-- ============================================================================


ds3_noAvailablePositions : Test
ds3_noAvailablePositions =
    describe "DS3: no available positions"
        [ test "canClickPosition with empty availablePositions is False" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = True
                            , currentTile = Just "168"
                            , availablePositions = []
                        }
                in
                Expect.equal False (canClickPosition model)
        , test "MovePlayed on last position leaves empty availablePositions" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = True
                            , currentTile = Just "168"
                            , currentTileImage = Just "image/168.png"
                            , availablePositions = [ 7 ]
                        }

                    result =
                        handleMovePlayedPure model
                            { position = 7
                            , points = 20
                            , aiTiles = []
                            , aiScore = 0
                            , isGameOver = True
                            }
                in
                Expect.equal [] result.availablePositions
        ]



-- ============================================================================
-- DS4: Session sans GameState
-- ============================================================================


ds4_sessionNoGameState : Test
ds4_sessionNoGameState =
    describe "DS4: session exists but no game state"
        [ test "isDeadState with session but no gameState is True" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | hasSession = True
                            , hasGameState = False
                            , gameStateIsFinished = False
                        }
                in
                Expect.equal True (isDeadState model)
        , test "isDeadState with session and gameState is not dead (from DS4)" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | hasSession = True
                            , hasGameState = True
                            , gameStateIsFinished = False
                        }
                in
                -- Not dead from DS4 (could be dead from DS2 if myTurn=True + no tile)
                Expect.equal False (isDeadState model)
        ]



-- ============================================================================
-- DS5: GameFinished sans plateaux
-- ============================================================================


ds5_gameFinishedNoPlateaus : Test
ds5_gameFinishedNoPlateaus =
    describe "DS5: GameFinished with empty plateaus"
        [ test "handleGameFinishedPure with empty allPlateaus returns empty list" <|
            \_ ->
                let
                    result =
                        handleGameFinishedPure
                            { players = [ { id = "p1", name = "Alice", score = 100 } ]
                            , playerTiles = List.repeat 19 "image/168.png"
                            , aiTiles = List.repeat 19 "image/924.png"
                            , allPlateaus = []
                            }
                in
                Expect.equal [] result.allPlayerPlateaus
        , test "handleGameFinishedPure sets gameStateIsFinished" <|
            \_ ->
                let
                    result =
                        handleGameFinishedPure
                            { players = []
                            , playerTiles = []
                            , aiTiles = []
                            , allPlateaus = []
                            }
                in
                Expect.equal True result.gameStateIsFinished
        , test "handleGameFinishedPure resolves player names in plateaus" <|
            \_ ->
                let
                    result =
                        handleGameFinishedPure
                            { players =
                                [ { id = "p1", name = "Alice", score = 100 } ]
                            , playerTiles = List.repeat 19 ""
                            , aiTiles = List.repeat 19 ""
                            , allPlateaus =
                                [ ( "p1", List.repeat 19 "image/168.png" )
                                , ( "mcts_ai", List.repeat 19 "image/924.png" )
                                ]
                            }
                in
                case result.allPlayerPlateaus of
                    ( _, name1, _ ) :: ( _, name2, _ ) :: [] ->
                        Expect.all
                            [ \_ -> Expect.equal "Alice" name1
                            , \_ -> Expect.equal "IA" name2
                            ]
                            ()

                    _ ->
                        Expect.fail "expected exactly 2 plateaus"
        ]



-- ============================================================================
-- DS6: Tile picker bloque (real game)
-- ============================================================================


ds6_tilePickerBlocked : Test
ds6_tilePickerBlocked =
    describe "DS6: tile picker blocks clicks in real game mode"
        [ test "SelectRealTile closes tile picker" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | isRealGameMode = True
                            , showTilePicker = True
                            , myTurn = True
                        }

                    result =
                        handleSelectRealTilePure model "168"
                in
                Expect.equal False result.showTilePicker
        , test "canClickPosition with showTilePicker=True in real game is False" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | isRealGameMode = True
                            , showTilePicker = True
                            , myTurn = True
                            , currentTile = Just "168"
                        }
                in
                Expect.equal False (canClickPosition model)
        , test "canClickPosition with showTilePicker=False in real game is True" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | isRealGameMode = True
                            , showTilePicker = False
                            , myTurn = True
                            , currentTile = Just "168"
                        }
                in
                Expect.equal True (canClickPosition model)
        ]



-- ============================================================================
-- DS7: AI position invalide (real game)
-- ============================================================================


ds7_aiInvalidPosition : Test
ds7_aiInvalidPosition =
    describe "DS7: AI returns invalid position"
        [ test "position=-1 results in pendingAiPosition=Nothing" <|
            \_ ->
                let
                    result =
                        handleAiMoveResultPure -1 ""
                in
                Expect.equal Nothing result.pendingAiPosition
        , test "position=19 results in pendingAiPosition=Nothing" <|
            \_ ->
                let
                    result =
                        handleAiMoveResultPure 19 ""
                in
                Expect.equal Nothing result.pendingAiPosition
        , test "position=7 results in pendingAiPosition=Just 7" <|
            \_ ->
                let
                    result =
                        handleAiMoveResultPure 7 ""
                in
                Expect.equal (Just 7) result.pendingAiPosition
        , test "position=0 (boundary) results in pendingAiPosition=Just 0" <|
            \_ ->
                let
                    result =
                        handleAiMoveResultPure 0 ""
                in
                Expect.equal (Just 0) result.pendingAiPosition
        , test "position=18 (boundary) results in pendingAiPosition=Just 18" <|
            \_ ->
                let
                    result =
                        handleAiMoveResultPure 18 ""
                in
                Expect.equal (Just 18) result.pendingAiPosition
        ]



-- ============================================================================
-- DS8: Initial startTurn lost (ReadySet → TurnStarted never arrives)
-- ============================================================================


ds8_initialTurnLost : Test
ds8_initialTurnLost =
    describe "DS8: Initial startTurn response lost after game start"
        [ test "PollTurn recovers when myTurn=False at turn 0 (initial state)" <|
            \_ ->
                let
                    -- State after ReadySet: game started but TurnStarted never arrived
                    model =
                        { defaultModel
                            | myTurn = False
                            , currentTile = Nothing
                            , currentTurnNumber = 0
                            , loading = True
                        }

                    cmd =
                        handlePollTurnPure model
                in
                Expect.equal True (isSendStartTurn cmd)
        , test "isDeadState detects myTurn=False + no tile + game not finished (waiting for TurnStarted)" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = False
                            , currentTile = Nothing
                            , currentTurnNumber = 0
                            , hasSession = True
                            , hasGameState = True
                            , gameStateIsFinished = False
                        }
                in
                -- Not a dead state per se (PollTurn will recover), but canClickPosition is False
                Expect.equal False (canClickPosition model)
        , test "TurnStarted resolves the initial stuck state" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = False
                            , currentTile = Nothing
                            , currentTurnNumber = 0
                            , loading = True
                            , playerId = Just "player-1"
                        }

                    result =
                        handleTurnStartedPure model
                            { tile = "923"
                            , tileImage = "image/923.png"
                            , turnNumber = 0
                            , positions = List.range 0 18
                            , waiting = [ "player-1" ]
                            }
                in
                Expect.all
                    [ \r -> Expect.equal True r.myTurn
                    , \r -> Expect.equal (Just "923") r.currentTile
                    , \r -> Expect.equal False r.loading
                    , \r -> Expect.equal 19 (List.length r.availablePositions)
                    ]
                    result
        , test "PollTurn is NoCmd once TurnStarted has been processed (myTurn=True)" <|
            \_ ->
                let
                    model =
                        { defaultModel
                            | myTurn = True
                            , currentTile = Just "923"
                        }
                in
                Expect.equal NoCmd (handlePollTurnPure model)
        ]



-- ============================================================================
-- Helpers
-- ============================================================================


isSendStartTurn : CmdIntent -> Bool
isSendStartTurn cmd =
    case cmd of
        SendStartTurn _ ->
            True

        _ ->
            False


isSchedulePollTurn : CmdIntent -> Bool
isSchedulePollTurn cmd =
    case cmd of
        SchedulePollTurn _ ->
            True

        _ ->
            False
