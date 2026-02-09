port module Main exposing (main)

{-| Take It Easy - Frontend Elm avec architecture MVU pure
-}

import Browser
import Browser.Navigation as Nav
import Html exposing (..)
import Html.Attributes exposing (..)
import Html.Events exposing (..)
import Json.Decode as Decode
import Json.Encode as Encode
import TileSvg exposing (parseTileFromPath, viewEmptyHexSvg, viewTileSvg)
import Url



-- ============================================================================
-- PORTS (Communication avec JavaScript pour gRPC et localStorage)
-- ============================================================================


port sendToJs : Encode.Value -> Cmd msg


port receiveFromJs : (Decode.Value -> msg) -> Sub msg



-- ============================================================================
-- MODEL
-- ============================================================================


type alias User =
    { id : String
    , email : String
    , username : String
    , emailVerified : Bool
    }


type alias Player =
    { id : String
    , name : String
    , score : Int
    , isReady : Bool
    , isConnected : Bool
    }


type alias Session =
    { sessionId : String
    , playerId : String
    , sessionCode : String
    }


type alias GameMode =
    { id : String
    , name : String
    , description : String
    , icon : String
    , simulations : Maybe Int
    , difficulty : Maybe String
    }


type SessionState
    = Waiting
    | InProgress
    | Finished
    | Cancelled


type alias GameState =
    { sessionCode : String
    , state : SessionState
    , players : List Player
    , currentTurn : Maybe String
    }


type View
    = LoginView
    | ModeSelectionView
    | GameView


type AuthView
    = Login
    | Register
    | ForgotPassword
    | ResetPassword


type alias Model =
    { -- Navigation
      key : Nav.Key
    , url : Url.Url
    , currentView : View

    -- Auth
    , isAuthenticated : Bool
    , user : Maybe User
    , token : Maybe String
    , authView : AuthView
    , authLoading : Bool
    , authError : String

    -- Auth Form
    , emailInput : String
    , usernameInput : String
    , passwordInput : String
    , confirmPasswordInput : String
    , resetToken : String
    , resetMessage : String

    -- Game Mode Selection
    , selectedGameMode : Maybe GameMode
    , gameModes : List GameMode

    -- Session
    , playerName : String
    , sessionCode : String
    , session : Maybe Session
    , gameState : Maybe GameState

    -- Gameplay
    , currentTile : Maybe String
    , currentTileImage : Maybe String
    , plateauTiles : List String
    , aiPlateauTiles : List String
    , availablePositions : List Int
    , myTurn : Bool
    , currentTurnNumber : Int

    -- Real Game Mode (Jeu Réel)
    , isRealGameMode : Bool
    , showTilePicker : Bool
    , usedTiles : List String
    , realGameScore : Int
    , pendingAiPosition : Maybe Int

    -- Solo Mode
    , isSoloMode : Bool
    , aiScore : Int
    , showAiBoard : Bool

    -- UI
    , loading : Bool
    , error : String
    , statusMessage : String
    }


initialModel : Nav.Key -> Url.Url -> Model
initialModel key url =
    { key = key
    , url = url
    , currentView = LoginView

    -- Auth
    , isAuthenticated = False
    , user = Nothing
    , token = Nothing
    , authView = Login
    , authLoading = False
    , authError = ""

    -- Auth Form
    , emailInput = ""
    , usernameInput = ""
    , passwordInput = ""
    , confirmPasswordInput = ""
    , resetToken = ""
    , resetMessage = ""

    -- Game Mode Selection
    , selectedGameMode = Nothing
    , gameModes = defaultGameModes

    -- Session
    , playerName = ""
    , sessionCode = ""
    , session = Nothing
    , gameState = Nothing

    -- Gameplay
    , currentTile = Nothing
    , currentTileImage = Nothing
    , plateauTiles = List.repeat 19 ""
    , aiPlateauTiles = List.repeat 19 ""
    , availablePositions = List.range 0 18
    , myTurn = False
    , currentTurnNumber = 0

    -- Real Game Mode (Jeu Réel)
    , isRealGameMode = False
    , showTilePicker = False
    , usedTiles = []
    , realGameScore = 0
    , pendingAiPosition = Nothing

    -- Solo Mode
    , isSoloMode = False
    , aiScore = 0
    , showAiBoard = False

    -- UI
    , loading = False
    , error = ""
    , statusMessage = ""
    }


defaultGameModes : List GameMode
defaultGameModes =
    [ { id = "single-player"
      , name = "Solo"
      , description = "Affrontez l'IA Graph Transformer (149 pts)"
      , icon = "🤖"
      , simulations = Nothing
      , difficulty = Nothing
      }
    , { id = "real-game"
      , name = "Jeu Réel"
      , description = "Jouez avec le vrai jeu - sélectionnez les tuiles tirées"
      , icon = "🎲"
      , simulations = Nothing
      , difficulty = Nothing
      }
    , { id = "multiplayer"
      , name = "Multijoueur"
      , description = "Jouez contre d'autres joueurs en ligne"
      , icon = "👥"
      , simulations = Nothing
      , difficulty = Nothing
      }
    ]



-- ============================================================================
-- MSG (Messages)
-- ============================================================================


type Msg
    = -- Navigation
      UrlRequested Browser.UrlRequest
    | UrlChanged Url.Url
      -- Auth UI
    | SetEmailInput String
    | SetUsernameInput String
    | SetPasswordInput String
    | SetConfirmPasswordInput String
    | SwitchAuthView AuthView
    | SkipAuth
      -- Auth Actions
    | SubmitLogin
    | SubmitRegister
    | SubmitForgotPassword
    | SubmitResetPassword
    | Logout
    | CheckAuth
      -- Auth Responses (from JS)
    | LoginSuccess User String
    | LoginFailure String
    | RegisterSuccess User String
    | RegisterFailure String
    | ForgotPasswordSuccess String
    | ForgotPasswordFailure String
    | ResetPasswordSuccess String
    | ResetPasswordFailure String
    | CheckAuthSuccess User String
    | CheckAuthFailure
      -- Game Mode
    | SelectGameMode GameMode
    | StartGame
    | BackToModeSelection
    | ToggleAiBoard
    | RestartSoloGame
      -- Session
    | SetPlayerName String
    | SetSessionCode String
    | CreateSession
    | JoinSession
    | LeaveSession
    | SetReady
      -- Session Responses (from JS)
    | SessionCreated Session GameState
    | SessionJoined Session GameState
    | SessionLeft
    | ReadySet Bool
    | SessionError String
      -- Gameplay
    | StartTurn
    | PlayMove Int
      -- Real Game Mode
    | OpenTilePicker
    | SelectRealTile String
    | PlaceRealTile Int
    | ResetRealGame
    | AiMoveResult Int String
      -- Gameplay Responses (from JS)
    | TurnStarted String String Int (List Int) (List Player)
    | MovePlayed Int Int (List String) Int
    | GameStateUpdated GameState
    | GameFinished (List Player) (List String) (List String)
    | GameError String
      -- JS Interop
    | ReceivedFromJs Decode.Value



