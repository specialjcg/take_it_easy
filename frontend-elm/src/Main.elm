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
import Process
import Task
import GameLogic
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
    = Welcome
    | Login
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
    , waitingForPlayers : List String

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

    -- End of game: all player boards (id, name, tiles)
    , allPlayerPlateaus : List ( String, String, List String )

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
    , authView = Welcome
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
    , waitingForPlayers = []

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

    -- End of game
    , allPlayerPlateaus = []

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
    | GoToLogin
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
    | PollSession
    | SessionPolled GameState
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
    | TurnStarted String String Int (List Int) (List Player) (List String)
    | MovePlayed Int Int (List String) Int Bool
    | PollTurn
    | GameStateUpdated GameState
    | GameFinished (List Player) (List String) (List String) (List ( String, List String ))
    | GameError String
      -- JS Interop
    | ReceivedFromJs Decode.Value



-- ============================================================================
-- UPDATE
-- ============================================================================


toGameModel : Model -> GameLogic.GameModel
toGameModel model =
    { sessionId = model.session |> Maybe.map .sessionId
    , playerId = model.session |> Maybe.map .playerId
    , currentTile = model.currentTile
    , currentTileImage = model.currentTileImage
    , plateauTiles = model.plateauTiles
    , aiPlateauTiles = model.aiPlateauTiles
    , availablePositions = model.availablePositions
    , myTurn = model.myTurn
    , currentTurnNumber = model.currentTurnNumber
    , waitingForPlayers = model.waitingForPlayers
    , isRealGameMode = model.isRealGameMode
    , showTilePicker = model.showTilePicker
    , usedTiles = model.usedTiles
    , pendingAiPosition = model.pendingAiPosition
    , isSoloMode = model.isSoloMode
    , loading = model.loading
    , statusMessage = model.statusMessage
    , hasSession = model.session /= Nothing
    , hasGameState = model.gameState /= Nothing
    , gameStateIsFinished =
        model.gameState
            |> Maybe.map (\gs -> gs.state == Finished)
            |> Maybe.withDefault False
    }


