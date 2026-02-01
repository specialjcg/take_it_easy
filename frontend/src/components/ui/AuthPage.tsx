// AuthPage.tsx - Page de connexion/inscription
import { Component, Show, createSignal } from 'solid-js';
import { getStore } from '../../mvu';
import './styles/AuthPage.css';

const AuthPage: Component = () => {
    const store = getStore();

    // Champs du formulaire
    const [email, setEmail] = createSignal('');
    const [username, setUsername] = createSignal('');
    const [password, setPassword] = createSignal('');
    const [confirmPassword, setConfirmPassword] = createSignal('');

    const isLogin = () => store.model().auth.authView === 'login';

    const handleSubmit = (e: Event) => {
        e.preventDefault();

        if (isLogin()) {
            store.dispatch(store.msg.login(email(), password()));
        } else {
            if (password() !== confirmPassword()) {
                store.dispatch({ type: 'REGISTER_FAILURE', error: 'Les mots de passe ne correspondent pas' });
                return;
            }
            store.dispatch(store.msg.register(email(), username(), password()));
        }
    };

    const handleSkip = () => {
        store.dispatch(store.msg.skipAuth());
    };

    const switchView = () => {
        const newView = isLogin() ? 'register' : 'login';
        store.dispatch(store.msg.switchAuthView(newView));
        // Reset form
        setEmail('');
        setUsername('');
        setPassword('');
        setConfirmPassword('');
    };

    const isLoading = () => store.model().auth.loginLoading || store.model().auth.registerLoading;

    return (
        <div class="auth-page">
            <div class="auth-container glass-container">
                <div class="auth-header">
                    <h1>Take It Easy</h1>
                    <p class="auth-subtitle">
                        {isLogin() ? 'Connectez-vous pour jouer' : 'Créez votre compte'}
                    </p>
                </div>

                <Show when={store.model().auth.authError}>
                    <div class="auth-error">
                        {store.model().auth.authError}
                    </div>
                </Show>

                <form onSubmit={handleSubmit} class="auth-form">
                    <div class="form-group">
                        <label for="email">Email</label>
                        <input
                            type="email"
                            id="email"
                            value={email()}
                            onInput={(e) => setEmail(e.currentTarget.value)}
                            placeholder="votre@email.com"
                            required
                            disabled={isLoading()}
                        />
                    </div>

                    <Show when={!isLogin()}>
                        <div class="form-group">
                            <label for="username">Nom d'utilisateur</label>
                            <input
                                type="text"
                                id="username"
                                value={username()}
                                onInput={(e) => setUsername(e.currentTarget.value)}
                                placeholder="Votre pseudo"
                                required={!isLogin()}
                                minLength={3}
                                maxLength={30}
                                disabled={isLoading()}
                            />
                        </div>
                    </Show>

                    <div class="form-group">
                        <label for="password">Mot de passe</label>
                        <input
                            type="password"
                            id="password"
                            value={password()}
                            onInput={(e) => setPassword(e.currentTarget.value)}
                            placeholder="••••••••"
                            required
                            minLength={8}
                            disabled={isLoading()}
                        />
                    </div>

                    <Show when={!isLogin()}>
                        <div class="form-group">
                            <label for="confirmPassword">Confirmer le mot de passe</label>
                            <input
                                type="password"
                                id="confirmPassword"
                                value={confirmPassword()}
                                onInput={(e) => setConfirmPassword(e.currentTarget.value)}
                                placeholder="••••••••"
                                required={!isLogin()}
                                disabled={isLoading()}
                            />
                        </div>
                    </Show>

                    <button
                        type="submit"
                        class="auth-submit-button"
                        disabled={isLoading()}
                    >
                        {isLoading() ? (
                            <span class="loading-spinner"></span>
                        ) : (
                            isLogin() ? 'Se connecter' : 'Créer mon compte'
                        )}
                    </button>
                </form>

                <div class="auth-switch">
                    <p>
                        {isLogin() ? "Pas encore de compte ?" : "Déjà un compte ?"}
                        <button
                            type="button"
                            class="auth-switch-button"
                            onClick={switchView}
                            disabled={isLoading()}
                        >
                            {isLogin() ? "S'inscrire" : "Se connecter"}
                        </button>
                    </p>
                </div>

                <div class="auth-skip">
                    <button
                        type="button"
                        class="skip-button"
                        onClick={handleSkip}
                        disabled={isLoading()}
                    >
                        Jouer en mode invité
                    </button>
                </div>
            </div>
        </div>
    );
};

export default AuthPage;