-- ============================================================================
-- UPDATE
-- ============================================================================


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        -- Navigation
        UrlRequested urlRequest ->
            case urlRequest of
                Browser.Internal url ->
                    ( model, Nav.pushUrl model.key (Url.toString url) )

                Browser.External href ->
                    ( model, Nav.load href )

        UrlChanged url ->
            ( { model | url = url }, Cmd.none )

        -- Auth UI
        SetEmailInput email ->
            ( { model | emailInput = email }, Cmd.none )

        SetUsernameInput username ->
            ( { model | usernameInput = username }, Cmd.none )

        SetPasswordInput password ->
            ( { model | passwordInput = password }, Cmd.none )

        SetConfirmPasswordInput password ->
            ( { model | confirmPasswordInput = password }, Cmd.none )

        SwitchAuthView newAuthView ->
            ( { model
                | authView = newAuthView
                , authError = ""
                , emailInput = ""
                , usernameInput = ""
                , passwordInput = ""
                , confirmPasswordInput = ""
              }
            , Cmd.none
            )

        SkipAuth ->
            ( { model | currentView = ModeSelectionView, isAuthenticated = False }
            , Cmd.none
            )

        -- Auth Actions
        SubmitLogin ->
            ( { model | authLoading = True, authError = "" }
            , sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "login" )
                    , ( "email", Encode.string model.emailInput )
                    , ( "password", Encode.string model.passwordInput )
                    ]
            )

        SubmitRegister ->
            if model.passwordInput /= model.confirmPasswordInput then
                ( { model | authError = "Les mots de passe ne correspondent pas" }, Cmd.none )

            else
                ( { model | authLoading = True, authError = "" }
                , sendToJs <|
                    Encode.object
                        [ ( "type", Encode.string "register" )
                        , ( "email", Encode.string model.emailInput )
                        , ( "username", Encode.string model.usernameInput )
                        , ( "password", Encode.string model.passwordInput )
                        ]
                )

        SubmitForgotPassword ->
            ( { model | authLoading = True, authError = "", resetMessage = "" }
            , sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "forgotPassword" )
                    , ( "email", Encode.string model.emailInput )
                    ]
            )

        SubmitResetPassword ->
            if model.passwordInput /= model.confirmPasswordInput then
                ( { model | authError = "Les mots de passe ne correspondent pas" }, Cmd.none )

            else
                ( { model | authLoading = True, authError = "" }
                , sendToJs <|
                    Encode.object
                        [ ( "type", Encode.string "resetPassword" )
                        , ( "token", Encode.string model.resetToken )
                        , ( "newPassword", Encode.string model.passwordInput )
                        ]
                )

        Logout ->
            ( { model
                | isAuthenticated = False
                , user = Nothing
                , token = Nothing
                , currentView = LoginView
              }
            , sendToJs <| Encode.object [ ( "type", Encode.string "logout" ) ]
            )

        CheckAuth ->
            ( model
            , sendToJs <| Encode.object [ ( "type", Encode.string "checkAuth" ) ]
            )

        -- Auth Responses
        LoginSuccess user token ->
            ( { model
                | isAuthenticated = True
                , user = Just user
                , token = Just token
                , authLoading = False
                , authError = ""
                , currentView = ModeSelectionView
                , playerName = user.username
              }
            , Cmd.none
            )

        LoginFailure error ->
            ( { model | authLoading = False, authError = error }, Cmd.none )

        RegisterSuccess user token ->
            ( { model
                | isAuthenticated = True
                , user = Just user
                , token = Just token
                , authLoading = False
                , authError = ""
                , currentView = ModeSelectionView
                , playerName = user.username
              }
            , Cmd.none
            )

        RegisterFailure error ->
            ( { model | authLoading = False, authError = error }, Cmd.none )

        ForgotPasswordSuccess message ->
            ( { model | authLoading = False, resetMessage = message, authError = "" }, Cmd.none )

        ForgotPasswordFailure error ->
            ( { model | authLoading = False, authError = error }, Cmd.none )

        ResetPasswordSuccess message ->
            ( { model
                | authLoading = False
                , resetMessage = message
                , authError = ""
                , authView = Login
                , passwordInput = ""
                , confirmPasswordInput = ""
                , resetToken = ""
              }
            , Cmd.none
            )

        ResetPasswordFailure error ->
            ( { model | authLoading = False, authError = error }, Cmd.none )

        CheckAuthSuccess user token ->
            ( { model
                | isAuthenticated = True
                , user = Just user
                , token = Just token
                , currentView = ModeSelectionView
                , playerName = user.username
              }
            , Cmd.none
            )

        CheckAuthFailure ->
            ( { model | isAuthenticated = False, user = Nothing, token = Nothing }, Cmd.none )

        -- Game Mode
        SelectGameMode mode ->
            ( { model | selectedGameMode = Just mode }, Cmd.none )

        StartGame ->
            case model.selectedGameMode of
                Just mode ->
                    if mode.id == "real-game" then
                        -- Mode Jeu Réel: pas besoin de serveur
                        ( { model
                            | currentView = GameView
                            , isRealGameMode = True
                            , showTilePicker = True
                            , usedTiles = []
                            , plateauTiles = List.repeat 19 ""
                            , availablePositions = List.range 0 18
                            , currentTurnNumber = 0
                            , realGameScore = 0
                            , currentTile = Nothing
                            , currentTileImage = Nothing
                            , myTurn = True
                          }
                        , Cmd.none
                        )

                    else
                        ( { model | currentView = GameView, isRealGameMode = False }, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        BackToModeSelection ->
            ( { model
                | currentView = ModeSelectionView
                , session = Nothing
                , gameState = Nothing
                , selectedGameMode = Nothing
                , error = ""
                , statusMessage = ""
              }
            , Cmd.none
            )

        ToggleAiBoard ->
            ( { model | showAiBoard = not model.showAiBoard }, Cmd.none )

        RestartSoloGame ->
            -- Reset game state and create new session
            let
                gameMode =
                    model.selectedGameMode
                        |> Maybe.map .id
                        |> Maybe.withDefault "single-player"
            in
            ( { model
                | session = Nothing
                , gameState = Nothing
                , plateauTiles = List.repeat 19 ""
                , aiPlateauTiles = List.repeat 19 ""
                , availablePositions = List.range 0 18
                , currentTurnNumber = 0
                , currentTile = Nothing
                , currentTileImage = Nothing
                , aiScore = 0
                , showAiBoard = False
                , loading = True
                , error = ""
                , statusMessage = ""
              }
            , sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "createSession" )
                    , ( "playerName", Encode.string model.playerName )
                    , ( "gameMode", Encode.string gameMode )
                    ]
            )

        -- Session
        SetPlayerName name ->
            ( { model | playerName = name }, Cmd.none )

        SetSessionCode code ->
            ( { model | sessionCode = code }, Cmd.none )

        CreateSession ->
            ( { model | loading = True, error = "" }
            , sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "createSession" )
                    , ( "playerName", Encode.string model.playerName )
                    , ( "gameMode", Encode.string (Maybe.withDefault "multiplayer" (Maybe.map .id model.selectedGameMode)) )
                    ]
            )

        JoinSession ->
            ( { model | loading = True, error = "" }
            , sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "joinSession" )
                    , ( "sessionCode", Encode.string model.sessionCode )
                    , ( "playerName", Encode.string model.playerName )
                    ]
            )

        LeaveSession ->
            case model.session of
                Just session ->
                    ( { model | loading = True }
                    , sendToJs <|
                        Encode.object
                            [ ( "type", Encode.string "leaveSession" )
                            , ( "sessionId", Encode.string session.sessionId )
                            , ( "playerId", Encode.string session.playerId )
                            ]
                    )

                Nothing ->
                    ( model, Cmd.none )

        SetReady ->
            case model.session of
                Just session ->
                    ( { model | loading = True }
                    , sendToJs <|
                        Encode.object
                            [ ( "type", Encode.string "setReady" )
                            , ( "sessionId", Encode.string session.sessionId )
                            , ( "playerId", Encode.string session.playerId )
                            ]
                    )

                Nothing ->
                    ( model, Cmd.none )

        -- Session Responses
        SessionCreated session gameState ->
            let
                -- Auto-ready pour les modes solo
                isSoloMode =
                    case model.selectedGameMode of
                        Just mode ->
                            String.startsWith "single-player" mode.id

                        Nothing ->
                            False

                autoReadyCmd =
                    if isSoloMode then
                        sendToJs <|
                            Encode.object
                                [ ( "type", Encode.string "setReady" )
                                , ( "sessionId", Encode.string session.sessionId )
                                , ( "playerId", Encode.string session.playerId )
                                ]
                    else
                        Cmd.none
            in
            ( { model
                | session = Just session
                , gameState = Just gameState
                , loading = isSoloMode  -- Reste en loading si auto-ready
                , isSoloMode = isSoloMode
                , statusMessage = "Session créée: " ++ session.sessionCode
              }
            , autoReadyCmd
            )

        SessionJoined session gameState ->
            ( { model
                | session = Just session
                , gameState = Just gameState
                , loading = False
                , statusMessage = "Rejoint la session: " ++ session.sessionCode
              }
            , Cmd.none
            )

        SessionLeft ->
            ( { model
                | session = Nothing
                , gameState = Nothing
                , loading = False
                , currentView = ModeSelectionView
              }
            , Cmd.none
            )

        ReadySet gameStarted ->
            let
                newStatusMessage =
                    if gameStarted then
                        "La partie commence!"

                    else
                        "Prêt! En attente des autres joueurs..."

                -- Auto-start turn pour les modes solo
                autoStartCmd =
                    if gameStarted then
                        case model.session of
                            Just session ->
                                sendToJs <|
                                    Encode.object
                                        [ ( "type", Encode.string "startTurn" )
                                        , ( "sessionId", Encode.string session.sessionId )
                                        ]

                            Nothing ->
                                Cmd.none
                    else
                        Cmd.none
            in
            ( { model | loading = gameStarted, statusMessage = newStatusMessage }, autoStartCmd )

        SessionError error ->
            ( { model | loading = False, error = error }, Cmd.none )

        -- Gameplay
        StartTurn ->
            case model.session of
                Just session ->
                    ( { model | loading = True }
                    , sendToJs <|
                        Encode.object
                            [ ( "type", Encode.string "startTurn" )
                            , ( "sessionId", Encode.string session.sessionId )
                            ]
                    )

                Nothing ->
                    ( model, Cmd.none )

        PlayMove position ->
            case model.session of
                Just session ->
                    ( { model | loading = True }
                    , sendToJs <|
                        Encode.object
                            [ ( "type", Encode.string "playMove" )
                            , ( "sessionId", Encode.string session.sessionId )
                            , ( "playerId", Encode.string session.playerId )
                            , ( "position", Encode.int position )
                            ]
                    )

                Nothing ->
                    ( model, Cmd.none )

        -- Real Game Mode
        OpenTilePicker ->
            ( { model | showTilePicker = True }, Cmd.none )

        SelectRealTile tileCode ->
            -- tileCode est comme "168" pour la tuile avec v1=1, v2=6, v3=8
            -- Calculer les positions disponibles pour l'IA
            let
                aiAvailablePositions =
                    List.indexedMap
                        (\i tile -> ( i, tile ))
                        model.aiPlateauTiles
                        |> List.filter (\( _, tile ) -> tile == "")
                        |> List.map Tuple.first
            in
            ( { model
                | currentTile = Just tileCode
                , currentTileImage = Just ("image/" ++ tileCode ++ ".png")
                , showTilePicker = False
                , usedTiles = tileCode :: model.usedTiles
              }
            , sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "getAiMove" )
                    , ( "tileCode", Encode.string tileCode )
                    , ( "boardState", Encode.list Encode.string model.aiPlateauTiles )
                    , ( "availablePositions", Encode.list Encode.int aiAvailablePositions )
                    , ( "turnNumber", Encode.int model.currentTurnNumber )
                    ]
            )

        PlaceRealTile position ->
            let
                tileImage =
                    model.currentTileImage |> Maybe.withDefault ""

                -- Placer la tuile du joueur
                newPlateauTiles =
                    List.indexedMap
                        (\i tile ->
                            if i == position then
                                tileImage

                            else
                                tile
                        )
                        model.plateauTiles

                -- Placer la tuile de l'IA (même tuile, position différente)
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
            ( { model
                | plateauTiles = newPlateauTiles
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
            , Cmd.none
            )

        ResetRealGame ->
            ( { model
                | plateauTiles = List.repeat 19 ""
                , aiPlateauTiles = List.repeat 19 ""
                , availablePositions = List.range 0 18
                , currentTurnNumber = 0
                , usedTiles = []
                , currentTile = Nothing
                , currentTileImage = Nothing
                , pendingAiPosition = Nothing
                , showTilePicker = True
                , realGameScore = 0
                , statusMessage = ""
              }
            , Cmd.none
            )

        AiMoveResult position errorMsg ->
            -- L'IA a choisi une position, on la stocke pour la placer après le joueur
            if position >= 0 && position < 19 then
                ( { model
                    | pendingAiPosition = Just position
                    , statusMessage =
                        if errorMsg /= "" then
                            "IA: " ++ errorMsg

                        else
                            ""
                  }
                , Cmd.none
                )

            else
                ( { model
                    | pendingAiPosition = Nothing
                    , statusMessage = "IA: position invalide"
                  }
                , Cmd.none
                )

        -- Gameplay Responses
        TurnStarted tile tileImage turnNumber positions players ->
            let
                -- Mettre à jour gameState.state vers InProgress et les joueurs
                updatedGameState =
                    Maybe.map
                        (\gs ->
                            { gs
                                | state = InProgress
                                , players =
                                    if List.isEmpty players then
                                        gs.players
                                    else
                                        players
                            }
                        )
                        model.gameState
            in
            ( { model
                | currentTile = Just tile
                , currentTileImage = Just tileImage
                , currentTurnNumber = turnNumber
                , availablePositions = positions
                , myTurn = True
                , loading = False
                , gameState = updatedGameState
              }
            , Cmd.none
            )

        MovePlayed position points aiTiles aiScore ->
            let
                -- Place la tuile actuelle sur le plateau
                newPlateauTiles =
                    List.indexedMap
                        (\i tile ->
                            if i == position then
                                -- Fix path: ../image/X.png -> image/X.png
                                model.currentTileImage
                                    |> Maybe.map (String.replace "../" "")
                                    |> Maybe.withDefault tile

                            else
                                tile
                        )
                        model.plateauTiles

                -- Retire la position des positions disponibles
                newAvailablePositions =
                    List.filter (\p -> p /= position) model.availablePositions

                -- Update AI tiles if provided
                newAiPlateauTiles =
                    if List.isEmpty aiTiles then
                        model.aiPlateauTiles
                    else
                        aiTiles
            in
            ( { model
                | myTurn = False
                , loading = False
                , statusMessage = "+" ++ String.fromInt points ++ " points"
                , plateauTiles = newPlateauTiles
                , aiPlateauTiles = newAiPlateauTiles
                , aiScore = aiScore
                , availablePositions = newAvailablePositions
                , currentTile = Nothing
                , currentTileImage = Nothing
              }
            , -- Auto-start next turn
              case model.session of
                Just session ->
                    sendToJs <|
                        Encode.object
                            [ ( "type", Encode.string "startTurn" )
                            , ( "sessionId", Encode.string session.sessionId )
                            ]

                Nothing ->
                    Cmd.none
            )

        GameStateUpdated gameState ->
            ( { model | gameState = Just gameState }, Cmd.none )

        GameFinished players playerTiles aiTiles ->
            let
                -- Merge final scores with existing player names
                mergePlayerScores existingPlayers newPlayers =
                    List.map
                        (\newP ->
                            let
                                existingName =
                                    List.filter (\p -> p.id == newP.id) existingPlayers
                                        |> List.head
                                        |> Maybe.map .name
                                        |> Maybe.withDefault newP.name
                            in
                            { newP | name = existingName }
                        )
                        newPlayers

                newGameState =
                    Maybe.map
                        (\gs ->
                            { gs
                                | state = Finished
                                , players = mergePlayerScores gs.players players
                            }
                        )
                        model.gameState
            in
            ( { model
                | gameState = newGameState
                , statusMessage = "Partie terminée!"
                , plateauTiles = playerTiles
                , aiPlateauTiles = aiTiles
              }
            , Cmd.none
            )

        GameError error ->
            ( { model | loading = False, error = error }, Cmd.none )

        -- JS Interop
        ReceivedFromJs value ->
            handleJsMessage value model


handleJsMessage : Decode.Value -> Model -> ( Model, Cmd Msg )
handleJsMessage value model =
    case Decode.decodeValue jsMessageDecoder value of
        Ok jsMsg ->
            case jsMsg of
                JsLoginSuccess user token ->
                    update (LoginSuccess user token) model

                JsLoginFailure error ->
                    update (LoginFailure error) model

                JsRegisterSuccess user token ->
                    update (RegisterSuccess user token) model

                JsRegisterFailure error ->
                    update (RegisterFailure error) model

                JsForgotPasswordSuccess message ->
                    update (ForgotPasswordSuccess message) model

                JsForgotPasswordFailure error ->
                    update (ForgotPasswordFailure error) model

                JsResetPasswordSuccess message ->
                    update (ResetPasswordSuccess message) model

                JsResetPasswordFailure error ->
                    update (ResetPasswordFailure error) model

                JsCheckAuthSuccess user token ->
                    update (CheckAuthSuccess user token) model

                JsCheckAuthFailure ->
                    update CheckAuthFailure model

                JsSessionCreated session gameState ->
                    update (SessionCreated session gameState) model

                JsSessionJoined session gameState ->
                    update (SessionJoined session gameState) model

                JsSessionLeft ->
                    update SessionLeft model

                JsReadySet gameStarted ->
                    update (ReadySet gameStarted) model

                JsSessionError error ->
                    update (SessionError error) model

                JsTurnStarted tile tileImage turnNumber positions players ->
                    update (TurnStarted tile tileImage turnNumber positions players) model

                JsMovePlayed position points aiTiles aiScore ->
                    update (MovePlayed position points aiTiles aiScore) model

                JsGameStateUpdated gameState ->
                    update (GameStateUpdated gameState) model

                JsGameFinished players playerTiles aiTiles ->
                    update (GameFinished players playerTiles aiTiles) model

                JsGameError error ->
                    update (GameError error) model

                JsAiMoveResult position error ->
                    update (AiMoveResult position error) model

        Err _ ->
            ( model, Cmd.none )


type JsMessage
    = JsLoginSuccess User String
    | JsLoginFailure String
    | JsRegisterSuccess User String
    | JsRegisterFailure String
    | JsForgotPasswordSuccess String
    | JsForgotPasswordFailure String
    | JsResetPasswordSuccess String
    | JsResetPasswordFailure String
    | JsCheckAuthSuccess User String
    | JsCheckAuthFailure
    | JsSessionCreated Session GameState
    | JsSessionJoined Session GameState
    | JsSessionLeft
    | JsReadySet Bool
    | JsSessionError String
    | JsTurnStarted String String Int (List Int) (List Player)
    | JsMovePlayed Int Int (List String) Int
    | JsGameStateUpdated GameState
    | JsGameFinished (List Player) (List String) (List String)
    | JsGameError String
    | JsAiMoveResult Int String


jsMessageDecoder : Decode.Decoder JsMessage
jsMessageDecoder =
    Decode.field "type" Decode.string
        |> Decode.andThen jsMessageDecoderByType


jsMessageDecoderByType : String -> Decode.Decoder JsMessage
jsMessageDecoderByType msgType =
    case msgType of
        "loginSuccess" ->
            Decode.map2 JsLoginSuccess
                (Decode.field "user" userDecoder)
                (Decode.field "token" Decode.string)

        "loginFailure" ->
            Decode.map JsLoginFailure (Decode.field "error" Decode.string)

        "registerSuccess" ->
            Decode.map2 JsRegisterSuccess
                (Decode.field "user" userDecoder)
                (Decode.field "token" Decode.string)

        "registerFailure" ->
            Decode.map JsRegisterFailure (Decode.field "error" Decode.string)

        "forgotPasswordSuccess" ->
            Decode.map JsForgotPasswordSuccess (Decode.field "message" Decode.string)

        "forgotPasswordFailure" ->
            Decode.map JsForgotPasswordFailure (Decode.field "error" Decode.string)

        "resetPasswordSuccess" ->
            Decode.map JsResetPasswordSuccess (Decode.field "message" Decode.string)

        "resetPasswordFailure" ->
            Decode.map JsResetPasswordFailure (Decode.field "error" Decode.string)

        "checkAuthSuccess" ->
            Decode.map2 JsCheckAuthSuccess
                (Decode.field "user" userDecoder)
                (Decode.field "token" Decode.string)

        "checkAuthFailure" ->
            Decode.succeed JsCheckAuthFailure

        "sessionCreated" ->
            Decode.map2 JsSessionCreated
                (Decode.field "session" sessionDecoder)
                (Decode.field "gameState" gameStateDecoder)

        "sessionJoined" ->
            Decode.map2 JsSessionJoined
                (Decode.field "session" sessionDecoder)
                (Decode.field "gameState" gameStateDecoder)

        "sessionLeft" ->
            Decode.succeed JsSessionLeft

        "readySet" ->
            Decode.map JsReadySet (Decode.field "gameStarted" Decode.bool)

        "sessionError" ->
            Decode.map JsSessionError (Decode.field "error" Decode.string)

        "turnStarted" ->
            Decode.map5 JsTurnStarted
                (Decode.field "tile" Decode.string)
                (Decode.field "tileImage" Decode.string)
                (Decode.field "turnNumber" Decode.int)
                (Decode.field "positions" (Decode.list Decode.int))
                (Decode.oneOf
                    [ Decode.field "players" (Decode.list playerDecoder)
                    , Decode.succeed []
                    ]
                )

        "movePlayed" ->
            Decode.map4 JsMovePlayed
                (Decode.field "position" Decode.int)
                (Decode.field "points" Decode.int)
                (Decode.oneOf
                    [ Decode.field "aiTiles" (Decode.list Decode.string)
                    , Decode.succeed []
                    ]
                )
                (Decode.oneOf
                    [ Decode.field "aiScore" Decode.int
                    , Decode.succeed 0
                    ]
                )

        "gameStateUpdated" ->
            Decode.map JsGameStateUpdated (Decode.field "gameState" gameStateDecoder)

        "gameFinished" ->
            Decode.map3 JsGameFinished
                (Decode.field "players" (Decode.list playerDecoder))
                (Decode.oneOf
                    [ Decode.at [ "plateaus", "player" ] (Decode.list Decode.string)
                    , Decode.succeed (List.repeat 19 "")
                    ]
                    |> Decode.andThen
                        (\_ ->
                            -- Find player plateau (not mcts_ai)
                            Decode.field "plateaus" (Decode.keyValuePairs (Decode.list Decode.string))
                                |> Decode.map
                                    (\pairs ->
                                        pairs
                                            |> List.filter (\( k, _ ) -> k /= "mcts_ai")
                                            |> List.head
                                            |> Maybe.map Tuple.second
                                            |> Maybe.withDefault (List.repeat 19 "")
                                    )
                        )
                )
                (Decode.oneOf
                    [ Decode.at [ "plateaus", "mcts_ai" ] (Decode.list Decode.string)
                    , Decode.succeed (List.repeat 19 "")
                    ]
                )

        "gameError" ->
            Decode.map JsGameError (Decode.field "error" Decode.string)

        "aiMoveResult" ->
            Decode.map2 JsAiMoveResult
                (Decode.field "position" Decode.int)
                (Decode.oneOf
                    [ Decode.field "error" Decode.string
                    , Decode.succeed ""
                    ]
                )

        _ ->
            Decode.fail ("Unknown message type: " ++ msgType)


userDecoder : Decode.Decoder User
userDecoder =
    Decode.map4 User
        (Decode.field "id" Decode.string)
        (Decode.field "email" Decode.string)
        (Decode.field "username" Decode.string)
        (Decode.field "emailVerified" Decode.bool)


sessionDecoder : Decode.Decoder Session
sessionDecoder =
    Decode.map3 Session
        (Decode.field "sessionId" Decode.string)
        (Decode.field "playerId" Decode.string)
        (Decode.field "sessionCode" Decode.string)


playerDecoder : Decode.Decoder Player
playerDecoder =
    Decode.map5 Player
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "score" Decode.int)
        (Decode.field "isReady" Decode.bool)
        (Decode.field "isConnected" Decode.bool)