resolveCmdIntent : GameLogic.CmdIntent -> Cmd Msg
resolveCmdIntent intent =
    case intent of
        GameLogic.NoCmd ->
            Cmd.none

        GameLogic.SendStartTurn sessionId ->
            sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "startTurn" )
                    , ( "sessionId", Encode.string sessionId )
                    ]

        GameLogic.SchedulePollTurn delay ->
            Process.sleep delay
                |> Task.perform (\_ -> PollTurn)

        GameLogic.SendGetAiMove tileCode boardState availPos turnNum ->
            sendToJs <|
                Encode.object
                    [ ( "type", Encode.string "getAiMove" )
                    , ( "tileCode", Encode.string tileCode )
                    , ( "boardState", Encode.list Encode.string boardState )
                    , ( "availablePositions", Encode.list Encode.int availPos )
                    , ( "turnNumber", Encode.int turnNum )
                    ]

        GameLogic.BatchCmds cmds ->
            Cmd.batch (List.map resolveCmdIntent cmds)


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

        GoToLogin ->
            ( { model
                | currentView = LoginView
                , authView = Login
                , authError = ""
                , emailInput = ""
                , passwordInput = ""
              }
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
                , authView = Welcome
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
                , allPlayerPlateaus = []
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
                , allPlayerPlateaus = []
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

                cmd =
                    if isSoloMode then
                        Cmd.batch
                            [ sendToJs <|
                                Encode.object
                                    [ ( "type", Encode.string "setReady" )
                                    , ( "sessionId", Encode.string session.sessionId )
                                    , ( "playerId", Encode.string session.playerId )
                                    ]
                            , -- Safety: poll after 5s if ReadySet response is lost
                              Process.sleep 5000
                                |> Task.perform (\_ -> PollSession)
                            ]

                    else
                        -- Mode multijoueur: démarrer le polling du lobby
                        Process.sleep 2000
                            |> Task.perform (\_ -> PollSession)
            in
            ( { model
                | session = Just session
                , gameState = Just gameState
                , loading = isSoloMode  -- Reste en loading si auto-ready
                , isSoloMode = isSoloMode
                , statusMessage = "Session créée: " ++ session.sessionCode
              }
            , cmd
            )

        SessionJoined session gameState ->
            ( { model
                | session = Just session
                , gameState = Just gameState
                , loading = False
                , statusMessage = "Rejoint la session: " ++ session.sessionCode
              }
            , -- Démarrer le polling pour détecter les autres joueurs et le démarrage
              Process.sleep 2000
                |> Task.perform (\_ -> PollSession)
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

                cmd =
                    if gameStarted then
                        Cmd.batch
                            [ case model.session of
                                Just session ->
                                    sendToJs <|
                                        Encode.object
                                            [ ( "type", Encode.string "startTurn" )
                                            , ( "sessionId", Encode.string session.sessionId )
                                            ]

                                Nothing ->
                                    Cmd.none
                            , -- Safety: poll after 3s if TurnStarted response is lost
                              Process.sleep 3000
                                |> Task.perform (\_ -> PollTurn)
                            ]

                    else
                        -- Polling pour détecter quand la partie démarre
                        Process.sleep 2000
                            |> Task.perform (\_ -> PollSession)
            in
            ( { model | loading = gameStarted, statusMessage = newStatusMessage }, cmd )

        SessionError error ->
            ( { model | loading = False, error = error }, Cmd.none )

        PollSession ->
            case model.session of
                Just session ->
                    ( model
                    , sendToJs <|
                        Encode.object
                            [ ( "type", Encode.string "pollSession" )
                            , ( "sessionId", Encode.string session.sessionId )
                            ]
                    )

                Nothing ->
                    ( model, Cmd.none )

        SessionPolled gameState ->
            let
                gameStarted =
                    gameState.state == InProgress

                autoStartCmd =
                    if gameStarted then
                        Cmd.batch
                            [ case model.session of
                                Just session ->
                                    sendToJs <|
                                        Encode.object
                                            [ ( "type", Encode.string "startTurn" )
                                            , ( "sessionId", Encode.string session.sessionId )
                                            ]

                                Nothing ->
                                    Cmd.none
                            , -- Safety: poll after 3s if TurnStarted response is lost
                              Process.sleep 3000
                                |> Task.perform (\_ -> PollTurn)
                            ]

                    else
                        -- Continuer le polling si encore en attente
                        Process.sleep 2000
                            |> Task.perform (\_ -> PollSession)
            in
            ( { model
                | gameState = Just gameState
                , loading = gameStarted
              }
            , autoStartCmd
            )

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
            let
                result =
                    GameLogic.handleSelectRealTilePure (toGameModel model) tileCode
            in
            ( { model
                | currentTile = result.currentTile
                , currentTileImage = result.currentTileImage
                , showTilePicker = result.showTilePicker
                , usedTiles = result.usedTiles
              }
            , resolveCmdIntent result.cmdIntent
            )

        PlaceRealTile position ->
            let
                result =
                    GameLogic.handlePlaceRealTilePure (toGameModel model) position
            in
            ( { model
                | plateauTiles = result.plateauTiles
                , aiPlateauTiles = result.aiPlateauTiles
                , availablePositions = result.availablePositions
                , currentTurnNumber = result.currentTurnNumber
                , currentTile = result.currentTile
                , currentTileImage = result.currentTileImage
                , pendingAiPosition = result.pendingAiPosition
                , showTilePicker = result.showTilePicker
                , statusMessage = result.statusMessage
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
            let
                result =
                    GameLogic.handleAiMoveResultPure position errorMsg
            in
            ( { model
                | pendingAiPosition = result.pendingAiPosition
                , statusMessage = result.statusMessage
              }
            , Cmd.none
            )

        -- Gameplay Responses
        TurnStarted tile tileImage turnNumber positions players waiting ->
            let
                result =
                    GameLogic.handleTurnStartedPure (toGameModel model)
                        { tile = tile
                        , tileImage = tileImage
                        , turnNumber = turnNumber
                        , positions = positions
                        , waiting = waiting
                        }

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
                | currentTile = result.currentTile
                , currentTileImage = result.currentTileImage
                , currentTurnNumber = result.currentTurnNumber
                , availablePositions = result.availablePositions
                , myTurn = result.myTurn
                , loading = result.loading
                , gameState = updatedGameState
                , waitingForPlayers = result.waitingForPlayers
              }
            , resolveCmdIntent result.cmdIntent
            )

        MovePlayed position points aiTiles aiScore isGameOver ->
            let
                result =
                    GameLogic.handleMovePlayedPure (toGameModel model)
                        { position = position
                        , points = points
                        , aiTiles = aiTiles
                        , aiScore = aiScore
                        , isGameOver = isGameOver
                        }
            in
            ( { model
                | myTurn = result.myTurn
                , loading = result.loading
                , statusMessage = result.statusMessage
                , plateauTiles = result.plateauTiles
                , aiPlateauTiles = result.aiPlateauTiles
                , aiScore = aiScore
                , availablePositions = result.availablePositions
                , currentTile = result.currentTile
                , currentTileImage = result.currentTileImage
              }
            , resolveCmdIntent result.cmdIntent
            )

        GameStateUpdated gameState ->
            ( { model | gameState = Just gameState }, Cmd.none )

        GameFinished players playerTiles aiTiles allPlateaus ->
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

                mergedPlayers =
                    case model.gameState of
                        Just gs ->
                            mergePlayerScores gs.players players

                        Nothing ->
                            players

                newGameState =
                    Maybe.map
                        (\gs ->
                            { gs
                                | state = Finished
                                , players = mergedPlayers
                            }
                        )
                        model.gameState

                simplePlayers =
                    List.map (\p -> { id = p.id, name = p.name, score = p.score }) mergedPlayers

                result =
                    GameLogic.handleGameFinishedPure
                        { players = simplePlayers
                        , playerTiles = playerTiles
                        , aiTiles = aiTiles
                        , allPlateaus = allPlateaus
                        }
            in
            ( { model
                | gameState = newGameState
                , statusMessage = result.statusMessage
                , plateauTiles = result.plateauTiles
                , aiPlateauTiles = result.aiPlateauTiles
                , allPlayerPlateaus = result.allPlayerPlateaus
                , error = ""
                , myTurn = result.myTurn
                , waitingForPlayers = result.waitingForPlayers
              }
            , Cmd.none
            )

        GameError error ->
            ( { model | loading = False, error = error }, Cmd.none )

        PollTurn ->
            ( model, resolveCmdIntent (GameLogic.handlePollTurnPure (toGameModel model)) )

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

                JsSessionPolled gameState ->
                    update (SessionPolled gameState) model

                JsTurnStarted tile tileImage turnNumber positions players waiting ->
                    update (TurnStarted tile tileImage turnNumber positions players waiting) model

                JsMovePlayed position points aiTiles aiScore isGameOver ->
                    update (MovePlayed position points aiTiles aiScore isGameOver) model

                JsGameStateUpdated gameState ->
                    update (GameStateUpdated gameState) model

                JsGameFinished players playerTiles aiTiles allPlateaus ->
                    update (GameFinished players playerTiles aiTiles allPlateaus) model

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
    | JsSessionPolled GameState
    | JsTurnStarted String String Int (List Int) (List Player) (List String)
    | JsMovePlayed Int Int (List String) Int Bool
    | JsGameStateUpdated GameState
    | JsGameFinished (List Player) (List String) (List String) (List ( String, List String ))
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

        "sessionPolled" ->
            Decode.map JsSessionPolled (Decode.field "gameState" gameStateDecoder)

        "turnStarted" ->
            Decode.map6 JsTurnStarted
                (Decode.field "tile" Decode.string)
                (Decode.field "tileImage" Decode.string)
                (Decode.field "turnNumber" Decode.int)
                (Decode.field "positions" (Decode.list Decode.int))
                (Decode.oneOf
                    [ Decode.field "players" (Decode.list playerDecoder)
                    , Decode.succeed []
                    ]
                )
                (Decode.oneOf
                    [ Decode.field "waitingForPlayers" (Decode.list Decode.string)
                    , Decode.succeed []
                    ]
                )

        "movePlayed" ->
            Decode.map5 JsMovePlayed
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
                (Decode.oneOf
                    [ Decode.field "isGameOver" Decode.bool
                    , Decode.succeed False
                    ]
                )

        "gameStateUpdated" ->
            Decode.map JsGameStateUpdated (Decode.field "gameState" gameStateDecoder)

        "gameFinished" ->
            Decode.map4 JsGameFinished
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
                (Decode.oneOf
                    [ Decode.field "plateaus" (Decode.keyValuePairs (Decode.list Decode.string))
                    , Decode.succeed []
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
                Welcome ->
                    viewWelcome model

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
        Welcome ->
            "Le jeu de stratégie et de chance"

        Login ->
            "Connectez-vous pour jouer"

        Register ->
            "Créez votre compte"

        ForgotPassword ->
            "Réinitialiser votre mot de passe"

        ResetPassword ->
            "Choisissez un nouveau mot de passe"


viewWelcome : Model -> Html Msg
viewWelcome _ =
    div [ class "welcome-content" ]
        [ viewWelcomeBoard
        , p [ class "welcome-pitch" ]
            [ text "Placez vos tuiles, marquez des points et défiez l'IA ou vos amis !" ]
        , button
            [ class "welcome-play-button"
            , onClick SkipAuth
            ]
            [ text "Jouer maintenant" ]
        , div [ class "welcome-separator" ] [ text "ou" ]
        , div [ class "welcome-links" ]
            [ button
                [ type_ "button"
                , class "link-button"
                , onClick (SwitchAuthView Login)
                ]
                [ text "Se connecter" ]
            , text " · "
            , button
                [ type_ "button"
                , class "link-button"
                , onClick (SwitchAuthView Register)
                ]
                [ text "Créer un compte" ]
            ]
        ]


{-| Animated hex board for the welcome page — mini-tutorial showing scoring
-}
viewWelcomeBoard : Html Msg
viewWelcomeBoard =
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

        gridOriginX =
            16

        gridOriginY =
            20

        tileInterval =
            0.4

        -- All 19 tiles for the tutorial board
        allTiles =
            [ ( 0, "963" ), ( 1, "974" ), ( 2, "928" )
            , ( 3, "164" ), ( 4, "123" ), ( 5, "524" ), ( 6, "568" )
            , ( 7, "563" ), ( 8, "173" ), ( 9, "924" ), ( 10, "168" ), ( 11, "178" )
            , ( 12, "964" ), ( 13, "573" ), ( 14, "124" ), ( 15, "973" )
            , ( 16, "523" ), ( 17, "528" ), ( 18, "574" )
            ]

        -- Randomized placement order (board index at each step)
        placementOrder =
            [ 7, 14, 2, 11, 5, 17, 0, 9, 15, 3, 12, 18, 6, 1, 10, 16, 8, 4, 13 ]

        -- For a board index, at which step is it placed?
        placementStep idx =
            placementOrder
                |> List.indexedMap (\s bi -> ( s, bi ))
                |> List.filter (\( _, bi ) -> bi == idx)
                |> List.head
                |> Maybe.map Tuple.first
                |> Maybe.withDefault idx

        -- Tiles reordered for the preview (follows placement order)
        orderedTilesForPreview =
            List.filterMap
                (\boardIdx ->
                    allTiles
                        |> List.filter (\( i, _ ) -> i == boardIdx)
                        |> List.head
                )
                placementOrder

        getTileCode idx =
            List.filter (\( i, _ ) -> i == idx) allTiles
                |> List.head
                |> Maybe.map Tuple.second

        -- Scoring overlays: for each position, list of (phase CSS class, animation delay)
        getScoringOverlays idx =
            (if List.member idx [ 0, 1, 2 ] then [ ( "phase-v1", 8.5 ) ] else [])
                ++ (if List.member idx [ 16, 17, 18 ] then [ ( "phase-v1", 8.5 ) ] else [])
                ++ (if List.member idx [ 0, 3, 7 ] then [ ( "phase-v2", 11.0 ) ] else [])
                ++ (if List.member idx [ 11, 15, 18 ] then [ ( "phase-v2", 11.0 ) ] else [])
                ++ (if List.member idx [ 2, 6, 11 ] then [ ( "phase-v3", 13.5 ) ] else [])

        viewScoringOverlay ( phaseClass, delay ) =
            div
                [ class ("scoring-overlay " ++ phaseClass)
                , style "animation-delay" (String.fromFloat delay ++ "s")
                ]
                []
    in
    div [ class "welcome-board-wrapper" ]
        [ -- Tile preview above the board (visible during placement phase, fades before scoring)
          div [ class "welcome-tile-preview" ]
            [ div [ class "preview-label" ] [ text "Tuile a placer" ]
            , div [ class "preview-tile-area" ]
                (List.indexedMap
                    (\step ( _, tileCode ) ->
                        case parseTileFromPath tileCode of
                            Just tileData ->
                                div
                                    [ class
                                        (if step == 18 then
                                            "preview-tile preview-last"

                                         else
                                            "preview-tile"
                                        )
                                    , style "animation-delay"
                                        (String.fromFloat (toFloat step * tileInterval) ++ "s")
                                    ]
                                    [ div [ class "hex-tile-svg" ] [ viewTileSvg tileData ] ]

                            Nothing ->
                                text ""
                    )
                    orderedTilesForPreview
                )
            , div [ class "preview-arrow" ] [ text "\u{2193}" ]
            ]

        , -- Direction labels above the board
          div [ class "welcome-direction-labels" ]
            [ div
                [ class "direction-label phase-v1"
                , style "animation-delay" "8.5s"
                ]
                [ text "Colonnes ↕" ]
            , div
                [ class "direction-label phase-v2"
                , style "animation-delay" "11.0s"
                ]
                [ text "Diagonales ↗" ]
            , div
                [ class "direction-label phase-v3"
                , style "animation-delay" "13.5s"
                ]
                [ text "Diagonales ↘" ]
            ]

        , -- The hex board with tiles, overlays, and score labels
          div
            [ class "hex-board welcome-hex-board"
            , style "position" "relative"
            , style "width" "320px"
            , style "height" "360px"
            ]
            (List.indexedMap
                (\index ( col, row ) ->
                    let
                        x =
                            gridOriginX + col * spacingX

                        y =
                            gridOriginY + row * spacingY

                        tileDelay =
                            toFloat (placementStep index) * tileInterval

                        overlays =
                            getScoringOverlays index
                    in
                    case getTileCode index of
                        Just tileCode ->
                            case parseTileFromPath tileCode of
                                Just tileData ->
                                    div
                                        [ class "hex-cell filled welcome-tile"
                                        , style "left" (String.fromFloat x ++ "px")
                                        , style "top" (String.fromFloat y ++ "px")
                                        , style "width" (String.fromFloat hexWidth ++ "px")
                                        , style "height" (String.fromFloat hexHeight ++ "px")
                                        , style "animation-delay" (String.fromFloat tileDelay ++ "s")
                                        ]
                                        (div [ class "hex-tile-svg" ]
                                            [ viewTileSvg tileData ]
                                            :: List.map viewScoringOverlay overlays
                                        )

                                Nothing ->
                                    text ""

                        Nothing ->
                            text ""
                )
                hexPositions
                ++ -- Score labels positioned at edges of scoring lines
                   [ div
                        [ class "score-label phase-v1"
                        , style "animation-delay" "8.5s"
                        , style "left" "-22px"
                        , style "top" "158px"
                        ]
                        [ text "27" ]
                   , div
                        [ class "score-label phase-v1"
                        , style "animation-delay" "8.5s"
                        , style "left" "310px"
                        , style "top" "158px"
                        ]
                        [ text "15" ]
                   , div
                        [ class "score-label phase-v2"
                        , style "animation-delay" "11.0s"
                        , style "left" "166px"
                        , style "top" "8px"
                        ]
                        [ text "18" ]
                   , div
                        [ class "score-label phase-v2"
                        , style "animation-delay" "11.0s"
                        , style "left" "310px"
                        , style "top" "218px"
                        ]
                        [ text "21" ]
                   , div
                        [ class "score-label phase-v3"
                        , style "animation-delay" "13.5s"
                        , style "left" "166px"
                        , style "top" "305px"
                        ]
                        [ text "24" ]
                   ]
            )

        , -- Total score
          div
            [ class "welcome-score"
            , style "animation-delay" "16.0s"
            ]
            [ text "Score : 105 pts" ]
        ]


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
    if model.authView == Welcome then
        text ""

    else
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
                , button [ class "login-link", onClick GoToLogin ] [ text "Se connecter" ]
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
                    if not model.myTurn && not (List.isEmpty model.waitingForPlayers) then
                        div [ class "waiting-message" ]
                            [ p [ style "opacity" "0.8" ]
                                [ text ("En attente de " ++ String.fromInt (List.length model.waitingForPlayers) ++ " joueur(s)...") ]
                            ]

                    else
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


{-| Calcule le score d'un plateau Take It Easy.
    Pour chaque ligne, si toutes les tuiles ont la même valeur
    dans la direction de la ligne, score = valeur × nombre de tuiles.
-}
calculateBoardScore : List String -> Int
calculateBoardScore tiles =
    let
        parsedTiles =
            List.indexedMap (\i t -> ( i, parseTileFromPath t )) tiles

        tileAt pos =
            List.filter (\( i, _ ) -> i == pos) parsedTiles
                |> List.head
                |> Maybe.andThen Tuple.second

        -- Lignes verticales (v1: 1, 5, 9)
        v1Lines =
            [ [ 0, 1, 2 ], [ 3, 4, 5, 6 ], [ 7, 8, 9, 10, 11 ], [ 12, 13, 14, 15 ], [ 16, 17, 18 ] ]

        -- Diagonales haut-gauche vers bas-droit (v2: 2, 6, 7)
        v2Lines =
            [ [ 7, 12, 16 ], [ 3, 8, 13, 17 ], [ 0, 4, 9, 14, 18 ], [ 1, 5, 10, 15 ], [ 2, 6, 11 ] ]

        -- Diagonales haut-droit vers bas-gauche (v3: 3, 4, 8)
        v3Lines =
            [ [ 0, 3, 7 ], [ 1, 4, 8, 12 ], [ 2, 5, 9, 13, 16 ], [ 6, 10, 14, 17 ], [ 11, 15, 18 ] ]

        scoreLine getValue positions =
            let
                values =
                    List.filterMap (\pos -> tileAt pos |> Maybe.map getValue) positions
            in
            if List.length values == List.length positions then
                case values of
                    first :: rest ->
                        if List.all (\v -> v == first) rest then
                            first * List.length positions

                        else
                            0

                    [] ->
                        0

            else
                0
    in
    List.sum (List.map (scoreLine .v1) v1Lines)
        + List.sum (List.map (scoreLine .v2) v2Lines)
        + List.sum (List.map (scoreLine .v3) v3Lines)


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
            55

        gridOriginY =
            25
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
            10

        gridOriginY =
            17
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
            10

        gridOriginY =
            17
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
        , if List.length model.allPlayerPlateaus > 2 then
            -- Multiplayer: show all player boards
            let
                myId =
                    model.session |> Maybe.map .playerId |> Maybe.withDefault ""
            in
            div [ class "finished-boards" ]
                (List.map
                    (\( pid, pname, tiles ) ->
                        let
                            isMe =
                                pid == myId

                            isAi =
                                pid == "mcts_ai"

                            label =
                                if isAi then
                                    "🤖 " ++ pname

                                else if isMe then
                                    "👤 " ++ pname ++ " (vous)"

                                else
                                    "👤 " ++ pname

                            boardClass =
                                "final-board glass-container"
                                    ++ (if isMe then
                                            " current-player-board"

                                        else
                                            ""
                                       )
                        in
                        div [ class boardClass ]
                            [ h3 [] [ text label ]
                            , viewFinalHexBoard tiles
                            ]
                    )
                    model.allPlayerPlateaus
                )

          else
            -- Solo / 1v1: show player + AI boards
            div [ class "finished-boards" ]
                [ div [ class "final-board glass-container" ]
                    [ h3 [] [ text "👤 Votre plateau" ]
                    , viewFinalHexBoard model.plateauTiles
                    ]
                , div [ class "final-board glass-container" ]
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
    let
        playerScore =
            calculateBoardScore model.plateauTiles

        aiScore =
            calculateBoardScore model.aiPlateauTiles
    in
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
                [ h3 [] [ text ("Votre plateau - " ++ String.fromInt playerScore ++ " pts") ]
                , viewRealGameBoard model
                ]
            , div [ class "game-board glass-container ai-board" ]
                [ h3 [] [ text ("Plateau IA - " ++ String.fromInt aiScore ++ " pts") ]
                , viewAiRealGameBoard model
                ]
            ]
        , if model.currentTurnNumber >= 19 then
            div [ class "game-over glass-container" ]
                [ h2 [] [ text "Partie terminée!" ]
                , p [ style "font-size" "1.2em" ]
                    [ text ("Votre score: " ++ String.fromInt playerScore ++ " pts") ]
                , p [ style "font-size" "1.2em" ]
                    [ text ("Score IA: " ++ String.fromInt aiScore ++ " pts") ]
                , if playerScore > aiScore then
                    p [ style "font-size" "1.3em", style "font-weight" "bold" ] [ text "Vous avez gagné!" ]

                  else if aiScore > playerScore then
                    p [ style "font-size" "1.3em", style "font-weight" "bold" ] [ text "L'IA a gagné!" ]

                  else
                    p [ style "font-size" "1.3em", style "font-weight" "bold" ] [ text "Égalité!" ]
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
            45

        gridOriginY =
            20
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
            45

        gridOriginY =
            20
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
