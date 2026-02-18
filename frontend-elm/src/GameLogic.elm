module GameLogic exposing
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

{-| Pure game logic extracted from Main.elm for testability.
    No ports, no Cmd Msg — only pure functions returning CmdIntent.
-}


{-| Intent representing a side effect, resolved to Cmd Msg in Main.elm.
-}
type CmdIntent
    = NoCmd
    | SendStartTurn String
    | SchedulePollTurn Float
    | SendGetAiMove String (List String) (List Int) Int
    | BatchCmds (List CmdIntent)


{-| Subset of Model fields relevant to game logic.
-}
type alias GameModel =
    { sessionId : Maybe String
    , playerId : Maybe String
    , currentTile : Maybe String
    , currentTileImage : Maybe String
    , plateauTiles : List String
    , aiPlateauTiles : List String
    , availablePositions : List Int
    , myTurn : Bool
    , currentTurnNumber : Int
    , waitingForPlayers : List String
    , isRealGameMode : Bool
    , showTilePicker : Bool
    , usedTiles : List String
    , pendingAiPosition : Maybe Int
    , isSoloMode : Bool
    , loading : Bool
    , statusMessage : String
    , hasSession : Bool
    , hasGameState : Bool
    , gameStateIsFinished : Bool
    }



-- ============================================================================
-- TRANSITION: TurnStarted
-- ============================================================================


type alias TurnStartedInput =
    { tile : String
    , tileImage : String
    , turnNumber : Int
    , positions : List Int
    , waiting : List String
    }


type alias TurnStartedOutput =
    { currentTile : Maybe String
    , currentTileImage : Maybe String
    , currentTurnNumber : Int
    , availablePositions : List Int
    , myTurn : Bool
    , loading : Bool
    , waitingForPlayers : List String
    , cmdIntent : CmdIntent
    }


handleTurnStartedPure : GameModel -> TurnStartedInput -> TurnStartedOutput
handleTurnStartedPure model input =
    let
        playerId =
            model.playerId |> Maybe.withDefault ""

        isMyTurn =
            List.member playerId input.waiting

        pollCmd =
            if not isMyTurn then
                SchedulePollTurn 2000

            else
                NoCmd
    in
    { currentTile =
        if isMyTurn then
            Just input.tile

        else
            Nothing
    , currentTileImage =
        if isMyTurn then
            Just input.tileImage

        else
            Nothing
    , currentTurnNumber = input.turnNumber
    , availablePositions = input.positions
    , myTurn = isMyTurn
    , loading = False
    , waitingForPlayers = input.waiting
    , cmdIntent = pollCmd
    }



-- ============================================================================
-- TRANSITION: MovePlayed
-- ============================================================================


type alias MovePlayedInput =
    { position : Int
    , points : Int
    , aiTiles : List String
    , aiScore : Int
    , isGameOver : Bool
    }


type alias MovePlayedOutput =
    { myTurn : Bool
    , loading : Bool
    , statusMessage : String
    , plateauTiles : List String
    , aiPlateauTiles : List String
    , availablePositions : List Int
    , currentTile : Maybe String
    , currentTileImage : Maybe String
    , cmdIntent : CmdIntent
    }


handleMovePlayedPure : GameModel -> MovePlayedInput -> MovePlayedOutput
handleMovePlayedPure model input =
    let
        newPlateauTiles =
            List.indexedMap
                (\i tile ->
                    if i == input.position then
                        model.currentTileImage
                            |> Maybe.map (String.replace "../" "")
                            |> Maybe.withDefault tile

                    else
                        tile
                )
                model.plateauTiles

        newAvailablePositions =
            List.filter (\p -> p /= input.position) model.availablePositions

        newAiPlateauTiles =
            if List.isEmpty input.aiTiles then
                model.aiPlateauTiles

            else
                input.aiTiles

        cmdIntent =
            if input.isGameOver then
                NoCmd

            else
                case model.sessionId of
                    Just sid ->
                        BatchCmds
                            [ SendStartTurn sid
                            , SchedulePollTurn 3000
                            ]

                    Nothing ->
                        NoCmd
    in
    { myTurn = False
    , loading = False
    , statusMessage = "+" ++ String.fromInt input.points ++ " points"
    , plateauTiles = newPlateauTiles
    , aiPlateauTiles = newAiPlateauTiles
    , availablePositions = newAvailablePositions
    , currentTile = Nothing
    , currentTileImage = Nothing
    , cmdIntent = cmdIntent
    }



-- ============================================================================
-- TRANSITION: PollTurn
-- ============================================================================


handlePollTurnPure : GameModel -> CmdIntent
handlePollTurnPure model =
    case model.sessionId of
        Just sid ->
            if not model.myTurn then
                SendStartTurn sid

            else
                NoCmd

        Nothing ->
            NoCmd



-- ============================================================================
-- TRANSITION: GameFinished
-- ============================================================================


type alias GameFinishedInput =
    { players : List { id : String, name : String, score : Int }
    , playerTiles : List String
    , aiTiles : List String
    , allPlateaus : List ( String, List String )
    }


type alias GameFinishedOutput =
    { statusMessage : String
    , plateauTiles : List String
    , aiPlateauTiles : List String
    , allPlayerPlateaus : List ( String, String, List String )
    , myTurn : Bool
    , waitingForPlayers : List String
    , gameStateIsFinished : Bool
    }