gameStateDecoder : Decode.Decoder GameState
gameStateDecoder =
    Decode.map4 GameState
        (Decode.field "sessionCode" Decode.string)
        (Decode.field "state" sessionStateDecoder)
        (Decode.field "players" (Decode.list playerDecoder))
        (Decode.maybe (Decode.field "currentTurn" Decode.string))


sessionStateDecoder : Decode.Decoder SessionState
sessionStateDecoder =
    Decode.int
        |> Decode.andThen
            (\n ->
                case n of
                    0 ->
                        Decode.succeed Waiting

                    1 ->
                        Decode.succeed InProgress

                    2 ->
                        Decode.succeed Finished

                    3 ->
                        Decode.succeed Cancelled

                    _ ->
                        Decode.succeed Waiting
            )



-- ============================================================================
-- VIEW
-- ============================================================================


view : Model -> Browser.Document Msg
view model =
    { title = "Take It Easy - Elm"
    , body =
        [ div [ class "app-container" ]
            [ case model.currentView of
                LoginView ->
                    viewAuth model

                ModeSelectionView ->
                    viewModeSelection model

                GameView ->
                    viewGame model
            ]
        ]
    }


viewAuth : Model -> Html Msg
viewAuth model =
    div [ class "auth-page" ]
        [ div [ class "auth-container glass-container" ]
            [ div [ class "auth-header" ]
                [ h1 [] [ text "Take It Easy" ]
                , p [ class "auth-subtitle" ]
                    [ text (authSubtitle model.authView) ]
                ]
            , if model.authError /= "" then
                div [ class "auth-error" ] [ text model.authError ]

              else if model.resetMessage /= "" then
                div [ class "auth-success" ] [ text model.resetMessage ]

              else
                text ""
            , case model.authView of
                ForgotPassword ->
                    viewForgotPasswordForm model

                ResetPassword ->
                    viewResetPasswordForm model

                _ ->
                    viewLoginRegisterForm model
            , viewAuthFooter model
            ]
        ]


authSubtitle : AuthView -> String
authSubtitle authView =
    case authView of
        Login ->
            "Connectez-vous pour jouer"

        Register ->
            "Créez votre compte"

        ForgotPassword ->
            "Réinitialiser votre mot de passe"

        ResetPassword ->
            "Choisissez un nouveau mot de passe"


viewLoginRegisterForm : Model -> Html Msg
viewLoginRegisterForm model =
    Html.form [ onSubmitPreventDefault (if model.authView == Login then SubmitLogin else SubmitRegister), class "auth-form" ]
        [ div [ class "form-group" ]
            [ label [ for "email" ] [ text "Email" ]
            , input
                [ type_ "email"
                , id "email"
                , value model.emailInput
                , onInput SetEmailInput
                , placeholder ""
                , required True
                , disabled model.authLoading
                ]
                []
            ]
        , if model.authView == Register then
            div [ class "form-group" ]
                [ label [ for "username" ] [ text "Nom d'utilisateur" ]
                , input
                    [ type_ "text"
                    , id "username"
                    , value model.usernameInput
                    , onInput SetUsernameInput
                    , placeholder ""
                    , required True
                    , minlength 3
                    , maxlength 30
                    , disabled model.authLoading
                    ]
                    []
                ]

          else
            text ""
        , div [ class "form-group" ]
            [ label [ for "password" ] [ text "Mot de passe" ]
            , input
                [ type_ "password"
                , id "password"
                , value model.passwordInput
                , onInput SetPasswordInput
                , placeholder ""
                , required True
                , minlength 8
                , disabled model.authLoading
                , attribute "autocomplete" (if model.authView == Register then "new-password" else "current-password")
                ]
                []
            ]
        , if model.authView == Register then
            div [ class "form-group" ]
                [ label [ for "confirmPassword" ] [ text "Confirmer le mot de passe" ]
                , input
                    [ type_ "password"
                    , id "confirmPassword"
                    , value model.confirmPasswordInput
                    , onInput SetConfirmPasswordInput
                    , placeholder ""
                    , required True
                    , disabled model.authLoading
                    , attribute "autocomplete" "new-password"
                    ]
                    []
                ]

          else
            text ""
        , button
            [ type_ "button"
            , class "auth-submit-button"
            , disabled model.authLoading
            , onClick
                (if model.authView == Login then
                    SubmitLogin

                 else
                    SubmitRegister
                )
            ]
            [ if model.authLoading then
                span [ class "loading-spinner" ] []

              else
                text
                    (if model.authView == Login then
                        "Se connecter"

                     else
                        "Créer mon compte"
                    )
            ]
        , if model.authView == Login then
            div [ class "forgot-password-link" ]
                [ button
                    [ type_ "button"
                    , class "link-button"
                    , onClick (SwitchAuthView ForgotPassword)
                    ]
                    [ text "Mot de passe oublié ?" ]
                ]

          else
            text ""
        ]