handleGameFinishedPure : GameFinishedInput -> GameFinishedOutput
handleGameFinishedPure input =
    let
        resolvedPlateaus =
            List.map
                (\( id, tiles ) ->
                    let
                        name =
                            List.filter (\p -> p.id == id) input.players
                                |> List.head
                                |> Maybe.map .name
                                |> Maybe.withDefault
                                    (if id == "mcts_ai" then
                                        "IA"

                                     else
                                        "Joueur"
                                    )
                    in
                    ( id, name, tiles )
                )
                input.allPlateaus
    in
    { statusMessage = "Partie terminée!"
    , plateauTiles = input.playerTiles
    , aiPlateauTiles = input.aiTiles
    , allPlayerPlateaus = resolvedPlateaus
    , myTurn = False
    , waitingForPlayers = []
    , gameStateIsFinished = True
    }



-- ============================================================================
-- TRANSITION: PlaceRealTile
-- ============================================================================


type alias PlaceRealTileOutput =
    { plateauTiles : List String
    , aiPlateauTiles : List String
    , availablePositions : List Int
    , currentTurnNumber : Int
    , currentTile : Maybe String
    , currentTileImage : Maybe String
    , pendingAiPosition : Maybe Int
    , showTilePicker : Bool
    , statusMessage : String
    }


handlePlaceRealTilePure : GameModel -> Int -> PlaceRealTileOutput
handlePlaceRealTilePure model position =
    let
        tileImage =
            model.currentTileImage |> Maybe.withDefault ""

        newPlateauTiles =
            List.indexedMap
                (\i tile ->
                    if i == position then
                        tileImage

                    else
                        tile
                )
                model.plateauTiles

        newAiPlateauTiles =
            case model.pendingAiPosition of
                Just aiPos ->
                    List.indexedMap
                        (\i tile ->
                            if i == aiPos then
                                tileImage

                            else
                                tile
                        )
                        model.aiPlateauTiles

                Nothing ->
                    model.aiPlateauTiles

        newAvailablePositions =
            List.filter (\p -> p /= position) model.availablePositions

        newTurnNumber =
            model.currentTurnNumber + 1

        isGameOver =
            newTurnNumber >= 19

        aiMessage =
            case model.pendingAiPosition of
                Just aiPos ->
                    "IA joue en position " ++ String.fromInt aiPos

                Nothing ->
                    ""
    in
    { plateauTiles = newPlateauTiles
    , aiPlateauTiles = newAiPlateauTiles
    , availablePositions = newAvailablePositions
    , currentTurnNumber = newTurnNumber
    , currentTile = Nothing
    , currentTileImage = Nothing
    , pendingAiPosition = Nothing
    , showTilePicker = not isGameOver
    , statusMessage =
        if isGameOver then
            "Partie terminée! Calculez votre score."

        else
            aiMessage
    }



-- ============================================================================
-- TRANSITION: SelectRealTile
-- ============================================================================


type alias SelectRealTileOutput =
    { currentTile : Maybe String
    , currentTileImage : Maybe String
    , showTilePicker : Bool
    , usedTiles : List String
    , cmdIntent : CmdIntent
    }


handleSelectRealTilePure : GameModel -> String -> SelectRealTileOutput
handleSelectRealTilePure model tileCode =
    let
        aiAvailablePositions =
            List.indexedMap (\i tile -> ( i, tile )) model.aiPlateauTiles
                |> List.filter (\( _, tile ) -> tile == "")
                |> List.map Tuple.first
    in
    { currentTile = Just tileCode
    , currentTileImage = Just ("image/" ++ tileCode ++ ".png")
    , showTilePicker = False
    , usedTiles = tileCode :: model.usedTiles
    , cmdIntent =
        SendGetAiMove tileCode model.aiPlateauTiles aiAvailablePositions model.currentTurnNumber
    }



-- ============================================================================
-- TRANSITION: AiMoveResult
-- ============================================================================


type alias AiMoveResultOutput =
    { pendingAiPosition : Maybe Int
    , statusMessage : String
    }


handleAiMoveResultPure : Int -> String -> AiMoveResultOutput
handleAiMoveResultPure position errorMsg =
    if position >= 0 && position < 19 then
        { pendingAiPosition = Just position
        , statusMessage =
            if errorMsg /= "" then
                "IA: " ++ errorMsg

            else
                ""
        }

    else
        { pendingAiPosition = Nothing
        , statusMessage = "IA: position invalide"
        }



-- ============================================================================
-- QUERIES
-- ============================================================================


{-| Can the player click a board position to place a tile?
-}
canClickPosition : GameModel -> Bool
canClickPosition model =
    if model.isRealGameMode then
        -- In real game mode, can't click while tile picker is shown
        not model.showTilePicker
            && model.myTurn
            && (model.currentTile /= Nothing)
            && not (List.isEmpty model.availablePositions)

    else
        model.myTurn
            && (model.currentTile /= Nothing)
            && not (List.isEmpty model.availablePositions)


{-| Detect dead states — situations where the game is stuck.
-}
isDeadState : GameModel -> Bool
isDeadState model =
    -- DS4: Session exists but no game state
    (model.hasSession && not model.hasGameState && not model.gameStateIsFinished)
        -- DS2: It's my turn but no tile to place (and game not finished)
        || (model.myTurn && model.currentTile == Nothing && not model.gameStateIsFinished && not model.showTilePicker)