viewForgotPasswordForm : Model -> Html Msg
viewForgotPasswordForm model =
    Html.form [ onSubmitPreventDefault SubmitForgotPassword, class "auth-form" ]
        [ div [ class "form-group" ]
            [ label [ for "email" ] [ text "Email" ]
            , input
                [ type_ "email"
                , id "email"
                , value model.emailInput
                , onInput SetEmailInput
                , placeholder ""
                , required True
                , disabled model.authLoading
                ]
                []
            ]
        , button
            [ type_ "button"
            , class "auth-submit-button"
            , disabled model.authLoading
            , onClick SubmitForgotPassword
            ]
            [ if model.authLoading then
                span [ class "loading-spinner" ] []

              else
                text "Envoyer le lien de réinitialisation"
            ]
        , div [ class "back-to-login" ]
            [ button
                [ type_ "button"
                , class "link-button"
                , onClick (SwitchAuthView Login)
                ]
                [ text "← Retour à la connexion" ]
            ]
        ]


viewResetPasswordForm : Model -> Html Msg
viewResetPasswordForm model =
    Html.form [ onSubmitPreventDefault SubmitResetPassword, class "auth-form" ]
        [ div [ class "form-group" ]
            [ label [ for "password" ] [ text "Nouveau mot de passe" ]
            , input
                [ type_ "password"
                , id "password"
                , value model.passwordInput
                , onInput SetPasswordInput
                , placeholder ""
                , required True
                , minlength 8
                , disabled model.authLoading
                , attribute "autocomplete" "new-password"
                ]
                []
            ]
        , div [ class "form-group" ]
            [ label [ for "confirmPassword" ] [ text "Confirmer le mot de passe" ]
            , input
                [ type_ "password"
                , id "confirmPassword"
                , value model.confirmPasswordInput
                , onInput SetConfirmPasswordInput
                , placeholder ""
                , required True
                , disabled model.authLoading
                , attribute "autocomplete" "new-password"
                ]
                []
            ]
        , button
            [ type_ "button"
            , class "auth-submit-button"
            , disabled model.authLoading
            , onClick SubmitResetPassword
            ]
            [ if model.authLoading then
                span [ class "loading-spinner" ] []

              else
                text "Réinitialiser le mot de passe"
            ]
        ]


viewAuthFooter : Model -> Html Msg
viewAuthFooter model =
    div []
        [ case model.authView of
            Login ->
                div [ class "auth-switch" ]
                    [ p []
                        [ text "Pas encore de compte ? "
                        , button
                            [ type_ "button"
                            , class "auth-switch-button"
                            , onClick (SwitchAuthView Register)
                            , disabled model.authLoading
                            ]
                            [ text "S'inscrire" ]
                        ]
                    ]

            Register ->
                div [ class "auth-switch" ]
                    [ p []
                        [ text "Déjà un compte ? "
                        , button
                            [ type_ "button"
                            , class "auth-switch-button"
                            , onClick (SwitchAuthView Login)
                            , disabled model.authLoading
                            ]
                            [ text "Se connecter" ]
                        ]
                    ]

            _ ->
                text ""
        , div [ class "auth-skip" ]
            [ button
                [ type_ "button"
                , class "skip-button"
                , onClick SkipAuth
                , disabled model.authLoading
                ]
                [ text "Jouer en mode invité" ]
            ]
        ]


onSubmitPreventDefault : Msg -> Attribute Msg
onSubmitPreventDefault msg =
    Html.Events.preventDefaultOn "submit" (Decode.succeed ( msg, True ))


viewModeSelection : Model -> Html Msg
viewModeSelection model =
    div [ class "game-mode-selector" ]
        [ viewUserHeader model
        , div [ class "header" ]
            [ h1 [] [ text "Take It Easy" ]
            , p [] [ text "Choisissez votre mode de jeu" ]
            ]
        , div [ class "modes-grid" ]
            (List.map (viewModeCard model.selectedGameMode) model.gameModes)
        , case model.selectedGameMode of
            Just mode ->
                div [ class "action-panel" ]
                    [ div [ class "selected-mode-info" ]
                        [ h3 [] [ text (mode.icon ++ " " ++ mode.name) ]
                        , p [] [ text mode.description ]
                        ]
                    , button [ class "start-button", onClick StartGame ]
                        [ text "Commencer"
                        , span [ class "start-icon" ] [ text " →" ]
                        ]
                    ]

            Nothing ->
                text ""
        ]


viewUserHeader : Model -> Html Msg
viewUserHeader model =
    div [ class "user-header" ]
        [ if model.isAuthenticated then
            case model.user of
                Just user ->
                    div [ class "user-info" ]
                        [ span [ class "user-name" ]
                            [ text "Connecté: "
                            , strong [] [ text user.username ]
                            ]
                        , button [ class "logout-button", onClick Logout ] [ text "Déconnexion" ]
                        ]

                Nothing ->
                    text ""

          else
            div [ class "guest-info" ]
                [ span [] [ text "Mode invité" ]
                , button [ class "login-link", onClick (SwitchAuthView Login) ] [ text "Se connecter" ]
                ]
        ]


viewModeCard : Maybe GameMode -> GameMode -> Html Msg
viewModeCard selectedMode mode =
    let
        isSelected =
            Maybe.map .id selectedMode == Just mode.id
    in
    div
        [ class
            ("mode-card"
                ++ (if isSelected then
                        " selected"

                    else
                        ""
                   )
            )
        , onClick (SelectGameMode mode)
        ]
        [ case mode.difficulty of
            Just diff ->
                span [ class ("difficulty-badge difficulty-" ++ diff) ] [ text diff ]

            Nothing ->
                text ""
        , div [ class "mode-icon" ] [ text mode.icon ]
        , h3 [] [ text mode.name ]
        , p [ class "mode-description" ] [ text mode.description ]
        ]


viewGame : Model -> Html Msg
viewGame model =
    div [ class "game-container" ]
        [ div [ class "header-section" ]
            [ div [ class "title-with-back" ]
                [ button [ class "back-button", onClick BackToModeSelection ] [ text "← Retour" ]
                , h1 []
                    [ text
                        (case model.selectedGameMode of
                            Just mode ->
                                mode.icon ++ " " ++ mode.name

                            Nothing ->
                                "Take It Easy"
                        )
                    ]
                ]
            ]
        , if model.error /= "" then
            div [ class "error-message" ] [ text model.error ]

          else
            text ""
        , if model.statusMessage /= "" then
            div [ class "status-message" ] [ text model.statusMessage ]

          else
            text ""
        , if model.isRealGameMode then
            viewRealGame model

          else
            case model.session of
                Nothing ->
                    viewConnectionInterface model

                Just session ->
                    viewGameSession model session
        ]


viewConnectionInterface : Model -> Html Msg
viewConnectionInterface model =
    div [ class "connection-interface glass-container" ]
        [ h2 [] [ text "Connexion à une partie" ]
        , div [ class "form-group" ]
            [ label [ for "playerName" ] [ text "Votre nom" ]
            , input
                [ type_ "text"
                , id "playerName"
                , value model.playerName
                , onInput SetPlayerName
                , placeholder "Entrez votre nom"
                , disabled model.loading
                ]
                []
            ]
        , div [ class "connection-buttons" ]
            [ button
                [ class "create-button"
                , onClick CreateSession
                , disabled (model.loading || model.playerName == "")
                ]
                [ text "Créer une partie" ]
            ]
        , div [ class "join-section" ]
            [ h3 [] [ text "Ou rejoindre une partie" ]
            , div [ class "form-group" ]
                [ input
                    [ type_ "text"
                    , value model.sessionCode
                    , onInput SetSessionCode
                    , placeholder "Code de session"
                    , disabled model.loading
                    ]
                    []
                ]
            , button
                [ class "join-button"
                , onClick JoinSession
                , disabled (model.loading || model.playerName == "" || model.sessionCode == "")
                ]
                [ text "Rejoindre" ]
            ]
        ]


viewGameSession : Model -> Session -> Html Msg
viewGameSession model session =
    div [ class "game-session" ]
        [ div [ class "session-info glass-container" ]
            [ h2 [] [ text ("Session: " ++ session.sessionCode) ]
            , p [] [ text ("Joueur: " ++ model.playerName) ]
            , button [ class "leave-button", onClick LeaveSession ] [ text "Quitter" ]
            ]
        , case model.gameState of
            Just gameState ->
                viewGameState model gameState session

            Nothing ->
                div [ class "loading" ] [ text "Chargement..." ]
        ]


viewGameState : Model -> GameState -> Session -> Html Msg
viewGameState model gameState session =
    div [ class "game-state" ]
        [ div [ class "players-list glass-container" ]
            [ h3 [] [ text "Joueurs" ]
            , ul []
                (List.map (viewPlayer session.playerId) gameState.players)
            ]
        , case gameState.state of
            Waiting ->
                viewWaitingState model session gameState

            InProgress ->
                viewInProgressState model session

            Finished ->
                viewFinishedState model gameState

            Cancelled ->
                div [ class "cancelled" ] [ text "Partie annulée" ]
        ]


viewPlayer : String -> Player -> Html Msg
viewPlayer myPlayerId player =
    li
        [ class
            ("player-item"
                ++ (if player.id == myPlayerId then
                        " self"

                    else if player.id == "mcts_ai" then
                        " ai"

                    else
                        ""
                   )
            )
        ]
        [ span [ class "player-name" ]
            [ text
                (if player.id == "mcts_ai" then
                    "🤖 IA"

                 else
                    "👤 " ++ player.name
                )
            ]
        , span [ class "player-score" ] [ text (String.fromInt player.score ++ " pts") ]
        , if player.isReady then
            span [ class "ready-badge" ] [ text "✓" ]

          else
            text ""
        ]


viewWaitingState : Model -> Session -> GameState -> Html Msg
viewWaitingState model session gameState =
    let
        currentPlayer =
            List.filter (\p -> p.id == session.playerId) gameState.players
                |> List.head

        isReady =
            Maybe.map .isReady currentPlayer |> Maybe.withDefault False
    in
    div [ class "waiting-state glass-container" ]
        [ h3 [] [ text "En attente des joueurs" ]
        , if isReady then
            p [] [ text "Vous êtes prêt! En attente des autres joueurs..." ]

          else
            button
                [ class "ready-button"
                , onClick SetReady
                , disabled model.loading
                ]
                [ text "Je suis prêt!" ]
        ]


viewInProgressState : Model -> Session -> Html Msg
viewInProgressState model session =
    div [ class "in-progress-state" ]
        [ div [ class "turn-info glass-container" ]
            [ h3 [] [ text ("Tour " ++ String.fromInt model.currentTurnNumber ++ "/19") ]
            , case model.currentTile of
                Just _ ->
                    div [ class "current-tile" ]
                        [ case model.currentTileImage of
                            Just img ->
                                case parseTileFromPath img of
                                    Just tileData ->
                                        div [ class "tile-svg-container" ]
                                            [ viewTileSvg tileData ]

                                    Nothing ->
                                        Html.img [ src img, class "tile-image" ] []

                            Nothing ->
                                text ""
                        ]

                Nothing ->
                    button
                        [ class "start-turn-button"
                        , onClick StartTurn
                        , disabled model.loading
                        ]
                        [ text "Commencer le tour" ]
            ]
        , div [ class "game-board glass-container" ]
            [ h3 [] [ text "Plateau de jeu" ]
            , viewHexBoard model
            , -- Solo mode: Toggle button to show AI board
              if model.isSoloMode then
                div [ style "margin-top" "15px", style "text-align" "center" ]
                    [ button
                        [ class "toggle-ai-board-button"
                        , onClick ToggleAiBoard
                        , style "padding" "8px 16px"
                        , style "border-radius" "8px"
                        , style "border" "none"
                        , style "background" "rgba(255,255,255,0.2)"
                        , style "cursor" "pointer"
                        ]
                        [ text
                            (if model.showAiBoard then
                                "🤖 Masquer plateau IA"

                             else
                                "🤖 Voir plateau IA"
                            )
                        ]
                    ]

              else
                text ""
            ]
        , -- Show AI board if toggled
          if model.isSoloMode && model.showAiBoard then
            div [ class "game-board glass-container", style "margin-top" "20px" ]
                [ h3 [] [ text ("🤖 Plateau IA - " ++ String.fromInt model.aiScore ++ " pts") ]
                , viewAiHexBoard model.aiPlateauTiles
                ]

          else
            text ""
        ]


getPlayerScore : Model -> Int
getPlayerScore model =
    case model.gameState of
        Just gs ->
            case model.session of
                Just session ->
                    List.filter (\p -> p.id == session.playerId) gs.players
                        |> List.head
                        |> Maybe.map .score
                        |> Maybe.withDefault 0

                Nothing ->
                    0

        Nothing ->
            0


viewHexBoard : Model -> Html Msg
viewHexBoard model =
    let
        -- Configuration hexagonale FLAT-TOP (plat en haut)
        hexRadius =
            60

        -- Pour flat-top: width = 2*radius, height = sqrt(3)*radius
        hexWidth =
            2 * hexRadius

        hexHeight =
            1.732 * hexRadius

        -- Espacement horizontal: 75% de la largeur (les hexs se chevauchent)
        spacingX =
            0.75 * hexWidth

        -- Espacement vertical: hauteur complète
        spacingY =
            hexHeight

        -- Positions du plateau Take It Easy (19 cases en losange)
        -- Format: (colonne, rang dans la colonne) -> converti en position x,y
        -- Colonnes: 3-4-5-4-3 hexagones
        hexPositions =
            [ -- Colonne 0 (3 hexs, commence à y=1)
              ( 0, 1 ), ( 0, 2 ), ( 0, 3 )
            -- Colonne 1 (4 hexs, commence à y=0.5)
            , ( 1, 0.5 ), ( 1, 1.5 ), ( 1, 2.5 ), ( 1, 3.5 )
            -- Colonne 2 (5 hexs, commence à y=0)
            , ( 2, 0 ), ( 2, 1 ), ( 2, 2 ), ( 2, 3 ), ( 2, 4 )
            -- Colonne 3 (4 hexs, commence à y=0.5)
            , ( 3, 0.5 ), ( 3, 1.5 ), ( 3, 2.5 ), ( 3, 3.5 )
            -- Colonne 4 (3 hexs, commence à y=1)
            , ( 4, 1 ), ( 4, 2 ), ( 4, 3 )
            ]

        gridOriginX =
            60

        gridOriginY =
            45
    in
    div [ class "hex-board", style "position" "relative", style "width" "600px", style "height" "570px", style "margin" "0 auto" ]
        (List.indexedMap
            (\index ( col, row ) ->
                let
                    x =
                        gridOriginX + col * spacingX

                    y =
                        gridOriginY + row * spacingY

                    tile =
                        List.head (List.drop index model.plateauTiles) |> Maybe.withDefault ""

                    isAvailable =
                        List.member index model.availablePositions && model.myTurn

                    canClick =
                        isAvailable && model.currentTile /= Nothing
                in
                div
                    [ class
                        ("hex-cell"
                            ++ (if isAvailable then
                                    " available"

                                else
                                    ""
                               )
                            ++ (if tile /= "" then
                                    " filled"

                                else
                                    ""
                               )
                        )
                    , style "left" (String.fromFloat x ++ "px")
                    , style "top" (String.fromFloat y ++ "px")
                    , style "width" (String.fromFloat hexWidth ++ "px")
                    , style "height" (String.fromFloat hexHeight ++ "px")
                    , if canClick then
                        onClick (PlayMove index)

                      else
                        class ""
                    ]
                    [ if tile /= "" then
                        case parseTileFromPath tile of
                            Just tileData ->
                                div [ class "hex-tile-svg" ]
                                    [ viewTileSvg tileData ]

                            Nothing ->
                                Html.img [ src tile, class "hex-tile-image" ] []

                      else
                        viewEmptyHexSvg isAvailable index
                    ]
            )
            hexPositions
        )


-- Smaller hex board for side-by-side display in Solo mode (player's board with interaction)
viewHexBoardSmall : List String -> List Int -> Bool -> Maybe String -> Html Msg
viewHexBoardSmall tiles availablePositions myTurn currentTile =
    let
        hexRadius =
            40

        hexWidth =
            2 * hexRadius

        hexHeight =
            1.732 * hexRadius

        spacingX =
            0.75 * hexWidth

        spacingY =
            hexHeight

        hexPositions =
            [ ( 0, 1 ), ( 0, 2 ), ( 0, 3 )
            , ( 1, 0.5 ), ( 1, 1.5 ), ( 1, 2.5 ), ( 1, 3.5 )
            , ( 2, 0 ), ( 2, 1 ), ( 2, 2 ), ( 2, 3 ), ( 2, 4 )
            , ( 3, 0.5 ), ( 3, 1.5 ), ( 3, 2.5 ), ( 3, 3.5 )
            , ( 4, 1 ), ( 4, 2 ), ( 4, 3 )
            ]

        gridOriginX =
            20

        gridOriginY =
            20
    in
    div [ class "hex-board", style "position" "relative", style "width" "340px", style "height" "380px", style "margin" "0 auto" ]
        (List.indexedMap
            (\index ( col, row ) ->
                let
                    x =
                        gridOriginX + col * spacingX

                    y =
                        gridOriginY + row * spacingY

                    tile =
                        List.head (List.drop index tiles) |> Maybe.withDefault ""

                    isAvailable =
                        List.member index availablePositions && myTurn

                    canClick =
                        isAvailable && currentTile /= Nothing
                in
                div
                    [ class
                        ("hex-cell"
                            ++ (if isAvailable then
                                    " available"

                                else
                                    ""
                               )
                            ++ (if tile /= "" then
                                    " filled"

                                else
                                    ""
                               )
                        )
                    , style "left" (String.fromFloat x ++ "px")
                    , style "top" (String.fromFloat y ++ "px")
                    , style "width" (String.fromFloat hexWidth ++ "px")
                    , style "height" (String.fromFloat hexHeight ++ "px")
                    , if canClick then
                        onClick (PlayMove index)

                      else
                        class ""
                    ]
                    [ if tile /= "" then
                        case parseTileFromPath tile of
                            Just tileData ->
                                div [ class "hex-tile-svg" ]
                                    [ viewTileSvg tileData ]

                            Nothing ->
                                Html.img [ src tile, class "hex-tile-image" ] []

                      else
                        viewEmptyHexSvg isAvailable index
                    ]
            )
            hexPositions
        )


-- AI hex board for Solo mode (display only, no interaction)
viewAiHexBoard : List String -> Html Msg
viewAiHexBoard tiles =
    let
        hexRadius =
            40

        hexWidth =
            2 * hexRadius

        hexHeight =
            1.732 * hexRadius

        spacingX =
            0.75 * hexWidth

        spacingY =
            hexHeight

        hexPositions =
            [ ( 0, 1 ), ( 0, 2 ), ( 0, 3 )
            , ( 1, 0.5 ), ( 1, 1.5 ), ( 1, 2.5 ), ( 1, 3.5 )
            , ( 2, 0 ), ( 2, 1 ), ( 2, 2 ), ( 2, 3 ), ( 2, 4 )
            , ( 3, 0.5 ), ( 3, 1.5 ), ( 3, 2.5 ), ( 3, 3.5 )
            , ( 4, 1 ), ( 4, 2 ), ( 4, 3 )
            ]

        gridOriginX =
            20

        gridOriginY =
            20
    in
    div [ class "hex-board ai-board", style "position" "relative", style "width" "340px", style "height" "380px", style "margin" "0 auto" ]
        (List.indexedMap
            (\index ( col, row ) ->
                let
                    x =
                        gridOriginX + col * spacingX

                    y =
                        gridOriginY + row * spacingY

                    tile =
                        List.head (List.drop index tiles) |> Maybe.withDefault ""
                in
                div
                    [ class
                        ("hex-cell"
                            ++ (if tile /= "" then
                                    " filled"

                                else
                                    ""
                               )
                        )
                    , style "left" (String.fromFloat x ++ "px")
                    , style "top" (String.fromFloat y ++ "px")
                    , style "width" (String.fromFloat hexWidth ++ "px")
                    , style "height" (String.fromFloat hexHeight ++ "px")
                    ]
                    [ if tile /= "" then
                        case parseTileFromPath tile of
                            Just tileData ->
                                div [ class "hex-tile-svg" ]
                                    [ viewTileSvg tileData ]

                            Nothing ->
                                Html.img [ src tile, class "hex-tile-image" ] []

                      else
                        viewEmptyHexSvg False index
                    ]
            )
            hexPositions
        )


viewFinishedState : Model -> GameState -> Html Msg
viewFinishedState model gameState =
    let
        sortedPlayers =
            List.sortBy (\p -> -p.score) gameState.players

        winner =
            List.head sortedPlayers
    in
    div [ class "finished-state" ]
        [ div [ class "finished-header glass-container" ]
            [ h2 [] [ text "🎉 Partie terminée!" ]
            , case winner of
                Just w ->
                    div [ class "winner" ]
                        [ text ("🏆 Gagnant: " ++ w.name ++ " avec " ++ String.fromInt w.score ++ " points!")
                        ]

                Nothing ->
                    text ""
            ]
        , div [ class "finished-content" ]
            [ -- Scores panel
              div [ class "final-scores glass-container" ]
                [ h3 [] [ text "Classement final" ]
                , ul []
                    (List.indexedMap
                        (\i p ->
                            li [ class "final-score-item" ]
                                [ span [ class "rank" ] [ text (String.fromInt (i + 1) ++ ".") ]
                                , span [ class "name" ]
                                    [ text
                                        (if p.id == "mcts_ai" then
                                            "🤖 IA"

                                         else
                                            "👤 " ++ p.name
                                        )
                                    ]
                                , span [ class "score" ] [ text (String.fromInt p.score ++ " pts") ]
                                ]
                        )
                        sortedPlayers
                    )
                ]
            ]
        , div [ class "finished-boards" ]
            [ -- Player's final plateau
              div [ class "final-board glass-container" ]
                [ h3 [] [ text "👤 Votre plateau" ]
                , viewFinalHexBoard model.plateauTiles
                ]
            , -- AI's final plateau
              div [ class "final-board glass-container" ]
                [ h3 [] [ text "🤖 Plateau IA" ]
                , viewFinalHexBoard model.aiPlateauTiles
                ]
            ]
        , if model.isSoloMode then
            button [ class "play-again-button", onClick RestartSoloGame ] [ text "🔄 Rejouer" ]

          else
            button [ class "play-again-button", onClick BackToModeSelection ] [ text "Rejouer" ]
        ]


viewFinalHexBoard : List String -> Html Msg
viewFinalHexBoard tiles =
    let
        hexRadius =
            36

        hexWidth =
            2 * hexRadius

        hexHeight =
            1.732 * hexRadius

        spacingX =
            0.75 * hexWidth

        spacingY =
            hexHeight

        hexPositions =
            [ ( 0, 1 ), ( 0, 2 ), ( 0, 3 )
            , ( 1, 0.5 ), ( 1, 1.5 ), ( 1, 2.5 ), ( 1, 3.5 )
            , ( 2, 0 ), ( 2, 1 ), ( 2, 2 ), ( 2, 3 ), ( 2, 4 )
            , ( 3, 0.5 ), ( 3, 1.5 ), ( 3, 2.5 ), ( 3, 3.5 )
            , ( 4, 1 ), ( 4, 2 ), ( 4, 3 )
            ]

        -- Calcul pour centrer: largeur totale = 4 * spacingX + hexWidth = 4*54 + 72 = 288
        -- Container width = 320, donc offset = (320 - 288) / 2 = 16
        gridOriginX =
            16

        gridOriginY =
            20
    in
    div [ class "hex-board final-hex-board", style "position" "relative", style "width" "320px", style "height" "340px", style "margin" "0 auto" ]
        (List.indexedMap
            (\index ( col, row ) ->
                let
                    x =
                        gridOriginX + col * spacingX

                    y =
                        gridOriginY + row * spacingY

                    tile =
                        List.head (List.drop index tiles) |> Maybe.withDefault ""
                in
                div
                    [ class
                        ("hex-cell"
                            ++ (if tile /= "" then
                                    " filled"

                                else
                                    ""
                               )
                        )
                    , style "left" (String.fromFloat x ++ "px")
                    , style "top" (String.fromFloat y ++ "px")
                    , style "width" (String.fromFloat hexWidth ++ "px")
                    , style "height" (String.fromFloat hexHeight ++ "px")
                    ]
                    [ if tile /= "" then
                        case parseTileFromPath tile of
                            Just tileData ->
                                div [ class "hex-tile-svg" ]
                                    [ viewTileSvg tileData ]

                            Nothing ->
                                Html.img [ src tile, class "hex-tile-image" ] []

                      else
                        viewEmptyHexSvg False index
                    ]
            )
            hexPositions
        )



-- ============================================================================
-- REAL GAME MODE (Jeu Réel avec tuiles physiques)
-- ============================================================================


{-| Vue principale du mode Jeu Réel
-}
viewRealGame : Model -> Html Msg
viewRealGame model =
    div [ class "real-game-container" ]
        [ div [ class "real-game-info glass-container" ]
            [ h2 [] [ text ("Tour " ++ String.fromInt (model.currentTurnNumber + 1) ++ "/19") ]
            , p [] [ text ("Tuiles utilisées: " ++ String.fromInt (List.length model.usedTiles) ++ "/27") ]
            , button [ class "reset-button", onClick ResetRealGame ] [ text "🔄 Recommencer" ]
            ]
        , if model.showTilePicker then
            viewTilePicker model

          else
            div [ class "current-tile-section glass-container" ]
                [ h3 [] [ text "Tuile sélectionnée" ]
                , case model.currentTileImage of
                    Just img ->
                        case parseTileFromPath img of
                            Just tileData ->
                                div [ class "selected-tile-display" ]
                                    [ viewTileSvg tileData ]

                            Nothing ->
                                text ""

                    Nothing ->
                        text ""
                , p [] [ text "Cliquez sur une case pour placer la tuile" ]
                ]
        , div [ class "real-game-boards" ]
            [ div [ class "game-board glass-container" ]
                [ h3 [] [ text "Votre plateau" ]
                , viewRealGameBoard model
                ]
            , div [ class "game-board glass-container ai-board" ]
                [ h3 [] [ text "🤖 Plateau IA" ]
                , viewAiRealGameBoard model
                ]
            ]
        , if model.currentTurnNumber >= 19 then
            div [ class "game-over glass-container" ]
                [ h2 [] [ text "🎉 Partie terminée!" ]
                , p [] [ text "Comptez vos points sur le plateau!" ]
                , button [ class "play-again-button", onClick ResetRealGame ] [ text "Nouvelle partie" ]
                ]

          else
            text ""
        ]


{-| Grille de sélection des 27 tuiles - 3 lignes par valeur verticale
-}
viewTilePicker : Model -> Html Msg
viewTilePicker model =
    let
        -- Génère les 9 tuiles pour une valeur v1 donnée
        tilesForV1 v1 =
            List.concatMap
                (\v2 ->
                    List.map
                        (\v3 ->
                            String.fromInt v1 ++ String.fromInt v2 ++ String.fromInt v3
                        )
                        [ 3, 4, 8 ]
                )
                [ 2, 6, 7 ]
    in
    div [ class "tile-picker glass-container" ]
        [ h3 [] [ text "🎲 Sélectionnez la tuile tirée" ]
        , div [ class "tiles-rows" ]
            [ div [ class "tiles-row" ]
                [ span [ class "row-label" ] [ text "1" ]
                , div [ class "row-tiles" ] (List.map (viewPickerTile model.usedTiles) (tilesForV1 1))
                ]
            , div [ class "tiles-row" ]
                [ span [ class "row-label" ] [ text "5" ]
                , div [ class "row-tiles" ] (List.map (viewPickerTile model.usedTiles) (tilesForV1 5))
                ]
            , div [ class "tiles-row" ]
                [ span [ class "row-label" ] [ text "9" ]
                , div [ class "row-tiles" ] (List.map (viewPickerTile model.usedTiles) (tilesForV1 9))
                ]
            ]
        ]


{-| Une tuile dans le sélecteur
-}
viewPickerTile : List String -> String -> Html Msg
viewPickerTile usedTiles tileCode =
    let
        isUsed =
            List.member tileCode usedTiles

        tileData =
            parseTileFromPath ("image/" ++ tileCode ++ ".png")
    in
    div
        [ class
            ("picker-tile"
                ++ (if isUsed then
                        " used"

                    else
                        ""
                   )
            )
        , if isUsed then
            class ""

          else
            onClick (SelectRealTile tileCode)
        ]
        [ case tileData of
            Just td ->
                viewTileSvg td

            Nothing ->
                text tileCode
        , if isUsed then
            div [ class "used-overlay" ] [ text "✓" ]

          else
            text ""
        ]


{-| Plateau de jeu pour le mode Jeu Réel
-}
viewRealGameBoard : Model -> Html Msg
viewRealGameBoard model =
    let
        hexRadius =
            45

        hexWidth =
            2 * hexRadius

        hexHeight =
            1.732 * hexRadius

        spacingX =
            0.75 * hexWidth

        spacingY =
            hexHeight

        hexPositions =
            [ ( 0, 1 ), ( 0, 2 ), ( 0, 3 )
            , ( 1, 0.5 ), ( 1, 1.5 ), ( 1, 2.5 ), ( 1, 3.5 )
            , ( 2, 0 ), ( 2, 1 ), ( 2, 2 ), ( 2, 3 ), ( 2, 4 )
            , ( 3, 0.5 ), ( 3, 1.5 ), ( 3, 2.5 ), ( 3, 3.5 )
            , ( 4, 1 ), ( 4, 2 ), ( 4, 3 )
            ]

        gridOriginX =
            70

        gridOriginY =
            35
    in
    div [ class "hex-board", style "position" "relative", style "width" "450px", style "height" "430px", style "margin" "0 auto" ]
        (List.indexedMap
            (\index ( col, row ) ->
                let
                    x =
                        gridOriginX + col * spacingX

                    y =
                        gridOriginY + row * spacingY

                    tile =
                        List.head (List.drop index model.plateauTiles) |> Maybe.withDefault ""

                    isAvailable =
                        List.member index model.availablePositions

                    canClick =
                        isAvailable && model.currentTile /= Nothing && not model.showTilePicker
                in
                div
                    [ class
                        ("hex-cell"
                            ++ (if isAvailable && not model.showTilePicker then
                                    " available"

                                else
                                    ""
                               )
                            ++ (if tile /= "" then
                                    " filled"

                                else
                                    ""
                               )
                        )
                    , style "left" (String.fromFloat x ++ "px")
                    , style "top" (String.fromFloat y ++ "px")
                    , style "width" (String.fromFloat hexWidth ++ "px")
                    , style "height" (String.fromFloat hexHeight ++ "px")
                    , if canClick then
                        onClick (PlaceRealTile index)

                      else
                        class ""
                    ]
                    [ if tile /= "" then
                        case parseTileFromPath tile of
                            Just tileData ->
                                div [ class "hex-tile-svg" ]
                                    [ viewTileSvg tileData ]

                            Nothing ->
                                text ""

                      else
                        viewEmptyHexSvg (isAvailable && not model.showTilePicker) index
                    ]
            )
            hexPositions
        )


{-| Plateau IA pour le mode Jeu Réel (non-interactif)
-}
viewAiRealGameBoard : Model -> Html Msg
viewAiRealGameBoard model =
    let
        hexRadius =
            45

        hexWidth =
            2 * hexRadius

        hexHeight =
            1.732 * hexRadius

        spacingX =
            0.75 * hexWidth

        spacingY =
            hexHeight

        hexPositions =
            [ ( 0, 1 ), ( 0, 2 ), ( 0, 3 )
            , ( 1, 0.5 ), ( 1, 1.5 ), ( 1, 2.5 ), ( 1, 3.5 )
            , ( 2, 0 ), ( 2, 1 ), ( 2, 2 ), ( 2, 3 ), ( 2, 4 )
            , ( 3, 0.5 ), ( 3, 1.5 ), ( 3, 2.5 ), ( 3, 3.5 )
            , ( 4, 1 ), ( 4, 2 ), ( 4, 3 )
            ]

        gridOriginX =
            70

        gridOriginY =
            35
    in
    div [ class "hex-board ai-hex-board", style "position" "relative", style "width" "450px", style "height" "430px", style "margin" "0 auto" ]
        (List.indexedMap
            (\index ( col, row ) ->
                let
                    x =
                        gridOriginX + col * spacingX

                    y =
                        gridOriginY + row * spacingY

                    tile =
                        List.head (List.drop index model.aiPlateauTiles) |> Maybe.withDefault ""
                in
                div
                    [ class
                        ("hex-cell"
                            ++ (if tile /= "" then
                                    " filled"

                                else
                                    ""
                               )
                        )
                    , style "left" (String.fromFloat x ++ "px")
                    , style "top" (String.fromFloat y ++ "px")
                    , style "width" (String.fromFloat hexWidth ++ "px")
                    , style "height" (String.fromFloat hexHeight ++ "px")
                    ]
                    [ if tile /= "" then
                        case parseTileFromPath tile of
                            Just tileData ->
                                div [ class "hex-tile-svg" ]
                                    [ viewTileSvg tileData ]

                            Nothing ->
                                text ""

                      else
                        viewEmptyHexSvg False index
                    ]
            )
            hexPositions
        )


-- ============================================================================
-- SUBSCRIPTIONS
-- ============================================================================


subscriptions : Model -> Sub Msg
subscriptions _ =
    receiveFromJs ReceivedFromJs



-- ============================================================================
-- MAIN
-- ============================================================================


main : Program () Model Msg
main =
    Browser.application
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        , onUrlRequest = UrlRequested
        , onUrlChange = UrlChanged
        }


init : () -> Url.Url -> Nav.Key -> ( Model, Cmd Msg )
init _ url key =
    let
        baseModel =
            initialModel key url

        -- Check for reset_token in URL query
        modelWithResetToken =
            case url.query of
                Just query ->
                    case extractResetToken query of
                        Just token ->
                            { baseModel
                                | authView = ResetPassword
                                , resetToken = token
                            }

                        Nothing ->
                            baseModel

                Nothing ->
                    baseModel
    in
    ( modelWithResetToken
    , sendToJs <| Encode.object [ ( "type", Encode.string "checkAuth" ) ]
    )


extractResetToken : String -> Maybe String
extractResetToken query =
    query
        |> String.split "&"
        |> List.filterMap
            (\param ->
                case String.split "=" param of
                    [ "reset_token", value ] ->
                        Just value

                    _ ->
                        Nothing
            )
        |> List.head
